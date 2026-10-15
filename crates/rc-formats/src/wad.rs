//! Insomniac's "WAD" LZ77 stream. Spec: docs/formats/disc_layout.md section 3.
//! Verified on retail data: all 4674 streams on the NTSC-U disc decode, and every
//! level's decompressed core data size matches its core index.

use crate::buf::{invalid, Buf, Result};

pub const WAD_HEADER_SIZE: usize = 0x10;

pub fn is_wad(b: &[u8]) -> bool { b.len() >= WAD_HEADER_SIZE && &b[..3] == b"WAD" }

/// Total stream size in bytes including the 16-byte header.
pub fn compressed_size(b: &[u8]) -> Result<u32> {
    if !is_wad(b) { return invalid("not a WAD stream"); }
    Ok(u32::from_le_bytes([b[3], b[4], b[5], b[6]]))
}

pub fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    let total = compressed_size(bytes)? as usize;
    if total < WAD_HEADER_SIZE || total > bytes.len() { return invalid("WAD: compressed size exceeds buffer"); }
    let stream = Buf(&bytes[WAD_HEADER_SIZE..total]);
    let end = stream.len();
    let mut ptr = 0usize;
    let mut out: Vec<u8> = Vec::with_capacity(total * 2);

    let next = |ptr: &mut usize| -> Result<u8> {
        if *ptr >= end { return invalid("WAD: read past end of stream"); }
        let v = stream.0[*ptr];
        *ptr += 1;
        Ok(v)
    };
    let literals = |ptr: &mut usize, out: &mut Vec<u8>, n: usize| -> Result<()> {
        if end - *ptr < n { return invalid("WAD: literal runs past end of stream"); }
        out.extend_from_slice(&stream.0[*ptr..*ptr + n]);
        *ptr += n;
        Ok(())
    };
    let copy_match = |out: &mut Vec<u8>, displacement: usize, len: usize| -> Result<()> {
        if displacement == 0 || displacement > out.len() { return invalid("WAD: match source before start of output"); }
        let src = out.len() - displacement;
        for i in 0..len { let v = out[src + i]; out.push(v); }   // overlapping copies are run-length expansion
        Ok(())
    };

    while ptr < end {
        let flag = next(&mut ptr)?;
        if flag < 0x10 {
            let n = if flag == 0 { next(&mut ptr)? as usize + 18 } else { flag as usize + 3 };
            literals(&mut ptr, &mut out, n)?;
            if ptr < end && stream.0[ptr] < 0x10 { return invalid("WAD: adjacent literal packets"); }
            continue;
        }
        let (match_len, displacement, little): (usize, usize, u8);
        if flag < 0x20 {
            let mut ml = (flag & 7) as usize;
            if ml == 0 { ml = next(&mut ptr)? as usize + 7; }
            let b0 = next(&mut ptr)?;
            let b1 = next(&mut ptr)?;
            let a = ((flag >> 3) & 1) as usize;
            let far = b1 as usize * 0x40 + (b0 >> 2) as usize;
            if a == 0 && far == 0 {
                if ml != 1 {
                    ptr = (ptr + 0xFFF) & !0xFFF;   // pad packet: skip to the next 0x1000 boundary
                    continue;
                }
                literals(&mut ptr, &mut out, (b0 & 3) as usize)?;   // dummy packet
                continue;
            }
            displacement = 0x4000 * (a + 1) + far;   // bit 3 extends the window to 15 bits (verified on retail data)
            match_len = ml + 2;
            little = b0;
        } else if flag < 0x40 {
            let mut ml = (flag & 0x1f) as usize;
            if ml == 0 { ml = next(&mut ptr)? as usize + 0x1f; }
            match_len = ml + 2;
            let b1 = next(&mut ptr)?;
            let b2 = next(&mut ptr)?;
            displacement = b2 as usize * 0x40 + (b1 >> 2) as usize + 1;
            little = b1;
        } else {
            let b1 = next(&mut ptr)?;
            match_len = (flag >> 5) as usize + 1;
            displacement = b1 as usize * 8 + ((flag >> 2) & 7) as usize + 1;
            little = flag;
        }
        copy_match(&mut out, displacement, match_len)?;
        literals(&mut ptr, &mut out, (little & 3) as usize)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wad(packets: &[u8]) -> Vec<u8> {
        let mut v = b"WAD\0\0\0\0TEST\0\0\0\0\0".to_vec();
        v.extend_from_slice(packets);
        let size = v.len() as u32;
        v[3..7].copy_from_slice(&size.to_le_bytes());
        v
    }
    #[test]
    fn literal_then_little_match() {
        let out = decompress(&wad(&[0x01, b'a', b'b', b'c', b'd', 0xC0, 0x00])).unwrap();
        assert_eq!(out, b"abcdddddddd");
    }
    #[test]
    fn medium_match_with_little_literal() {
        let out = decompress(&wad(&[0x01, b'x', b'y', b'z', b'w', 0x21, 0x02, 0x00, b'!', b'?'])).unwrap();
        assert_eq!(out, b"xyzwwww!?");
    }
    #[test]
    fn dummy_packet_and_pad() {
        assert_eq!(decompress(&wad(&[0x01, b'a', b'b', b'c', b'd', 0x11, 0x02, 0x00, b'e', b'f'])).unwrap(), b"abcdef");
        assert_eq!(decompress(&wad(&[0x01, b'a', b'b', b'c', b'd', 0x12, 0x00, 0x00, 0xEE])).unwrap(), b"abcd");
        assert!(decompress(&wad(&[0x01, b'a', b'b', b'c', b'd', 0x01, b'e', b'f', b'g', b'h'])).is_err());
    }
}
