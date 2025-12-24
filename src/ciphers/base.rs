use std::fmt::{self, Debug};
use std::error::Error;
use rug::Integer;
use smallvec::SmallVec;

use crate::data::message::{InterleavedMessageData, MessageData, MessageDataList};

#[derive(Debug)]
pub enum StandardCipherError {
    UnknownCipher,
    NotConfigurable,
    MissingConfiguration,
    BadConfiguration { msg: Box<str> },
}

impl fmt::Display for StandardCipherError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StandardCipherError::UnknownCipher => write!(f, "Unknown cipher"),
            StandardCipherError::NotConfigurable => write!(f, "This cipher is not configurable"),
            StandardCipherError::MissingConfiguration => write!(f, "This cipher needs configuration"),
            StandardCipherError::BadConfiguration { msg } => write!(f, "Bad cipher configuration: {}", msg),
        }
    }
}

impl Error for StandardCipherError {}

pub trait CipherKey: Sized + ToString {
    fn encode_to_buffer(&self) -> Box<[u8]>;
    fn from_buffer(buffer: &Box<[u8]>) -> Result<Self, Box<dyn Error>>;
}

// FIXME: the DECRYPT const generic is a bad solution. ideally this trait would
//        not know what it's used for, and the decrypt/encrypt concrete types
//        would be set via an associated type of the CipherWorkerContext, but
//        this is complicated heavily by the fact that CipherCodecContext has a
//        lifetime, and i can't figure out how to pass it to a closure
/// NOTE: use interior mutability if you need to cache results. For example, a
///       cipher that depends on previous values, like autokey ciphers
pub trait CipherCodecContext<'codec, const DECRYPT: bool, Key: CipherKey> {
    fn new(input_messages: &'codec InterleavedMessageData, key: &'codec Key) -> Self;
    fn get_input_messages(&self) -> &InterleavedMessageData;
    unsafe fn get_output_unchecked(&self, message_index: usize, unit_index: usize) -> u8;

    fn get_output(&self, message_index: usize, unit_index: usize) -> u8 {
        let in_msgs = &self.get_input_messages();
        assert!(message_index < in_msgs.get_message_count());
        // SAFETY: message_index bounds verified in previous assert
        assert!(unit_index < unsafe { in_msgs.get_unit_count(message_index) });
        // SAFETY: bounds verified in previous asserts
        unsafe { self.get_output_unchecked(message_index, unit_index) }
    }

    fn get_output_message(&self, message_index: usize) -> MessageData {
        let in_msgs = &self.get_input_messages();
        assert!(message_index < in_msgs.get_message_count());
        let mut data = SmallVec::new();
        // SAFETY: message_index bounds verified in previous assert
        for i in 0..unsafe { in_msgs.get_unit_count(message_index) } {
            // SAFETY: message_index is valid, otherwise the msg initialiser
            //         would have panicked by now, and i is valid since we're
            //         iterating over 0..in_msgs.get_unit_count(message_index)
            data.push(unsafe { self.get_output_unchecked(message_index, i) });
        }

        data
    }

    fn get_output_messages(&self) -> MessageDataList {
        let mut messages = MessageDataList::default();
        for m in 0..self.get_input_messages().get_message_count() {
            messages.push(self.get_output_message(m));
        }

        messages
    }
}

pub trait CipherWorkerContext<Key: CipherKey>: Send {
    type CodecContext<'codec, const DECRYPT: bool>: CipherCodecContext<'codec, DECRYPT, Key>;

    fn get_total_keys(&self) -> Integer;
    /**
     * key_callback must be called for each key
     * chunk_callback must be called at least every u32::MAX keys
     */
    fn permute_keys_interruptible<KC: FnMut(&Key), CC: FnMut(u32) -> bool>(&self, key_callback: KC, chunk_callback: CC);

    fn permute_keys<KC: FnMut(&Key)>(&self, key_callback: KC) {
        self.permute_keys_interruptible(key_callback, |_| { true });
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

    fn net_key_to_boxed_str(&self, net_key: &Box<[u8]>) -> Result<Box<str>, Box<dyn Error>> {
        Ok(Self::Key::from_buffer(net_key)?.to_string().into_boxed_str())
    }

    fn create_worker_context(&self) -> Self::Context {
        self.create_worker_context_parallel(0, 1)
    }
}