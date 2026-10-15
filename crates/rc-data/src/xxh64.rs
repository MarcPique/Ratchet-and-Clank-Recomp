//! XXH64 (Yann Collet's xxHash, 64-bit variant, seed 0), written from the published algorithm. It is the cache's
//! corruption check: fast enough to run on every lump the engine reads (a few ms for a 24 MiB core), and any
//! `xxhsum -H1` reproduces it. Not a defence against deliberate tampering; Tier 0 has the SHA-1 table for that.

const P1: u64 = 0x9E37_79B1_85EB_CA87;
const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const P3: u64 = 0x1656_67B1_9E37_79F9;
const P4: u64 = 0x85EB_CA77_C2B2_AE63;
const P5: u64 = 0x27D4_EB2F_1656_67C5;

#[inline(always)]
fn round(acc: u64, input: u64) -> u64 { acc.wrapping_add(input.wrapping_mul(P2)).rotate_left(31).wrapping_mul(P1) }

#[inline(always)]
fn merge(acc: u64, v: u64) -> u64 { (acc ^ round(0, v)).wrapping_mul(P1).wrapping_add(P4) }

#[inline(always)]
fn u64_at(b: &[u8], i: usize) -> u64 { u64::from_le_bytes(b[i..i + 8].try_into().unwrap()) }

/// XXH64 of `data` with seed 0.
pub fn xxh64(data: &[u8]) -> u64 {
    let len = data.len();
    let mut h;
    let mut tail = data;
    if len >= 32 {
        let (mut v1, mut v2, mut v3, mut v4) = (P1.wrapping_add(P2), P2, 0u64, 0u64.wrapping_sub(P1));
        let (stripes, rest) = data.as_chunks::<32>();
        for s in stripes {
            v1 = round(v1, u64_at(s, 0));
            v2 = round(v2, u64_at(s, 8));
            v3 = round(v3, u64_at(s, 16));
            v4 = round(v4, u64_at(s, 24));
        }
        tail = rest;
        h = v1.rotate_left(1).wrapping_add(v2.rotate_left(7)).wrapping_add(v3.rotate_left(12)).wrapping_add(v4.rotate_left(18));
        h = merge(merge(merge(merge(h, v1), v2), v3), v4);
    } else {
        h = P5;
    }
    h = h.wrapping_add(len as u64);
    let (words, mut rest) = tail.as_chunks::<8>();
    for w in words {
        h = (h ^ round(0, u64::from_le_bytes(*w))).rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
    }
    if rest.len() >= 4 {
        let k = u32::from_le_bytes(rest[..4].try_into().unwrap()) as u64;
        h = (h ^ k.wrapping_mul(P1)).rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        rest = &rest[4..];
    }
    for &b in rest {
        h = (h ^ (b as u64).wrapping_mul(P5)).rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

#[cfg(test)]
mod tests {
    use super::xxh64;

    #[test]
    fn reference_vectors() {
        // The xxHash reference results (seed 0): the empty input, the short path, and a 39-byte input that takes
        // the 32-byte stripe path plus the 8-, 4- and 1-byte tails.
        assert_eq!(xxh64(b""), 0xEF46_DB37_51D8_E999);
        assert_eq!(xxh64(b"a"), 0xD24E_C4F1_A98C_6E5B);
        assert_eq!(xxh64(b"abc"), 0x44BC_2CF5_AD77_0999);
        assert_eq!(xxh64(b"Nobody inspects the spammish repetition"), 0xFBCE_A83C_8A37_8BF1);
    }

    #[test]
    fn every_byte_matters() {
        let base: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 3) as u8).collect();
        let h = xxh64(&base);
        for i in [0, 31, 32, 500, 999] {
            let mut b = base.clone();
            b[i] ^= 1;
            assert_ne!(xxh64(&b), h, "byte {i}");
        }
        assert_ne!(xxh64(&base[..999]), h);
    }
}
