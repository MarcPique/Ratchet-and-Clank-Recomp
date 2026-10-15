//! A big-endian bit reader over an MPEG video elementary stream.

/// Reads bits MSB first. Reading past the end yields zero bits (the callers check [`BitReader::overrun`]).
#[derive(Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    /// Position in bits.
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], byte_pos: usize) -> Self { BitReader { data, pos: byte_pos * 8 } }

    /// Position in bits.
    #[inline]
    pub fn bit_pos(&self) -> usize { self.pos }

    /// Position in bytes (rounded down).
    #[inline]
    pub fn byte_pos(&self) -> usize { self.pos >> 3 }

    /// Whether the reader went past the end of the data.
    #[inline]
    pub fn overrun(&self) -> bool { self.pos > self.data.len() * 8 }

    #[inline]
    fn window(&self) -> u64 {
        let b = self.pos >> 3;
        match self.data.get(b..b + 8) {
            Some(s) => u64::from_be_bytes(s.try_into().unwrap()),
            None => {
                let mut w = [0u8; 8];
                if b < self.data.len() {
                    let n = self.data.len() - b;
                    w[..n].copy_from_slice(&self.data[b..]);
                }
                u64::from_be_bytes(w)
            }
        }
    }

    /// The next `n` bits (1..=32) without consuming them.
    #[inline]
    pub fn peek(&self, n: u32) -> u32 {
        debug_assert!((1..=32).contains(&n));
        ((self.window() << (self.pos & 7)) >> (64 - n)) as u32
    }

    #[inline]
    pub fn skip(&mut self, n: u32) { self.pos += n as usize; }

    /// Reads `n` bits (1..=32).
    #[inline]
    pub fn read(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.pos += n as usize;
        v
    }

    #[inline]
    pub fn bit(&mut self) -> bool { self.read(1) != 0 }

    /// Skips to the next byte boundary.
    pub fn align(&mut self) { self.pos = (self.pos + 7) & !7; }

    /// The next 23 bits are zero: a start code follows (the end of a slice's macroblocks).
    #[inline]
    pub fn at_start_code(&self) -> bool { self.peek(23) == 0 }
}

/// The offset of the next `00 00 01` at or after `from`, or None.
pub fn next_start_code(data: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 <= data.len() {
        // Skip fast over bytes that cannot end a start code prefix.
        if data[i + 2] > 1 {
            i += 3;
        } else if data[i + 2] == 1 && data[i] == 0 && data[i + 1] == 0 {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_msb_first_and_pads_with_zeros() {
        let d = [0b1011_0011u8, 0xff, 0x00, 0x80];
        let mut r = BitReader::new(&d, 0);
        assert_eq!(r.read(1), 1);
        assert_eq!(r.read(3), 0b011);
        assert_eq!(r.peek(8), 0b0011_1111);
        assert_eq!(r.read(12), 0b0011_1111_1111);
        assert_eq!(r.read(9), 0b0_0000_0001);
        assert!(!r.overrun());
        assert_eq!(r.read(32), 0);
        assert!(r.overrun());
        let mut r = BitReader::new(&d, 1);
        r.skip(3);
        r.align();
        assert_eq!(r.byte_pos(), 2);
    }

    #[test]
    fn finds_start_codes() {
        let d = [0xff, 0, 0, 1, 0xb3, 0, 0, 0, 1, 0, 0, 2, 0, 0, 1];
        assert_eq!(next_start_code(&d, 0), Some(1));
        assert_eq!(next_start_code(&d, 2), Some(6));
        assert_eq!(next_start_code(&d, 7), Some(12));
        assert_eq!(next_start_code(&d, 13), None);
    }
}
