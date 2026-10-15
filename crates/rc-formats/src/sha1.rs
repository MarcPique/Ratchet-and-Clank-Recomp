//! SHA-1 (FIPS 180-4), in-workspace so the extractor stays dependency-free. Used only to identify builds, to check
//! files against the committed size/SHA-1 table (`ratchet_clank-extract`, re-exported as `rc_extract::sha1`) and to hash
//! the loader snapshots (`data/loader_snapshots.tsv`); not for anything security-relevant.

#[derive(Clone)]
pub struct Sha1 {
    h: [u32; 5],
    block: [u8; 64],
    fill: usize,
    len: u64,
}

impl Default for Sha1 {
    fn default() -> Self { Self::new() }
}

impl Sha1 {
    pub fn new() -> Self {
        Sha1 { h: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0], block: [0; 64], fill: 0, len: 0 }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.len = self.len.wrapping_add(data.len() as u64);
        if self.fill > 0 {
            let n = (64 - self.fill).min(data.len());
            self.block[self.fill..self.fill + n].copy_from_slice(&data[..n]);
            self.fill += n;
            data = &data[n..];
            if self.fill < 64 { return; }
            let block = self.block;
            compress(&mut self.h, &block);
            self.fill = 0;
        }
        let (blocks, rest) = data.as_chunks::<64>();
        for b in blocks { compress(&mut self.h, b); }
        self.block[..rest.len()].copy_from_slice(rest);
        self.fill = rest.len();
    }

    pub fn finish(mut self) -> [u8; 20] {
        let bits = self.len.wrapping_mul(8);
        let mut pad = [0u8; 72];
        pad[0] = 0x80;
        let n = if self.fill < 56 { 56 - self.fill } else { 120 - self.fill };
        pad[n..n + 8].copy_from_slice(&bits.to_be_bytes());
        let len = self.len;
        self.update(&pad[..n + 8]);
        self.len = len;
        debug_assert_eq!(self.fill, 0);
        let mut out = [0u8; 20];
        for (o, h) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.h) { o.copy_from_slice(&h.to_be_bytes()); }
        out
    }
}

fn compress(h: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (i, c) in block.as_chunks::<4>().0.iter().enumerate() { w[i] = u32::from_be_bytes(*c); }
    for i in 16..80 { w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1); }
    let [mut a, mut b, mut c, mut d, mut e] = *h;
    macro_rules! rounds {
        ($range:expr, $k:expr, |$b:ident, $c:ident, $d:ident| $f:expr) => {
            for &wi in &w[$range] {
                let ($b, $c, $d) = (b, c, d);
                let t = a.rotate_left(5).wrapping_add($f).wrapping_add(e).wrapping_add($k).wrapping_add(wi);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = t;
            }
        };
    }
    rounds!(0..20, 0x5a827999u32, |x, y, z| (x & y) | (!x & z));
    rounds!(20..40, 0x6ed9eba1u32, |x, y, z| x ^ y ^ z);
    rounds!(40..60, 0x8f1bbcdcu32, |x, y, z| (x & y) | (x & z) | (y & z));
    rounds!(60..80, 0xca62c1d6u32, |x, y, z| x ^ y ^ z);
    for (x, v) in h.iter_mut().zip([a, b, c, d, e]) { *x = x.wrapping_add(v); }
}

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut s = Sha1::new();
    s.update(data);
    s.finish()
}

pub fn hex(d: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    d.iter().flat_map(|b| [H[(b >> 4) as usize] as char, H[(b & 15) as usize] as char]).collect()
}

/// Parses 40 hex digits (either case).
pub fn parse_hex(s: &str) -> Option<[u8; 20]> {
    let s = s.as_bytes();
    if s.len() != 40 { return None; }
    let nib = |c: u8| (c as char).to_digit(16).map(|v| v as u8);
    let mut out = [0u8; 20];
    for (i, o) in out.iter_mut().enumerate() { *o = nib(s[2 * i])? << 4 | nib(s[2 * i + 1])?; }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FIPS 180-2 appendix A / NIST CAVP short-message vectors.
    #[test]
    fn standard_vectors() {
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
        assert_eq!(
            hex(&sha1(b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu")),
            "a49b2446a02c645bf419f995b67091253a04a259"
        );
        assert_eq!(hex(&sha1(b"The quick brown fox jumps over the lazy dog")), "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12");
        assert_eq!(hex(&sha1(&vec![b'a'; 1_000_000])), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
    }

    #[test]
    fn padding_boundaries_and_incremental_updates_agree() {
        let data: Vec<u8> = (0..1000u32).map(|i| (i * 31 + 7) as u8).collect();
        for len in [0, 1, 55, 56, 57, 63, 64, 65, 119, 120, 127, 128, 129, 1000] {
            let whole = sha1(&data[..len]);
            for split in [1, 3, 17, 63, 64, 65, 200] {
                let mut s = Sha1::new();
                for c in data[..len].chunks(split) { s.update(c); }
                assert_eq!(s.finish(), whole, "len {len} split {split}");
            }
        }
        assert_eq!(parse_hex(&hex(&sha1(b"abc"))), Some(sha1(b"abc")));
        assert_eq!(parse_hex("A9993E364706816ABA3E25717850C26C9CD0D89D"), Some(sha1(b"abc")));
        assert_eq!(parse_hex("xyz"), None);
    }
}
