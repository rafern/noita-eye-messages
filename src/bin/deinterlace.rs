use std::path::PathBuf;

use noita_eye_messages::{data::{alphabet_io::import_csv_alphabet_or_default, message::{Message, MessageList}, message_io::{export_csv_messages, import_messages}}, main_error_wrap};
use clap::Parser;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
struct Args {
    /// Stride to deinterlace with
    stride: usize,
    /// Path to CSV or TXT file containing message data
    in_data_path: std::path::PathBuf,
    /// Path where CSV files with deinterlaced contents will be stored. A "-0" to "-3" suffix will be added to the file name if, for example, you are deinterlacing with a stride of 4
    out_data_path: std::path::PathBuf,
    /// Path to alphabet file for interpreting the units in the message data. Any character not present in the alphabet will not be included in the message. If not passed, then an ASCII alphabet which includes all units will be used by default
    #[arg(short, long)]
    alphabet: Option<std::path::PathBuf>,
}

fn main() { main_error_wrap!({
    let args = Args::parse();
    let alphabet = import_csv_alphabet_or_default(&args.alphabet)?;
    let messages_render_map = import_messages(&args.in_data_path, &alphabet)?;

    let out_data_path_osstr = std::path::absolute(args.out_data_path)?;
    let out_data_path = out_data_path_osstr.as_path();
    if out_data_path.is_dir() {
        return Err(format!("{} is a directory. Aborted", out_data_path_osstr.display()).into());
    }

    let out_dir = out_data_path.parent().unwrap();
    let (file_name_prefix, file_extension) = {
        let file_name = out_data_path.file_name().unwrap().to_str().unwrap();
        match file_name.rfind('.') {
            Some(idx) if idx > 0 => (&file_name[..idx], &file_name[idx..]),
            _ => (file_name, "")
        }
    };

    if file_extension.to_ascii_lowercase() == ".txt" {
        println!("Warning: output path has a .txt file extension, but will be saved in CSV despite this. continuing as normal and assuming you know what you're doing")
    }

    for offset in 0..args.stride {
        let mut messages_out = MessageList::default();

        for message in messages_render_map.get_messages().iter() {
            let mut message_out = Message::default();
            message_out.name = message.name.clone();

            for i in (offset..message.data.len()).step_by(args.stride) {
                message_out.data.push(message.data[i]);
            }

            if message_out.data.len() > 0 {
                messages_out.push(message_out);
            }
        }

        if messages_out.len() > 0 {
            let mut out_path_deint = PathBuf::from(out_dir);
            out_path_deint.push(format!("{file_name_prefix}-{offset}{file_extension}"));
            export_csv_messages(&out_path_deint, &messages_out)?;
        }
    };
}) }