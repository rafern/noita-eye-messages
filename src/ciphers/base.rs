use std::fmt::{self, Debug};
use std::error::Error;
use rug::Integer;

use crate::data::message::{Message, MessageList};
use crate::utils::stackvec::StackVec;

#[derive(Debug)]
pub enum StandardCipherError {
    UnknownCipher,
    NotConfigurable,
    MissingConfiguration,
}

impl fmt::Display for StandardCipherError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            StandardCipherError::UnknownCipher => "Unknown cipher",
            StandardCipherError::NotConfigurable => "This cipher is not configurable",
            StandardCipherError::MissingConfiguration => "This cipher needs configuration",
        })
    }
}

impl Error for StandardCipherError {}

pub trait CipherDecryptionContext<'a> {
    fn get_current_key_net(&self) -> Vec<u8>;
    fn get_plaintext_name(&self, message_index: usize) -> String;
    fn get_plaintext_count(&self) -> usize;
    fn get_plaintext_len(&self, message_index: usize) -> usize;
    fn decrypt(&mut self, message_index: usize, unit_index: usize) -> u8;

    fn get_plaintext(&mut self, message_index: usize) -> Message {
        let mut data = StackVec::default();
        for i in 0..self.get_plaintext_len(message_index) {
            data.push(self.decrypt(message_index, i));
        }

        Message { name: self.get_plaintext_name(message_index), data }
    }

    fn get_all_plaintexts(&mut self) -> MessageList {
        let mut messages = MessageList::default();
        for m in 0..self.get_plaintext_count() {
            messages.push(self.get_plaintext(m));
        }

        messages
    }
}

pub trait CipherContext: Send {
    fn get_total_keys(&self) -> Integer;
    fn get_ciphertexts(&self) -> &MessageList;
    /**
     * key_callback must be called for each key
     * occasional_callback must be called at least every u32::MAX keys
     */
    fn permute_keys_interruptible<'a>(&'a self, key_callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>), occasional_callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>, u32) -> bool);

    fn permute_keys<'a>(&'a self, key_callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>)) {
        self.permute_keys_interruptible(key_callback, &mut |_, _| { true });
    }
}

/**
 * XXX: Don't forget to register your new cipher in the deserialise_cipher
 *      function when implementing this trait, otherwise the CLI tools won't
 *      know that the new cipher exists (unless this is exactly what you want
 *      for weird reasons)
 */
pub trait Cipher {
    type Context: CipherContext;

    fn get_max_parallelism(&self) -> u32;
    fn create_context_parallel(&self, ciphertexts: MessageList, worker_id: u32, worker_total: u32) -> Self::Context;
    fn net_key_to_string(&self, net_key: Vec<u8>) -> String;

    fn create_context(&self, ciphertexts: MessageList) -> Self::Context {
        self.create_context_parallel(ciphertexts, 0, 1)
    }
}