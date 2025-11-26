use clap::Parser;
use noita_eye_messages::ciphers::base::{Cipher, CipherContext, CipherDecryptionContext};
use noita_eye_messages::ciphers::deserialise_cipher;
use rug::Integer;
use std::time::Instant;
use noita_eye_messages::critical_section;
use noita_eye_messages::utils::threading::{AsyncTaskList, Semaphore, get_parallelism};
use noita_eye_messages::data::message::MessageList;
use noita_eye_messages::utils::print::{MessagePrintConfig, format_big_float, format_big_uint, print_message};
use noita_eye_messages::utils::compare::{char_num, is_alphanum, is_ord, is_alpha, is_upper_alpha, is_lower_alpha, is_upper_atoi, is_lower_atoi, is_num};
use noita_eye_messages::data::csv_import::import_csv_messages_or_exit;

#[derive(Parser)]
struct Args {
    /// Path to CSV file containing message data
    data_path: std::path::PathBuf,
    /// Cipher to use
    cipher: String,
    /// Cipher configuration. Format is cipher-specific, but generally expected to be Rusty Object Notation. It's recommended to add this as the last argument after a "--"
    config: Option<String>,
    /// Disable parallelism (search messages using only the main thread)
    #[arg(short, long)]
    sequential: bool,
}

const KPS_PRINT_MASK: u32 = 0xffffff;

fn try_key(decrypt_ctx: &mut dyn CipherDecryptionContext, log_semaphore: &Semaphore) {
    // first message special case. put conditions for repeated sections here
    let pt_0_0 = decrypt_ctx.decrypt(0, 0);

    // if !is_alphanum(pt_0_0) { return }
    if !is_ord(pt_0_0) { return }

    // other messages
    for m in 1..decrypt_ctx.get_plaintext_count() {
        let pt_m_0 = decrypt_ctx.decrypt(m, 0);

        // if is_alpha(pt_m_0) != is_alpha(pt_0_0) { return }
        // if is_upper_alpha(pt_m_0) != is_upper_alpha(pt_0_0) { return }
        // if is_lower_alpha(pt_m_0) != is_lower_alpha(pt_0_0) { return }
        if is_upper_atoi(pt_m_0) != is_upper_atoi(pt_0_0) { return }
        if is_lower_atoi(pt_m_0) != is_lower_atoi(pt_0_0) { return }
        if is_num(pt_m_0) != is_num(pt_0_0) { return }
    }

    critical_section!(log_semaphore, {
        println!("{:?}:", decrypt_ctx.serialize_key());

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

fn search_task(worker_id: u32, ctx: Box<dyn CipherContext>, log_semaphore: Semaphore) {
    let mut keys_checked = Integer::new();
    let mut keys_checked_accum: u32 = 0;
    let mut last_print = Instant::now();
    let mut kps_accum_skips = 0;

    let worker_keys_total = ctx.get_total_keys();

    ctx.permute_keys(&mut |decrypt_ctx| {
        try_key(decrypt_ctx, &log_semaphore);

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

fn main() {
    let args = Args::parse();

    let messages = import_csv_messages_or_exit(&args.data_path);
    if messages.len() == 0 {
        eprintln!("Nothing to do; need at least one message");
        std::process::exit(1);
    }

    let cipher = match deserialise_cipher(&args.cipher, &args.config) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Cipher setup failed: {}", e);
            std::process::exit(1);
        },
    };

    // TODO use fasteval for search conditions, and pass them via CLI

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
        task_list.add_async(move || {
            search_task(worker_id, context, log_semaphore);
        });
    }

    let context = cipher.create_context_parallel(messages, 0, worker_total);
    keys_total += context.get_total_keys();
    preamble(context.get_ciphertexts(), keys_total);

    search_task(0, context, log_semaphore);

    task_list.wait();

    println!("All workers done");
}
