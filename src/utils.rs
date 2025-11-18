pub fn mod_add(c: u8, amount: i32) -> u8 {
    ((c as i32 + amount).rem_euclid(256)) as u8
}

pub fn rotate(c: u8, amount: i32) -> u8 {
    // +ive = right rot, -ive = left rot. left rot % 8 = right rot
    let r = amount.rem_euclid(8);
    if r > 0 {
        ((c & !(0xffu8 << r)) << (8 - r)) | (c >> r)
    } else {
        c
    }
}

pub const fn char_num(c: char) -> u8 {
    (c as u32) as u8
}

pub fn is_upper_alpha(b: u8) -> bool {
    b >= char_num('A') && b <= char_num('Z')
}

pub fn is_lower_alpha(b: u8) -> bool {
    b >= char_num('a') && b <= char_num('z')
}

pub fn is_num(b: u8) -> bool {
    b >= char_num('0') && b <= char_num('9')
}

pub fn is_alpha(b: u8) -> bool {
    is_upper_alpha(b) || is_lower_alpha(b)
}

pub fn is_alphanum(b: u8) -> bool {
    is_alpha(b) || is_num(b)
}

pub fn is_upper_atoi(b: u8) -> bool {
    b >= char_num('A') && b <= char_num('I')
}

pub fn is_lower_atoi(b: u8) -> bool {
    b >= char_num('a') && b <= char_num('i')
}

pub fn is_ord(b: u8) -> bool {
    is_upper_atoi(b) || is_lower_atoi(b) || is_num(b)
}

pub fn format_big_num(x: f64) -> String {
    if x >= 1_000_000_000_000f64 {
        format!("{:.2}T", x / 1_000_000_000_000f64)
    } else if x >= 1_000_000_000f64 {
        format!("{:.2}B", x / 1_000_000_000f64)
    } else if x >= 1_000_000f64 {
        format!("{:.2}M", x / 1_000_000f64)
    } else {
        format!("{}", x)
    }
}