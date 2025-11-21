use clap::Parser;
use std::time::Instant;
use noita_eye_messages::critical_section;
use noita_eye_messages::utils::threading::{AsyncTaskList, Semaphore};
use noita_eye_messages::data::message::{Message, MessageList};
use noita_eye_messages::utils::print::{print_message, format_big_num, MessagePrintConfig};
use noita_eye_messages::utils::compare::{char_num, is_alphanum, is_ord, is_alpha, is_upper_alpha, is_lower_alpha, is_upper_atoi, is_lower_atoi, is_num};
use noita_eye_messages::data::csv_import::import_csv_messages_or_exit;

#[derive(Parser)]
struct Args {
    /// Path to CSV file containing message data
    data_path: std::path::PathBuf,
    /// Disable parallelism (attempt to crack messages using only the main thread)
    #[arg(short, long)]
    sequential: bool,
}

const RAX_ORDER: i32 = 1; // RAX, ARX, XRA, RXA, AXR, XAR
const ROUND_COUNT: usize = 2;
const KPS_PRINT_MASK: u64 = 0xffffff;

#[derive(Debug)]
#[derive(Default)]
struct RAXRound {
    /** range: 0-7. u32 instead of u8 for performance reasons */
    rotate: u32,
    /** range: 0-255 */
    add: u8,
    /** range: 0-255 */
    xor: u8,
}

#[derive(Debug)]
#[derive(Default)]
struct Key {
    rounds: [RAXRound; ROUND_COUNT],
}

macro_rules! permute_round_parameter {
    ($param:expr, $range_max:expr, $callback:block) => {
        for x in 0..=$range_max {
            $param = x;
            $callback
        }
    }
}

macro_rules! _permute_round {
    ($round:expr, $callback:block) => {
        permute_round_parameter!($round.add, 255, {
            permute_round_parameter!($round.rotate, 7, {
                $callback
            });
        });
    };
}

macro_rules! permute_round {
    ($worker_id:expr, $worker_total:expr, $worker_keys_total:expr, $round:expr, $callback:block) => {
        let x_min = (($worker_id * 256) / $worker_total) as i32;
        let x_max = ((($worker_id + 1) * 256) / $worker_total) as i32;
        $worker_keys_total = ($worker_keys_total as f64 * ((x_max - x_min) as f64 / 256f64)) as u64;
        for x in x_min as u8..=(x_max - 1) as u8 {
            $round.xor = x;
            _permute_round!($round, $callback);
        }
    };
    ($round:expr, $callback:block) => {
        permute_round_parameter!($round.xor, 255, {
            _permute_round!($round, $callback);
        });
    };
}

macro_rules! permute_key {
    ($worker_id:expr, $worker_total:expr, $worker_keys_total:expr, $key:expr, $callback:block) => {
        // TODO it would be nice if this code could be generated, but i couldn't
        //      figure out how to do recursive macros
        permute_round!($worker_id, $worker_total, $worker_keys_total, $key.rounds[0], {
            permute_round!($key.rounds[1], {
                $callback
            });
        });
    };
}

fn apply_rax_round(in_byte: u8, round: &RAXRound) -> u8 {
    let mut byte: u8 = in_byte;
    match RAX_ORDER {
        0 => {
            byte = byte.rotate_right(round.rotate);
            byte = byte.wrapping_add(round.add);
            byte ^ round.xor
        },
        1 => {
            byte = byte.wrapping_add(round.add);
            byte = byte.rotate_right(round.rotate);
            byte ^ round.xor
        },
        2 => {
            byte ^= round.xor;
            byte = byte.rotate_right(round.rotate);
            byte.wrapping_add(round.add)
        },
        3 => {
            byte = byte.rotate_right(round.rotate);
            byte ^= round.xor;
            byte.wrapping_add(round.add)
        },
        4 => {
            byte = byte.wrapping_add(round.add);
            byte ^= round.xor;
            byte.rotate_right(round.rotate)
        },
        _ => {
            byte ^= round.xor;
            byte = byte.wrapping_add(round.add);
            byte.rotate_right(round.rotate)
        }
    }
}

fn decrypt(ct_msg: &Message, pt_msg: &mut Message, key: &Key) {
    // HACK only decrypting first char to get candidates for A-I, a-i or 0-9
    for i in 0..1/*ct_msg.data_len*/ {
        let mut byte = ct_msg.data[i];

        for round in &key.rounds {
            byte = apply_rax_round(byte, round);
        }

        pt_msg.data[i] = byte;
    }
}

fn try_key(key: &Key, working_messages: &mut MessageList, messages: &MessageList, log_semaphore: &Semaphore) {
    // first message special case. put conditions for repeated sections here
    let pt_msg_0 = &mut working_messages[0];
    decrypt(&messages[0], pt_msg_0, key);
    // if pt_msg_0.data[1] != char_num(':') { return }
    // if pt_msg_0.data[1] != char_num('.') { return }
    // if pt_msg_0.data[2] != char_num(' ') { return }

    let pt_msg_0_0 = pt_msg_0.data[0];
    // if !is_alphanum(pt_msg_0_0) { return }
    if !is_ord(pt_msg_0_0) { return }

    // other messages
    for m in 1..messages.len() {
        let pt_msg = &mut working_messages[m];
        decrypt(&messages[m], pt_msg, key);

        let pt_msg_m_0 = pt_msg.data[0];
        // if is_alpha(pt_msg_m_0) != is_alpha(pt_msg_0_0) { return }
        // if is_upper_alpha(pt_msg_m_0) != is_upper_alpha(pt_msg_0_0) { return }
        // if is_lower_alpha(pt_msg_m_0) != is_lower_alpha(pt_msg_0_0) { return }
        if is_upper_atoi(pt_msg_m_0) != is_upper_atoi(pt_msg_0_0) { return }
        if is_lower_atoi(pt_msg_m_0) != is_lower_atoi(pt_msg_0_0) { return }
        if is_num(pt_msg_m_0) != is_num(pt_msg_0_0) { return }
    }

    critical_section!(log_semaphore, {
        println!("{:?}:", key);

        for msg in working_messages.iter() {
            print_message(msg, MessagePrintConfig {
                multiview: true,
                max_len: 8,
            });
        }
    });
}

fn preamble(messages: &MessageList, keys_total: &mut u64) {
    let mut working_messages: MessageList = messages.clone();
    let mut key = Key::default();
    permute_round!(key.rounds[0], {
        *keys_total += 1;
    });
    *keys_total = keys_total.pow(ROUND_COUNT as u32);

    println!("Checking {} RAX rounds ({} total permutations). Ciphertexts (mod_add 32):", ROUND_COUNT, *keys_total);

    for m in 0..working_messages.len() {
        let msg = &mut working_messages[m];
        for i in 0..msg.data.len() {
            msg.data[i] = msg.data[i] + 32;
        }

        print_message(msg, MessagePrintConfig::default());
    }

    println!();
}

fn crack_task(messages: &MessageList, worker_id: u32, worker_total: u32, keys_total: u64, log_semaphore: Semaphore) {
    let mut working_messages: MessageList = messages.clone();
    let mut key = Key::default();
    let mut keys_checked: u64 = 0;
    let mut last_print = Instant::now();
    let mut kps_accum_skips = 0;
    let mut worker_keys_total = keys_total;

    permute_key!(worker_id, worker_total, worker_keys_total, key, {
        try_key(&key, &mut working_messages, messages, &log_semaphore);

        keys_checked += 1;
        // XXX this makes the last round *look* like it's not changing in the
        //     "last key checked" log, but it actually is. don't remove this
        //     check though, otherwise it dramatically slows everything down
        if keys_checked & KPS_PRINT_MASK == 0 {
            let now = Instant::now();
            let secs_since_last = now.duration_since(last_print).as_secs_f64();
            if secs_since_last >= 1f64 {
                critical_section!(log_semaphore, {
                    println!("[worker {}] {:.2}% checked ({}/{} keys, {} keys/sec). last key: {:?}", worker_id, (keys_checked as f64 / worker_keys_total as f64) * 100f64, format_big_num(keys_checked as f64), format_big_num(worker_keys_total as f64), format_big_num((KPS_PRINT_MASK * (kps_accum_skips + 1)) as f64 / secs_since_last), key);
                });
                last_print = now;
                kps_accum_skips = 0;
            } else {
                kps_accum_skips += 1;
            }
        }
    });

    critical_section!(log_semaphore, {
        println!("[worker {}] checked {} keys (done)", worker_id, keys_checked);
    });
}

fn main() {
    let args = Args::parse();
    let messages = import_csv_messages_or_exit(&args.data_path);
    if messages.len() == 0 {
        println!("Nothing to do; need at least one message");
        return;
    }

    let mut keys_total: u64 = 0;
    preamble(&messages, &mut keys_total);

    let worker_total = if args.sequential {
        1u32
    } else {
        (std::thread::available_parallelism().unwrap_or(unsafe { std::num::NonZero::new_unchecked(1) }).get() as u32).min(256)
    };

    println!("Using {} workers", worker_total);
    let log_semaphore = Semaphore::new();
    let mut task_list = AsyncTaskList::new();

    for worker_id in 1..worker_total {
        let log_semaphore = log_semaphore.clone();
        let messages = messages.clone();
        task_list.add_async(move || {
            crack_task(&messages, worker_id, worker_total, keys_total, log_semaphore);
        });
    }

    crack_task(&messages, 0, worker_total, keys_total, log_semaphore);

    task_list.wait();

    println!("All workers done");
}
