use std::fmt::{self, Debug};
use std::error::Error;
use rug::Integer;
use smallvec::SmallVec;

use crate::data::message::{Message, MessageList};

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

pub trait CipherKey: Sized + ToString {
    fn encode_to_buffer(&self) -> Vec<u8>;
    fn from_buffer(buffer: &Vec<u8>) -> Result<Self, Box<dyn Error>>;
}

/// NOTE: use interior mutability if you need to cache results. For example, a
///       cipher that depends on previous values, like autokey ciphers
pub trait CipherCodecContext<'codec, Key: CipherKey> {
    fn new(input_messages: &'codec MessageList, key: &'codec Key) -> Self;
    fn get_input_messages(&self) -> &MessageList;
    fn get_output(&self, message_index: usize, unit_index: usize) -> u8;

    fn get_output_message(&self, message_index: usize) -> Message {
        let mut data = SmallVec::new();
        let msg = &self.get_input_messages()[message_index];
        for i in 0..msg.data.len() {
            data.push(self.get_output(message_index, i));
        }

        Message { name: msg.name.clone(), data }
    }

    fn get_output_messages(&self) -> MessageList {
        let mut messages = MessageList::default();
        for m in 0..self.get_input_messages().len() {
            messages.push(self.get_output_message(m));
        }

        messages
    }
}

pub trait CipherWorkerContext<Key: CipherKey>: Send {
    type DecryptionContext<'codec>: CipherCodecContext<'codec, Key>;
    type EncryptionContext<'codec>: CipherCodecContext<'codec, Key>;

    fn get_total_keys(&self) -> Integer;
    /**
     * key_callback must be called for each key
     * chunk_callback must be called at least every u32::MAX keys
     */
    fn permute_keys_interruptible<KC: FnMut(&Key), CC: FnMut(&Key, u32) -> bool>(&self, key_callback: &mut KC, chunk_callback: &mut CC);

    fn permute_keys<KC: FnMut(&Key)>(&self, key_callback: &mut KC) {
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
    type Key: CipherKey;
    type Context: CipherWorkerContext<Self::Key>;

    fn get_max_parallelism(&self) -> u32;
    fn create_worker_context_parallel(&self, worker_id: u32, worker_total: u32) -> Self::Context;

    fn net_key_to_string(&self, net_key: Vec<u8>) -> Result<String, Box<dyn Error>> {
        Ok(Self::Key::from_buffer(&net_key)?.to_string())
    }

    fn create_worker_context(&self) -> Self::Context {
        self.create_worker_context_parallel(0, 1)
    }
}