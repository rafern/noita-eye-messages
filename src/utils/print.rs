use crate::data::message::Message;
use colored::Colorize;
use rug::Integer;

#[derive(Default)]
pub struct MessagePrintConfig {
    pub max_len: u32,
    pub multiview: bool,
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

pub fn print_ascii_single(c: u8) {
    // invalid ranges (control characters)
    if c > 127 {
        print!("{}", "#".red());
    } else if c < 32 || c > 126 {
        print!("{}", "#".yellow());
    } else {
        print!("{}", unsafe { std::char::from_u32_unchecked(c as u32) });
    }
}

pub fn print_binary_single(c: u8) {
    for i in 0..8 {
        print!("{}", if (c << i) & 0b10000000 > 0 { "1" } else { "0" });
    }
}

pub fn print_message(msg: &Message, config: MessagePrintConfig) {
    print!("{}", format!("{}, len {: >3}: ", msg.name, msg.data.len()).bright_black());

    let mut left = if config.max_len == 0 { u32::MAX } else { config.max_len };
    let mut first = true;

    for c in msg.data.iter() {
        if config.multiview {
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

        print_ascii_single(*c);

        if config.multiview {
            print!(" ");
            print_binary_single(*c);
        }

        left -= 1;
        first = false;
    }

    println!();
}