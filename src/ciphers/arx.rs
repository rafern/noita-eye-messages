use std::error::Error;

use prost::Message;
use rug::{Integer, ops::Pow};

use crate::{ciphers::base::{Cipher, CipherCodecContext, CipherWorkerContext, StandardCipherError}, data::message::InterleavedMessageData, utils::{run::AnyErrorResult, stackvec::StackVec, threading::get_worker_slice}};

use super::base::CipherKey;

/*
 * WARNING:
 * As Lymm stated on Discord, this is equivalent to a homophonic substitution
 * cipher, so this is now used as just a test-bench/example. Don't actually use
 * this to do cryptanalysis
 */

const KEYS_PER_ROUND: u32 = 524288;
const MAX_ROUNDS: usize = 8;

macro_rules! permute_round {
    ($round:expr, $add_min:expr, $add_max:expr, $callback:block) => {
        for add in $add_min as u8..=$add_max as u8 {
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
struct EncodedARXRound {
    #[prost(uint32, tag = "1")]
    /** range: 0-255 */
    pub add: u32,
    #[prost(uint32, tag = "2")]
    /** range: 0-7 */
    pub rot: u32,
    #[prost(uint32, tag = "3")]
    /** range: 0-255 */
    pub xor: u32,
}

#[derive(prost::Message)]
struct EncodedARXKey {
    #[prost(message, repeated, tag = "1")]
    pub rounds: Vec<EncodedARXRound>,
}

#[derive(Default)]
pub struct ARXRound {
    /** range: 0-255 */
    pub add: u8,
    /** range: 0-7 */
    pub rot: u8,
    /** range: 0-255 */
    pub xor: u8,
}

#[derive(Default)]
pub struct ARXKey {
    pub rounds: StackVec<ARXRound, MAX_ROUNDS>,
}

impl ToString for ARXKey {
    fn to_string(&self) -> String {
        let mut parts = Vec::<String>::new();
        for round in self.rounds.iter() {
            if round.add != 0 { parts.push(format!("a{}", round.add)) }
            if round.rot != 0 { parts.push(format!("r{}", round.rot)) }
            if round.xor != 0 { parts.push(format!("x{}", round.xor)) }
        }

        if parts.len() == 0 {
            String::from("[no-op key]")
        } else {
            format!("[{}]", parts.join("->"))
        }
    }
}

impl CipherKey for ARXKey {
    fn encode_to_buffer(&self) -> Box<[u8]> {
        let mut enc_key = EncodedARXKey::default();
        for round in self.rounds.iter() {
            enc_key.rounds.push(EncodedARXRound {
                rot: round.rot as u32,
                add: round.add as u32,
                xor: round.xor as u32,
            });
        }

        enc_key.encode_to_vec().into()
    }

    fn from_buffer(buffer: &Box<[u8]>) -> Result<Self, Box<dyn Error>> {
        let enc_key = EncodedARXKey::decode(buffer.iter().as_slice())?;
        if enc_key.rounds.len() > MAX_ROUNDS {
            return Err("Max round count exceeded".into());
        }

        let mut key = ARXKey::default();
        for enc_round in enc_key.rounds {
            key.rounds.push(ARXRound {
                rot: enc_round.rot.try_into()?,
                add: enc_round.add.try_into()?,
                xor: enc_round.xor.try_into()?,
            });
        }

        Ok(key)
    }
}

pub struct ARXCodecContext<'codec, const DECRYPT: bool> {
    key: &'codec ARXKey,
    input_messages: &'codec InterleavedMessageData,
}

impl<'codec, const DECRYPT: bool> CipherCodecContext<'codec, DECRYPT, ARXKey> for ARXCodecContext<'codec, DECRYPT> {
    fn new(input_messages: &'codec InterleavedMessageData, key: &'codec ARXKey) -> Self {
        ARXCodecContext { input_messages, key }
    }

    fn get_input_messages(&self) -> &InterleavedMessageData {
        self.input_messages
    }

    unsafe fn get_output_unchecked(&self, message_index: usize, unit_index: usize) -> u8 {
        // SAFETY: bounds must be verified by caller
        let mut byte = unsafe { *self.input_messages.get_unchecked(message_index, unit_index) };

        if const { DECRYPT } {
            self.key.rounds.for_each(|round| {
                byte = byte.wrapping_add(round.add).rotate_right(round.rot as u32) ^ round.xor;
            });
        } else {
            self.key.rounds.for_each_rev(|round| {
                byte = (byte ^ round.xor).rotate_left(round.rot as u32).wrapping_sub(round.add);
            });
        }

        byte
    }
}

pub struct ARXWorkerContext {
    round_count: usize,
    a_min: u8,
    a_max: u8,
}

impl ARXWorkerContext {
    unsafe fn permute_additional_round<KC: FnMut(&ARXKey), CC: FnMut(u32) -> bool>(&self, r: usize, r_max: usize, key: &mut ARXKey, key_callback: &mut KC, chunk_callback: &mut CC) -> bool {
        // TODO maybe do macro for this entire pattern, including the part in
        //      the other method?
        if r == r_max {
            // last round, do occasional callback and don't recurse
            // SAFETY: the caller must guarantee that r_max < key.rounds.len(),
            //         and that r <= r_max
            permute_round!(unsafe { key.rounds.get_unchecked_mut(r) }, {
                key_callback(key)
            });

            chunk_callback(KEYS_PER_ROUND)
        } else {
            // middle round, recurse
            // SAFETY: the caller must guarantee that r_max < key.rounds.len(),
            //         and that r <= r_max
            permute_round!(unsafe { key.rounds.get_unchecked_mut(r) }, {
                // SAFETY: r must be < r_max when calling this method, so this
                //         is only invalid when the caller passes bad arguments
                //         (hence why this method is unsafe)
                if !unsafe { self.permute_additional_round(r + 1, r_max, key, key_callback, chunk_callback) } {
                    return false;
                }
            });

            true
        }
    }
}

impl CipherWorkerContext<ARXKey> for ARXWorkerContext {
    type CodecContext<'codec, const DECRYPT: bool> = ARXCodecContext<'codec, DECRYPT>;

    fn get_total_keys(&self) -> Integer {
        if self.round_count == 0 { return Integer::new(); }
        let mut total = Integer::from(((self.a_max - self.a_min) as u64 + 1) * 2048);
        total *= Integer::from(KEYS_PER_ROUND).pow((self.round_count - 1) as u32);
        total
    }

    fn permute_keys_interruptible<KC: FnMut(&ARXKey), CC: FnMut(u32) -> bool>(&self, mut key_callback: KC, mut chunk_callback: CC) {
        let round_count: usize = self.round_count;
        if round_count == 0 { return }

        let mut key = ARXKey { rounds: StackVec::new() };
        key.rounds.resize_with(round_count, ARXRound::default);

        if round_count == 1 {
            permute_round!(key.rounds[0], self.a_min, self.a_max, {
                key_callback(&key);
            });

            chunk_callback((self.a_max as u32 - self.a_min as u32 + 1) * 256 * 8);
        } else {
            permute_round!(key.rounds[0], self.a_min, self.a_max, {
                // SAFETY: round_count must be at least 2 to reach this block,
                //         so 1 is guaranteed to be <= r_max, as r_max is
                //         round_count - 1, which is 2 - 1 = 1 at minimum
                if !unsafe { self.permute_additional_round(1, round_count - 1, &mut key, &mut key_callback, &mut chunk_callback) } { return }
            });
        }
    }
}

#[derive(Debug)]
pub struct ARXCipher {
    round_count: usize,
}

impl ARXCipher {
    pub fn new(config: Option<&str>) -> AnyErrorResult<ARXCipher> {
        match config {
            Some(s) => {
                let round_count = s.parse::<usize>()?;
                if round_count == 0 || round_count > MAX_ROUNDS {
                    Err(StandardCipherError::BadConfiguration { msg: "Round count must be in the range 1..=8".into() }.into())
                } else {
                    Ok(ARXCipher { round_count })
                }
            },
            None => Err(StandardCipherError::MissingConfiguration.into()),
        }
    }
}

impl Cipher for ARXCipher {
    type Key = ARXKey;
    type Context = ARXWorkerContext;

    fn get_max_parallelism(&self) -> u32 { 256 }

    fn create_worker_context_parallel(&self, worker_id: u32, worker_total: u32) -> <ARXCipher as Cipher>::Context {
        let (a_min, a_max) = get_worker_slice::<u8>(255, worker_id, worker_total);

        ARXWorkerContext {
            round_count: self.round_count,
            a_min,
            a_max,
        }
    }
}
