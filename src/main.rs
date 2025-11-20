use clap::{Parser, Subcommand};
use commands::crack::crack;
use data::csv_import::import_csv_messages_or_exit;

mod utils;
mod commands;
mod data;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Attempt to crack messages in given data file
    Crack {
        /// Path to CSV file containing message data. Use "data.csv" from the repository if you want to use the standard eye messages
        data_path: std::path::PathBuf,
        /// Disable parallelism (attempt to crack messages using only the main thread)
        #[arg(short, long)]
        sequential: bool,
    },
}

fn main() {
    let args = Args::parse();

    match &args.command {
        Commands::Crack { data_path, sequential } => crack(import_csv_messages_or_exit(data_path), *sequential),
    }
}
