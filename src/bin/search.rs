use clap::Parser;
use noita_eye_messages::analysis::freq::UnitFrequency;
use noita_eye_messages::ciphers::base::{Cipher, CipherContext};
use noita_eye_messages::ciphers::deserialise_cipher;
use noita_eye_messages::utils::user_condition::UserCondition;
use rug::{Integer, Rational};
use std::num::NonZeroU32;
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::time::{Duration, Instant};
use noita_eye_messages::{cached_var, critical_section};
use noita_eye_messages::utils::threading::{Semaphore, get_parallelism};
use noita_eye_messages::data::message::MessageList;
use noita_eye_messages::utils::print::{MessagePrintConfig, format_big_float, format_big_uint, format_seconds_left, print_message};
use noita_eye_messages::data::csv_import::{import_csv_languages_or_exit, import_csv_messages_or_exit};

#[derive(clap::Parser)]
struct Args {
    /// Path to CSV file containing message data
    data_path: std::path::PathBuf,
    /// Condition to match. Values greater than 0 are treated as true, which should make it easy to use heuristics with thresholds as conditions (simply subtract the threshold value from the heuristic)
    condition: String,
    /// Cipher to use
    cipher: String,
    /// Cipher configuration. Format is cipher-specific, but generally expected to be Rusty Object Notation. It's recommended to add this as the last argument after a "--"
    config: Option<String>,
    /// Disable parallelism (search messages using only the main thread). Equivalent to setting max parallelism to 1, but takes priority over max parallelism
    #[arg(short, long)]
    sequential: bool,
    /// Maximum number of workers (including main thread). Using all available cores has diminishing returns, so tweaking this value is recommended
    #[arg(short, long)]
    max_parallelism: Option<NonZeroU32>,
    /// Path to CSV file containing a language's letter frequency distribution. Used to register languages. Refer to a language by its index (0-based) in the order specified in the terminal
    #[clap(short, long)]
    language: Vec<std::path::PathBuf>,
}

enum TaskPacket {
    Finished {
        worker_id: u32,
    },
    Progress {
        keys: u32,
    },
}

const RECV_TIMEOUT: Duration = Duration::from_secs(1);

// TODO output matched keys to file
// TODO suspend to/resume from file

fn preamble(messages: &MessageList, worker_total: u32, keys_total: &Integer) {
    let mut working_messages: MessageList = messages.clone();

    println!("Searching {} keys with {} workers. Ciphertexts (mod_add 32):", format_big_uint(keys_total), worker_total);

    for m in 0..working_messages.len() {
        let msg = &mut working_messages[m];
        for i in 0..msg.data.len() {
            msg.data[i] = msg.data[i] + 32;
        }

        print_message(msg, MessagePrintConfig::default());
    }

    println!();
}

fn search_task(worker_id: u32, ctx: Box<dyn CipherContext>, cond: &UserCondition, languages: &Vec<UnitFrequency>, log_semaphore: &Semaphore, tx: SyncSender<TaskPacket>) {
    ctx.permute_keys_interruptible(&mut |decrypt_ctx| {
        let mut pt_freq_dist: Option<UnitFrequency> = None;

        if !cond.eval_condition(&mut |name:&str, args:Vec<f64>| -> Option<f64> {
            match name {
                "pt" => {
                    if args.len() < 2 { return None }

                    let m = args[0] as usize;
                    if m > decrypt_ctx.get_plaintext_count() { return None }

                    let u = args[1] as usize;
                    if u > decrypt_ctx.get_plaintext_len(m) { return None }

                    Some(decrypt_ctx.decrypt(m, u) as f64)
                },
                "pt_freq_dist_error" => {
                    if args.len() < 1 { return None }

                    let l = args[0] as usize;
                    if l >= languages.len() { return None }

                    Some(languages[l].get_error(cached_var!(pt_freq_dist, {
                        UnitFrequency::from_messages(&decrypt_ctx.get_all_plaintexts())
                    })))
                },
                _ => None,
            }
        }).unwrap() { return }

        critical_section!(log_semaphore, {
            println!("match with key {}:", decrypt_ctx.serialize_key());

            for m in 0..decrypt_ctx.get_plaintext_count() {
                print_message(&decrypt_ctx.get_plaintext(m), MessagePrintConfig {
                    multiview: true,
                    max_len: 8,
                });
            }
        });
    }, &mut |_decrypt_ctx, keys| {
        tx.send(TaskPacket::Progress { keys }).unwrap();
        true
    });

    tx.send(TaskPacket::Finished { worker_id }).unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let cond = UserCondition::new(&args.condition)?;

    let languages = import_csv_languages_or_exit(&args.language);

    let messages = import_csv_messages_or_exit(&args.data_path);
    if messages.len() == 0 {
        // TODO return error result instead
        eprintln!("Nothing to do; need at least one message");
        std::process::exit(1);
    }

    let cipher = deserialise_cipher(&args.cipher, &args.config)?;

    let worker_total = if args.sequential {
        1u32
    } else {
        let mut max_parallelism: u32 = args.max_parallelism.unwrap_or(unsafe { NonZeroU32::new_unchecked(u32::MAX) }).into();
        max_parallelism = max_parallelism.min(cipher.get_max_parallelism());
        get_parallelism().min(max_parallelism)
    };

    let log_semaphore = Semaphore::new();
    let (tx, rx) = sync_channel::<TaskPacket>(64);

    std::thread::scope(|scope| {
        let mut keys_total = Integer::new();
        let mut contexts = Vec::<Box<dyn CipherContext>>::new();

        for worker_id in 0..worker_total {
            let context = cipher.create_context_parallel(messages.clone(), worker_id, worker_total);
            keys_total += context.get_total_keys();
            contexts.push(context);
        }

        preamble(&messages, worker_total, &keys_total);

        let start_time = Instant::now();

        let mut worker_id = 0;
        for context in contexts {
            let worker_id_clone = worker_id.clone();
            let cond = &cond;
            let languages = &languages;
            let log_semaphore = &log_semaphore;
            let tx = tx.clone();

            scope.spawn(move || {
                search_task(worker_id_clone, context, cond, languages, log_semaphore, tx);
            });

            worker_id += 1;
        }

        drop(tx);

        let mut keys_checked = Integer::new();
        let mut keys_checked_since_last_print = Integer::new();
        let mut last_print = start_time.clone();
        let mut workers_waiting = worker_total;

        while workers_waiting > 0 {
            match rx.recv_timeout(RECV_TIMEOUT) {
                Ok(packet) => {
                    match packet {
                        TaskPacket::Finished { worker_id } => {
                            workers_waiting -= 1;
                            critical_section!(log_semaphore, {
                                println!("worker {worker_id} finished task");
                            });
                        },
                        TaskPacket::Progress { keys } => {
                            keys_checked_since_last_print += keys;
                        },
                    }
                },
                Err(err) => {
                    match err {
                        RecvTimeoutError::Timeout => { /* do nothing */ },
                        RecvTimeoutError::Disconnected => {
                            critical_section!(log_semaphore, {
                                println!("worker channel disconnected (thread died?)");
                            });

                            return Err(err)?;
                        },
                    }
                },
            }

            let now = Instant::now();
            let secs_since_last = now.duration_since(last_print).as_secs_f64();
            if secs_since_last >= 5f64 {
                keys_checked += &keys_checked_since_last_print;
                let secs_since_start = now.duration_since(start_time).as_secs_f64();

                let percent = Rational::from(((&keys_checked * 100), &keys_total)).to_f32();
                let kps = keys_checked_since_last_print.to_f64() / secs_since_last;
                let secs_left = Rational::from((&keys_total - &keys_checked, &keys_checked)).to_f64() * secs_since_start;

                critical_section!(log_semaphore, {
                    println!("{percent:.2}% checked ({}/{} keys, {} keys/sec, {})", format_big_uint(&keys_checked), format_big_uint(&keys_total), format_big_float(kps), format_seconds_left(secs_left));
                });

                last_print = now;
                keys_checked_since_last_print = Integer::new();
            }
        }

        println!("All workers done");
        Ok(())
    })
}
