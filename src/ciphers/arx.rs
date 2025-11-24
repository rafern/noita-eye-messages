pub const ARX_ROUND_COUNT: usize = 2;

#[derive(Debug)]
#[derive(Default)]
pub struct ARXRound {
    /** range: 0-7. u32 instead of u8 for performance reasons */
    pub rotate: u8,
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
