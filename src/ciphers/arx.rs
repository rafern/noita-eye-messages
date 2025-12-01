use prost::Message;
use rug::{Integer, ops::Pow};

use crate::{ciphers::base::{Cipher, CipherWorkerContext, CipherCodecContext, StandardCipherError}, data::message::MessageList, utils::threading::get_worker_slice};

/*
 * WARNING:
 * As Lymm stated on Discord, this is equivalent to a homophonic substitution
 * cipher, so this is now used as just a test-bench/example. Don't actually use
 * this to do cryptanalysis
 */

const KEYS_PER_ROUND: u32 = 524288;

macro_rules! permute_round {
    ($round:expr, $add_min:expr, $add_max:expr, $callback:block) => {
        for add in $add_min..=$add_max {
            $round.add = add;
            for xor in 0..=255 {
                $round.xor = xor;
                for rot in 0..=7 {
                    $round.rot = rot;
                    $callback;
                }
            }
        }
    };
    ($round:expr, $callback:block) => {
        permute_round!($round, 0, 255, $callback)
    };
}

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

pub struct ARXCodecContext {
    key: ARXKey,
    ciphertexts: MessageList,
}

impl CipherCodecContext for ARXCodecContext {
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

    fn decrypt(&self, message_index: usize, unit_index: usize) -> u8 {
        let mut byte = self.ciphertexts[message_index].data[unit_index];

        for round in &self.key.rounds {
            byte = byte.wrapping_add(round.add as u8).rotate_right(round.rot) ^ (round.xor as u8);
        }

        byte
    }
}

pub struct ARXWorkerContext {
    a_min: u32,
    a_max: u32,
    round_count: usize,
}

impl ARXWorkerContext {
    fn permute_additional_round<KC: FnMut(&ARXCodecContext), CC: FnMut(&ARXCodecContext, u32) -> bool>(&self, r: usize, r_max: usize, codec_ctx: &mut ARXCodecContext, key_callback: &mut KC, chunk_callback: &mut CC) -> bool {
        // TODO maybe do macro for this entire pattern, including the part in
        //      the other method?
        if r == unsafe { r_max.unchecked_sub(1) } {
            // last round, do occasional callback and don't recurse
            permute_round!(codec_ctx.key.rounds[r], {
                key_callback(codec_ctx)
            });

            chunk_callback(codec_ctx, KEYS_PER_ROUND)
        } else {
            // middle round, recurse
            permute_round!(codec_ctx.key.rounds[r], {
                if !self.permute_additional_round(r + 1, r_max, codec_ctx, key_callback, chunk_callback) {
                    return false;
                }
            });

            true
        }
    }
}

impl CipherWorkerContext for ARXWorkerContext {
    type DecryptionContext = ARXCodecContext;

    fn get_total_keys(&self) -> Integer {
        if self.round_count == 0 { return Integer::new(); }
        let mut total = Integer::from(((self.a_max - self.a_min) as u64 + 1) * 2048);
        total *= Integer::from(KEYS_PER_ROUND).pow((self.round_count - 1) as u32);
        total
    }

    fn permute_keys_interruptible<KC: FnMut(&ARXCodecContext), CC: FnMut(&ARXCodecContext, u32) -> bool>(&self, ciphertexts: &MessageList, key_callback: &mut KC, chunk_callback: &mut CC) {
        let r_max: usize = self.round_count;
        if r_max == 0 { return }

        let mut codec_ctx = ARXCodecContext {
            key: ARXKey { rounds: Vec::with_capacity(r_max) },
            ciphertexts: ciphertexts.clone(),
        };
        codec_ctx.key.rounds.resize_with(r_max, ARXRound::default);

        if r_max == 1 {
            permute_round!(codec_ctx.key.rounds[0], self.a_min, self.a_max, {
                key_callback(&mut codec_ctx);
            });

            chunk_callback(&mut codec_ctx, (self.a_max - self.a_min + 1) * 256 * 8);
        } else {
            permute_round!(codec_ctx.key.rounds[0], self.a_min, self.a_max, {
                if !self.permute_additional_round(1, r_max, &mut codec_ctx, key_callback, chunk_callback) { return }
            });
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
    type Context = ARXWorkerContext;

    fn get_max_parallelism(&self) -> u32 { 256 }

    fn create_worker_context_parallel(&self, worker_id: u32, worker_total: u32) -> <ARXCipher as Cipher>::Context {
        let (a_min, a_max) = get_worker_slice::<u32>(255, worker_id, worker_total);

        ARXWorkerContext {
            round_count: self.round_count,
            a_min,
            a_max,
        }
    }

    fn net_key_to_string(&self, net_key: Vec<u8>) -> String {
        format!("{:?}", ARXKey::decode(net_key.as_slice()))
    }
}
