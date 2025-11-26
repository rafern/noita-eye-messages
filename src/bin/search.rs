use clap::Parser;
use noita_eye_messages::ciphers::base::{Cipher, CipherContext, CipherDecryptionContext};
use noita_eye_messages::ciphers::deserialise_cipher;
use noita_eye_messages::utils::user_condition::UserCondition;
use rug::Integer;
use std::time::Instant;
use noita_eye_messages::critical_section;
use noita_eye_messages::utils::threading::{AsyncTaskList, Semaphore, get_parallelism};
use noita_eye_messages::data::message::MessageList;
use noita_eye_messages::utils::print::{MessagePrintConfig, format_big_float, format_big_uint, print_message};
use noita_eye_messages::data::csv_import::import_csv_messages_or_exit;

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
    /// Disable parallelism (search messages using only the main thread)
    #[arg(short, long)]
    sequential: bool,
}

const KPS_PRINT_MASK: u32 = 0xffffff;

fn try_key(decrypt_ctx: &mut dyn CipherDecryptionContext, cond: &UserCondition, log_semaphore: &Semaphore) {
    let mut val_cb = |name:&str, args:Vec<f64>| -> Option<f64> {
        match name {
            "pt" => {
                let m: usize = (*args.get(0)?) as usize;
                if m > decrypt_ctx.get_plaintext_count() { return None }

                let u: usize = (*args.get(1)?) as usize;
                if u > decrypt_ctx.get_plaintext_len(m) { return None }

                Some(decrypt_ctx.decrypt(m, u) as f64)
            },
            _ => None,
        }
    };

    if !cond.eval_condition(&mut val_cb).unwrap() { return }

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

fn preamble(messages: &MessageList, keys_total: Integer) {
    let mut working_messages: MessageList = messages.clone();

    println!("Searching {} keys. Ciphertexts (mod_add 32):", keys_total);

    for m in 0..working_messages.len() {
        let msg = &mut working_messages[m];
        for i in 0..msg.data.len() {
            msg.data[i] = msg.data[i] + 32;
        }

        print_message(msg, MessagePrintConfig::default());
    }

    println!();
}

fn search_task(worker_id: u32, ctx: Box<dyn CipherContext>, cond: UserCondition, log_semaphore: Semaphore) {
    let mut keys_checked = Integer::new();
    let mut keys_checked_accum: u32 = 0;
    let mut last_print = Instant::now();
    let mut kps_accum_skips = 0;

    let worker_keys_total = ctx.get_total_keys();

    ctx.permute_keys(&mut |decrypt_ctx| {
        try_key(decrypt_ctx, &cond, &log_semaphore);

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
                critical_section!(log_semaphore, {
                    let percent = TryInto::<u32>::try_into((&keys_checked * Integer::from(10000)) / &worker_keys_total).unwrap() as f32 / 100.0;
                    println!("[worker {worker_id}] {percent:.2}% checked ({}/{} keys, {} keys/sec). last key: {}", format_big_uint(&keys_checked), format_big_uint(&worker_keys_total), format_big_float((KPS_PRINT_MASK * (kps_accum_skips + 1)) as f64 / secs_since_last), decrypt_ctx.serialize_key());
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
        get_parallelism().min(cipher.get_max_parallelism())
    };

    println!("Using {} workers", worker_total);
    let log_semaphore = Semaphore::new();
    let mut task_list = AsyncTaskList::new();
    let mut keys_total = Integer::new();

    for worker_id in 1..worker_total {
        let log_semaphore = log_semaphore.clone();
        let context = cipher.create_context_parallel(messages.clone(), worker_id, worker_total);
        keys_total += context.get_total_keys();
        let cond_expr_str = args.condition.clone();
        task_list.add_async(move || {
            search_task(worker_id, context, UserCondition::new(&cond_expr_str).unwrap(), log_semaphore);
        });
    }

    let context = cipher.create_context_parallel(messages, 0, worker_total);
    keys_total += context.get_total_keys();
    preamble(context.get_ciphertexts(), keys_total);

    search_task(0, context, cond, log_semaphore);

    task_list.wait();

    println!("All workers done");
    Ok(())
}
