use clap::Parser;
use noita_eye_messages::analysis::freq::UnitFrequency;
use noita_eye_messages::ciphers::base::{Cipher, CipherContext, CipherDecryptionContext};
use noita_eye_messages::ciphers::deserialise_cipher;
use noita_eye_messages::utils::user_condition::UserCondition;
use rug::{Integer, Rational};
use std::num::NonZeroU32;
use std::time::Instant;
use noita_eye_messages::{cached_var, critical_section};
use noita_eye_messages::utils::threading::{AsyncTaskList, Semaphore, get_cores, try_pinning_core};
use noita_eye_messages::data::message::MessageList;
use noita_eye_messages::utils::print::{MessagePrintConfig, format_big_float, format_big_uint, format_seconds, print_message};
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

// TODO output matched keys to file
// TODO suspend to/resume from file

const KPS_PRINT_MASK: u32 = 0xfffff;

fn try_key(decrypt_ctx: &mut dyn CipherDecryptionContext, cond: &UserCondition, languages: &Vec<UnitFrequency>, log_semaphore: &Semaphore) {
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
}

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

fn search_task(worker_id: u32, ctx: Box<dyn CipherContext>, cond: UserCondition, languages: Vec<UnitFrequency>, log_semaphore: Semaphore) {
    let mut keys_checked = Integer::new();
    let mut keys_checked_accum: u32 = 0;
    let mut kps_accum_skips = 0;
    let worker_keys_total = ctx.get_total_keys();
    let start_time = Instant::now();
    let mut last_print = start_time.clone();

    ctx.permute_keys(&mut |decrypt_ctx| {
        try_key(decrypt_ctx, &cond, &languages, &log_semaphore);

        keys_checked_accum += 1;
        // XXX this makes the last round *look* like it's not changing in the
        //     "last key checked" log, but it actually is. don't remove this
        //     check though, otherwise it dramatically slows everything down
        if keys_checked_accum == KPS_PRINT_MASK {
            keys_checked += keys_checked_accum;
            keys_checked_accum = 0;

            let now = Instant::now();
            let secs_since_last = now.duration_since(last_print).as_secs_f64();
            if secs_since_last >= 1f64 {
                let secs_since_start = now.duration_since(start_time).as_secs_f64();

                let percent = Rational::from(((&keys_checked * 100), &worker_keys_total)).to_f32();
                let kps = (KPS_PRINT_MASK * Integer::from(kps_accum_skips + 1)).to_f64() / secs_since_last;
                let secs_left = Rational::from((&worker_keys_total - &keys_checked, &keys_checked)).to_f64() * secs_since_start;

                critical_section!(log_semaphore, {
                    println!("[worker {worker_id}] {percent:.2}% checked ({}/{} keys, {} keys/sec, {} left). last key: {}", format_big_uint(&keys_checked), format_big_uint(&worker_keys_total), format_big_float(kps), format_seconds(secs_left), decrypt_ctx.serialize_key());
                });
                last_print = now;
                kps_accum_skips = 0;
            } else {
                kps_accum_skips += 1;
            }
        }

        true
    });

    critical_section!(log_semaphore, {
        println!("[worker {}] checked {} keys (done)", worker_id, keys_checked);
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // XXX this won't be cloned for other threads, because fasteval doesn't
    //     support it :/. it's still helpful to do this early so that it fails
    //     fast, but the condition has to be re-parsed for each thread
    let cond = UserCondition::new(&args.condition)?;

    let languages = import_csv_languages_or_exit(&args.language);

    let messages = import_csv_messages_or_exit(&args.data_path);
    if messages.len() == 0 {
        // TODO return error result instead
        eprintln!("Nothing to do; need at least one message");
        std::process::exit(1);
    }

    let cipher = deserialise_cipher(&args.cipher, &args.config)?;

    let max_parallelism = if args.sequential {
        1u32
    } else {
        cipher.get_max_parallelism().min(args.max_parallelism.unwrap_or(unsafe { NonZeroU32::new_unchecked(u32::MAX) }).into())
    };

    let log_semaphore = Semaphore::new();
    let mut task_list = AsyncTaskList::new();
    let mut keys_total = Integer::new();

    let worker_total = match get_cores(max_parallelism) {
        Some(core_ids) => {
            let worker_total = core_ids.len() as u32;

            for worker_id in 1..worker_total {
                let log_semaphore = log_semaphore.clone();
                let context = cipher.create_context_parallel(messages.clone(), worker_id, worker_total);
                keys_total += context.get_total_keys();
                let cond_expr_str = args.condition.clone();
                let languages = languages.clone();
                let core_id = core_ids[worker_id as usize];
                task_list.add_async(move || {
                    try_pinning_core(worker_id, core_id);
                    search_task(worker_id, context, UserCondition::new(&cond_expr_str).unwrap(), languages, log_semaphore);
                });
            }

            try_pinning_core(0, core_ids[0]);
            worker_total
        },
        None => {
            println!("Core info not available, falling back to single unpinned thread");
            1u32
        },
    };

    let context = cipher.create_context_parallel(messages, 0, worker_total);
    keys_total += context.get_total_keys();
    preamble(context.get_ciphertexts(), worker_total, &keys_total);

    search_task(0, context, cond, languages, log_semaphore);

    task_list.wait();

    println!("All workers done");
    Ok(())
}
