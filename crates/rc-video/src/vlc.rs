//! The variable-length code tables of ISO/IEC 13818-2 Annex B that the movies use, as direct lookup tables
//! (one peek of the table's longest code, one lookup).

use crate::bits::BitReader;
use std::sync::OnceLock;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Entry {
    pub value: i16,
    /// Code length; 0 = no code starts with these bits.
    pub len: u8,
}

/// A prefix code as a lookup table indexed by the next `bits` bits.
pub struct Vlc {
    bits: u32,
    table: Vec<Entry>,
}

impl Vlc {
    /// Builds the table; panics when two codes overlap (one is a prefix of another).
    pub fn build(codes: &[(&str, i16)]) -> Vlc {
        let parsed: Vec<(u32, u32, i16)> = codes
            .iter()
            .map(|(c, v)| {
                let bits: String = c.chars().filter(|ch| !ch.is_whitespace()).collect();
                assert!(!bits.is_empty() && bits.len() <= 16 && bits.chars().all(|ch| ch == '0' || ch == '1'), "bad code {c:?}");
                (u32::from_str_radix(&bits, 2).unwrap(), bits.len() as u32, *v)
            })
            .collect();
        let max = parsed.iter().map(|p| p.1).max().unwrap_or(1);
        let mut table = vec![Entry::default(); 1 << max];
        for &(code, len, value) in &parsed {
            let shift = max - len;
            let base = (code << shift) as usize;
            for e in &mut table[base..base + (1 << shift)] {
                assert!(e.len == 0, "overlapping codes in a VLC table at {code:0len$b}", len = len as usize);
                *e = Entry { value, len: len as u8 };
            }
        }
        Vlc { bits: max, table }
    }

    /// Decodes one code; None when the bits match no code (nothing consumed).
    #[inline]
    pub fn decode(&self, r: &mut BitReader) -> Option<i16> {
        let e = self.table[r.peek(self.bits) as usize];
        if e.len == 0 { return None; }
        r.skip(e.len as u32);
        Some(e.value)
    }

    /// Longest code length.
    pub fn max_len(&self) -> u32 { self.bits }

    /// Σ 2^−len over the codes (1.0 for a complete code).
    pub fn kraft(&self) -> f64 { self.table.iter().map(|e| if e.len == 0 { 0.0 } else { 1.0 / (1u64 << self.bits) as f64 }).sum() }
}

// ---------------------------------------------------------------------------------------------------------
// Values

/// `macroblock_address_increment` escape (+33) and MPEG-1 stuffing.
pub const MBA_ESCAPE: i16 = -1;
pub const MBA_STUFFING: i16 = -2;

/// `macroblock_type` flags.
pub const MB_QUANT: i16 = 1;
pub const MB_FORWARD: i16 = 2;
pub const MB_BACKWARD: i16 = 4;
pub const MB_PATTERN: i16 = 8;
pub const MB_INTRA: i16 = 16;

/// DCT coefficient codes: `run << 8 | level` (level without its sign bit), or these.
pub const DCT_EOB: i16 = -1;
pub const DCT_ESCAPE: i16 = -2;

const fn rl(run: i16, level: i16) -> i16 { run << 8 | level }

// ---------------------------------------------------------------------------------------------------------
// Tables

/// Table B-1.
const MBA: &[(&str, i16)] = &[
    ("1", 1), ("011", 2), ("010", 3), ("0011", 4), ("0010", 5), ("0001 1", 6), ("0001 0", 7), ("0000 111", 8), ("0000 110", 9),
    ("0000 1011", 10), ("0000 1010", 11), ("0000 1001", 12), ("0000 1000", 13), ("0000 0111", 14), ("0000 0110", 15),
    ("0000 0101 11", 16), ("0000 0101 10", 17), ("0000 0101 01", 18), ("0000 0101 00", 19), ("0000 0100 11", 20),
    ("0000 0100 10", 21), ("0000 0100 011", 22), ("0000 0100 010", 23), ("0000 0100 001", 24), ("0000 0100 000", 25),
    ("0000 0011 111", 26), ("0000 0011 110", 27), ("0000 0011 101", 28), ("0000 0011 100", 29), ("0000 0011 011", 30),
    ("0000 0011 010", 31), ("0000 0011 001", 32), ("0000 0011 000", 33), ("0000 0001 000", MBA_ESCAPE), ("0000 0001 111", MBA_STUFFING),
];

/// Table B-2 (I pictures).
const MB_TYPE_I: &[(&str, i16)] = &[("1", MB_INTRA), ("01", MB_INTRA | MB_QUANT)];

/// Table B-3 (P pictures).
const MB_TYPE_P: &[(&str, i16)] = &[
    ("1", MB_FORWARD | MB_PATTERN),
    ("01", MB_PATTERN),
    ("001", MB_FORWARD),
    ("0001 1", MB_INTRA),
    ("0001 0", MB_QUANT | MB_FORWARD | MB_PATTERN),
    ("0000 1", MB_QUANT | MB_PATTERN),
    ("0000 01", MB_QUANT | MB_INTRA),
];

/// Table B-4 (B pictures).
const MB_TYPE_B: &[(&str, i16)] = &[
    ("10", MB_FORWARD | MB_BACKWARD),
    ("11", MB_FORWARD | MB_BACKWARD | MB_PATTERN),
    ("010", MB_BACKWARD),
    ("011", MB_BACKWARD | MB_PATTERN),
    ("0010", MB_FORWARD),
    ("0011", MB_FORWARD | MB_PATTERN),
    ("0001 1", MB_INTRA),
    ("0001 0", MB_QUANT | MB_FORWARD | MB_BACKWARD | MB_PATTERN),
    ("0000 11", MB_QUANT | MB_FORWARD | MB_PATTERN),
    ("0000 10", MB_QUANT | MB_BACKWARD | MB_PATTERN),
    ("0000 01", MB_QUANT | MB_INTRA),
];

/// Table B-9 (`coded_block_pattern`, 4:2:0).
const CBP: &[(&str, i16)] = &[
    ("111", 60), ("1101", 4), ("1100", 8), ("1011", 16), ("1010", 32), ("1001 1", 12), ("1001 0", 48), ("1000 1", 20), ("1000 0", 40),
    ("0111 1", 28), ("0111 0", 44), ("0110 1", 52), ("0110 0", 56), ("0101 1", 1), ("0101 0", 61), ("0100 1", 2), ("0100 0", 62),
    ("0011 11", 24), ("0011 10", 36), ("0011 01", 3), ("0011 00", 63), ("0010 111", 5), ("0010 110", 9), ("0010 101", 17),
    ("0010 100", 33), ("0010 011", 6), ("0010 010", 10), ("0010 001", 18), ("0010 000", 34), ("0001 1111", 7), ("0001 1110", 11),
    ("0001 1101", 19), ("0001 1100", 35), ("0001 1011", 13), ("0001 1010", 49), ("0001 1001", 21), ("0001 1000", 41),
    ("0001 0111", 14), ("0001 0110", 50), ("0001 0101", 22), ("0001 0100", 42), ("0001 0011", 15), ("0001 0010", 51),
    ("0001 0001", 23), ("0001 0000", 43), ("0000 1111", 25), ("0000 1110", 37), ("0000 1101", 26), ("0000 1100", 38),
    ("0000 1011", 29), ("0000 1010", 45), ("0000 1001", 53), ("0000 1000", 57), ("0000 0111", 30), ("0000 0110", 46),
    ("0000 0101", 54), ("0000 0100", 58), ("0000 0011 1", 31), ("0000 0011 0", 47), ("0000 0010 1", 55), ("0000 0010 0", 59),
    ("0000 0001 1", 27), ("0000 0001 0", 39), ("0000 0000 1", 0),
];

/// Table B-10 (`motion_code`), magnitudes; a non-zero code is followed by its sign bit.
const MOTION: &[(&str, i16)] = &[
    ("1", 0), ("01", 1), ("001", 2), ("0001", 3), ("0000 11", 4), ("0000 101", 5), ("0000 100", 6), ("0000 011", 7),
    ("0000 0101 1", 8), ("0000 0101 0", 9), ("0000 0100 1", 10), ("0000 0100 01", 11), ("0000 0100 00", 12),
    ("0000 0011 11", 13), ("0000 0011 10", 14), ("0000 0011 01", 15), ("0000 0011 00", 16),
];

/// Table B-12 (`dct_dc_size_luminance`).
const DC_LUMA: &[(&str, i16)] = &[
    ("100", 0), ("00", 1), ("01", 2), ("101", 3), ("110", 4), ("1110", 5), ("1111 0", 6), ("1111 10", 7), ("1111 110", 8),
    ("1111 1110", 9), ("1111 1111 0", 10), ("1111 1111 1", 11),
];

/// Table B-13 (`dct_dc_size_chrominance`).
const DC_CHROMA: &[(&str, i16)] = &[
    ("00", 0), ("01", 1), ("10", 2), ("110", 3), ("1110", 4), ("1111 0", 5), ("1111 10", 6), ("1111 110", 7), ("1111 1110", 8),
    ("1111 1111 0", 9), ("1111 1111 10", 10), ("1111 1111 11", 11),
];

/// The codes of 14 to 16 bits (without the sign), common to tables B-14 and B-15.
const DCT_LONG: &[(&str, i16)] = &[
    ("0000 0000 0111 11", rl(0, 16)), ("0000 0000 0111 10", rl(0, 17)), ("0000 0000 0111 01", rl(0, 18)), ("0000 0000 0111 00", rl(0, 19)),
    ("0000 0000 0110 11", rl(0, 20)), ("0000 0000 0110 10", rl(0, 21)), ("0000 0000 0110 01", rl(0, 22)), ("0000 0000 0110 00", rl(0, 23)),
    ("0000 0000 0101 11", rl(0, 24)), ("0000 0000 0101 10", rl(0, 25)), ("0000 0000 0101 01", rl(0, 26)), ("0000 0000 0101 00", rl(0, 27)),
    ("0000 0000 0100 11", rl(0, 28)), ("0000 0000 0100 10", rl(0, 29)), ("0000 0000 0100 01", rl(0, 30)), ("0000 0000 0100 00", rl(0, 31)),
    ("0000 0000 0011 000", rl(0, 32)), ("0000 0000 0010 111", rl(0, 33)), ("0000 0000 0010 110", rl(0, 34)), ("0000 0000 0010 101", rl(0, 35)),
    ("0000 0000 0010 100", rl(0, 36)), ("0000 0000 0010 011", rl(0, 37)), ("0000 0000 0010 010", rl(0, 38)), ("0000 0000 0010 001", rl(0, 39)),
    ("0000 0000 0010 000", rl(0, 40)), ("0000 0000 0011 111", rl(1, 8)), ("0000 0000 0011 110", rl(1, 9)), ("0000 0000 0011 101", rl(1, 10)),
    ("0000 0000 0011 100", rl(1, 11)), ("0000 0000 0011 011", rl(1, 12)), ("0000 0000 0011 010", rl(1, 13)), ("0000 0000 0011 001", rl(1, 14)),
    ("0000 0000 0001 0011", rl(1, 15)), ("0000 0000 0001 0010", rl(1, 16)), ("0000 0000 0001 0001", rl(1, 17)), ("0000 0000 0001 0000", rl(1, 18)),
    ("0000 0000 0001 0100", rl(6, 3)), ("0000 0000 0001 1010", rl(11, 2)), ("0000 0000 0001 1001", rl(12, 2)), ("0000 0000 0001 1000", rl(13, 2)),
    ("0000 0000 0001 0111", rl(14, 2)), ("0000 0000 0001 0110", rl(15, 2)), ("0000 0000 0001 0101", rl(16, 2)), ("0000 0000 0001 1111", rl(27, 1)),
    ("0000 0000 0001 1110", rl(28, 1)), ("0000 0000 0001 1101", rl(29, 1)), ("0000 0000 0001 1100", rl(30, 1)), ("0000 0000 0001 1011", rl(31, 1)),
];

/// Table B-14 (DCT coefficients, table zero), without the sign bits and without the codes of [`DCT_LONG`].
/// "11" is (0, 1) except as the first coefficient of a non-intra block, where "1" is (see the decoder).
const DCT_B14: &[(&str, i16)] = &[
    ("10", DCT_EOB), ("11", rl(0, 1)), ("011", rl(1, 1)), ("0100", rl(0, 2)), ("0101", rl(2, 1)), ("0010 1", rl(0, 3)), ("0011 1", rl(3, 1)),
    ("0011 0", rl(4, 1)), ("0001 10", rl(1, 2)), ("0001 11", rl(5, 1)), ("0001 01", rl(6, 1)), ("0001 00", rl(7, 1)), ("0000 110", rl(0, 4)),
    ("0000 100", rl(2, 2)), ("0000 111", rl(8, 1)), ("0000 101", rl(9, 1)), ("0000 01", DCT_ESCAPE), ("0010 0110", rl(0, 5)),
    ("0010 0001", rl(0, 6)), ("0010 0101", rl(1, 3)), ("0010 0100", rl(3, 2)), ("0010 0111", rl(10, 1)), ("0010 0011", rl(11, 1)),
    ("0010 0010", rl(12, 1)), ("0010 0000", rl(13, 1)), ("0000 0010 10", rl(0, 7)), ("0000 0011 00", rl(1, 4)), ("0000 0010 11", rl(2, 3)),
    ("0000 0011 11", rl(4, 2)), ("0000 0010 01", rl(5, 2)), ("0000 0011 10", rl(14, 1)), ("0000 0011 01", rl(15, 1)), ("0000 0010 00", rl(16, 1)),
    ("0000 0001 1101", rl(0, 8)), ("0000 0001 1000", rl(0, 9)), ("0000 0001 0011", rl(0, 10)), ("0000 0001 0000", rl(0, 11)),
    ("0000 0001 1011", rl(1, 5)), ("0000 0001 0100", rl(2, 4)), ("0000 0001 1100", rl(3, 3)), ("0000 0001 0010", rl(4, 3)),
    ("0000 0001 1110", rl(6, 2)), ("0000 0001 0101", rl(7, 2)), ("0000 0001 0001", rl(8, 2)), ("0000 0001 1111", rl(17, 1)),
    ("0000 0001 1010", rl(18, 1)), ("0000 0001 1001", rl(19, 1)), ("0000 0001 0111", rl(20, 1)), ("0000 0001 0110", rl(21, 1)),
    ("0000 0000 1101 0", rl(0, 12)), ("0000 0000 1100 1", rl(0, 13)), ("0000 0000 1100 0", rl(0, 14)), ("0000 0000 1011 1", rl(0, 15)),
    ("0000 0000 1011 0", rl(1, 6)), ("0000 0000 1010 1", rl(1, 7)), ("0000 0000 1010 0", rl(2, 5)), ("0000 0000 1001 1", rl(3, 4)),
    ("0000 0000 1001 0", rl(5, 3)), ("0000 0000 1000 1", rl(9, 2)), ("0000 0000 1000 0", rl(10, 2)), ("0000 0000 1111 1", rl(22, 1)),
    ("0000 0000 1111 0", rl(23, 1)), ("0000 0000 1110 1", rl(24, 1)), ("0000 0000 1110 0", rl(25, 1)), ("0000 0000 1101 1", rl(26, 1)),
];

/// Table B-15 (DCT coefficients, table one: intra blocks with `intra_vlc_format` = 1), without the sign bits and
/// without the codes of [`DCT_LONG`].
const DCT_B15: &[(&str, i16)] = &[
    ("0110", DCT_EOB), ("10", rl(0, 1)), ("010", rl(1, 1)), ("110", rl(0, 2)), ("0010 1", rl(2, 1)), ("0111", rl(0, 3)), ("0011 1", rl(3, 1)),
    ("0001 10", rl(4, 1)), ("0011 0", rl(1, 2)), ("0001 11", rl(5, 1)), ("0000 110", rl(6, 1)), ("0000 100", rl(7, 1)), ("1110 0", rl(0, 4)),
    ("0000 111", rl(2, 2)), ("0000 101", rl(8, 1)), ("1111 000", rl(9, 1)), ("0000 01", DCT_ESCAPE), ("1110 1", rl(0, 5)),
    ("0001 01", rl(0, 6)), ("1111 001", rl(1, 3)), ("0010 0110", rl(3, 2)), ("1111 010", rl(10, 1)), ("0010 0001", rl(11, 1)),
    ("0010 0101", rl(12, 1)), ("0010 0100", rl(13, 1)), ("0001 00", rl(0, 7)), ("0010 0111", rl(1, 4)), ("1111 1100", rl(2, 3)),
    ("1111 1101", rl(4, 2)), ("0000 0010 0", rl(5, 2)), ("0000 0010 1", rl(14, 1)), ("0000 0011 1", rl(15, 1)), ("0000 0011 01", rl(16, 1)),
    ("1111 011", rl(0, 8)), ("1111 100", rl(0, 9)), ("0010 0011", rl(0, 10)), ("0010 0010", rl(0, 11)), ("0010 0000", rl(1, 5)),
    ("0000 0011 00", rl(2, 4)), ("0000 0001 1100", rl(3, 3)), ("0000 0001 0010", rl(4, 3)), ("0000 0001 1110", rl(6, 2)),
    ("0000 0001 0101", rl(7, 2)), ("0000 0001 0001", rl(8, 2)), ("0000 0001 1111", rl(17, 1)), ("0000 0001 1010", rl(18, 1)),
    ("0000 0001 1001", rl(19, 1)), ("0000 0001 0111", rl(20, 1)), ("0000 0001 0110", rl(21, 1)), ("1111 1010", rl(0, 12)),
    ("1111 1011", rl(0, 13)), ("1111 1110", rl(0, 14)), ("1111 1111", rl(0, 15)), ("0000 0000 1011 0", rl(1, 6)),
    ("0000 0000 1010 1", rl(1, 7)), ("0000 0000 1010 0", rl(2, 5)), ("0000 0000 1001 1", rl(3, 4)), ("0000 0000 1001 0", rl(5, 3)),
    ("0000 0000 1000 1", rl(9, 2)), ("0000 0000 1000 0", rl(10, 2)), ("0000 0000 1111 1", rl(22, 1)), ("0000 0000 1111 0", rl(23, 1)),
    ("0000 0000 1110 1", rl(24, 1)), ("0000 0000 1110 0", rl(25, 1)), ("0000 0000 1101 1", rl(26, 1)),
];

/// All tables, built once.
pub struct Tables {
    pub mba: Vlc,
    pub mb_type: [Vlc; 3],
    pub cbp: Vlc,
    pub motion: Vlc,
    pub dc_luma: Vlc,
    pub dc_chroma: Vlc,
    pub dct_b14: Vlc,
    pub dct_b15: Vlc,
}

pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Tables {
        mba: Vlc::build(MBA),
        mb_type: [Vlc::build(MB_TYPE_I), Vlc::build(MB_TYPE_P), Vlc::build(MB_TYPE_B)],
        cbp: Vlc::build(CBP),
        motion: Vlc::build(MOTION),
        dc_luma: Vlc::build(DC_LUMA),
        dc_chroma: Vlc::build(DC_CHROMA),
        dct_b14: Vlc::build(&[DCT_B14, DCT_LONG].concat()),
        dct_b15: Vlc::build(&[DCT_B15, DCT_LONG].concat()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_levels(t: &[(&str, i16)]) -> Vec<i16> {
        let mut v: Vec<i16> = t.iter().chain(DCT_LONG).map(|e| e.1).filter(|&x| x >= 0).collect();
        v.sort();
        v
    }

    /// The tables are prefix codes (the build panics otherwise) with the expected sizes and code spaces:
    /// the macroblock-type, DC-size and motion codes are complete apart from the codes the standard leaves unused;
    /// both DCT tables code the same 111 (run, level) pairs plus EOB and escape.
    #[test]
    fn tables_are_consistent() {
        let t = tables();
        assert_eq!(t.mb_type[0].kraft(), 0.75);
        assert_eq!(t.mb_type[1].kraft(), 1.0 - 1.0 / 64.0);
        assert_eq!(t.mb_type[2].kraft(), 1.0 - 1.0 / 64.0);
        assert_eq!(t.dc_luma.kraft(), 1.0);
        assert_eq!(t.dc_chroma.kraft(), 1.0);
        // Motion magnitudes (the sign bit follows): "1" plus lengths 2, 3, 4, 6, 7×3, 9×3, 10×6; the unused codes
        // 0000 0000 xx and 0000 0001 xx are 3/256 of the space.
        assert_eq!(t.motion.kraft(), 0.98828125);
        assert_eq!(MBA.len(), 35);
        assert_eq!(CBP.len(), 64);
        let (a, b) = (run_levels(DCT_B14), run_levels(DCT_B15));
        assert_eq!(a.len(), 111);
        assert_eq!(a, b);
        a.windows(2).for_each(|w| assert!(w[0] < w[1], "duplicate run/level {:#x}", w[0]));
        // Every 6-bit CBP value except 0 appears once (0 only as the 4:2:2 code).
        let mut cbps: Vec<i16> = CBP.iter().map(|e| e.1).collect();
        cbps.sort();
        assert_eq!(cbps, (0..64).collect::<Vec<i16>>());
    }

    #[test]
    fn decodes_codes() {
        let t = tables();
        // "0000 0101 10" = increment 17, then "011" = 2.
        let d = [0b0000_0101u8, 0b1001_1000];
        let mut r = BitReader::new(&d, 0);
        assert_eq!(t.mba.decode(&mut r), Some(17));
        assert_eq!(t.mba.decode(&mut r), Some(2));
        assert_eq!(r.bit_pos(), 13);
        // B-15 "0110" = EOB, B-14 "0000 01" = escape.
        let d = [0b0110_0000u8, 0b0100_0000];
        let mut r = BitReader::new(&d, 0);
        assert_eq!(t.dct_b15.decode(&mut r), Some(DCT_EOB));
        assert_eq!(t.dct_b14.decode(&mut r), Some(DCT_ESCAPE));
    }
}
