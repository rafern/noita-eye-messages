use crate::data::message::{Message, MessageList};

pub trait CipherDecryptionContext<'a> {
    fn decrypt(&mut self, message_index: usize, unit_index: usize) -> u8;
    fn get_plaintext_count(&self) -> usize;
    fn get_plaintext_len(&self, message_index: usize) -> usize;
    fn get_plaintext(&mut self, message_index: usize) -> Message;
    fn serialize_key(&self) -> String;
}

pub trait CipherContext: Send {
    fn get_total_keys(&self) -> u64;
    fn get_ciphertexts(&self) -> &MessageList;
    fn permute_keys<'a>(&'a self, callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>) -> bool);
}

pub trait Cipher {
    fn get_max_parallelism(&self) -> u32;
    fn create_context_parallel(&self, ciphertexts: MessageList, worker_id: u32, worker_total: u32) -> Box<dyn CipherContext>;

    fn create_context(&self, ciphertexts: MessageList) -> Box<dyn CipherContext> {
        self.create_context_parallel(ciphertexts, 0, 1)
    }
}