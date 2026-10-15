//! Minimal ISO 9660 reader for PS2 disc images. Spec: docs/formats/disc_layout.md section 1.
//! Ported from the retired C++ reference extractor's reader (git 2230812).
//!
//! RAC1 only puts `SYSTEM.CNF`, the boot ELF and `IOPRP243.IMG` in the filesystem; everything
//! else is addressed by absolute sector from the TOC, so the reader exposes both the directory
//! tree and raw sector reads. Reads seek to exactly the bytes asked for: the image (~4 GB for the
//! NTSC-U DVD) is never read whole.
//!
//! Plain 2048-byte-sector images (`.iso`) and raw 2352-byte CD dumps (`.bin`, mode 1 or mode 2
//! form 1, detected by the 12-byte sync pattern) are both accepted; only the 2048 user bytes of
//! each sector are returned.

use crate::buf::{Buf, FormatError};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

pub const SECTOR_SIZE: usize = 2048;
const RAW_SECTOR_SIZE: usize = 2352;
const PVD_SECTOR: u32 = 16;
const MAX_DEPTH: u32 = 8;
const MAX_DIR_BYTES: u32 = 16 << 20;

#[derive(Debug, Error)]
pub enum DiscError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Format(#[from] FormatError),
}
pub type Result<T> = std::result::Result<T, DiscError>;

pub(crate) fn bad<T>(msg: impl Into<String>) -> Result<T> { Err(DiscError::Format(FormatError::Invalid(msg.into()))) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsoEntry {
    /// Uppercase, without the `;1` version suffix.
    pub name: String,
    /// `/DIR/FILE.EXT`, uppercase.
    pub path: String,
    pub lba: u32,
    pub size: u32,
    pub is_directory: bool,
}

pub struct IsoImage<R = File> {
    reader: Mutex<R>,
    sector_count: u32,
    raw_sector_size: usize,
    raw_user_offset: usize,
    volume_id: String,
    volume_sectors: u32,
    entries: Vec<IsoEntry>,
}

impl IsoImage<File> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> { Self::new(File::open(path)?) }
}

impl<R: Read + Seek> IsoImage<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let file_size = reader.seek(SeekFrom::End(0))?;
        let mut head = [0u8; 16];
        reader.seek(SeekFrom::Start(0))?;
        let got = read_up_to(&mut reader, &mut head)?;
        const SYNC: [u8; 12] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
        let (raw_sector_size, raw_user_offset) = if got == 16 && head[..12] == SYNC {
            (RAW_SECTOR_SIZE, if head[15] == 2 { 24 } else { 16 })   // mode 2 form 1 vs mode 1
        } else {
            (SECTOR_SIZE, 0)
        };
        let sector_count = u32::try_from(file_size / raw_sector_size as u64).map_err(|_| FormatError::Invalid("image too large".into()))?;
        let mut iso = IsoImage { reader: Mutex::new(reader), sector_count, raw_sector_size, raw_user_offset, volume_id: String::new(), volume_sectors: 0, entries: Vec::new() };

        let pvd = iso.read_sectors(PVD_SECTOR, 1)?;
        let b = Buf(&pvd);
        if b.u8(0)? != 1 || &pvd[1..6] != b"CD001" { return bad("no ISO 9660 primary volume descriptor at sector 16"); }
        iso.volume_id = String::from_utf8_lossy(&pvd[40..72]).trim_end_matches(' ').to_string();
        iso.volume_sectors = b.u32(80)?;
        if b.u16(128)? as usize != SECTOR_SIZE { return bad("unexpected ISO 9660 logical block size"); }
        // The root directory record is embedded at PVD+156.
        let (root_lba, root_size) = (b.u32(156 + 2)?, b.u32(156 + 10)?);
        let mut entries = Vec::new();
        iso.walk_directory(root_lba, root_size, "", 0, &mut entries)?;
        iso.entries = entries;
        Ok(iso)
    }

    fn walk_directory(&self, lba: u32, size: u32, prefix: &str, depth: u32, out: &mut Vec<IsoEntry>) -> Result<()> {
        if depth > MAX_DEPTH { return bad("ISO 9660: directory nesting too deep"); }
        if size > MAX_DIR_BYTES { return bad(format!("ISO 9660: implausible directory size {size:#x}")); }
        let dir = self.read_sectors(lba, (size as usize).div_ceil(SECTOR_SIZE) as u32)?;
        let b = Buf(&dir);
        let mut pos = 0usize;
        while pos < size as usize {
            let len = b.u8(pos)? as usize;
            if len == 0 {
                // Records never straddle sectors; a zero length means "continue at the next sector".
                pos = (pos / SECTOR_SIZE + 1) * SECTOR_SIZE;
                continue;
            }
            let (e_lba, e_size, flags, name_len) = (b.u32(pos + 2)?, b.u32(pos + 10)?, b.u8(pos + 25)?, b.u8(pos + 32)? as usize);
            let raw_name = b.sub(pos + 33, name_len, "directory record name")?.0;
            pos += len;
            // "." and ".." are the single bytes 0x00 and 0x01.
            if raw_name.is_empty() || raw_name[0] == 0 || raw_name[0] == 1 { continue; }
            let mut name = String::from_utf8_lossy(raw_name).to_ascii_uppercase();
            if let Some(semi) = name.find(';') { name.truncate(semi); }
            let path = format!("{prefix}/{name}");
            let is_directory = flags & 2 != 0;
            out.push(IsoEntry { name, path: path.clone(), lba: e_lba, size: e_size, is_directory });
            if is_directory { self.walk_directory(e_lba, e_size, &path, depth + 1, out)?; }
        }
        Ok(())
    }

    pub fn sector_count(&self) -> u32 { self.sector_count }
    /// 2048 for a plain `.iso`, 2352 for a raw CD dump.
    pub fn raw_sector_size(&self) -> usize { self.raw_sector_size }
    pub fn volume_id(&self) -> &str { &self.volume_id }
    /// The PVD's volume space size (PVD+80, logical blocks). An image with fewer sectors than this is truncated.
    pub fn volume_sectors(&self) -> u32 { self.volume_sectors }
    pub fn entries(&self) -> &[IsoEntry] { &self.entries }

    /// Case-insensitive lookup by path; the leading `/` and a `;1` suffix are optional.
    pub fn find(&self, path: &str) -> Option<&IsoEntry> {
        let mut want = path.to_ascii_uppercase();
        if let Some(semi) = want.rfind(';') { want.truncate(semi); }
        if !want.starts_with('/') { want.insert(0, '/'); }
        self.entries.iter().find(|e| e.path == want)
    }

    pub fn read_file(&self, e: &IsoEntry) -> Result<Vec<u8>> { self.read_bytes(e.lba as u64 * SECTOR_SIZE as u64, e.size as u64) }

    pub fn read_sectors(&self, lba: u32, count: u32) -> Result<Vec<u8>> {
        self.read_bytes(lba as u64 * SECTOR_SIZE as u64, count as u64 * SECTOR_SIZE as u64)
    }

    /// Reads `size` user-data bytes starting at logical byte `offset` (= sector * 2048 + in-sector offset).
    pub fn read_bytes(&self, offset: u64, size: u64) -> Result<Vec<u8>> {
        let ss = SECTOR_SIZE as u64;
        let end = offset.checked_add(size).ok_or_else(|| FormatError::Invalid("byte range overflows".into()))?;
        if end > self.sector_count as u64 * ss {
            return bad(format!("byte range {offset:#x}+{size:#x} beyond end of image ({} sectors)", self.sector_count));
        }
        let mut out = vec![0u8; size as usize];
        let mut r = self.reader.lock().unwrap_or_else(|e| e.into_inner());
        if self.raw_sector_size == SECTOR_SIZE {
            r.seek(SeekFrom::Start(offset))?;
            r.read_exact(&mut out)?;
        } else {
            let mut done = 0usize;
            while done < out.len() {
                let pos = offset + done as u64;
                let (sector, within) = (pos / ss, (pos % ss) as usize);
                let n = (SECTOR_SIZE - within).min(out.len() - done);
                r.seek(SeekFrom::Start(sector * self.raw_sector_size as u64 + (self.raw_user_offset + within) as u64))?;
                r.read_exact(&mut out[done..done + n])?;
                done += n;
            }
        }
        Ok(out)
    }
}

fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? { 0 => break, k => n += k }
    }
    Ok(n)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Cursor;

    fn both_endian32(b: &mut [u8], o: usize, v: u32) {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        b[o + 4..o + 8].copy_from_slice(&v.to_be_bytes());
    }
    fn dir_record(lba: u32, size: u32, dir: bool, name: &[u8]) -> Vec<u8> {
        let len = (33 + name.len() + 1) & !1;   // records are padded to even length
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        both_endian32(&mut r, 2, lba);
        both_endian32(&mut r, 10, size);
        r[25] = if dir { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    /// A tiny image: root holds `SYSTEM.CNF;1`, `SCUS_971.99;1` and directory `DATA` with `A.BIN;1`.
    /// Sector 20 = root, 21 = DATA, 22 = SYSTEM.CNF, 23..24 = boot "ELF" (3000 bytes), 25 = A.BIN.
    pub(crate) fn mini_iso(total_sectors: usize) -> (Vec<u8>, Vec<u8>) {
        let mut img = vec![0u8; total_sectors * SECTOR_SIZE];
        let pvd = &mut img[16 * SECTOR_SIZE..17 * SECTOR_SIZE];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        pvd[8..40].copy_from_slice(format!("{:<32}", "PLAYSTATION").as_bytes());
        pvd[40..72].copy_from_slice(format!("{:<32}", "RATCHETANDCLANK").as_bytes());
        both_endian32(pvd, 80, total_sectors as u32);
        pvd[128..130].copy_from_slice(&(SECTOR_SIZE as u16).to_le_bytes());
        let root = dir_record(20, SECTOR_SIZE as u32, true, &[0]);
        pvd[156..156 + 34].copy_from_slice(&root);

        let cnf = b"BOOT2 = cdrom0:\\SCUS_971.99;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n\r\n".to_vec();
        let mut elf = vec![0u8; 3000];
        elf[..4].copy_from_slice(b"\x7fELF");
        for (i, v) in elf.iter_mut().enumerate().skip(4) { *v = (i * 7) as u8; }

        let mut root_dir = Vec::new();
        root_dir.extend(dir_record(20, SECTOR_SIZE as u32, true, &[0]));
        root_dir.extend(dir_record(20, SECTOR_SIZE as u32, true, &[1]));
        root_dir.extend(dir_record(21, SECTOR_SIZE as u32, true, b"DATA"));
        root_dir.extend(dir_record(23, elf.len() as u32, false, b"SCUS_971.99;1"));
        root_dir.extend(dir_record(22, cnf.len() as u32, false, b"SYSTEM.CNF;1"));
        img[20 * SECTOR_SIZE..20 * SECTOR_SIZE + root_dir.len()].copy_from_slice(&root_dir);
        let mut data_dir = Vec::new();
        data_dir.extend(dir_record(21, SECTOR_SIZE as u32, true, &[0]));
        data_dir.extend(dir_record(20, SECTOR_SIZE as u32, true, &[1]));
        data_dir.extend(dir_record(25, 5, false, b"a.bin;1"));
        img[21 * SECTOR_SIZE..21 * SECTOR_SIZE + data_dir.len()].copy_from_slice(&data_dir);
        img[22 * SECTOR_SIZE..22 * SECTOR_SIZE + cnf.len()].copy_from_slice(&cnf);
        img[23 * SECTOR_SIZE..23 * SECTOR_SIZE + elf.len()].copy_from_slice(&elf);
        img[25 * SECTOR_SIZE..25 * SECTOR_SIZE + 5].copy_from_slice(b"hello");
        (img, elf)
    }

    /// Re-encodes a 2048-byte image as raw 2352-byte mode-2 form-1 sectors.
    fn to_raw(img: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for s in img.chunks(SECTOR_SIZE) {
            let mut raw = vec![0u8; RAW_SECTOR_SIZE];
            raw[..12].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
            raw[15] = 2;
            raw[24..24 + SECTOR_SIZE].copy_from_slice(s);
            raw[24 + SECTOR_SIZE..].fill(0xAA);   // EDC/ECC stand-in; must never leak into user data
            out.extend(raw);
        }
        out
    }

    fn check(iso: &IsoImage<Cursor<Vec<u8>>>, elf: &[u8]) {
        assert_eq!(iso.volume_id(), "RATCHETANDCLANK");
        assert_eq!(iso.volume_sectors(), 32);
        let names: Vec<&str> = iso.entries().iter().map(|e| e.path.as_str()).collect();
        assert_eq!(names, ["/DATA", "/DATA/A.BIN", "/SCUS_971.99", "/SYSTEM.CNF"]);
        assert_eq!(iso.read_file(iso.find("/scus_971.99;1").unwrap()).unwrap(), elf);
        assert_eq!(iso.read_file(iso.find("data/a.bin").unwrap()).unwrap(), b"hello");
        assert!(iso.find("/DATA").unwrap().is_directory);
        assert!(iso.find("/NOPE").is_none());
        // Unaligned reads crossing a sector boundary.
        assert_eq!(iso.read_bytes(23 * 2048 + 2040, 20).unwrap(), &elf[2040..2060]);
        assert!(iso.read_sectors(iso.sector_count(), 1).is_err());
        assert!(iso.read_bytes(iso.sector_count() as u64 * 2048 - 1, 2).is_err());
    }

    #[test]
    fn reads_a_synthetic_2048_byte_image() {
        let (img, elf) = mini_iso(32);
        let iso = IsoImage::new(Cursor::new(img)).unwrap();
        assert_eq!(iso.raw_sector_size(), 2048);
        assert_eq!(iso.sector_count(), 32);
        check(&iso, &elf);
    }

    #[test]
    fn reads_a_synthetic_raw_2352_byte_image() {
        let (img, elf) = mini_iso(32);
        let iso = IsoImage::new(Cursor::new(to_raw(&img))).unwrap();
        assert_eq!(iso.raw_sector_size(), 2352);
        assert_eq!(iso.sector_count(), 32);
        check(&iso, &elf);
    }

    #[test]
    fn rejects_images_without_a_pvd() {
        assert!(IsoImage::new(Cursor::new(vec![0u8; 32 * 2048])).is_err());
        assert!(IsoImage::new(Cursor::new(vec![0u8; 100])).is_err());
    }
}
