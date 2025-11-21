use crate::data::message::Message;

const ARX_ORDER: i32 = 1; // ARX, RAX, XAR, AXR, RXA, XRA
pub const ARX_ROUND_COUNT: usize = 2;

#[derive(Debug)]
#[derive(Default)]
pub struct ARXRound {
    /** range: 0-7. u32 instead of u8 for performance reasons */
    pub rotate: u32,
    /** range: 0-255 */
    pub add: u8,
    /** range: 0-255 */
    pub xor: u8,
}

#[derive(Debug)]
#[derive(Default)]
pub struct ARXKey {
    pub rounds: [ARXRound; ARX_ROUND_COUNT],
}

pub fn apply_arx_round(in_byte: u8, round: &ARXRound) -> u8 {
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

pub fn decrypt_arx(ct_msg: &Message, pt_msg: &mut Message, key: &ARXKey) {
    // HACK only decrypting first char to get candidates for A-I, a-i or 0-9
    for i in 0..1/*ct_msg.data_len*/ {
        let mut byte = ct_msg.data[i];

        for round in &key.rounds {
            byte = apply_arx_round(byte, round);
        }

        pt_msg.data[i] = byte;
    }
}