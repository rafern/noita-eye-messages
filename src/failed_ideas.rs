const CAULDRON_KEY: u32 = 0b1100111101001111001010010101101; // original key
// const CAULDRON_KEY: u32 = 0b1011010100101001111001011110011; // reversed key
const CAULDRON_KEY_LEN: u32 = 31;

/**
 * just simple majority vote error-correction, since we have an odd number of
 * messages
 */
fn error_correction(messages: Vec<Vec<u8>>) -> Vec<u8> {
    assert!(messages.len() > 0);

    let mut max_len = messages[0].len();
    for m in &messages[..] {
        max_len = std::cmp::min(max_len, m.len());
    }

    let threshold = messages.len() / 2;
    let mut output: Vec<u8> = Vec::new();

    for i in 0..max_len {
        let mut byte = 0u8;
        for b in 0..8 {
            let mut ones = 0;
            let mask = 1 << b;
            for m in &messages[..] {
                if (m[i] & mask) > 0 {
                    ones += 1;
                }
            }

            if ones > threshold {
                byte |= mask;
            }
        }

        output.push(byte);
    }

    output
}

fn get_key_bits(key_left: &mut u32, wanted_bits: u32) -> u32 {
    assert!(wanted_bits <= CAULDRON_KEY_LEN);

    let mask = !(0xffffffffu32 << wanted_bits);
    let mut key_bits: u32;

    if *key_left >= wanted_bits {
        *key_left -= wanted_bits;
        key_bits = (CAULDRON_KEY >> *key_left) & mask;
    } else {
        key_bits = (CAULDRON_KEY << (wanted_bits - *key_left)) & mask;
        *key_left = CAULDRON_KEY_LEN - wanted_bits + *key_left;
        key_bits |= (CAULDRON_KEY >> *key_left) & mask;
    }

    key_bits
}

fn get_key_byte(key_left: &mut u32) -> u8 {
    get_key_bits(key_left, 8) as u8
}

fn get_key_bit(key_left: &mut u32) -> bool {
    get_key_bits(key_left, 1) > 0
}

fn xor_cessation_calendar(ct_msg: &Message, pt_msg: &mut Message, key: &Key) {
    let mut key_left: u32 = CAULDRON_KEY_LEN;
    for i in 0..ct_msg.data_len {
        pt_msg.data[i] = ct_msg.data[i] ^ get_key_byte(&mut key_left);
    }
}