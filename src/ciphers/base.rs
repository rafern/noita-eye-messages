use std::fmt::{self, Debug};
use std::error::Error;
use rug::Integer;

use crate::data::message::{Message, MessageList};

#[derive(Debug)]
pub enum StandardCipherError {
    UnknownCipher,
    NotConfigurable,
}

impl fmt::Display for StandardCipherError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            StandardCipherError::UnknownCipher => "Unknown cipher",
            StandardCipherError::NotConfigurable => "This cipher is not configurable",
        })
    }
}

impl Error for StandardCipherError {}

pub trait CipherDecryptionContext<'a> {
    fn decrypt(&mut self, message_index: usize, unit_index: usize) -> u8;
    fn get_plaintext_count(&self) -> usize;
    fn get_plaintext_len(&self, message_index: usize) -> usize;
    fn get_plaintext(&mut self, message_index: usize) -> Message;
    fn serialize_key(&self) -> String;
}

pub trait CipherContext: Send {
    fn get_total_keys(&self) -> Integer;
    fn get_ciphertexts(&self) -> &MessageList;
    fn permute_keys<'a>(&'a self, callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>) -> bool);
}

/**
 * XXX: Don't forget to register your new cipher in the deserialise_cipher
 *      function when implementing this trait, otherwise the CLI tools won't
 *      know that the new cipher exists (unless this is exactly what you want
 *      for weird reasons)
 */
pub trait Cipher {
    fn get_max_parallelism(&self) -> u32;
    fn create_context_parallel(&self, ciphertexts: MessageList, worker_id: u32, worker_total: u32) -> Box<dyn CipherContext>;

    fn create_context(&self, ciphertexts: MessageList) -> Box<dyn CipherContext> {
        self.create_context_parallel(ciphertexts, 0, 1)
    }
}