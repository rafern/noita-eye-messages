/**
 * WARNING:
 * As Lymm stated on Discord, this is equivalent to a homophonic substitution
 * cipher, so this is now used as just a test-bench/example. Don't actually use
 * this to do cryptanalysis
 */

use crate::{data::message::{Message, MessageList}, utils::{stackvec::StackVec, threading::get_worker_slice}};

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
                permute_round_parameter!($round.rotate, 7, {
                    $callback
                });
            });
        });
    };
}

const ARX_ORDER: i32 = 1; // ARX, RAX, XAR, AXR, RXA, XRA
pub const ARX_ROUND_COUNT: usize = 2;

#[derive(Debug)]
#[derive(Default)]
struct ARXRound {
    /** range: 0-7. u32 instead of u8 for performance reasons */
    pub rotate: u32,
    /** range: 0-255 */
    pub add: u8,
    /** range: 0-255 */
    pub xor: u8,
}

#[derive(Debug)]
#[derive(Default)]
struct ARXKey {
    pub rounds: [ARXRound; ARX_ROUND_COUNT],
}

fn apply_arx_round(in_byte: u8, round: &ARXRound) -> u8 {
    let mut byte: u8 = in_byte;
    match ARX_ORDER {
        0 => {
            byte = byte.rotate_right(round.rotate);
            byte = byte.wrapping_add(round.add);
            byte ^ round.xor
        },
        1 => {
            byte = byte.wrapping_add(round.add);
            byte = byte.rotate_right(round.rotate);
            byte ^ round.xor
        },
        2 => {
            byte ^= round.xor;
            byte = byte.rotate_right(round.rotate);
            byte.wrapping_add(round.add)
        },
        3 => {
            byte = byte.rotate_right(round.rotate);
            byte ^= round.xor;
            byte.wrapping_add(round.add)
        },
        4 => {
            byte = byte.wrapping_add(round.add);
            byte ^= round.xor;
            byte.rotate_right(round.rotate)
        },
        _ => {
            byte ^= round.xor;
            byte = byte.wrapping_add(round.add);
            byte.rotate_right(round.rotate)
        }
    }
}

pub struct ARXCipherDecryptContext<'a> {
    key: ARXKey,
    ctx: &'a ARXCipherContext,
}

impl<'a> CipherDecryptionContext<'a> for ARXCipherDecryptContext<'a> {
    fn decrypt(&mut self, message_index: usize, unit_index: usize) -> u8 {
        let mut byte = self.ctx.ciphertexts[message_index].data[unit_index];

        for round in &self.key.rounds {
            byte = apply_arx_round(byte, round);
        }

        byte
    }

    fn get_plaintext_count(&self) -> usize {
        self.ctx.ciphertexts.len()
    }

    fn get_plaintext_len(&self, message_index: usize) -> usize {
        self.ctx.ciphertexts[message_index].data.len()
    }

    fn get_plaintext(&mut self, message_index: usize) -> Message {
        let ct = &self.ctx.ciphertexts[message_index];
        let mut data = StackVec::default();

        for i in 0..ct.data.len() {
            data[i] = self.decrypt(message_index, i);
        }

        Message {
            name: ct.name.clone(),
            data,
        }
    }

    fn serialize_key(&self) -> String {
        // TODO use serde instead, this is temporary. deserialization will be
        //      supported in the future
        format!("{:?}", self.key)
    }
}

pub struct ARXCipherContext {
    ciphertexts: MessageList,
    a_min: u8,
    a_max: u8,
}

impl CipherContext for ARXCipherContext {
    fn get_total_keys(&self) -> u64 {
        ((self.a_max - self.a_min) as u64 + 1) * 2048 * (524288u64.pow(ARX_ROUND_COUNT as u32 - 1))
    }

    fn get_ciphertexts(&self) -> &MessageList {
        &self.ciphertexts
    }

    fn permute_keys<'a>(&'a self, callback: &mut dyn FnMut(&mut dyn CipherDecryptionContext<'a>) -> bool) {
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
                    decrypt_ctx.key.rounds[0].rotate = r0r;
                    permute_round!(decrypt_ctx.key.rounds[1], {
                        if !callback(&mut decrypt_ctx) { return }
                    });
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
}
