#![allow(dead_code)]

use std::time::Instant;

use colored::Colorize;

const ROUND_COUNT: usize = 2;
const KPS_PRINT_MASK: u64 = 0xffffff;
const MSG_LEN_MAX: usize = 137;

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
    /** range: 0-5 */
    order: i32, // RAX, ARX, XRA, RXA, AXR, XAR
}

#[derive(Debug)]
#[derive(Default)]
struct Key {
    rounds: [RAXRound; ROUND_COUNT],
}

struct Message {
    name: &'static str,
    data_len: usize,
    // zero-padded array, so that all messages have the same size
    data: [u8; MSG_LEN_MAX],
}

type MessageList = [Message; 9];

#[derive(Default)]
struct MessagePrintConfig<'a> {
    analysis_messages: Option<&'a MessageList>,
    max_len: u32,
}

/**
 * - maybe the reason why each start is so similar, except for the first character, is that it says:
 *   - "1. ABCDEFGHIJKLMNOPQRSTUV..."
 *   - "2. ABCDEFGHIJKLMNOPQRSTUV..."
 *   - "3. ABCDEFGHIJKLMNOPQRSTUV..."
 *   - "4. WXYZ..."
 *   - "5. WXYZABCD..."
 *   - "6. WXYZ..."
 *   - "7. WXYZABCDEFGHIJK..."
 *   - "8. WXYZABCDEFGHIJK..."
 *   - "9. WXYZABCDEFGHIJK..."
 */
const MESSAGES: MessageList = [
    Message {
        name: "east-1",
        data_len: 99,
        data: [50,66,5,48,62,13,75,29,24,61,42,70,66,62,32,14,81,8,15,78,2,29,13,49,1,80,82,40,63,81,21,19,0,40,51,65,26,14,21,70,47,44,48,42,19,48,13,47,19,49,72,31,5,24,3,43,59,67,33,49,41,60,21,26,30,5,25,20,71,11,74,56,4,74,19,71,4,51,41,43,80,72,54,63,79,81,15,16,44,31,30,12,33,57,28,13,64,43,48, /* 38-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
    Message {
        name: "west-1",
        data_len: 103,
        data: [80,66,5,48,62,13,75,29,24,61,42,70,66,62,32,14,81,8,15,78,2,29,13,49,1,29,11,30,52,81,21,19,0,25,26,54,20,14,21,70,47,44,48,42,19,48,13,47,19,49,44,26,59,77,64,43,79,28,72,64,1,30,73,23,67,6,33,25,64,81,68,46,17,36,13,17,21,68,13,9,46,67,57,34,62,82,15,10,73,62,2,11,65,72,37,44,10,43,68,62,9,34,18, /* 34-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
    Message {
        name: "east-2",
        data_len: 118,
        data: [36,66,5,48,62,13,75,29,24,61,42,70,66,62,32,14,81,8,15,78,2,29,13,49,1,69,76,52,9,48,66,80,22,64,57,40,49,78,3,16,56,19,47,40,80,6,13,64,29,49,64,63,6,49,31,13,16,10,45,24,26,77,10,60,81,61,34,54,70,21,15,4,66,77,42,37,30,22,0,11,41,72,57,20,23,57,65,41,23,18,72,42,5,3,26,78,8,5,54,45,77,25,64,61,16,44,54,51,20,63,25,11,26,45,53,60,38,34, /* 19-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
    Message {
        name: "west-2",
        data_len: 102,
        data: [76,66,5,49,75,54,69,46,32,1,42,60,26,48,50,80,32,24,55,61,47,12,21,12,49,54,34,25,36,15,56,55,20,9,8,62,13,82,9,44,29,60,53,82,42,80,5,43,71,3,80,77,47,78,34,25,62,18,10,49,62,64,52,81,11,66,62,13,47,17,52,70,26,23,32,31,64,23,35,32,50,6,1,25,8,37,47,43,26,76,65,68,80,17,7,45,63,14,53,63,60,16, /* 35-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
    Message {
        name: "east-3",
        data_len: 137,
        data: [63,66,5,49,75,54,2,60,29,40,78,47,60,75,67,71,60,2,65,7,47,14,45,74,59,41,80,13,60,13,81,22,35,50,40,39,2,59,48,31,76,2,80,75,1,56,67,11,21,8,40,65,45,75,55,39,60,42,13,3,22,57,2,6,58,9,70,1,58,56,63,68,25,79,7,20,19,64,2,66,73,30,71,16,12,30,65,37,20,13,22,63,18,46,64,59,41,81,82,22,78,36,47,17,4,6,17,5,36,79,63,1,64,69,15,43,4,58,56,31,14,64,58,18,44,78,69,1,0,46,20,71,73,25,35,8,24],
    },
    Message {
        name: "west-3",
        data_len: 124,
        data: [34,66,5,49,75,54,23,74,11,13,28,26,19,48,67,57,37,60,34,28,74,10,17,32,11,18,19,43,19,81,42,4,62,9,46,49,32,51,76,58,4,43,47,17,67,79,21,32,44,16,30,37,26,28,41,68,57,34,51,10,69,70,8,6,46,43,18,39,47,43,15,13,33,30,35,62,37,0,37,5,38,55,37,13,40,25,9,21,11,64,5,79,42,68,11,71,11,48,3,67,61,40,22,14,35,50,61,39,11,2,66,49,51,53,17,73,36,75,74,54,24,30,54,70, /* 13-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
    Message {
        name: "east-4",
        data_len: 119,
        data: [27,66,5,49,75,54,2,60,29,40,2,55,9,15,59,18,68,3,36,5,47,77,44,38,1,18,28,76,4,34,60,63,58,80,17,54,79,75,48,54,55,19,62,64,14,47,51,70,75,5,11,47,45,58,68,69,79,25,38,45,73,47,68,50,34,45,78,26,79,57,4,56,22,60,18,75,43,60,59,67,63,42,49,33,40,65,79,77,7,3,26,62,31,78,26,57,69,40,4,23,26,13,67,42,38,72,11,39,65,60,25,6,80,66,68,77,59,78,19, /* 18-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
    Message {
        name: "west-4",
        data_len: 120,
        data: [77,66,5,49,75,54,2,60,29,40,2,55,9,15,59,18,68,3,36,5,47,60,21,80,1,72,55,16,82,35,57,19,1,66,18,27,39,17,74,81,39,14,78,0,25,65,43,66,64,38,81,23,24,50,57,30,71,75,26,68,54,57,56,50,71,73,14,21,8,32,26,63,5,37,19,43,66,47,53,34,66,23,73,31,54,38,77,67,11,63,79,6,22,21,51,69,74,21,5,17,67,37,29,21,60,14,82,44,30,4,20,42,35,1,31,54,46,20,40,30, /* 17-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
    Message {
        name: "east-5",
        data_len: 114,
        data: [33,66,5,49,75,54,2,60,29,40,2,55,9,15,59,18,68,3,36,5,47,33,21,59,44,18,28,76,59,34,60,63,79,27,12,54,5,49,48,54,55,52,62,72,69,10,57,22,58,48,67,53,7,34,32,30,31,19,26,8,34,46,7,30,71,55,34,75,54,9,6,60,5,23,25,45,42,80,25,12,22,76,20,51,62,21,40,9,41,10,44,73,8,33,70,73,6,31,21,72,5,40,61,51,42,66,64,74,61,25,63,42,24,41, /* 23-byte padding */ 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    },
];

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
                    // permute_round_parameter!($round.order, 5, {
                        $callback
                    // });
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

fn mod_add(c: u8, amount: i32) -> u8 {
    ((c as i32 + amount).rem_euclid(256)) as u8
}

fn rotate(c: u8, amount: i32) -> u8 {
    // +ive = right rot, -ive = left rot. left rot % 8 = right rot
    let r = amount.rem_euclid(8);
    if r > 0 {
        ((c & !(0xffu8 << r)) << (8 - r)) | (c >> r)
    } else {
        c
    }
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
    // RAX, ARX, XRA, RXA, AXR, XAR
    let mut byte: u8 = in_byte;
    // match round.order {
    //     0 => {
            byte = rotate(byte, round.rotate as i32);
            byte = mod_add(byte, round.add as i32);
            byte ^ round.xor as u8
    //     },
    //     1 => {
    //         byte = mod_add(byte, round.add as i32);
    //         byte = rotate(byte, round.rotate as i32);
    //         byte ^ round.xor as u8
    //     },
    //     2 => {
    //         byte ^= round.xor as u8;
    //         byte = rotate(byte, round.rotate as i32);
    //         mod_add(byte, round.add as i32)
    //     },
    //     3 => {
    //         byte = rotate(byte, round.rotate as i32);
    //         byte ^= round.xor as u8;
    //         mod_add(byte, round.add as i32)
    //     },
    //     4 => {
    //         byte = mod_add(byte, round.add as i32);
    //         byte ^= round.xor as u8;
    //         rotate(byte, round.rotate as i32)
    //     },
    //     _ => {
    //         byte ^= round.xor as u8;
    //         byte = mod_add(byte, round.add as i32);
    //         rotate(byte, round.rotate as i32)
    //     }
    // }
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

const fn char_num(c: char) -> u8 {
    (c as u32) as u8
}

fn is_upper_alpha(b: u8) -> bool {
    b >= char_num('A') && b <= char_num('Z')
}

fn is_lower_alpha(b: u8) -> bool {
    b >= char_num('a') && b <= char_num('z')
}

fn is_num(b: u8) -> bool {
    b >= char_num('0') && b <= char_num('9')
}

fn is_alpha(b: u8) -> bool {
    is_upper_alpha(b) || is_lower_alpha(b)
}

fn is_alphanum(b: u8) -> bool {
    is_alpha(b) || is_num(b)
}

fn is_upper_atoi(b: u8) -> bool {
    b >= char_num('A') && b <= char_num('I')
}

fn is_lower_atoi(b: u8) -> bool {
    b >= char_num('a') && b <= char_num('i')
}

fn is_ord(b: u8) -> bool {
    is_upper_atoi(b) || is_lower_atoi(b) || is_num(b)
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

fn format_big_num(x: f64) -> String {
    if x >= 1_000_000_000_000f64 {
        format!("{:.2}T", x / 1_000_000_000_000f64)
    } else if x >= 1_000_000_000f64 {
        format!("{:.2}B", x / 1_000_000_000f64)
    } else if x >= 1_000_000f64 {
        format!("{:.2}M", x / 1_000_000f64)
    } else {
        format!("{}", x)
    }
}

fn try_key(key: &Key, working_messages: &mut MessageList) {
    // first message special case. put conditions for repeated sections here
    let pt_msg_0 = &mut working_messages[0];
    decrypt(&MESSAGES[0], pt_msg_0, key);

    // if pt_msg_0.data[1] != char_num(':') { return }
    // if pt_msg_0.data[1] != char_num('.') { return }
    // if pt_msg_0.data[2] != char_num(' ') { return }

    let pt_msg_0_0 = pt_msg_0.data[0];
    if !is_ord(pt_msg_0_0) { return }

    // other messages
    for m in 1..MESSAGES.len() {
        let pt_msg = &mut working_messages[m];
        decrypt(&MESSAGES[m], pt_msg, key);

        let pt_msg_m_0 = pt_msg.data[0];
        if is_upper_atoi(pt_msg_m_0) != is_upper_atoi(pt_msg_0_0) { return }
        if is_lower_atoi(pt_msg_m_0) != is_lower_atoi(pt_msg_0_0) { return }
        if is_num(pt_msg_m_0) != is_num(pt_msg_0_0) { return }
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
            msg.data[i] = mod_add(msg.data[i], 32);
        }

        print_message(msg, MessagePrintConfig::default());
    }

    println!();
}

fn main() {
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
        if keys_checked & KPS_PRINT_MASK == 0 {
            let now = Instant::now();
            let secs_since_last = now.duration_since(last_print).as_secs_f64();
            if secs_since_last >= 1f64 {
                println!("{:.2}% checked ({}/{} keys, {} keys/sec)", (keys_checked as f64 / keys_total as f64) * 100f64, format_big_num(keys_checked as f64), format_big_num(keys_total as f64), format_big_num((KPS_PRINT_MASK * (kps_accum_skips + 1)) as f64 / secs_since_last));
                last_print = now;
                kps_accum_skips = 0;
            } else {
                kps_accum_skips += 1;
            }
        }
    });

    println!("checked {} keys (done)", keys_checked);
}
