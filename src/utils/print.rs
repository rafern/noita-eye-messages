use super::message::Message;
use colored::Colorize;

#[derive(Default)]
pub struct MessagePrintConfig {
    pub max_len: u32,
    pub multiview: bool,
}

pub fn format_big_num(x: f64) -> String {
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