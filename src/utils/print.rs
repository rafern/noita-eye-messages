use crate::{analysis::alphabet::Alphabet, data::message::{Message, MessageRenderGroup, MessageRenderMap, RenderMessage}};
use colored::Colorize;
use rug::Integer;

#[derive(Default)]
pub struct MessagePrintConfig {
    pub max_len: u32,
    pub multiview: bool,
    pub unit_count_digits_hint: Option<usize>,
    pub msg_len_digits_hint: Option<usize>,
    pub msg_name_len_hint: Option<usize>,
}

#[derive(Default)]
pub struct MessagesPrintConfig {
    pub max_len: u32,
    pub multiview: bool,
}

pub struct UnitPrintConfig {
    pub is_original: bool,
    pub allow_long: bool,
}

pub fn format_big_float(x: f64) -> String {
    if x >= 1_000_000_000_000f64 {
        format!("{:.2}T", x / 1_000_000_000_000f64)
    } else if x >= 1_000_000_000f64 {
        format!("{:.2}B", x / 1_000_000_000f64)
    } else if x >= 1_000_000f64 {
        format!("{:.2}M", x / 1_000_000f64)
    } else {
        format!("{:.2}", x)
    }
}

/**
 * Format big integer as a fixed-point number with 2 decimal places
 */
fn format_big_num_fp2(x: Integer) -> String {
    let (div, rem) = x.div_rem_euc(Integer::from(100));
    format!("{}.{:02}", div, rem)
}

pub fn format_big_uint(x: &Integer) -> String {
    if *x >= Integer::from(1_000_000_000_000u64) {
        format!("{}T", format_big_num_fp2(x / Integer::from(1_000_000_000_0u64)))
    } else if *x >= Integer::from(1_000_000_000) {
        format!("{}B", format_big_num_fp2(x / Integer::from(1_000_000_0)))
    } else if *x >= Integer::from(1_000_000) {
        format!("{}M", format_big_num_fp2(x / Integer::from(1_000_0)))
    } else {
        format!("{}", x)
    }
}

pub fn format_seconds(mut secs: f64) -> String {
    if secs < 1.0 {
        return format!("{}ms", (secs * 1000.0).floor());
    }

    let mut parts = Vec::<String>::new();

    if secs >= 604800.0 { // weeks
        parts.push(format!("{}w", (secs / 604800.0).floor()));
        secs %= 604800.0;
    }

    if secs >= 86400.0 { // days
        parts.push(format!("{}d", (secs / 86400.0).floor()));
        secs %= 86400.0;
    }

    if secs >= 3600.0 { // hours
        parts.push(format!("{}h", (secs / 3600.0).floor()));
        secs %= 3600.0;
    }

    if secs >= 60.0 { // minutes
        parts.push(format!("{}m", (secs / 60.0).floor()));
        secs %= 60.0;
    }

    if secs >= 1.0 { // seconds
        parts.push(format!("{}s", secs.floor()));
    }

    parts.join(" ")
}

pub fn format_seconds_left(secs: f64) -> String {
    if secs >= 0.0 {
        format!("{} left", format_seconds(secs))
    } else {
        format!("delayed (should have finished {} ago)", format_seconds(-secs))
    }
}

pub fn format_hex_char(c: u8) -> String {
    let mut long_char = format!("{c:#x}");
    long_char.replace_range(..1, "\\");
    long_char
}

pub fn print_unit_single(u: u8, alphabet: &Alphabet, config: &UnitPrintConfig) {
    if let Some(alpha_unit) = alphabet.get_unit(u) {
        if config.is_original {
            print!("{}", alpha_unit.grapheme.bright_green());
        } else {
            print!("{}", alpha_unit.grapheme);
        }
    } else {
        let display = if config.allow_long { format_hex_char(u) } else { String::from("#") };
        if config.is_original {
            print!("{}", display.yellow());
        } else {
            print!("{}", display.red());
        }
    }
}

pub fn print_binary_single(c: u8) {
    for i in 0..8 {
        print!("{}", if (c << i) & 0b10000000 > 0 { "1" } else { "0" });
    }
}

pub fn print_message(msg: &Message, render_message: &RenderMessage, alphabet: &Alphabet, config: &MessagePrintConfig) {
    let unit_digits = config.unit_count_digits_hint.unwrap_or(0);
    let len_digits = config.msg_len_digits_hint.unwrap_or(0);
    let name_len = config.msg_name_len_hint.unwrap_or(0);
    print!("{}", format!("{: >name_len$}, {: >unit_digits$} units, {: >len_digits$} len: ", msg.name, msg.data.len(), render_message.get_msg_len()).bright_black());

    let mut left = if config.max_len == 0 { u32::MAX } else { config.max_len };

    let unit_config = UnitPrintConfig {
        is_original: false,
        allow_long: !config.multiview,
    };

    for render_group in render_message.get_render_groups() {
        if left == 0 {
            print!("{}", "...".bright_black());
            break;
        }

        match render_group {
            MessageRenderGroup::Plaintext { grapheme } => {
                print!("{}", grapheme.bright_green());
                left -= 1;
            },
            MessageRenderGroup::HexUnit { unit } => {
                print!("{}", format_hex_char(*unit).yellow());
                left -= 1;
            },
            MessageRenderGroup::CiphertextRange { from, to } => {
                let from = *from;

                if config.multiview {
                    print!("{}", "|".bright_black());
                }

                for i in from..*to {
                    if config.multiview && i != from {
                        print!("{}", "|".bright_black());
                    }

                    if left == 0 {
                        print!("{}", "...".bright_black());
                        break;
                    }

                    let u = msg.data[i];
                    print_unit_single(u, alphabet, &unit_config);

                    if config.multiview {
                        print!(" ");
                        print_binary_single(u);
                    }

                    left -= 1;
                }

                if config.multiview {
                    print!("{}", "|".bright_black());
                }
            },
        }
    }

    println!();
}

pub fn print_messages(title: String, message_render_map: &MessageRenderMap, alphabet: &Alphabet, config: &MessagesPrintConfig) {
    let min_unit_alphabet = alphabet.get_unit_min();
    let mut min_unprintable_unit: Option<u8> = None;
    let messages = message_render_map.get_messages();
    let render_messages = message_render_map.get_render_messages();
    let mut max_unit_count = 0usize;
    let mut max_msg_len = 0usize;
    let mut max_name_len = 0usize;

    for m in 0..messages.len() {
        let message = &messages[m];

        for u in message.data.iter() {
            let u = *u;
            if !alphabet.get_unit(u).is_some_and(|x| x.is_printable()) {
                if let Some(old) = min_unprintable_unit {
                    if u < old {
                        min_unprintable_unit = Some(u)
                    }
                } else {
                    min_unprintable_unit = Some(u);
                }
            }
        }

        max_unit_count = max_unit_count.max(message.data.len());
        max_msg_len = max_msg_len.max(message_render_map.get_render_messages()[m].get_msg_len());
        max_name_len = max_name_len.max(message.name.len());
    }

    let msg_config = MessagePrintConfig {
        max_len: config.max_len,
        multiview: config.multiview,
        unit_count_digits_hint: Some(max_unit_count.checked_ilog10().unwrap_or(0) as usize + 1),
        msg_len_digits_hint: Some(max_msg_len.checked_ilog10().unwrap_or(0) as usize + 1),
        msg_name_len_hint: Some(max_name_len),
    };

    if let Some(min_u) = min_unprintable_unit {
        let add = min_unit_alphabet.wrapping_sub(min_u);

        println!("{title} [transformed for presentation purposes: (unit + {add}) % 256]:");

        for m in 0..messages.len() {
            let mut add_msg = messages[m].clone();
            for i in 0..add_msg.data.len() {
                add_msg.data[i] = add_msg.data[i].wrapping_add(add);
            }

            print_message(&add_msg, &render_messages[m], alphabet, &msg_config);
        }
    } else {
        println!("{title}:");

        for m in 0..messages.len() {
            print_message(&messages[m], &render_messages[m], alphabet, &msg_config);
        }
    }
}