use clap::Parser;
use noita_eye_messages::{analysis::{plot::{bar_chart, freq_bar_chart}, unit_freq::UnitFrequency, unit_totals::UnitTotals}, data::{alphabet_io::import_csv_alphabet_or_default, language_io::import_csv_languages, message_io::import_messages}, main_error_wrap, utils::threading::AsyncTaskList};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
struct Args {
    /// Path to CSV or TXT file containing message data
    data_path: std::path::PathBuf,
    /// Path to CSV file containing an alphabet with letter frequency distribution
    #[clap(short, long)]
    language: Vec<std::path::PathBuf>,
    /// Path to alphabet file for interpreting the units in the message data. Any character not present in the alphabet will not be included in the message. If not passed, then an ASCII alphabet which includes all units will be used by default
    #[arg(short, long)]
    alphabet: Option<std::path::PathBuf>,
}

fn main() { main_error_wrap!({
    let args = Args::parse();

    let mut freqs = import_csv_languages(&args.language)?;
    let alphabet = import_csv_alphabet_or_default(&args.alphabet)?;
    let messages = import_messages(&args.data_path, &alphabet)?;

    let unit_totals = UnitTotals::from_messages(&messages);
    let freq = UnitFrequency::from_unit_totals_with_name("Ciphertext", &unit_totals);

    for other in &freqs {
        println!("Frequency distribution error for {} and {}: {}", freq.name, other.name, freq.get_error(other));
    }

    freqs.push(freq);

    let mut task_list = AsyncTaskList::new();
    bar_chart(&mut task_list, "Unit totals", "Unit", "Total", &unit_totals);
    freq_bar_chart(&mut task_list, "Unit frequency", "Unit", "Frequency", freqs);
}) }