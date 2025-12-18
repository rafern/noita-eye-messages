use crate::{analysis::unit_freq::UnitFrequency, data::alphabet_io::import_csv_alphabet, utils::run::AnyErrorResult};

pub fn import_csv_languages(paths: &Vec<std::path::PathBuf>) -> AnyErrorResult<Vec<UnitFrequency>> {
    let mut freqs: Vec<UnitFrequency> = Vec::new();

    for path in paths {
        freqs.push(UnitFrequency::from_alphabet(&import_csv_alphabet(path)?));
    }

    Ok(freqs)
}