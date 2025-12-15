use std::path::PathBuf;

use noita_eye_messages::data::{csv_export::export_csv_messages_or_exit, csv_import::import_csv_messages_or_exit, message::{Message, MessageList}};
use clap::Parser;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
struct Args {
    /// Stride to deinterlace with
    stride: usize,
    /// Path to CSV file containing message data
    in_data_path: std::path::PathBuf,
    /// Path where CSV files with deinterlaced contents will be stored. A "-0" to "-3" suffix will be added to the file name if, for example, you are deinterlacing with a stride of 4
    out_data_path: std::path::PathBuf,
}

fn main() {
    let args = Args::parse();
    let messages = import_csv_messages_or_exit(&args.in_data_path);

    let out_data_path_osstr = std::path::absolute(args.out_data_path).expect("Expected a valid file path");
    let out_data_path = out_data_path_osstr.as_path();
    if out_data_path.is_dir() {
        eprintln!("{} is a directory. Aborted", out_data_path_osstr.display());
        std::process::exit(1);
    }

    let out_dir = out_data_path.parent().unwrap();
    let (file_name_prefix, file_extension): (String, String) = {
        let file_name = String::from(out_data_path.file_name().unwrap().to_str().unwrap());
        match file_name.rfind('.') {
            Some(idx) if idx > 0 => (String::from(&file_name[..idx]), String::from(&file_name[idx..])),
            _ => (file_name, String::new())
        }
    };

    for offset in 0..args.stride {
        let mut messages_out = MessageList::default();

        for message in messages.iter() {
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
            export_csv_messages_or_exit(&out_path_deint, &messages_out);
        }
    }
}