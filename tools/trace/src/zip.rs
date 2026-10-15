//! Minimal ZIP reader for PCSX2 savestates (`.p2s`), plus a tiny writer used by the tests.
//!
//! PCSX2 >= 1.7 writes savestates with libzip (`pcsx2/SaveState.cpp`): one entry per component,
//! compressed with Zstandard (ZIP method 93, the default), Deflate (8) or stored (0), and the
//! version indicator always stored. We support exactly those three methods, ZIP64 sizes/offsets
//! (never needed for a 32 MiB entry, but cheap), and verify every entry's CRC-32.

use anyhow::{bail, ensure, Context, Result};
use std::io::Read;

pub const METHOD_STORE: u16 = 0;
pub const METHOD_DEFLATE: u16 = 8;
pub const METHOD_ZSTD: u16 = 93;

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub method: u16,
    pub crc32: u32,
    pub compressed_size: u64,
    pub size: u64,
    pub local_header_offset: u64,
}

pub struct Archive {
    bytes: Vec<u8>,
    pub entries: Vec<Entry>,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(b.get(o..o + 2).context("truncated zip")?.try_into().unwrap()))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(b.get(o..o + 4).context("truncated zip")?.try_into().unwrap()))
}
fn u64_at(b: &[u8], o: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(b.get(o..o + 8).context("truncated zip")?.try_into().unwrap()))
}

impl Archive {
    pub fn open(path: &std::path::Path) -> Result<Archive> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Archive::from_bytes(bytes).with_context(|| format!("parsing {} as a zip archive", path.display()))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Archive> {
        let b = &bytes[..];
        ensure!(b.len() >= 22, "file too small for a zip archive");
        // End of central directory: last occurrence of PK\5\6 within the final 64 KiB + 22 bytes.
        let lo = b.len().saturating_sub(22 + 0xffff);
        let eocd = (lo..=b.len() - 22).rev().find(|&i| &b[i..i + 4] == b"PK\x05\x06").context("no end-of-central-directory record (not a zip / .p2s?)")?;
        let mut count = u16_at(b, eocd + 10)? as u64;
        let mut cd_ofs = u32_at(b, eocd + 16)? as u64;
        // ZIP64 end-of-central-directory locator sits right before the EOCD.
        if eocd >= 20 && &b[eocd - 20..eocd - 16] == b"PK\x06\x07" {
            let z64 = u64_at(b, eocd - 20 + 8)? as usize;
            ensure!(b.get(z64..z64 + 4) == Some(b"PK\x06\x06"), "bad zip64 end-of-central-directory");
            count = u64_at(b, z64 + 32)?;
            cd_ofs = u64_at(b, z64 + 48)?;
        }
        let mut entries = Vec::new();
        let mut p = cd_ofs as usize;
        for _ in 0..count {
            ensure!(b.get(p..p + 4) == Some(b"PK\x01\x02"), "bad central directory entry at {p:#x}");
            let method = u16_at(b, p + 10)?;
            let crc32 = u32_at(b, p + 16)?;
            let mut csize = u32_at(b, p + 20)? as u64;
            let mut size = u32_at(b, p + 24)? as u64;
            let (nlen, xlen, clen) = (u16_at(b, p + 28)? as usize, u16_at(b, p + 30)? as usize, u16_at(b, p + 32)? as usize);
            let mut lho = u32_at(b, p + 42)? as u64;
            let name = String::from_utf8_lossy(b.get(p + 46..p + 46 + nlen).context("truncated name")?).into_owned();
            // ZIP64 extended information: present fields appear in order size, csize, offset.
            let mut x = p + 46 + nlen;
            let xend = x + xlen;
            while x + 4 <= xend {
                let (id, len) = (u16_at(b, x)?, u16_at(b, x + 2)? as usize);
                if id == 1 {
                    let mut q = x + 4;
                    if size == 0xffff_ffff { size = u64_at(b, q)?; q += 8; }
                    if csize == 0xffff_ffff { csize = u64_at(b, q)?; q += 8; }
                    if lho == 0xffff_ffff { lho = u64_at(b, q)?; }
                }
                x += 4 + len;
            }
            entries.push(Entry { name, method, crc32, compressed_size: csize, size, local_header_offset: lho });
            p = xend + clen;
        }
        Ok(Archive { bytes, entries })
    }

    pub fn entry(&self, name: &str) -> Option<&Entry> { self.entries.iter().find(|e| e.name == name) }

    /// Decompresses one entry and checks its size and CRC-32.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let e = self.entry(name).with_context(|| format!("archive has no entry {name:?} (entries: {:?})", self.names()))?;
        let b = &self.bytes[..];
        let h = e.local_header_offset as usize;
        ensure!(b.get(h..h + 4) == Some(b"PK\x03\x04"), "bad local header for {name}");
        let start = h + 30 + u16_at(b, h + 26)? as usize + u16_at(b, h + 28)? as usize;
        let data = b.get(start..start + e.compressed_size as usize).with_context(|| format!("truncated data for {name}"))?;
        let out = match e.method {
            METHOD_STORE => data.to_vec(),
            METHOD_DEFLATE => miniz_oxide::inflate::decompress_to_vec(data).map_err(|err| anyhow::anyhow!("deflate error in {name}: {err:?}"))?,
            METHOD_ZSTD => {
                let mut dec = ruzstd::decoding::StreamingDecoder::new(data).map_err(|err| anyhow::anyhow!("zstd header error in {name}: {err}"))?;
                let mut v = Vec::with_capacity(e.size as usize);
                dec.read_to_end(&mut v).with_context(|| format!("zstd error in {name}"))?;
                v
            }
            m => bail!("{name}: unsupported zip compression method {m} (PCSX2 uses 0, 8 or 93)"),
        };
        ensure!(out.len() as u64 == e.size, "{name}: size {} != {}", out.len(), e.size);
        ensure!(crc32(&out) == e.crc32, "{name}: CRC-32 mismatch");
        Ok(out)
    }

    pub fn names(&self) -> Vec<&str> { self.entries.iter().map(|e| e.name.as_str()).collect() }
}

/// CRC-32 (IEEE 802.3, reflected, as used by ZIP).
pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 { c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 }; }
            *e = c;
        }
        t
    });
    !data.iter().fold(!0u32, |c, &b| t[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8))
}

/// Builds a zip archive (no ZIP64, no data descriptors). For tests and synthetic savestates.
pub fn build(entries: &[(&str, &[u8], u16)]) -> Vec<u8> {
    let (mut out, mut cd) = (Vec::new(), Vec::new());
    for &(name, data, method) in entries {
        let comp = match method {
            METHOD_STORE => data.to_vec(),
            METHOD_DEFLATE => miniz_oxide::deflate::compress_to_vec(data, 6),
            METHOD_ZSTD => ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest),
            m => panic!("unsupported method {m}"),
        };
        let (crc, ofs) = (crc32(data), out.len() as u32);
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes()); // version needed
            v.extend_from_slice(&0u16.to_le_bytes()); // flags
            v.extend_from_slice(&method.to_le_bytes());
            v.extend_from_slice(&[0; 4]); // time, date
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes()); // extra len
        };
        out.extend_from_slice(b"PK\x03\x04");
        common(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&comp);
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&20u16.to_le_bytes()); // version made by
        common(&mut cd);
        cd.extend_from_slice(&[0; 10]); // comment len, disk start, internal attrs, external attrs
        cd.extend_from_slice(&ofs.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let cd_ofs = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&[0; 4]);
    let n = (entries.len() as u16).to_le_bytes();
    out.extend_from_slice(&n);
    out.extend_from_slice(&n);
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_ofs.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn roundtrip_all_methods() {
        let big: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8 ^ (i >> 12) as u8).collect();
        let zeros = vec![0u8; 1 << 20];
        let z = build(&[("a.bin", &big, METHOD_ZSTD), ("b.bin", &big, METHOD_DEFLATE), ("c.bin", b"hello", METHOD_STORE), ("z.bin", &zeros, METHOD_ZSTD)]);
        let a = Archive::from_bytes(z).unwrap();
        assert_eq!(a.names(), ["a.bin", "b.bin", "c.bin", "z.bin"]);
        assert_eq!(a.read("a.bin").unwrap(), big);
        assert_eq!(a.read("b.bin").unwrap(), big);
        assert_eq!(a.read("c.bin").unwrap(), b"hello");
        assert_eq!(a.read("z.bin").unwrap(), zeros);
        assert!(a.read("missing").is_err());
    }

    #[test]
    fn corrupt_data_is_detected() {
        let mut z = build(&[("c.bin", b"hello world", METHOD_STORE)]);
        z[30 + 5 + 2] ^= 1; // flip a data byte
        assert!(Archive::from_bytes(z).unwrap().read("c.bin").unwrap_err().to_string().contains("CRC"));
    }
}
