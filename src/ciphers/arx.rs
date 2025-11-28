use prost::Message;
use rug::{Integer, ops::Pow};

/**
 * WARNING:
 * As Lymm stated on Discord, this is equivalent to a homophonic substitution
 * cipher, so this is now used as just a test-bench/example. Don't actually use
 * this to do cryptanalysis
 */

use crate::{data::message::MessageList, proto::arx::{EncodedArxKey, EncodedArxRound}, utils::threading::get_worker_slice};

use super::base::{Cipher, CipherContext, CipherDecryptionContext, StandardCipherError};

macro_rules! permute_round_parameter {
    ($param:expr, $range_max:expr, $callback:block) => {
        for x in 0..=$range_max {
            $param = x;
            $callback
        }
    }
}

macro_rules! permute_round {
    ($round:expr, $callback:block) => {
        permute_round_parameter!($round.add, 255, {
            permute_round_parameter!($round.xor, 255, {
                permute_round_parameter!($round.rot, 7, {
                    $callback
                });
            });
        });
    };
}

const ARX_ROUND_COUNT: usize = 2;

#[derive(Default)]
struct ARXRound {
    /** range: 0-7. u32 instead of u8 for performance reasons */
    pub rot: u32,
    /** range: 0-255 */
    pub add: u8,
    /** range: 0-255 */
    pub xor: u8,
}

#[derive(Default)]
struct ARXKey {
    pub rounds: [ARXRound; ARX_ROUND_COUNT],
}

pub struct ARXCipherDecryptContext<'a> {
    key: ARXKey,
    ctx: &'a ARXCipherContext,
}

impl<'a> CipherDecryptionContext<'a> for ARXCipherDecryptContext<'a> {
    fn get_current_key_net(&self) -> Vec<u8> {
        let mut e_key = EncodedArxKey::default();
        for round in &self.key.rounds {
            let mut e_round = EncodedArxRound::default();
            e_round.add = round.add as u32;
            e_round.rot = round.rot;
            e_round.xor = round.xor as u32;
            e_key.rounds.push(e_round);
        }

        e_key.encode_to_vec()
    }

    fn get_plaintext_name(&self, message_index: usize) -> String {
        self.ctx.ciphertexts[message_index].name.clone()
    }

    fn get_plaintext_count(&self) -> usize {
        self.ctx.ciphertexts.len()
    }

    fn get_plaintext_len(&self, message_index: usize) -> usize {
        self.ctx.ciphertexts[message_index].data.len()
    }

    fn decrypt(&mut self, message_index: usize, unit_index: usize) -> u8 {
        let mut byte = self.ctx.ciphertexts[message_index].data[unit_index];

        for round in &self.key.rounds {
            byte = byte.wrapping_add(round.add).rotate_right(round.rot) ^ round.xor;
        }

        byte
    }
}

pub struct ARXCipherContext {
    ciphertexts: MessageList,
    a_min: u8,
    a_max: u8,
}

impl CipherContext for ARXCipherContext {
    fn get_total_keys(&self) -> Integer {
        let mut total = Integer::from(((self.a_max - self.a_min) as u64 + 1) * 2048);
        total *= Integer::from(524288).pow(ARX_ROUND_COUNT as u32 - 1);
        total
    }

    fn get_ciphertexts(&self) -> &MessageList {
        &self.ciphertexts
    }

    fn permute_keys<'a>(&'a self, callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>)) {
        let mut decrypt_ctx = ARXCipherDecryptContext {
            key: ARXKey::default(),
            ctx: &self,
        };

        // TODO (or don't idc anymore) - make this react to round count changes
        for r0a in self.a_min..=self.a_max {
            decrypt_ctx.key.rounds[0].add = r0a;
            for r0x in 0..=255 {
                decrypt_ctx.key.rounds[0].xor = r0x;
                for r0r in 0..=7 {
                    decrypt_ctx.key.rounds[0].rot = r0r;
                    permute_round!(decrypt_ctx.key.rounds[1], {
                        callback(&mut decrypt_ctx);
                    });
                }
            }
        }
    }

    fn permute_keys_interruptible<'a>(&'a self, key_callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>), occasional_callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>, u32) -> bool) {
        let mut decrypt_ctx = ARXCipherDecryptContext {
            key: ARXKey::default(),
            ctx: &self,
        };

        // TODO (or don't idc anymore) - make this react to round count changes
        for r0a in self.a_min..=self.a_max {
            decrypt_ctx.key.rounds[0].add = r0a;
            for r0x in 0..=255 {
                decrypt_ctx.key.rounds[0].xor = r0x;
                for r0r in 0..=7 {
                    decrypt_ctx.key.rounds[0].rot = r0r;
                    permute_round!(decrypt_ctx.key.rounds[1], {
                        key_callback(&mut decrypt_ctx);
                    });

                    if !occasional_callback(&mut decrypt_ctx, 524288) { return }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ARXCipher {}

impl ARXCipher {
    pub fn new(config: &Option<String>) -> Result<ARXCipher, Box<dyn std::error::Error>> {
        match config {
            Some(_) => Err(StandardCipherError::NotConfigurable.into()),
            None => Ok(ARXCipher {}.into()),
        }
    }
}

impl Cipher for ARXCipher {
    fn get_max_parallelism(&self) -> u32 { 256 }

    fn create_context_parallel(&self, ciphertexts: MessageList, worker_id: u32, worker_total: u32) -> Box<dyn CipherContext> {
        let (a_min, a_max) = get_worker_slice::<u8>(255, worker_id, worker_total);

        Box::new(ARXCipherContext {
            ciphertexts,
            a_min,
            a_max,
        })
    }

    fn net_key_to_string(&self, net_key: Vec<u8>) -> String {
        format!("{:?}", EncodedArxKey::decode(net_key.as_slice()))
    }
}
