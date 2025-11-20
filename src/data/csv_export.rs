use std::io::Write;
use std::process;
use std::error::Error;
use super::message::MessageList;

pub fn export_csv_messages(path: &std::path::PathBuf, messages: &MessageList) -> Result<(), Box<dyn Error>> {
    let mut file = std::fs::File::create(path)?;

    for message in messages.iter() {
        file.write(message.name.as_bytes())?;
        for c in message.data.iter() {
            file.write(format!(",{}", c).as_bytes())?;
        }
    }

    Ok(())
}

pub fn export_csv_messages_or_exit(path: &std::path::PathBuf, messages: &MessageList) {
    match export_csv_messages(path, messages) {
        Err(e) => {
            eprintln!("Failed to write data CSV: {}", e);
            process::exit(1);
        },
        Ok(v) => v
    }
}