use std::time::Instant;

use colored::Colorize;

mod codegen;
mod utils;
mod messages;

use messages::{Message, MessageList, MESSAGES};

const RAX_ORDER: i32 = 0; // RAX, ARX, XRA, RXA, AXR, XAR
const ROUND_COUNT: usize = 2;
const KPS_PRINT_MASK: u64 = 0xffffff;

#[derive(Debug)]
#[derive(Default)]
/** Note that members are i32 instead of u8 for performance reasons */
struct RAXRound {
    /** range: 0-7 */
    rotate: i32,
    /** range: 0-255 */
    add: i32,
    /** range: 0-255 */
    xor: i32,
}

#[derive(Debug)]
#[derive(Default)]
struct Key {
    rounds: [RAXRound; ROUND_COUNT],
}

#[derive(Default)]
struct MessagePrintConfig<'a> {
    analysis_messages: Option<&'a MessageList>,
    max_len: u32,
}

macro_rules! permute_round_parameter {
    ($param:expr, $range_max:expr, $callback:block) => {
        for x in 0..=$range_max {
            $param = x;
            $callback
        }
    }
}

macro_rules! permute_round {
    ($round:expr, $callback:block) => {
        permute_round_parameter!($round.xor, 255, {
            permute_round_parameter!($round.add, 255, {
                permute_round_parameter!($round.rotate, 7, {
                    $callback
                });
            });
        });
    };
}

macro_rules! permute_key {
    ($key:expr, $callback:block) => {
        // TODO it would be nice if this code could be generated, but i couldn't
        //      figure out how to do recursive macros
        permute_round!($key.rounds[0], {
            permute_round!($key.rounds[1], {
                $callback
            });
        });
    };
}

fn print_ascii_single(c: u8) {
    // invalid ranges (control characters)
    if c > 127 {
        print!("{}", "#".red());
    } else if c < 32 || c > 126 {
        print!("{}", "#".yellow());
    } else {
        print!("{}", unsafe { std::char::from_u32_unchecked(c as u32) });
    }
}

fn print_binary_single(c: u8) {
    for i in 0..8 {
        print!("{}", if (c << i) & 0b10000000 > 0 { "1" } else { "0" });
    }
}

fn print_message(msg: &Message, config: MessagePrintConfig) {
    print!("{}", format!("message {}, len {: >3}: ", msg.name, msg.data_len).bright_black());

    let mut left = if config.max_len == 0 { u32::MAX } else { config.max_len };
    let mut first = true;
    let ref_msg: Option<&Message> = match config.analysis_messages {
        Some(list) => Some(&list[0]),
        None => None,
    };

    for i in 0..msg.data_len {
        if ref_msg.is_some() {
            if left == 0 {
                print!("{}", "|...".bright_black());
                break;
            }

            if !first {
                print!("{}", "|".bright_black());
            }
        } else {
            if left == 0 {
                print!("{}", "...".bright_black());
                break;
            }
        }

        let c = msg.data[i];
        print_ascii_single(c);

        if ref_msg.is_some() {
            print!(" ");
            print_binary_single(c);

            let ref_msg_uw = ref_msg.unwrap();
            let ref_msg_len = ref_msg_uw.data_len;
            if i >= ref_msg_len {
                print!("{}", "  N/A".bright_black());
            } else {
                print!("{}", format!(" {: >4}", c as i32 - ref_msg_uw.data[i] as i32).bright_black());
            }
        }

        left -= 1;
        first = false;
    }

    println!();
}

fn apply_rax_round(in_byte: u8, round: &RAXRound) -> u8 {
    let mut byte: u8 = in_byte;
    match RAX_ORDER {
        0 => {
            byte = utils::rotate(byte, round.rotate as i32);
            byte = utils::mod_add(byte, round.add as i32);
            byte ^ round.xor as u8
        },
        1 => {
            byte = utils::mod_add(byte, round.add as i32);
            byte = utils::rotate(byte, round.rotate as i32);
            byte ^ round.xor as u8
        },
        2 => {
            byte ^= round.xor as u8;
            byte = utils::rotate(byte, round.rotate as i32);
            utils::mod_add(byte, round.add as i32)
        },
        3 => {
            byte = utils::rotate(byte, round.rotate as i32);
            byte ^= round.xor as u8;
            utils::mod_add(byte, round.add as i32)
        },
        4 => {
            byte = utils::mod_add(byte, round.add as i32);
            byte ^= round.xor as u8;
            utils::rotate(byte, round.rotate as i32)
        },
        _ => {
            byte ^= round.xor as u8;
            byte = utils::mod_add(byte, round.add as i32);
            utils::rotate(byte, round.rotate as i32)
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

fn print_key_match(key: &Key, working_messages: &MessageList) {
    println!("{:?}:", key);

    for msg in working_messages {
        print_message(msg, MessagePrintConfig {
            analysis_messages: Some(&working_messages),
            max_len: 8,
        });
    }
}

fn try_key(key: &Key, working_messages: &mut MessageList) {
    // first message special case. put conditions for repeated sections here
    let pt_msg_0 = &mut working_messages[0];
    decrypt(&MESSAGES[0], pt_msg_0, key);

    // if pt_msg_0.data[1] != utils::char_num(':') { return }
    // if pt_msg_0.data[1] != utils::char_num('.') { return }
    // if pt_msg_0.data[2] != utils::char_num(' ') { return }

    let pt_msg_0_0 = pt_msg_0.data[0];
    if !utils::is_ord(pt_msg_0_0) { return }

    // other messages
    for m in 1..MESSAGES.len() {
        let pt_msg = &mut working_messages[m];
        decrypt(&MESSAGES[m], pt_msg, key);

        let pt_msg_m_0 = pt_msg.data[0];
        if utils::is_upper_atoi(pt_msg_m_0) != utils::is_upper_atoi(pt_msg_0_0) { return }
        if utils::is_lower_atoi(pt_msg_m_0) != utils::is_lower_atoi(pt_msg_0_0) { return }
        if utils::is_num(pt_msg_m_0) != utils::is_num(pt_msg_0_0) { return }
    }

    print_key_match(key, &working_messages);
}

fn preamble(working_messages: &mut MessageList, keys_total: &mut u64) {
    let mut key = Key::default();
    permute_round!(key.rounds[0], {
        *keys_total += 1;
    });
    *keys_total = keys_total.pow(ROUND_COUNT as u32);

    println!("Checking {} RAX rounds ({} total permutations). Ciphertexts (mod_add 32):", ROUND_COUNT, *keys_total);

    for msg in working_messages {
        for i in 0..msg.data_len {
            msg.data[i] = utils::mod_add(msg.data[i], 32);
        }

        print_message(msg, MessagePrintConfig::default());
    }

    println!();
}

fn crack() {
    let mut working_messages: MessageList = MESSAGES;
    let mut keys_total: u64 = 0;

    preamble(&mut working_messages, &mut keys_total);

    let mut key = Key::default();
    let mut keys_checked: u64 = 0;
    let mut last_print = Instant::now();
    let mut kps_accum_skips = 0;

    permute_key!(key, {
        try_key(&key, &mut working_messages);

        keys_checked += 1;
        // XXX this makes the last round *look* like it's not changing in the
        //     "last key checked" log, but it actually is. don't remove this
        //     check though, otherwise it dramatically slows everything down
        if keys_checked & KPS_PRINT_MASK == 0 {
            let now = Instant::now();
            let secs_since_last = now.duration_since(last_print).as_secs_f64();
            if secs_since_last >= 1f64 {
                println!("{:.2}% checked ({}/{} keys, {} keys/sec)", (keys_checked as f64 / keys_total as f64) * 100f64, utils::format_big_num(keys_checked as f64), utils::format_big_num(keys_total as f64), utils::format_big_num((KPS_PRINT_MASK * (kps_accum_skips + 1)) as f64 / secs_since_last));
                println!("- last key checked: {:?}", key);
                last_print = now;
                kps_accum_skips = 0;
            } else {
                kps_accum_skips += 1;
            }
        }
    });

    println!("checked {} keys (done)", keys_checked);
}

fn print_help() {
    println!("Arguments:");
    println!("--help, -h : Prints this help screen");
    println!("--codegen : Generates message list declaration code");
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    args.remove(0);

    if args.len() > 1 {
        eprintln!("Expected at most one argument, got {}", args.len());
        print_help();
        std::process::exit(1);
    } else if args.len() == 0 {
        crack();
    } else {
        let arg0 = &args[0];
        if arg0 == "--help" || arg0 == "-h" {
            print_help();
        } else if arg0 == "--codegen" {
            codegen::gen_message_structs(0);
        }
    }
}
