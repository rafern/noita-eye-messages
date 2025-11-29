use prost::Message;
use rug::{Integer, ops::Pow};

use crate::{ciphers::base::{Cipher, CipherContext, CipherDecryptionContext, StandardCipherError}, data::message::MessageList, utils::threading::get_worker_slice};

/*
 * WARNING:
 * As Lymm stated on Discord, this is equivalent to a homophonic substitution
 * cipher, so this is now used as just a test-bench/example. Don't actually use
 * this to do cryptanalysis
 */

const KEYS_PER_ROUND: u32 = 524288;

#[derive(prost::Message)]
struct ARXRound {
    #[prost(uint32, tag = "2")]
    /** range: 0-7 */
    pub rot: u32,
    #[prost(uint32, tag = "1")]
    /** range: 0-255 */
    pub add: u32,
    #[prost(uint32, tag = "3")]
    /** range: 0-255 */
    pub xor: u32,
}

#[derive(prost::Message)]
struct ARXKey {
    #[prost(message, repeated, tag = "1")]
    pub rounds: Vec<ARXRound>,
}

pub struct ARXCipherDecryptContext {
    key: ARXKey,
    ciphertexts: MessageList,
}

impl CipherDecryptionContext for ARXCipherDecryptContext {
    fn get_current_key_net(&self) -> Vec<u8> {
        self.key.encode_to_vec()
    }

    fn get_plaintext_name(&self, message_index: usize) -> String {
        self.ciphertexts[message_index].name.clone()
    }

    fn get_plaintext_count(&self) -> usize {
        self.ciphertexts.len()
    }

    fn get_plaintext_len(&self, message_index: usize) -> usize {
        self.ciphertexts[message_index].data.len()
    }

    fn decrypt(&mut self, message_index: usize, unit_index: usize) -> u8 {
        let mut byte = self.ciphertexts[message_index].data[unit_index];

        for round in &self.key.rounds {
            byte = byte.wrapping_add(round.add as u8).rotate_right(round.rot) ^ (round.xor as u8);
        }

        byte
    }
}

pub struct ARXCipherContext {
    ciphertexts: MessageList,
    a_min: u32,
    a_max: u32,
    round_count: usize,
}

impl ARXCipherContext {
    fn permute_additional_round<KC: FnMut(&mut ARXCipherDecryptContext), OC: FnMut(&mut ARXCipherDecryptContext, u32) -> bool>(&self, r: usize, r_max: usize, decrypt_ctx: &mut ARXCipherDecryptContext, key_callback: &mut KC, occasional_callback: &mut OC) -> bool {
        // TODO maybe do macro for this entire pattern, including the part in
        //      the other method?
        if r == unsafe { r_max.unchecked_sub(1) } {
            // last round, do occasional callback and don't recurse
            for add in 0..=255 {
                decrypt_ctx.key.rounds[r].add = add;
                for xor in 0..=255 {
                    decrypt_ctx.key.rounds[r].xor = xor;
                    for rot in 0..=7 {
                        decrypt_ctx.key.rounds[r].rot = rot;
                        key_callback(decrypt_ctx);
                    }
                }
            }

            occasional_callback(decrypt_ctx, KEYS_PER_ROUND)
        } else {
            // middle round, recurse
            for add in 0..=255 {
                decrypt_ctx.key.rounds[r].add = add;
                for xor in 0..=255 {
                    decrypt_ctx.key.rounds[r].xor = xor;
                    for rot in 0..=7 {
                        decrypt_ctx.key.rounds[r].rot = rot;
                        if !self.permute_additional_round(r + 1, r_max, decrypt_ctx, key_callback, occasional_callback) {
                            return false;
                        }
                    }
                }
            }

            true
        }
    }
}

impl CipherContext for ARXCipherContext {
    type DecryptionContext = ARXCipherDecryptContext;

    fn get_total_keys(&self) -> Integer {
        if self.round_count == 0 { return Integer::new(); }
        let mut total = Integer::from(((self.a_max - self.a_min) as u64 + 1) * 2048);
        total *= Integer::from(KEYS_PER_ROUND).pow((self.round_count - 1) as u32);
        total
    }

    fn permute_keys_interruptible<KC: FnMut(&mut ARXCipherDecryptContext), OC: FnMut(&mut ARXCipherDecryptContext, u32) -> bool>(&self, key_callback: &mut KC, occasional_callback: &mut OC) {
        let r_max: usize = self.round_count;
        if r_max == 0 { return }

        let mut decrypt_ctx = ARXCipherDecryptContext {
            key: ARXKey { rounds: Vec::with_capacity(r_max) },
            ciphertexts: self.ciphertexts.clone(), // FIXME figure out how to make this clone unnecesary
        };
        decrypt_ctx.key.rounds.resize_with(r_max, ARXRound::default);

        if r_max == 1 {
            for add in self.a_min..=self.a_max {
                decrypt_ctx.key.rounds[0].add = add;
                for xor in 0..=255 {
                    decrypt_ctx.key.rounds[0].xor = xor;
                    for rot in 0..=7 {
                        decrypt_ctx.key.rounds[0].rot = rot;
                        key_callback(&mut decrypt_ctx);
                    }
                }
            }

            occasional_callback(&mut decrypt_ctx, (self.a_max - self.a_min + 1) * 256 * 8);
        } else {
            for add in self.a_min..=self.a_max {
                decrypt_ctx.key.rounds[0].add = add;
                for xor in 0..=255 {
                    decrypt_ctx.key.rounds[0].xor = xor;
                    for rot in 0..=7 {
                        decrypt_ctx.key.rounds[0].rot = rot;

                        if !self.permute_additional_round(1, r_max, &mut decrypt_ctx, key_callback, occasional_callback) { return }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ARXCipher {
    round_count: usize,
}

impl ARXCipher {
    pub fn new(config: &Option<String>) -> Result<ARXCipher, Box<dyn std::error::Error>> {
        match config {
            Some(s) => Ok(ARXCipher { round_count: s.parse::<usize>()? }),
            None => Err(StandardCipherError::MissingConfiguration.into()),
        }
    }
}

impl Cipher for ARXCipher {
    type Context = ARXCipherContext;

    fn get_max_parallelism(&self) -> u32 { 256 }

    fn create_context_parallel(&self, ciphertexts: MessageList, worker_id: u32, worker_total: u32) -> <ARXCipher as Cipher>::Context {
        let (a_min, a_max) = get_worker_slice::<u32>(255, worker_id, worker_total);

        ARXCipherContext {
            round_count: self.round_count,
            ciphertexts,
            a_min,
            a_max,
        }
    }

    fn net_key_to_string(&self, net_key: Vec<u8>) -> String {
        format!("{:?}", ARXKey::decode(net_key.as_slice()))
    }
}
