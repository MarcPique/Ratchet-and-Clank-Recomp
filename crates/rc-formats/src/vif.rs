//! VIF1 command-list parsing. Spec: docs/formats/tfrag_rac1.md 2.1. Ported from the retired C++ reference extractor (git 2230812).

use crate::buf::{invalid, Result};

/// VIF command numbers (bits 30..24 of the code word) used by this crate.
pub const NOP: u8 = 0x00;
pub const STCYCL: u8 = 0x01;
pub const OFFSET: u8 = 0x02;
pub const BASE: u8 = 0x03;
pub const ITOP: u8 = 0x04;
pub const STMOD: u8 = 0x05;
pub const MSKPATH3: u8 = 0x06;
pub const MARK: u8 = 0x07;
pub const FLUSHE: u8 = 0x10;
pub const FLUSH: u8 = 0x11;
pub const FLUSHA: u8 = 0x13;
pub const MSCAL: u8 = 0x14;
pub const MSCALF: u8 = 0x15;
pub const MSCNT: u8 = 0x17;
pub const STMASK: u8 = 0x20;
pub const STROW: u8 = 0x30;
pub const STCOL: u8 = 0x31;
pub const MPG: u8 = 0x4a;
pub const DIRECT: u8 = 0x50;
pub const DIRECTHL: u8 = 0x51;

/// One VIF1 code with its inline payload.
#[derive(Clone, Copy, Debug)]
pub struct VifPacket<'a> {
    /// Bits 30..24 of the code word (the interrupt bit 31 is dropped).
    pub cmd: u8,
    /// Bits 23..16 (0 means 256 for unpacks).
    pub num: u8,
    /// Bits 15..0.
    pub imm: u16,
    /// Byte offset of the code word in the list.
    pub offset: usize,
    /// Inline payload (unpack data, STROW rows, ...).
    pub data: &'a [u8],
}

impl VifPacket<'_> {
    pub fn is_unpack(&self) -> bool { self.cmd & 0x60 == 0x60 }
    /// 0..3 = 1..4 components.
    pub fn vn(&self) -> u8 { (self.cmd >> 2) & 3 }
    /// 0 = 32-bit, 1 = 16-bit, 2 = 8-bit, 3 = 5-bit.
    pub fn vl(&self) -> u8 { self.cmd & 3 }
    /// Unpack USN bit (1 = unsigned).
    pub fn usn(&self) -> bool { (self.imm >> 14) & 1 != 0 }
    /// Unpack destination quadword address.
    pub fn addr(&self) -> u16 { self.imm & 0x3ff }
    /// Unpack element count (NUM, with 0 meaning 256).
    pub fn count(&self) -> u32 { if self.num == 0 { 256 } else { self.num as u32 } }
    /// Bytes per unpacked element on disk.
    pub fn element_size(&self) -> u32 { ((32u32 >> self.vl()) * (self.vn() as u32 + 1)) / 8 }
}

/// Parses a VIF command list up to the end of the buffer.
pub fn parse_vif(list: &[u8]) -> Result<Vec<VifPacket<'_>>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 4 <= list.len() {
        let code = u32::from_le_bytes(list[pos..pos + 4].try_into().unwrap());
        let mut p = VifPacket { cmd: ((code >> 24) & 0x7f) as u8, num: ((code >> 16) & 0xff) as u8, imm: (code & 0xffff) as u16, offset: pos, data: &[] };
        let payload = if p.is_unpack() {
            (p.count() as usize * p.element_size() as usize + 3) & !3
        } else {
            match p.cmd {
                STMASK => 4,
                STROW | STCOL => 16,
                MPG => p.count() as usize * 8,
                DIRECT | DIRECTHL => (if p.imm == 0 { 65536 } else { p.imm as usize }) * 16,
                _ => 0,
            }
        };
        if pos + 4 + payload > list.len() { return invalid("VIF packet runs past end of list"); }
        p.data = &list[pos + 4..pos + 4 + payload];
        out.push(p);
        pos += 4 + payload;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(ws: &[u32]) -> Vec<u8> { ws.iter().flat_map(|w| w.to_le_bytes()).collect() }

    #[test]
    fn decodes_code_fields() {
        // UNPACK V4_16 USN, FLG, addr 0x12, NUM 5, interrupt bit set (must be masked off).
        let bytes = [words(&[0xed05_c012]), vec![0u8; 40]].concat();
        let ps = parse_vif(&bytes).unwrap();
        assert_eq!(ps.len(), 1);
        let p = ps[0];
        assert_eq!((p.cmd, p.num, p.imm, p.offset), (0x6d, 5, 0xc012, 0));
        assert!(p.is_unpack());
        assert_eq!((p.vn(), p.vl(), p.usn(), p.addr(), p.count(), p.element_size()), (3, 1, true, 0x12, 5, 8));
        assert_eq!(p.data.len(), 40);
    }

    #[test]
    fn unpack_sizes_and_num_zero() {
        // V3_16 x3 = 18 bytes, padded to 20; V4_8 NUM=0 = 256 x 4 bytes; V4_32 x1 = 16.
        let bytes = [words(&[0x6903_8000]), vec![0; 20], words(&[0x6e00_0000]), vec![0; 1024], words(&[0x6c01_0000]), vec![0; 16]].concat();
        let ps = parse_vif(&bytes).unwrap();
        assert_eq!(ps.len(), 3);
        assert_eq!((ps[0].vn(), ps[0].vl(), ps[0].element_size(), ps[0].usn(), ps[0].data.len()), (2, 1, 6, false, 20));
        assert_eq!((ps[1].count(), ps[1].element_size(), ps[1].data.len(), ps[1].offset), (256, 4, 1024, 24));
        assert_eq!((ps[2].vn(), ps[2].vl(), ps[2].element_size(), ps[2].data.len()), (3, 0, 16, 16));
    }

    #[test]
    fn non_unpack_payloads() {
        let bytes = [
            words(&[0x0100_0102, 0x0500_0001]),                 // STCYCL, STMOD: no payload
            words(&[0x3000_0000, 1, 2, 3, 4]),                  // STROW: 4 words
            words(&[0x2000_0000, 0xffff_ffff]),                 // STMASK: 1 word
            words(&[0x4a02_0000]), vec![0; 16],                 // MPG NUM=2: 2 x 8 bytes
            words(&[0x5000_0001]), vec![0; 16],                 // DIRECT imm=1: 1 qword
            words(&[0x1400_0000]),                              // MSCAL
        ].concat();
        let ps = parse_vif(&bytes).unwrap();
        let cmds: Vec<u8> = ps.iter().map(|p| p.cmd).collect();
        assert_eq!(cmds, [STCYCL, STMOD, STROW, STMASK, MPG, DIRECT, MSCAL]);
        let sizes: Vec<usize> = ps.iter().map(|p| p.data.len()).collect();
        assert_eq!(sizes, [0, 0, 16, 4, 16, 16, 0]);
        assert!(!ps[2].is_unpack());
        assert_eq!(&ps[2].data[4..8], &2u32.to_le_bytes());
        assert_eq!(ps[1].imm, 1);
    }

    #[test]
    fn truncated_payload_is_an_error_and_trailing_bytes_are_ignored() {
        assert!(parse_vif(&[words(&[0x3000_0000, 1, 2]), vec![]].concat()).is_err());
        // Fewer than 4 trailing bytes are not a code word.
        assert_eq!(parse_vif(&[0, 0, 0, 0, 0, 0]).unwrap().len(), 1);
    }
}
