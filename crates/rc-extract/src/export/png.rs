//! A small PNG writer (no dependencies): CRC-32, Adler-32, and a zlib stream of one fixed-Huffman deflate block
//! with greedy LZ77 matching (hash chains, 32 KiB window). Enough for exported textures to be a fraction of their
//! raw size and readable by every PNG decoder.
//!
//! Two pixel forms:
//! * **indexed** (colour type 3, 8-bit): the PS2 textures are 8-bit indices into a 256-entry CLUT, so the PNG keeps
//!   the indices exactly; `PLTE` holds the CLUT in linear (un-swizzled) order and `tRNS` its alpha scaled
//!   0x80 → 255. Decoding gives exactly `rc_formats::texture::decode_indexed8`.
//! * **RGBA** (colour type 6, 8-bit), for images that are not paletted (decoded mip chains).
//!
//! Scanlines use filter 0 (none). The tests decode our own output with an in-test inflater (stored and fixed
//! blocks) and check every chunk CRC and the Adler-32.

const fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
}
const CRC_TABLE: [u32; 256] = crc_table();

/// CRC-32 (ISO 3309 / PNG), continuing from `crc` (start with 0).
pub fn crc32(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in data { c = CRC_TABLE[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8); }
    !c
}

/// Adler-32 (zlib).
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk { a += x as u32; b += a; }
        a %= 65521;
        b %= 65521;
    }
    b << 16 | a
}

struct BitWriter { out: Vec<u8>, acc: u64, n: u32 }

impl BitWriter {
    /// `len` bits of `v`, least significant first (deflate's bit order for everything but Huffman codes).
    fn bits(&mut self, v: u32, len: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += len;
        while self.n >= 8 { self.out.push(self.acc as u8); self.acc >>= 8; self.n -= 8; }
    }
    /// A Huffman code, most significant bit first.
    fn code(&mut self, code: u32, len: u32) { self.bits(code.reverse_bits() >> (32 - len), len); }
    fn finish(mut self) -> Vec<u8> { if self.n > 0 { self.out.push(self.acc as u8); } self.out }
}

const LEN_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// Fixed literal/length code (RFC 1951 §3.2.6).
fn lit(w: &mut BitWriter, sym: u32) {
    match sym {
        0..=143 => w.code(0x30 + sym, 8),
        144..=255 => w.code(0x190 + sym - 144, 9),
        256..=279 => w.code(sym - 256, 7),
        _ => w.code(0xc0 + sym - 280, 8),
    }
}

fn length_distance(w: &mut BitWriter, len: usize, dist: usize) {
    let li = LEN_BASE.iter().rposition(|&b| b as usize <= len).unwrap();
    lit(w, 257 + li as u32);
    w.bits((len - LEN_BASE[li] as usize) as u32, LEN_EXTRA[li] as u32);
    let di = DIST_BASE.iter().rposition(|&b| b as usize <= dist).unwrap();
    w.code(di as u32, 5);
    w.bits((dist - DIST_BASE[di] as usize) as u32, DIST_EXTRA[di] as u32);
}

const WINDOW: usize = 32 * 1024;
const MAX_MATCH: usize = 258;
const MAX_CHAIN: usize = 48;
const HASH_BITS: u32 = 15;

/// Raw deflate: one final fixed-Huffman block, greedy LZ77.
pub fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter { out: Vec::with_capacity(data.len() / 2 + 16), acc: 0, n: 0 };
    w.bits(1, 1); // BFINAL
    w.bits(1, 2); // BTYPE = 01, fixed Huffman
    let n = data.len();
    let hash = |i: usize| (((data[i] as u32) << 10 ^ (data[i + 1] as u32) << 5 ^ data[i + 2] as u32).wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize;
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let insert = |head: &mut [usize], prev: &mut [usize], p: usize| {
        if p + 3 <= n {
            let h = hash(p);
            prev[p % WINDOW] = head[h];
            head[h] = p;
        }
    };
    let mut i = 0;
    while i < n {
        let (mut best, mut dist) = (0usize, 0usize);
        if i + 3 <= n {
            let max = (n - i).min(MAX_MATCH);
            let mut cand = head[hash(i)];
            let mut chain = 0;
            while cand != usize::MAX && cand < i && i - cand <= WINDOW && chain < MAX_CHAIN {
                if data[cand + best.min(max - 1)] == data[i + best.min(max - 1)] {
                    let l = data[cand..cand + max].iter().zip(&data[i..i + max]).take_while(|(a, b)| a == b).count();
                    if l > best { best = l; dist = i - cand; if l == max { break; } }
                }
                cand = prev[cand % WINDOW];
                chain += 1;
            }
        }
        if best >= 3 {
            length_distance(&mut w, best, dist);
            for p in i..i + best { insert(&mut head, &mut prev, p); }
            i += best;
        } else {
            lit(&mut w, data[i] as u32);
            insert(&mut head, &mut prev, i);
            i += 1;
        }
    }
    lit(&mut w, 256);
    w.finish()
}

/// zlib stream (RFC 1950): CMF 0x78 (deflate, 32 KiB window), FLG 0x01, the deflate data, Adler-32.
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    out.extend(deflate_fixed(data));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(crc32(0, ty), data).to_be_bytes());
}

fn png(width: u32, height: u32, color_type: u8, bpp: usize, pixels: &[u8], extra: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    assert!(width > 0 && height > 0 && pixels.len() == width as usize * height as usize * bpp, "png: pixel buffer size");
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, color_type, 0, 0, 0]); // bit depth 8, deflate, filter method 0, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    for (ty, data) in extra { chunk(&mut out, ty, data); }
    let row = width as usize * bpp;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for r in pixels.chunks_exact(row) { raw.push(0); raw.extend_from_slice(r); }
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// 8-bit indexed PNG. `palette` is RGBA per index (linear order); entries past the last used one are kept so the
/// file carries the whole CLUT.
pub fn encode_indexed(width: u32, height: u32, indices: &[u8], palette: &[[u8; 4]]) -> Vec<u8> {
    assert!(!palette.is_empty() && palette.len() <= 256, "png: palette size");
    let plte: Vec<u8> = palette.iter().flat_map(|c| [c[0], c[1], c[2]]).collect();
    let mut trns: Vec<u8> = palette.iter().map(|c| c[3]).collect();
    while trns.last() == Some(&255) { trns.pop(); }
    let mut extra = vec![(b"PLTE", plte)];
    if !trns.is_empty() { extra.push((b"tRNS", trns)); }
    png(width, height, 3, 1, indices, &extra)
}

/// 8-bit RGBA PNG.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> { png(width, height, 6, 4, rgba, &[]) }

#[cfg(test)]
pub(crate) mod decode {
    //! Test-only PNG reader for our own output: chunk CRCs, zlib header and Adler-32, stored and fixed-Huffman
    //! deflate blocks, filter 0 scanlines, colour types 3 and 6 at 8 bits.

    use super::*;

    struct Bits<'a> { d: &'a [u8], pos: usize }

    impl Bits<'_> {
        fn bit(&mut self) -> Result<u32, String> {
            let b = *self.d.get(self.pos / 8).ok_or("deflate: out of data")?;
            let v = (b >> (self.pos % 8)) & 1;
            self.pos += 1;
            Ok(v as u32)
        }
        fn bits(&mut self, n: u32) -> Result<u32, String> { let mut v = 0; for k in 0..n { v |= self.bit()? << k; } Ok(v) }
        fn code(&mut self, n: u32) -> Result<u32, String> { let mut v = 0; for _ in 0..n { v = v << 1 | self.bit()?; } Ok(v) }
    }

    fn fixed_lit(b: &mut Bits) -> Result<u32, String> {
        let c7 = b.code(7)?;
        if c7 <= 0x17 { return Ok(256 + c7); }
        let c8 = c7 << 1 | b.bit()?;
        if (0x30..=0xbf).contains(&c8) { return Ok(c8 - 0x30); }
        if (0xc0..=0xc7).contains(&c8) { return Ok(280 + c8 - 0xc0); }
        let c9 = c8 << 1 | b.bit()?;
        if (0x190..=0x1ff).contains(&c9) { return Ok(144 + c9 - 0x190); }
        Err("deflate: bad fixed code".into())
    }

    pub fn inflate(d: &[u8]) -> Result<Vec<u8>, String> {
        let mut b = Bits { d, pos: 0 };
        let mut out: Vec<u8> = Vec::new();
        loop {
            let last = b.bits(1)?;
            match b.bits(2)? {
                0 => {
                    b.pos = b.pos.div_ceil(8) * 8;
                    let at = b.pos / 8;
                    let len = u16::from_le_bytes([d[at], d[at + 1]]) as usize;
                    let nlen = u16::from_le_bytes([d[at + 2], d[at + 3]]) as usize;
                    if len != !nlen & 0xffff { return Err("deflate: stored LEN/NLEN".into()); }
                    out.extend_from_slice(d.get(at + 4..at + 4 + len).ok_or("deflate: stored block past end")?);
                    b.pos = (at + 4 + len) * 8;
                }
                1 => loop {
                    let s = fixed_lit(&mut b)?;
                    match s {
                        0..=255 => out.push(s as u8),
                        256 => break,
                        257..=285 => {
                            let i = (s - 257) as usize;
                            let len = LEN_BASE[i] as usize + b.bits(LEN_EXTRA[i] as u32)? as usize;
                            let di = b.code(5)? as usize;
                            if di >= 30 { return Err("deflate: bad distance code".into()); }
                            let dist = DIST_BASE[di] as usize + b.bits(DIST_EXTRA[di] as u32)? as usize;
                            if dist > out.len() { return Err("deflate: distance before start".into()); }
                            for _ in 0..len { out.push(out[out.len() - dist]); }
                        }
                        _ => return Err("deflate: bad length symbol".into()),
                    }
                },
                t => return Err(format!("deflate: block type {t} not supported by the test reader")),
            }
            if last == 1 { return Ok(out); }
        }
    }

    pub struct Image { pub width: u32, pub height: u32, pub color_type: u8, pub rgba: Vec<u8>, pub indices: Option<Vec<u8>>, pub palette: Vec<[u8; 4]> }

    pub fn decode(png: &[u8]) -> Result<Image, String> {
        if png.get(..8) != Some(b"\x89PNG\r\n\x1a\n") { return Err("png: signature".into()); }
        let mut at = 8;
        let (mut ihdr, mut idat, mut plte, mut trns) = (None, Vec::new(), None, None);
        let mut ended = false;
        while at < png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let ty: [u8; 4] = png[at + 4..at + 8].try_into().unwrap();
            let data = png.get(at + 8..at + 8 + len).ok_or("png: chunk past end")?;
            let crc = u32::from_be_bytes(png.get(at + 8 + len..at + 12 + len).ok_or("png: crc past end")?.try_into().unwrap());
            if crc != crc32(crc32(0, &ty), data) { return Err(format!("png: bad CRC in {}", String::from_utf8_lossy(&ty))); }
            match &ty {
                b"IHDR" => ihdr = Some(data.to_vec()),
                b"PLTE" => plte = Some(data.to_vec()),
                b"tRNS" => trns = Some(data.to_vec()),
                b"IDAT" => idat.extend_from_slice(data),
                b"IEND" => { ended = true; }
                _ => {}
            }
            at += 12 + len;
        }
        if !ended || at != png.len() { return Err("png: no IEND or trailing bytes".into()); }
        let h = ihdr.ok_or("png: no IHDR")?;
        let (width, height) = (u32::from_be_bytes(h[0..4].try_into().unwrap()), u32::from_be_bytes(h[4..8].try_into().unwrap()));
        let (depth, color_type) = (h[8], h[9]);
        if depth != 8 || h[10] != 0 || h[11] != 0 || h[12] != 0 { return Err("png: unsupported IHDR".into()); }
        if idat.len() < 6 || idat[0] != 0x78 || !(idat[0] as u32 * 256 + idat[1] as u32).is_multiple_of(31) { return Err("png: zlib header".into()); }
        let raw = inflate(&idat[2..idat.len() - 4])?;
        if adler32(&raw).to_be_bytes() != idat[idat.len() - 4..] { return Err("png: adler32".into()); }
        let bpp = match color_type { 3 => 1, 6 => 4, _ => return Err("png: colour type".into()) };
        let row = width as usize * bpp;
        if raw.len() != (row + 1) * height as usize { return Err("png: image data size".into()); }
        let mut px = Vec::with_capacity(row * height as usize);
        for r in raw.chunks_exact(row + 1) {
            if r[0] != 0 { return Err("png: filter type".into()); }
            px.extend_from_slice(&r[1..]);
        }
        if color_type == 6 { return Ok(Image { width, height, color_type, rgba: px, indices: None, palette: Vec::new() }); }
        let plte = plte.ok_or("png: indexed without PLTE")?;
        let trns = trns.unwrap_or_default();
        let palette: Vec<[u8; 4]> = plte.as_chunks::<3>().0.iter().enumerate().map(|(i, c)| [c[0], c[1], c[2], trns.get(i).copied().unwrap_or(255)]).collect();
        let mut rgba = Vec::with_capacity(px.len() * 4);
        for &i in &px { rgba.extend_from_slice(palette.get(i as usize).ok_or("png: index past palette")?); }
        Ok(Image { width, height, color_type, rgba, indices: Some(px), palette })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_reference_values() {
        assert_eq!(crc32(0, b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(0, b"IEND"), 0xae42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        assert_eq!(adler32(&[]), 1);
    }

    #[test]
    fn deflate_round_trips_through_the_reader() {
        let mut rng = 0x1234_5678u32;
        let mut noise = Vec::new();
        for _ in 0..70_000 { rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5; noise.push((rng >> 24) as u8 & 0x0f); }
        let cases: Vec<Vec<u8>> = vec![
            vec![],
            b"a".to_vec(),
            b"abcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabc".to_vec(),
            vec![7; 100_000],
            (0..=255u8).cycle().take(90_000).collect(),
            noise,
        ];
        for c in cases {
            let z = deflate_fixed(&c);
            assert_eq!(decode::inflate(&z).unwrap(), c, "length {}", c.len());
        }
        // Long runs compress to almost nothing.
        assert!(deflate_fixed(&[7; 100_000]).len() < 1_000);
    }

    #[test]
    fn indexed_and_rgba_pngs_decode_to_their_pixels() {
        let palette: Vec<[u8; 4]> = (0..256u32).map(|i| [i as u8, (255 - i) as u8, (i * 7) as u8, if i < 4 { (i * 60) as u8 } else { 255 }]).collect();
        let (w, h) = (37u32, 19u32);
        let indices: Vec<u8> = (0..w * h).map(|i| (i * 31 % 256) as u8).collect();
        let img = decode::decode(&encode_indexed(w, h, &indices, &palette)).unwrap();
        assert_eq!((img.width, img.height, img.color_type), (w, h, 3));
        assert_eq!(img.indices.as_deref(), Some(&indices[..]));
        assert_eq!(img.palette, palette);
        let expect: Vec<u8> = indices.iter().flat_map(|&i| palette[i as usize]).collect();
        assert_eq!(img.rgba, expect);

        let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i % 253) as u8).collect();
        let img = decode::decode(&encode_rgba(w, h, &rgba)).unwrap();
        assert_eq!((img.width, img.height, img.color_type), (w, h, 6));
        assert_eq!(img.rgba, rgba);

        // A corrupted byte is caught by the chunk CRC.
        let mut bad = encode_rgba(2, 2, &[1; 16]);
        let n = bad.len();
        bad[n - 20] ^= 1;
        assert!(decode::decode(&bad).is_err());
    }
}
