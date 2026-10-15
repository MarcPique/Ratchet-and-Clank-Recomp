//! The persistent game state's on-card format (RAC1, SCUS_971.99): the chunk descriptor tables, the
//! `save%d.bin` file (header + global section + 20 level sections), its CRC-16, the disc's `save_game`
//! lump (icon files + blank template) and the item tables the level-start rules read.
//! Spec: `docs/plan/game_state.md` §1–§4.
//!
//! Nothing here is copied from the disc: the descriptor tables are read from the boot ELF
//! ([`ChunkTables::from_boot_elf`]), the template from the disc lump ([`SaveGameLump::parse`]), the item
//! tables from the boot ELF and the level overlay ([`ItemTables::load`]). Only layouts and the CRC are code.
//!
//! * **Descriptor** (boot 0x1a04c0 global, 0x1a07c0 per level; 16 bytes): `{u32 addr, u32 size, s32 id,
//!   s32 restore_status}`, ended by `addr == 0`. The fourth word is scratch `RestoreData` writes (1 / −1 / −2).
//!   A per-level chunk of level slot `L` lives at `addr + L·size`.
//! * **Section** (`PrepData` 0x20ad78): `{u32 data_size, u32 crc16(data)}` then `data` = chunks
//!   `{s32 id, u32 size, bytes[size], pad to 4}` and the terminator `{s32 −1, u32 0}`; `data_size` =
//!   `GetDataSize(table) − 8`. `PrepData` does **not** write the pad bytes (they are whatever the buffer
//!   held), so [`Chunk::pad`] keeps them for byte-exact round trips; freshly built chunks pad with zero.
//! * **File**: `{u32 global_size, u32 level_size}` (= `GetDataSize` of each table; the version check),
//!   the global section at 8, level section `L` at `8 + global_size + L·level_size`.

use crate::buf::{invalid, Buf, Result};
use crate::font::{parse_overlay_sections, read_overlay, OverlaySection};
use crate::tfrag_light::elf_read;

/// Boot-ELF address of the global chunk descriptor table (L01 0x184a40; identical in every overlay).
pub const BOOT_GLOBAL_CHUNKS_VADDR: u32 = 0x001a_04c0;
/// Boot-ELF address of the per-level chunk descriptor table (L01 0x184d40).
pub const BOOT_LEVEL_CHUNKS_VADDR: u32 = 0x001a_07c0;
/// Level slots in a save (`MakeWholeSave` / `RestoreGame` loop count; 19 levels are used).
pub const LEVEL_SLOTS: usize = 20;
/// Save slots on a card (`save0.bin` .. `save4.bin`).
pub const SAVE_SLOTS: usize = 5;
/// `memcard_Checksum` returns 0 (an always-failing checksum) above this length.
pub const CRC_MAX_LEN: usize = 0x1800;
/// Chunk id of the section terminator.
pub const END_ID: i32 = -1;
/// Descriptor stride in the boot ELF.
pub const DESC_SIZE: usize = 16;

/// One `{addr, size, id}` descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkDesc {
    /// EE address of the data (level slot 0 for per-level chunks).
    pub addr: u32,
    pub size: u32,
    pub id: i32,
}

impl ChunkDesc {
    /// EE address of this chunk for level slot `slot` (global chunks: slot 0).
    pub fn addr_for(&self, slot: usize) -> u32 { self.addr + slot as u32 * self.size }
}

/// Reads a descriptor table at `vaddr` of the boot ELF, up to its `addr == 0` terminator.
pub fn read_chunk_table(elf: &[u8], vaddr: u32) -> Result<Vec<ChunkDesc>> {
    let mut out = Vec::new();
    for i in 0..256u32 {
        let e = Buf(elf_read(elf, vaddr + i * DESC_SIZE as u32, DESC_SIZE)?);
        let (addr, size, id) = (e.u32(0)?, e.u32(4)?, e.i32(8)?);
        if addr == 0 { return Ok(out); }
        if id == END_ID { return invalid(format!("chunk table {vaddr:#x}: entry {i} uses the terminator id")); }
        out.push(ChunkDesc { addr, size, id });
    }
    invalid(format!("chunk table {vaddr:#x}: no terminator in 256 entries"))
}

/// `memcard_GetDataSize` 0x20ac88: `8 + Σ(align4(size) + 8) + 8` = the section's byte size incl. its header.
pub fn section_size(descs: &[ChunkDesc]) -> usize {
    8 + descs.iter().map(|d| align4(d.size as usize) + 8).sum::<usize>() + 8
}

fn align4(n: usize) -> usize { (n + 3) & !3 }

/// Both descriptor tables, in file order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkTables {
    pub global: Vec<ChunkDesc>,
    pub level: Vec<ChunkDesc>,
}

impl ChunkTables {
    /// The tables of the boot ELF (`extracted/boot/SCUS_971.99` / `Disc::boot_elf`).
    pub fn from_boot_elf(elf: &[u8]) -> Result<ChunkTables> {
        Ok(ChunkTables { global: read_chunk_table(elf, BOOT_GLOBAL_CHUNKS_VADDR)?, level: read_chunk_table(elf, BOOT_LEVEL_CHUNKS_VADDR)? })
    }
    /// `GetDataSize(global)` (0x1530 retail).
    pub fn global_size(&self) -> usize { section_size(&self.global) }
    /// `GetDataSize(level)` (0xaa4 retail).
    pub fn level_size(&self) -> usize { section_size(&self.level) }
    /// Size of a `save%d.bin` (0xea08 retail).
    pub fn file_size(&self) -> usize { 8 + self.global_size() + LEVEL_SLOTS * self.level_size() }
}

/// `memcard_Checksum` 0x20acc0: CRC-16, MSB first, register seeded with 0xedb88320 (only the low 16 bits
/// reach the result, so effectively init 0x8320), polynomial 0x1f45, no final xor. 0 above [`CRC_MAX_LEN`].
pub fn crc16(data: &[u8]) -> u16 {
    if data.len() > CRC_MAX_LEN { return 0; }
    let mut r: u32 = 0xedb8_8320;
    for &b in data {
        r ^= (b as u32) << 8;
        for _ in 0..8 { r = if r & 0x8000 != 0 { (r << 1) ^ 0x1f45 } else { r << 1 }; }
    }
    (r & 0xffff) as u16
}

/// One tagged chunk of a section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub id: i32,
    pub data: Vec<u8>,
    /// The `align4(len) − len` bytes after `data` (not written by `PrepData`; zero for fresh chunks).
    pub pad: Vec<u8>,
}

impl Chunk {
    pub fn new(id: i32, data: Vec<u8>) -> Chunk {
        let pad = vec![0; align4(data.len()) - data.len()];
        Chunk { id, data, pad }
    }
}

/// A decoded or to-be-encoded section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub chunks: Vec<Chunk>,
    /// Decode: `TestChecksum` 0x20ad38 passed (stored CRC non-zero and equal to the data's). `RestoreData`
    /// copies nothing from a section that fails. Encode ignores it (always writes a fresh CRC).
    pub crc_ok: bool,
}

impl Section {
    pub fn new(chunks: Vec<Chunk>) -> Section { Section { chunks, crc_ok: true } }

    pub fn chunk(&self, id: i32) -> Option<&Chunk> { self.chunks.iter().find(|c| c.id == id) }

    /// Bytes of the encoded section (`8 + data_size`).
    pub fn encoded_len(&self) -> usize { 8 + self.chunks.iter().map(|c| 8 + align4(c.data.len())).sum::<usize>() + 8 }

    /// `PrepData`: `{data_size, crc16}` + chunks + `{−1, 0}`.
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.encoded_len());
        v.extend_from_slice(&[0; 8]);
        for c in &self.chunks {
            v.extend_from_slice(&c.id.to_le_bytes());
            v.extend_from_slice(&(c.data.len() as u32).to_le_bytes());
            v.extend_from_slice(&c.data);
            let n = align4(c.data.len()) - c.data.len();
            v.extend((0..n).map(|k| c.pad.get(k).copied().unwrap_or(0)));
        }
        v.extend_from_slice(&END_ID.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        let size = (v.len() - 8) as u32;
        let crc = crc16(&v[8..]) as u32;
        v[0..4].copy_from_slice(&size.to_le_bytes());
        v[4..8].copy_from_slice(&crc.to_le_bytes());
        v
    }

    /// Decodes a section at the start of `bytes` (walks chunks to the terminator, as `RestoreData`, but
    /// bounds-checked against `data_size`).
    pub fn decode(bytes: &[u8]) -> Result<Section> {
        let b = Buf(bytes);
        let data_size = b.u32(0)? as usize;
        let stored = b.u32(4)?;
        let data = b.sub(8, data_size, "section data")?;
        let crc_ok = stored != 0 && crc16(data.bytes()) as u32 == stored;
        let mut chunks = Vec::new();
        let mut p = 0usize;
        loop {
            let id = data.i32(p)?;
            let size = data.u32(p + 4)? as usize;
            if id == END_ID { break; }
            let body = data.sub(p + 8, align4(size), "chunk data")?.bytes();
            chunks.push(Chunk { id, data: body[..size].to_vec(), pad: body[size..].to_vec() });
            p += 8 + align4(size);
        }
        Ok(Section { chunks, crc_ok })
    }
}

/// A whole `save%d.bin`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveFile {
    pub global: Section,
    pub levels: [Section; LEVEL_SLOTS],
}

impl SaveFile {
    /// Parses a card file (or the disc template). Sections sit at the offsets the header sizes give.
    pub fn parse(bytes: &[u8]) -> Result<SaveFile> {
        let b = Buf(bytes);
        let (gs, ls) = (b.u32(0)? as usize, b.u32(4)? as usize);
        if gs < 16 || ls < 16 { return invalid(format!("save file: implausible section sizes {gs:#x}/{ls:#x}")); }
        b.check(0, 8 + gs + LEVEL_SLOTS * ls, "save file")?;
        let global = Section::decode(&bytes[8..8 + gs])?;
        let mut levels = Vec::with_capacity(LEVEL_SLOTS);
        for l in 0..LEVEL_SLOTS {
            let at = 8 + gs + l * ls;
            levels.push(Section::decode(&bytes[at..at + ls])?);
        }
        Ok(SaveFile { global, levels: levels.try_into().unwrap() })
    }

    /// `MakeWholeSave` 0x20abb0: the header then every section.
    pub fn to_bytes(&self) -> Vec<u8> {
        let g = self.global.encode();
        let lv: Vec<Vec<u8>> = self.levels.iter().map(Section::encode).collect();
        let mut v = Vec::with_capacity(8 + g.len() + lv.iter().map(Vec::len).sum::<usize>());
        v.extend_from_slice(&(g.len() as u32).to_le_bytes());
        v.extend_from_slice(&(lv[0].len() as u32).to_le_bytes());
        v.extend_from_slice(&g);
        for l in &lv { v.extend_from_slice(l); }
        v
    }

    /// Whether every section passed its checksum.
    pub fn all_crcs_ok(&self) -> bool { self.global.crc_ok && self.levels.iter().all(|s| s.crc_ok) }

    /// The incremental save (`memcard_Save` 0x20b178, `memcard_Update` state 0x10): opens the existing
    /// `card_file` for writing (no create, no truncate), seeks 8, writes the global section, seeks
    /// `level · level_size` from there and writes level section `level`. The header and the other 19 level
    /// sections on the card are **left untouched** (they keep whatever the card held: an earlier save's
    /// data or the template). `card_file` must already have the full size.
    pub fn write_incremental(&self, level: usize, card_file: &mut [u8]) -> Result<()> {
        if level >= LEVEL_SLOTS { return invalid(format!("level slot {level} out of range")); }
        let g = self.global.encode();
        let l = self.levels[level].encode();
        let at = 8 + g.len() + level * l.len();
        if card_file.len() < at + l.len() { return invalid("incremental save: card file shorter than a whole save"); }
        card_file[8..8 + g.len()].copy_from_slice(&g);
        card_file[at..at + l.len()].copy_from_slice(&l);
        Ok(())
    }
}

/// Card file name of save slot `slot` (`"save%d.bin"` at boot 0x13d270 + 0x14).
pub fn save_file_name(slot: usize) -> String { format!("save{slot}.bin") }
/// The icon files `memcard_Update` state 10 creates next to the saves.
pub const ICON_SYS_NAME: &str = "icon.sys";
pub const STATIC_ICO_NAME: &str = "static.ico";

/// `memcard_GetName` 0x209030: the card directory from `SYSTEM.CNF` (`BOOT2 = cdrom0:\SCUS_971.99;1` →
/// `/BASCUS-97199RATCHET`; char 18 `E` → `/BE…`). The template `/BA****-*****RATCHET` takes chars 16..20,
/// 21..24 and 25..27 of the file.
pub fn card_dir_name(system_cnf: &[u8]) -> Result<String> {
    let c = Buf(system_cnf);
    c.check(0, 27, "SYSTEM.CNF")?;
    let s = system_cnf;
    let region = if s[0x12] == b'E' { b'E' } else { b'A' };
    let mut n = b"/BA****-*****RATCHET".to_vec();
    n[2] = region;
    n[3..7].copy_from_slice(&s[16..20]);
    n[8..11].copy_from_slice(&s[21..24]);
    n[11..13].copy_from_slice(&s[25..27]);
    String::from_utf8(n).or_else(|_| invalid("SYSTEM.CNF: non-ASCII boot name"))
}

/// The global `save_game` lump (TOC +0x10; `Disc::save_game_lump`, `extracted/global/save_game.bin`).
/// Header: `{icon.sys off, size, static.ico off, size, template off, size}` (0x18 bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveGameLump {
    /// `icon.sys` (0x3c4 bytes retail), written to the card verbatim.
    pub icon_sys: Vec<u8>,
    /// `static.ico` (0x8158 bytes retail), written to the card verbatim.
    pub static_ico: Vec<u8>,
    /// The blank `save%d.bin` (0xea08 bytes retail; all levels empty, level −1): new-game defaults and the
    /// content of every freshly created save slot.
    pub template: Vec<u8>,
}

impl SaveGameLump {
    pub fn parse(lump: &[u8]) -> Result<SaveGameLump> {
        let b = Buf(lump);
        let part = |i: usize, what: &'static str| -> Result<Vec<u8>> {
            Ok(b.sub(b.u32(i * 8)? as usize, b.u32(i * 8 + 4)? as usize, what)?.bytes().to_vec())
        };
        Ok(SaveGameLump { icon_sys: part(0, "icon.sys")?, static_ico: part(1, "static.ico")?, template: part(2, "save template")? })
    }
}

// ---------------------------------------------------------------------------------------------------
// Item tables used by the level-start rules (L01 0x251da0 / 0x251fe0 / GiveItem 0x275760).

/// Items (ids 0..36; the owned / ammo arrays).
pub const ITEM_COUNT: usize = 37;
/// Entries of the per-level vendor item table (`GiveItem` scans 20, the prune loop 19).
pub const VENDOR_TABLE_LEN: usize = 20;
/// Boot-ELF copy of the per-level vendor item table `u32[20]` (L01 0x1c4318; same bytes in every overlay).
pub const BOOT_VENDOR_ITEMS_VADDR: u32 = 0x001d_fd98;
/// Boot-ELF copy of the 0x18-byte item price records `[37]` (L01 0x1c4530; same bytes in every overlay).
pub const BOOT_ITEM_RECORDS_VADDR: u32 = 0x001d_ffb0;
/// Stride of the item definition records (L01 0x179f40; slot type at +8).
pub const ITEM_DEF_SIZE: usize = 0x4c;

/// One 0x18-byte item price record (vendor code reads `+0/+4` prices, `+8/+0xa` ammo prices, `+0xe` ammo cap).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemRecord(pub [u8; 0x18]);

impl ItemRecord {
    fn u16(&self, o: usize) -> u16 { u16::from_le_bytes([self.0[o], self.0[o + 1]]) }
    /// `+8` s16 ≠ 0: the item uses ammo (`GiveItem` then raises the ammo count).
    pub fn has_ammo(&self) -> bool { self.u16(8) != 0 }
    /// `+0x12` u16: ammo granted with the item (`ammo = max(ammo, this)`; bomb glove 10).
    pub fn grant_ammo(&self) -> u16 { self.u16(0x12) }
}

/// The tables the level-start rules and `GiveItem` read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemTables {
    /// Item sold by each level's vendor (0 = none): Novalis 16 (pyrocitor), ….
    pub vendor_items: [u32; VENDOR_TABLE_LEN],
    pub records: [ItemRecord; ITEM_COUNT],
    /// Item definition `+8`: slot type (0 hand, 1 feet, 2 head, 3 back, 4/5 extras, −1 none). Read from the
    /// level overlay (the boot copy of this table is zero in the file).
    pub slot_type: [i32; ITEM_COUNT],
    /// Address of the item definition table in the overlay it came from (L01 0x179f40).
    pub item_defs_addr: u32,
}

impl ItemTables {
    /// Vendor list and price records from the boot ELF; slot types from `overlay` (a level's raw overlay
    /// lump), located through `GiveItem`'s code (see [`find_item_defs`]).
    pub fn load(boot_elf: &[u8], overlay: &[u8]) -> Result<ItemTables> {
        let vt = elf_read(boot_elf, BOOT_VENDOR_ITEMS_VADDR, VENDOR_TABLE_LEN * 4)?;
        let vendor_items: [u32; VENDOR_TABLE_LEN] = std::array::from_fn(|i| u32::from_le_bytes(vt[i * 4..i * 4 + 4].try_into().unwrap()));
        let rec = elf_read(boot_elf, BOOT_ITEM_RECORDS_VADDR, ITEM_COUNT * 0x18)?;
        let records: [ItemRecord; ITEM_COUNT] = std::array::from_fn(|i| ItemRecord(rec[i * 0x18..i * 0x18 + 0x18].try_into().unwrap()));
        let sections = parse_overlay_sections(overlay)?;
        let item_defs_addr = find_item_defs(&sections, vt)?;
        let Some(defs) = read_overlay(&sections, item_defs_addr, ITEM_COUNT * ITEM_DEF_SIZE) else {
            return invalid(format!("item definitions {item_defs_addr:#x} outside the overlay"));
        };
        let slot_type = std::array::from_fn(|i| i32::from_le_bytes(defs[i * ITEM_DEF_SIZE + 8..i * ITEM_DEF_SIZE + 12].try_into().unwrap()));
        Ok(ItemTables { vendor_items, records, slot_type, item_defs_addr })
    }
}

fn find_bytes(sections: &[OverlaySection], pat: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    for s in sections.iter().filter(|s| s.kind != 8) {
        out.extend(s.data.windows(pat.len()).enumerate().filter(|(_, w)| *w == pat).map(|(i, _)| s.dest + i as u32));
    }
    out
}

/// Locates the item definition table (L01 0x179f40) in a level overlay, whose address differs per
/// overlay. The vendor item table `V` is found by the boot copy's bytes (`vendor_table`); then in
/// `GiveItem` (L01 0x275838..) the code reads: `lui rB,%hi(V)` · `addiu rA,rB,%lo(V)` · within 8 words
/// `lui rC,H` · then the first `addiu rD,rC,L` followed within 3 words by `lw rE,8(rD)`; the table is
/// `(H << 16) + sext(L)` (slot type at +8). Every candidate must agree.
pub fn find_item_defs(sections: &[OverlaySection], vendor_table: &[u8]) -> Result<u32> {
    let v = match find_bytes(sections, vendor_table).as_slice() {
        [v] => *v,
        other => return invalid(format!("vendor item table found {} times in the overlay", other.len())),
    };
    let (hi, lo) = ((v.wrapping_add(0x8000) >> 16) & 0xffff, v & 0xffff);
    let sext = |x: u32| (x & 0xffff) as u16 as i16 as i32;
    let (op, rs, rt, imm) = (|w: u32| w >> 26, |w: u32| (w >> 21) & 31, |w: u32| (w >> 16) & 31, |w: u32| w & 0xffff);
    const LUI: u32 = 0x0f;
    const ADDIU: u32 = 0x09;
    const LW: u32 = 0x23;
    let mut found: Vec<u32> = Vec::new();
    for s in sections.iter().filter(|s| s.kind != 8) {
        let w: Vec<u32> = s.data.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
        let n = w.len();
        for i in 0..n {
            if op(w[i]) != LUI || imm(w[i]) != hi { continue; }
            let Some(j) = (i + 1..n.min(i + 5)).find(|&j| op(w[j]) == ADDIU && rs(w[j]) == rt(w[i]) && imm(w[j]) == lo) else { continue };
            let Some(k) = (j + 1..n.min(j + 9)).find(|&k| op(w[k]) == LUI) else { continue };
            let rc = rt(w[k]);
            let Some(m) = (k + 1..n.min(k + 49)).find(|&m| op(w[m]) == ADDIU && rs(w[m]) == rc) else { continue };
            let rd = rt(w[m]);
            if (m + 1..n.min(m + 4)).any(|q| op(w[q]) == LW && rs(w[q]) == rd && imm(w[q]) == 8) {
                found.push(((imm(w[k]) << 16) as i32 + sext(w[m])) as u32);
            }
        }
    }
    found.dedup();
    match found.as_slice() {
        [a] => Ok(*a),
        [] => invalid("item definition table: GiveItem pattern not found"),
        _ => invalid(format!("item definition table: disagreeing candidates {found:x?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_crc(data: &[u8]) -> u16 {
        // Textbook MSB-first CRC-16 with init 0x8320 / poly 0x1f45: the documented equivalent.
        let mut r: u16 = 0x8320;
        for &b in data {
            r ^= (b as u16) << 8;
            for _ in 0..8 { r = if r & 0x8000 != 0 { (r << 1) ^ 0x1f45 } else { r << 1 }; }
        }
        r
    }

    #[test]
    fn crc_matches_the_textbook_form_and_its_limits() {
        assert_eq!(crc16(&[]), 0x8320);
        let msg = b"123456789";
        assert_eq!(crc16(msg), reference_crc(msg));
        let v: Vec<u8> = (0..0x1800u32).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(crc16(&v), reference_crc(&v));
        assert_eq!(crc16(&vec![1; 0x1801]), 0, "over 0x1800 bytes the game returns 0");
        assert_eq!(crc16(&[0x00]), reference_crc(&[0x00]));
    }

    #[test]
    fn crc_known_vector() {
        // Computed independently (Python transcription of `memcard_Checksum` with the 32-bit seed).
        assert_eq!(crc16(b"123456789"), 0xd989);
        assert_eq!(crc16(&[0x00]), 0x05bc);
    }

    #[test]
    fn section_round_trip_with_pads_and_terminator() {
        let s = Section::new(vec![Chunk::new(0, vec![0xff; 4]), Chunk::new(10, vec![1, 2, 3, 4, 5]), Chunk { id: 28, data: vec![7], pad: vec![9, 8, 7] }]);
        let e = s.encode();
        assert_eq!(e.len(), s.encoded_len());
        assert_eq!(e.len(), 8 + (8 + 4) + (8 + 8) + (8 + 4) + 8);
        assert_eq!(u32::from_le_bytes(e[0..4].try_into().unwrap()) as usize, e.len() - 8);
        assert_eq!(u32::from_le_bytes(e[4..8].try_into().unwrap()), crc16(&e[8..]) as u32);
        assert_eq!(&e[e.len() - 8..], &[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]);
        assert_eq!(&e[8 + 12 + 8 + 5..8 + 12 + 8 + 8], &[0, 0, 0], "fresh chunks pad with zero");
        let d = Section::decode(&e).unwrap();
        assert!(d.crc_ok);
        assert_eq!(d, s);
        assert_eq!(d.encode(), e);
        let mut bad = e.clone();
        bad[16] ^= 1;
        assert!(!Section::decode(&bad).unwrap().crc_ok);
        let mut zero = e.clone();
        zero[4..8].fill(0);
        assert!(!Section::decode(&zero).unwrap().crc_ok, "a stored CRC of 0 always fails");
    }

    #[test]
    fn section_size_is_get_data_size() {
        let d = [ChunkDesc { addr: 1, size: 37, id: 10 }, ChunkDesc { addr: 2, size: 4, id: 0 }];
        assert_eq!(section_size(&d), 8 + (40 + 8) + (4 + 8) + 8);
        let s = Section::new(vec![Chunk::new(10, vec![0; 37]), Chunk::new(0, vec![0; 4])]);
        assert_eq!(s.encoded_len(), section_size(&d));
    }

    #[test]
    fn incremental_write_touches_global_and_one_level_only() {
        let lv = |tag: u8| Section::new(vec![Chunk::new(3001, vec![tag])]);
        let file = SaveFile { global: Section::new(vec![Chunk::new(0, vec![0; 4])]), levels: std::array::from_fn(|l| lv(l as u8)) };
        let mut card = file.to_bytes();
        let before = card.clone();
        let mut next = file.clone();
        next.global = Section::new(vec![Chunk::new(0, vec![1, 0, 0, 0])]);
        for l in &mut next.levels { l.chunks[0].data[0] = 0xaa; }
        next.write_incremental(3, &mut card).unwrap();
        let (g, l) = (next.global.encoded_len(), next.levels[0].encoded_len());
        assert_eq!(&card[..8], &before[..8]);
        assert_eq!(&card[8..8 + g], &next.global.encode()[..]);
        for s in 0..LEVEL_SLOTS {
            let r = 8 + g + s * l..8 + g + (s + 1) * l;
            if s == 3 { assert_eq!(&card[r], &next.levels[3].encode()[..]) } else { assert_eq!(&card[r.clone()], &before[r]) }
        }
        let parsed = SaveFile::parse(&card).unwrap();
        assert_eq!(parsed.levels[3].chunks[0].data, [0xaa]);
        assert_eq!(parsed.levels[4].chunks[0].data, [4]);
    }

    fn extracted(rel: &str) -> Option<Vec<u8>> {
        std::fs::read(crate::test_data::root().join(rel)).ok()
    }

    /// Descriptor tables, template sizes / CRCs / round trip, and "template = boot ELF initial data".
    /// Skipped without `extracted/`.
    #[test]
    fn tables_and_template_from_the_disc() {
        let (Some(elf), Some(lump)) = (extracted("boot/SCUS_971.99"), extracted("global/save_game.bin")) else { eprintln!("skipped: no extracted/"); return; };
        let t = ChunkTables::from_boot_elf(&elf).unwrap();
        assert_eq!((t.global.len(), t.level.len()), (47, 11));
        assert_eq!((t.global_size(), t.level_size(), t.file_size()), (0x1530, 0xaa4, 0xea08));
        let ids: Vec<i32> = t.global.iter().map(|d| d.id).collect();
        assert_eq!(&ids[..14], &[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 19]);
        assert_eq!(t.level.iter().map(|d| d.id).collect::<Vec<_>>(), [3001, 3002, 3003, 3004, 3005, 3006, 3007, 3008, 4000, 4001, 4002]);

        let s = SaveGameLump::parse(&lump).unwrap();
        assert_eq!((s.icon_sys.len(), s.static_ico.len(), s.template.len()), (0x3c4, 0x8158, 0xea08));
        assert_eq!(&s.icon_sys[..4], b"PS2D");
        let tb = &s.template;
        assert_eq!(u32::from_le_bytes(tb[0..4].try_into().unwrap()) as usize, t.global_size());
        assert_eq!(u32::from_le_bytes(tb[4..8].try_into().unwrap()) as usize, t.level_size());
        assert_eq!(u32::from_le_bytes(tb[12..16].try_into().unwrap()), 0x9ad4);
        let f = SaveFile::parse(tb).unwrap();
        assert!(f.all_crcs_ok());
        assert_eq!(crc16(&tb[16..8 + t.global_size()]), 0x9ad4);
        let l0 = 8 + t.global_size();
        assert_eq!(crc16(&tb[l0 + 8..l0 + t.level_size()]), 0xdce3);
        assert_eq!(f.to_bytes(), *tb, "decode → encode reproduces the template byte for byte");
        assert!(f.global.chunks.iter().chain(f.levels.iter().flat_map(|l| &l.chunks)).all(|c| c.pad.iter().all(|&b| b == 0)));

        // Chunk order = table order; data = the boot ELF's initial data at the chunk address (level: −1 vs 0).
        for (d, c) in t.global.iter().zip(&f.global.chunks) {
            assert_eq!((c.id, c.data.len()), (d.id, d.size as usize));
            let ram = elf_read(&elf, d.addr, d.size as usize).unwrap();
            if d.id == 0 { assert_eq!((c.data.as_slice(), ram), (&[0xff; 4][..], &[0u8; 4][..])) } else { assert_eq!(c.data, ram, "global chunk {}", d.id) }
        }
        for (l, sec) in f.levels.iter().enumerate() {
            for (d, c) in t.level.iter().zip(&sec.chunks) {
                assert_eq!((c.id, c.data.len()), (d.id, d.size as usize));
                assert_eq!(c.data, elf_read(&elf, d.addr_for(l), d.size as usize).unwrap(), "level {l} chunk {}", d.id);
            }
        }
    }

    /// The item tables load from every overlay; the boot copies equal every overlay's bytes.
    #[test]
    fn item_tables_from_every_overlay() {
        let Some(elf) = extracted("boot/SCUS_971.99") else { eprintln!("skipped: no extracted/"); return; };
        let vt = elf_read(&elf, BOOT_VENDOR_ITEMS_VADDR, VENDOR_TABLE_LEN * 4).unwrap().to_vec();
        let rec = elf_read(&elf, BOOT_ITEM_RECORDS_VADDR, ITEM_COUNT * 0x18).unwrap().to_vec();
        let mut first: Option<ItemTables> = None;
        for l in 0..19 {
            let Some(ov) = extracted(&format!("levels/{l:02}/overlay.bin")) else { eprintln!("skipped: no overlay {l}"); return; };
            let secs = parse_overlay_sections(&ov).unwrap();
            assert_eq!(find_bytes(&secs, &rec).len(), 1, "level {l}: item records");
            assert_eq!(find_bytes(&secs, &vt).len(), 1, "level {l}: vendor table");
            let t = ItemTables::load(&elf, &ov).unwrap();
            if l == 1 { assert_eq!(t.item_defs_addr, 0x179f40); }
            if l == 0 { assert_eq!(t.item_defs_addr, 0x179ac0); }
            match &first {
                None => first = Some(t),
                Some(f) => assert_eq!((f.vendor_items, f.records, f.slot_type), (t.vendor_items, t.records, t.slot_type), "level {l}"),
            }
        }
        let t = first.unwrap();
        assert_eq!(&t.vendor_items[..3], &[0, 16, 0]);
        assert_eq!((t.records[10].has_ammo(), t.records[10].grant_ammo()), (true, 10));
        assert_eq!((t.slot_type[0], t.slot_type[8], t.slot_type[10], t.slot_type[2]), (-1, 0, 0, 3));
    }

    #[test]
    fn card_dir_from_system_cnf() {
        assert_eq!(card_dir_name(b"BOOT2 = cdrom0:\\SCUS_971.99;1\r\nVER = 1.00\r\n").unwrap(), "/BASCUS-97199RATCHET");
        assert_eq!(card_dir_name(b"BOOT2 = cdrom0:\\SCES_503.26;1\r\n").unwrap(), "/BESCES-50326RATCHET");
    }
}
