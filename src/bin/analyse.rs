use clap::Parser;
use noita_eye_messages::{analysis::{freq::{count_units, frequency_analysis}, plot::{bar_chart, freq_bar_chart}}, data::csv_import::{import_csv_languages_or_exit, import_csv_messages_or_exit}, utils::threading::AsyncTaskList};

#[derive(Parser)]
struct Args {
    /// Path to CSV file containing message data
    data_path: std::path::PathBuf,
    /// Path to CSV file containing a language's letter frequency distribution
    #[clap(short, long)]
    language: Vec<std::path::PathBuf>,
}

fn main() {
    let args = Args::parse();

    let mut freqs = import_csv_languages_or_exit(&args.language);
    let messages = import_csv_messages_or_exit(&args.data_path);

    let unit_totals = count_units(&messages);
    let freq = frequency_analysis("Ciphertext", &unit_totals);

    for other in &freqs {
        println!("Frequency distribution error for {} and {}: {}", freq.name, other.name, freq.get_error(other));
    }

    freqs.push(freq);

    let mut task_list = AsyncTaskList::new();
    bar_chart(&mut task_list, "Unit totals", "Unit", "Total", &unit_totals);
    freq_bar_chart(&mut task_list, "Unit frequency", "Unit", "Frequency", freqs);
}