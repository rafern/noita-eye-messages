use prost::Message;
use clap::Parser;
use noita_eye_messages::analysis::freq::UnitFrequency;
use noita_eye_messages::ciphers::base::{Cipher, CipherContext, CipherDecryptionContext};
use noita_eye_messages::ciphers::deserialise_cipher;
use noita_eye_messages::data::key_dump::KeyDumpMeta;
use noita_eye_messages::utils::user_condition::UserCondition;
use rug::{Integer, Rational};
use std::fs::File;
use std::io::Write;
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
    /// Path to key dump file, if you want to store matches in a file instead of logging to the console
    #[arg(short, long)]
    key_dump_path: Option<std::path::PathBuf>,
}

enum TaskPacket {
    Finished {
        worker_id: u32,
    },
    Progress {
        keys: u32,
    },
    Match {
        // XXX it doesn't really make sense to be passing around protobuf
        //     messages like this, but the project is still in a weird
        //     transition state where it doesn't support distributed computing
        //     yet, but will
        net_key: Vec<u8>,
    },
}

const RECV_TIMEOUT: Duration = Duration::from_secs(1);

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

fn print_progress(time_range: Option<(&Instant, &Instant)>, secs_since_last: f64, keys_total: &Integer, keys_checked: &Integer, keys_checked_since_last_print: &Integer, log_semaphore: &Semaphore) {
    let percent = if *keys_total == 0 {
        100.0
    } else {
        Rational::from((&*keys_checked * 100, &*keys_total)).to_f32()
    };

    let kps = keys_checked_since_last_print.to_f64() / secs_since_last;
    let print_begin = format!("Progress: {percent:.2}% checked ({}/{} keys), {} keys/sec", format_big_uint(&keys_checked), format_big_uint(&keys_total), format_big_float(kps));

    match time_range {
        Some((start_time, now)) => {
            let secs_left = Rational::from((&*keys_total - &*keys_checked, &*keys_checked)).to_f64() * now.duration_since(*start_time).as_secs_f64();
            critical_section!(log_semaphore, {
                println!("{} ({})", print_begin, format_seconds_left(secs_left));
            });
        },
        None => {
            critical_section!(log_semaphore, {
                println!("{}", print_begin);
            });
        }
    }
}

fn search_task(worker_id: u32, ctx: impl CipherContext, cond: &UserCondition, languages: &Vec<UnitFrequency>, _log_semaphore: &Semaphore, tx: SyncSender<TaskPacket>) {
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

        tx.send(TaskPacket::Match { net_key: decrypt_ctx.get_current_key_net() }).unwrap();
    }, &mut |_decrypt_ctx, keys| {
        tx.send(TaskPacket::Progress { keys }).unwrap();
        true
    });

    tx.send(TaskPacket::Finished { worker_id }).unwrap();
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
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

    let mut key_dump_file: Option<File> = match &args.key_dump_path {
        Some(path) => {
            let mut file = File::create_new(path)?;
            let meta = KeyDumpMeta {
                build_hash: String::from(env!("GIT_HASH")),
                cipher_name: args.cipher.clone(),
                cipher_config: args.config.clone(),
            };
            // FIXME is there a way to write directly to the file?
            file.write(meta.encode_to_vec().as_slice())?;
            Some(file)
        },
        None => None,
    };

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
        let mut contexts = Vec::new();

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
                                println!("Worker {worker_id} finished task");
                            });
                        },
                        TaskPacket::Progress { keys } => {
                            keys_checked_since_last_print += keys;
                        },
                        TaskPacket::Match { net_key } => {
                            match key_dump_file {
                                Some(ref mut file) => {
                                    // FIXME is there a way to write directly to the file?
                                    file.write(net_key.encode_to_vec().as_slice())?;
                                },
                                None => {
                                    critical_section!(log_semaphore, {
                                        println!("Matched key {}", cipher.net_key_to_string(net_key));
                                    });
                                },
                            }
                        }
                    }
                },
                Err(err) => {
                    match err {
                        RecvTimeoutError::Timeout => { /* do nothing */ },
                        RecvTimeoutError::Disconnected => {
                            critical_section!(log_semaphore, {
                                println!("Worker channel disconnected (thread died?)");
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

                print_progress(
                    Some((&start_time, &now)),
                    secs_since_last,
                    &keys_total,
                    &keys_checked,
                    &keys_checked_since_last_print,
                    &log_semaphore
                );

                last_print = now;
                keys_checked_since_last_print = Integer::new();
            }
        }

        keys_checked += &keys_checked_since_last_print;

        print_progress(
            None,
            Instant::now().duration_since(last_print).as_secs_f64(),
            &keys_total,
            &keys_checked,
            &keys_checked_since_last_print,
            &log_semaphore
        );

        Ok(())
    })
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}