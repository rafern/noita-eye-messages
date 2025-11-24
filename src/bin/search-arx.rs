use clap::Parser;
use mlua::Lua;
use noita_eye_messages::ciphers::arx::{ARX_ROUND_COUNT, ARXKey};
use std::error::Error;
use std::time::Instant;
use noita_eye_messages::critical_section;
use noita_eye_messages::utils::threading::{AsyncTaskList, Semaphore};
use noita_eye_messages::data::message::MessageList;
use noita_eye_messages::utils::print::{print_message, format_big_num, MessagePrintConfig};
use noita_eye_messages::utils::compare::{char_num, is_alphanum, is_ord, is_alpha, is_upper_alpha, is_lower_alpha, is_upper_atoi, is_lower_atoi, is_num};
use noita_eye_messages::data::csv_import::import_csv_messages_or_exit;

// TODO use mlua so that you can use custom predicates and decryption functions
//      provided by the user as a lua script

#[derive(Parser)]
struct Args {
    /// Path to CSV file containing message data
    data_path: std::path::PathBuf,
    /// Disable parallelism (search messages using only the main thread)
    #[arg(short, long)]
    sequential: bool,
}

const KPS_PRINT_MASK: u64 = 0xffffff;

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

fn try_key(key: &ARXKey, working_messages: &mut MessageList, messages: &MessageList, log_semaphore: &Semaphore, decrypt: &mlua::Function) {
    // first message special case. put conditions for repeated sections here
    let pt_msg_0 = &mut working_messages[0];
    pt_msg_0.data[0] = decrypt.call::<u8>((messages[0].data[0], [key.rounds[0].add, key.rounds[0].rotate, key.rounds[0].xor, key.rounds[1].add, key.rounds[1].rotate, key.rounds[1].xor])).unwrap();
    // if pt_msg_0.data[1] != char_num(':') { return }
    // if pt_msg_0.data[1] != char_num('.') { return }
    // if pt_msg_0.data[2] != char_num(' ') { return }

    let pt_msg_0_0 = pt_msg_0.data[0];
    // if !is_alphanum(pt_msg_0_0) { return }
    if !is_ord(pt_msg_0_0) { return }

    // other messages
    for m in 1..messages.len() {
        let pt_msg_m = &mut working_messages[m];
        pt_msg_m.data[0] = decrypt.call::<u8>((messages[m].data[0], [key.rounds[0].add, key.rounds[0].rotate, key.rounds[0].xor, key.rounds[1].add, key.rounds[1].rotate, key.rounds[1].xor])).unwrap();

        let pt_msg_m_0 = pt_msg_m.data[0];
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
    let mut key = ARXKey::default();
    permute_round!(key.rounds[0], {
        *keys_total += 1;
    });
    *keys_total = keys_total.pow(ARX_ROUND_COUNT as u32);

    println!("Checking {} ARX rounds ({} total permutations). Ciphertexts (mod_add 32):", ARX_ROUND_COUNT, *keys_total);

    for m in 0..working_messages.len() {
        let msg = &mut working_messages[m];
        for i in 0..msg.data.len() {
            msg.data[i] = msg.data[i] + 32;
        }

        print_message(msg, MessagePrintConfig::default());
    }

    println!();
}

fn setup_lua(log_semaphore: &Semaphore) -> Result<Lua, Box<dyn Error>> {
    let lua = Lua::new();

    let api_table = lua.create_table()?;
    let log_semaphore = log_semaphore.clone();

    api_table.set("print", lua.create_function(move |_, msg: String| {
        critical_section!(log_semaphore, {
            println!("[Lua] {}", msg);
        });
        Ok(())
    })?)?;

    api_table.set("mod_add_byte", lua.create_function(|_, (lhs, rhs): (u8, u8)| {
        Ok(lhs.wrapping_add(rhs))
    })?)?;

    api_table.set("rotate_byte", lua.create_function(|_, (lhs, rhs): (u8, u32)| {
        Ok(lhs.rotate_right(rhs))
    })?)?;

    api_table.set("xor_byte", lua.create_function(|_, (lhs, rhs): (u8, u8)| {
        Ok(lhs ^ rhs)
    })?)?;

    lua.globals().set("eyes", api_table)?;

    Ok(lua)
}

fn search_task(messages: &MessageList, worker_id: u32, worker_total: u32, keys_total: u64, log_semaphore: Semaphore) {
    let mut working_messages: MessageList = messages.clone();
    let mut key = ARXKey::default();
    let mut keys_checked: u64 = 0;
    let mut last_print = Instant::now();
    let mut kps_accum_skips = 0;
    let mut worker_keys_total = keys_total;

    let lua = setup_lua(&log_semaphore).unwrap();
    lua.load(r#"
        require "bit32"

        function decrypt(byte, key)
            byte = (byte + key[1]) % 256;
            if key[2] ~= 0 then
                byte = bit32.bor(bit32.lshift(bit32.extract(byte, 0, key[2]), 8 - key[2]), bit32.rshift(byte, key[2]));
            end
            byte = (bit32.bxor(byte, key[3]) + key[4]) % 256;
            if key[5] ~= 0 then
                byte = bit32.bor(bit32.lshift(bit32.extract(byte, 0, key[5]), 8 - key[5]), bit32.rshift(byte, key[5]));
            end
            return bit32.bxor(byte, key[6])
        end
    "#).exec().unwrap();

    let decrypt = lua.globals().get::<mlua::Function>("decrypt").unwrap();

    permute_key!(worker_id, worker_total, worker_keys_total, key, {
        try_key(&key, &mut working_messages, messages, &log_semaphore, &decrypt);

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
            search_task(&messages, worker_id, worker_total, keys_total, log_semaphore);
        });
    }

    search_task(&messages, 0, worker_total, keys_total, log_semaphore);

    task_list.wait();

    println!("All workers done");
}
