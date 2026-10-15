//! A RAC1 disc image: ISO 9660 filesystem (boot files) + the TOC at sector 1500 + per-level lumps.
//! Spec: docs/formats/disc_layout.md sections 1-2, docs/formats/wad_layouts_rac1.md 2.3.
//!
//! `Disc::level` produces exactly the raw lumps of the Tier 0 archive under `extracted/levels/NN/` (same lump
//! selection, same byte sizes as the retired C++ extractor's `unpack`), so the engine can load straight from the
//! user's disc image and the extracted tree stays a byte-for-byte check (by hand:
//! `cargo run --release -p rc-trace -- disc-check`; `ratchet_clank-extract` checks every file against its SHA-1 table).

use crate::iso9660::{bad, IsoImage, Result, SECTOR_SIZE};
use crate::level::{parse_level_data_header, ByteRange};
use crate::toc::{self, LevelHeader, SectorRange, StreamLump, LEVEL_HEADER_SIZE, TOC_SECTOR};
use crate::buf::Buf;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

const SS: u64 = SECTOR_SIZE as u64;

#[derive(Clone, Debug)]
pub struct TocLevel {
    /// Slot in the 19-entry level table at TOC+0x28c8.
    pub table_index: usize,
    /// Absolute sector of the 0x2434-byte amalgamated level header.
    pub header_sector: u32,
    pub header: LevelHeader,
}

/// The TOC blob (`header_size` bytes at sector 1500, 0x2960 on retail) and the levels it points at.
#[derive(Clone, Debug)]
pub struct Toc {
    pub raw: Vec<u8>,
    pub levels: Vec<TocLevel>,
}

/// Every raw lump of one level's "level" group, byte-identical to the extractor's files.
/// `None` = the extractor writes no file (empty range). Member <-> file under `levels/NN/`:
/// see `LevelFiles::files`.
#[derive(Clone, Debug)]
pub struct LevelFiles {
    pub id: u32,
    pub header: LevelHeader,
    /// `level_header.bin`: the 0x2434 bytes at the header sector.
    pub level_header: Vec<u8>,
    /// Lumps of the uncompressed data container (`header.data`), sliced by its 0x58-byte ByteRange table.
    pub overlay: Option<Vec<u8>>,
    pub sound_bank: Option<Vec<u8>>,
    pub core_index: Option<Vec<u8>>,
    pub gs_ram: Option<Vec<u8>>,
    pub hud_header: Option<Vec<u8>>,
    pub hud_banks: [Option<Vec<u8>>; 5],
    /// Still WAD-compressed, exactly as on disc.
    pub core_data: Option<Vec<u8>>,
    /// Whole sector ranges (WAD-compressed streams padded to the sector size).
    pub gameplay_ntsc: Option<Vec<u8>>,
    pub gameplay_pal: Option<Vec<u8>>,
    pub occlusion: Option<Vec<u8>>,
}

impl LevelFiles {
    /// `(file name under extracted/levels/NN/, bytes)` for every present member.
    pub fn files(&self) -> Vec<(String, &[u8])> {
        let mut named: Vec<(String, &Option<Vec<u8>>)> = vec![
            ("overlay.bin".into(), &self.overlay),
            ("sound_bank.bin".into(), &self.sound_bank),
            ("core_index.bin".into(), &self.core_index),
            ("gs_ram.bin".into(), &self.gs_ram),
            ("hud_header.bin".into(), &self.hud_header),
        ];
        named.extend(self.hud_banks.iter().enumerate().map(|(i, h)| (format!("hud_bank_{i}.bin"), h)));
        named.extend([
            ("core_data.bin".into(), &self.core_data),
            ("gameplay_ntsc.bin".into(), &self.gameplay_ntsc),
            ("gameplay_pal.bin".into(), &self.gameplay_pal),
            ("occlusion.bin".into(), &self.occlusion),
        ]);
        let mut v = vec![("level_header.bin".to_string(), self.level_header.as_slice())];
        v.extend(named.into_iter().filter_map(|(n, m)| m.as_deref().map(|b| (n, b))));
        v
    }

    /// The member the extractor writes as `levels/NN/<name>`.
    pub fn file(&self, name: &str) -> Option<&[u8]> { self.files().into_iter().find(|(n, _)| n == name).map(|(_, b)| b) }

    pub fn total_bytes(&self) -> usize { self.files().iter().map(|(_, b)| b.len()).sum() }
}

/// How a global-header field addresses its lumps (`EntryKind` of the retired C++ reader).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalKind {
    /// `{sector, sectors}`: whole sectors.
    SectorRange,
    /// `{sector, bytes}`: exact byte size.
    SectorByteRange,
    /// A bare sector; the size is probed from a VAG or WAD header (`toc::probe_lump_size`).
    Sector32,
}

/// One field of the RAC1 global header: `count` entries of `kind` at byte `offset` of the TOC.
#[derive(Clone, Copy, Debug)]
pub struct GlobalField { pub name: &'static str, pub offset: usize, pub count: usize, pub kind: GlobalKind }

const fn gf(name: &'static str, offset: usize, count: usize, kind: GlobalKind) -> GlobalField { GlobalField { name, offset, count, kind } }
const SR: GlobalKind = GlobalKind::SectorRange;
const SBR: GlobalKind = GlobalKind::SectorByteRange;
const S32: GlobalKind = GlobalKind::Sector32;

/// The RAC1 global header fields in TOC order (as the retired C++ `rac1_global_fields`, minus the level table).
/// A field with `count == 1` is written as `global/<name>.bin`, else as `global/<name>/NNN.bin`.
pub const RAC1_GLOBAL_FIELDS: &[GlobalField] = &[
    gf("debug_font", 0x0008, 1, SR),
    gf("save_game", 0x0010, 1, SR),
    gf("ratchet_seqs", 0x0018, 28, SR),
    gf("hud_seqs", 0x00f8, 20, SR),
    gf("vendor", 0x0198, 1, SR),
    gf("vendor_audio", 0x01a0, 37, SR),
    gf("help_controls", 0x02c8, 12, SR),
    gf("help_moves", 0x0328, 15, SR),
    gf("help_weapons", 0x03a0, 15, SR),
    gf("help_gadgets", 0x0418, 14, SR),
    gf("help_ss", 0x0488, 7, SR),
    gf("options_ss", 0x04c0, 7, SR),
    gf("frontbin", 0x04f8, 1, SR),
    gf("mission_ss", 0x0500, 81, SR),
    gf("planets", 0x0788, 19, SR),
    gf("unknown_0820", 0x0820, 38, SR),
    gf("goodies_images", 0x0950, 10, SR),
    gf("character_sketches", 0x09a0, 19, SR),
    gf("character_renders", 0x0a38, 19, SR),
    gf("skill_images", 0x0ad0, 31, SR),
    gf("epilogue_english", 0x0bc8, 12, SR),
    gf("epilogue_french", 0x0c28, 12, SR),
    gf("epilogue_italian", 0x0c88, 12, SR),
    gf("epilogue_german", 0x0ce8, 12, SR),
    gf("epilogue_spanish", 0x0d48, 12, SR),
    gf("sketchbook", 0x0da8, 30, SR),
    gf("commercials", 0x0e98, 4, SR),
    gf("item_images", 0x0eb8, 9, SR),
    gf("qwark_boss_audio", 0x0f00, 240, S32),
    gf("irx", 0x12c0, 1, SR),
    gf("spaceships", 0x12c8, 4, SR),
    gf("unknown_12e8", 0x12e8, 20, SR),
    gf("space_plates", 0x1388, 6, SR),
    gf("transition", 0x13b8, 1, SR),
    gf("space_audio", 0x13c0, 36, SR),
    gf("sound_bank", 0x14e0, 1, SR),
    gf("unknown_14e8", 0x14e8, 1, SR),
    gf("music", 0x14f0, 1, SR),
    gf("hud_header", 0x14f8, 1, SR),
    gf("hud_banks", 0x1500, 5, SR),
    gf("all_text", 0x1528, 1, SR),
    gf("unknown_1530", 0x1530, 28, SR),
    gf("post_credits_helpdesk_girl_seq", 0x1610, 1, SR),
    gf("post_credits_audio", 0x1618, 18, SR),
    gf("credits_images_ntsc", 0x16a8, 20, SR),
    gf("credits_images_pal", 0x1748, 20, SR),
    gf("unknown_17e8", 0x17e8, 2, SR),
    gf("mpegs", 0x17f8, 88, SBR),
    gf("help_audio", 0x1ab8, 900, S32),
];

/// One file of the Tier 0 archive: `bytes` contiguous user-data bytes of the image at byte `offset`
/// (= sector * 2048 + offset in sector), written as `path` (relative, `/`-separated, as under `extracted/`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscFile { pub path: String, pub offset: u64, pub bytes: u64 }

pub struct Disc<R = File> {
    iso: IsoImage<R>,
    toc: Toc,
}

impl Disc<File> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> { Self::new(IsoImage::open(path)?) }
}

impl<R: Read + Seek> Disc<R> {
    /// Reads the TOC and every level header it points at (as the retired C++ `read_rac1_toc`).
    pub fn new(iso: IsoImage<R>) -> Result<Self> {
        let head = iso.read_sectors(TOC_SECTOR as u32, 1)?;
        let hb = Buf(&head);
        if hb.i32(0)? != 1 { return bad(format!("TOC: version {} != 1; not a RAC1 disc?", hb.i32(0)?)); }
        let header_size = hb.i32(4)?;
        if header_size <= 8 || header_size > 0x200000 { return bad("TOC: implausible header size"); }
        let raw = iso.read_bytes(TOC_SECTOR * SS, header_size as u64)?;
        let mut levels = Vec::new();
        for (table_index, sector) in toc::level_header_sectors(&raw)?.into_iter().enumerate() {
            let Some(sector) = sector else { continue };
            if sector as u64 + 5 > iso.sector_count() as u64 { continue; }
            let hdr = iso.read_bytes(sector as u64 * SS, LEVEL_HEADER_SIZE as u64)?;
            // Entries without the 0x2434 signature are skipped, as the C++ reader does.
            let Ok(header) = toc::parse_level_header(&hdr) else { continue };
            levels.push(TocLevel { table_index, header_sector: sector, header });
        }
        Ok(Disc { iso, toc: Toc { raw, levels } })
    }

    pub fn iso(&self) -> &IsoImage<R> { &self.iso }
    pub fn toc(&self) -> &Toc { &self.toc }
    /// Level ids (the header's `id`, = the `NN` of `extracted/levels/NN`) in table order.
    pub fn level_ids(&self) -> Vec<u32> { self.toc.levels.iter().map(|l| l.header.id as u32).collect() }

    /// The boot ELF's filesystem path, from `SYSTEM.CNF`'s `BOOT2 = cdrom0:\NAME;1` (spec 1.4).
    pub fn boot_elf_path(&self) -> Result<String> {
        let Some(cnf) = self.iso.find("/SYSTEM.CNF") else { return bad("SYSTEM.CNF missing") };
        let cnf = self.iso.read_file(cnf)?;
        let text = String::from_utf8_lossy(&cnf);
        const KEY: &str = "BOOT2 = cdrom0:\\";
        let Some(at) = text.find(KEY) else { return bad("SYSTEM.CNF has no BOOT2 line") };
        let rest = &text[at + KEY.len()..];
        let name = &rest[..rest.find([';', '\r', '\n']).unwrap_or(rest.len())];
        Ok(format!("/{}", name.replace('\\', "/")))
    }

    /// The unwrapped boot ELF (`extracted/boot/SCUS_971.99` on NTSC-U). Falls back to the first
    /// root file starting with `\x7fELF` if `SYSTEM.CNF` does not name an existing file (spec 2.6 B).
    pub fn boot_elf(&self) -> Result<Vec<u8>> {
        if let Some(e) = self.boot_elf_path().ok().and_then(|p| self.iso.find(&p)) { return self.iso.read_file(e); }
        for e in self.iso.entries().iter().filter(|e| !e.is_directory && e.size > 4 && e.path.matches('/').count() == 1) {
            if self.iso.read_bytes(e.lba as u64 * SS, 4)? == b"\x7fELF" { return self.iso.read_file(e); }
        }
        bad("no boot ELF in the filesystem")
    }

    fn toc_level(&self, id: u32) -> Result<&TocLevel> {
        match self.toc.levels.iter().find(|l| l.header.id as u32 == id) {
            Some(l) => Ok(l),
            None => bad(format!("no level {id} in the TOC")),
        }
    }

    fn sector_range(&self, r: SectorRange) -> Result<Option<Vec<u8>>> {
        if r.offset == 0 && r.size == 0 { return Ok(None); }
        if r.offset < 0 || r.size < 0 { return bad(format!("negative sector range {r:?}")); }
        Ok(Some(self.iso.read_bytes(r.offset as u64 * SS, r.size as u64 * SS)?))
    }

    /// The level-group lumps of level `id`, exactly as the Tier 0 archive holds them.
    pub fn level(&self, id: u32) -> Result<LevelFiles> {
        let lv = self.toc_level(id)?;
        let h = lv.header;
        let level_header = self.iso.read_bytes(lv.header_sector as u64 * SS, LEVEL_HEADER_SIZE as u64)?;

        // The data container: uncompressed, byte-offset lumps. Read in one go, as the extractor does.
        if h.data.offset <= 0 || h.data.size <= 0 { return bad(format!("level {id}: empty data range")); }
        let data = self.iso.read_bytes(h.data.offset as u64 * SS, h.data.size as u64 * SS)?;
        let dh = parse_level_data_header(&data)?;
        let lump = |r: ByteRange, what: &'static str| -> Result<Option<Vec<u8>>> {
            if !r.present() { return Ok(None); }
            Ok(Some(Buf(&data).sub(r.offset as usize, r.size as usize, what)?.0.to_vec()))
        };
        let mut hud_banks: [Option<Vec<u8>>; 5] = Default::default();
        for (dst, r) in hud_banks.iter_mut().zip(dh.hud_banks) { *dst = lump(r, "hud bank")?; }
        Ok(LevelFiles {
            id,
            header: h,
            level_header,
            overlay: lump(dh.overlay, "overlay")?,
            sound_bank: lump(dh.sound_bank, "sound bank")?,
            core_index: lump(dh.core_index, "core index")?,
            gs_ram: lump(dh.gs_ram, "gs ram")?,
            hud_header: lump(dh.hud_header, "hud header")?,
            hud_banks,
            core_data: lump(dh.core_data, "core data")?,
            gameplay_ntsc: self.sector_range(h.gameplay_ntsc)?,
            gameplay_pal: self.sector_range(h.gameplay_pal)?,
            occlusion: self.sector_range(h.occlusion)?,
        })
    }

    /// Every global lump, in TOC order, exactly as the Tier 0 archive names and sizes them: `name` is the
    /// path under `global/` minus `.bin` (`save_game`, `mpegs/073`). Entries with sector 0 and size 0 are
    /// skipped; a repeated name gets `.2`, `.3`, … (the C++ `seen[name]` rule; unused on the retail disc).
    pub fn global_lumps(&self) -> Result<Vec<StreamLump>> {
        let b = Buf(&self.toc.raw);
        let mut out = Vec::new();
        let mut seen: HashMap<String, u32> = HashMap::new();
        for f in RAC1_GLOBAL_FIELDS {
            for i in 0..f.count {
                let base = if f.count == 1 { f.name.to_string() } else { format!("{}/{i:03}", f.name) };
                let (sector, bytes) = match f.kind {
                    GlobalKind::SectorRange | GlobalKind::SectorByteRange => {
                        let (off, size) = (b.i32(f.offset + i * 8)?, b.i32(f.offset + i * 8 + 4)?);
                        if off == 0 && size == 0 { continue; }
                        if off < 0 || size < 0 { return bad(format!("TOC: negative range for global {base}")); }
                        (off as u32, if f.kind == GlobalKind::SectorRange { size as u64 * SS } else { size as u64 })
                    }
                    GlobalKind::Sector32 => {
                        let off = b.i32(f.offset + i * 4)?;
                        if off == 0 { continue; }
                        if off < 0 { return bad(format!("TOC: negative sector for global {base}")); }
                        (off as u32, toc::probe_lump_size(&self.iso.read_sectors(off as u32, 1)?).0)
                    }
                };
                let n = seen.entry(base.clone()).or_insert(0);
                *n += 1;
                let name = if *n > 1 { format!("{base}.{n}") } else { base };
                out.push(StreamLump { name, sector, bytes });
            }
        }
        Ok(out)
    }

    /// The complete Tier 0 archive plan: every file the archive holds as a raw lump (its `.bin`
    /// files and the boot files; none of its derived `.dec`, split, dump or preview files), in its order:
    /// - `boot/<NAME>`: every ISO 9660 file (on RAC1: `SYSTEM.CNF`, the boot ELF, `IOPRP243.IMG`);
    /// - `toc.bin`; `global/…` (`global_lumps`);
    /// - per level `levels/NN/`: `level_header.bin`, the `LevelFiles::files` members, then the
    ///   `level_stream_lumps` (`bindata/`, `music/`, `speech/`, `scene/`).
    ///
    /// Every file is one contiguous byte range of the image, checked to lie inside it.
    pub fn archive_files(&self) -> Result<Vec<DiscFile>> {
        let mut out = Vec::new();
        let mut entries: Vec<&crate::iso9660::IsoEntry> = self.iso.entries().iter().filter(|e| !e.is_directory).collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        for e in entries {
            out.push(DiscFile { path: format!("boot{}", e.path), offset: e.lba as u64 * SS, bytes: e.size as u64 });
        }
        out.push(DiscFile { path: "toc.bin".into(), offset: TOC_SECTOR * SS, bytes: self.toc.raw.len() as u64 });
        for l in self.global_lumps()? {
            out.push(DiscFile { path: format!("global/{}.bin", l.name), offset: l.sector as u64 * SS, bytes: l.bytes });
        }
        for lv in &self.toc.levels {
            let (h, id) = (lv.header, lv.header.id);
            let dir = format!("levels/{id:02}");
            out.push(DiscFile { path: format!("{dir}/level_header.bin"), offset: lv.header_sector as u64 * SS, bytes: LEVEL_HEADER_SIZE as u64 });
            if h.data.offset <= 0 || h.data.size <= 0 { return bad(format!("level {id}: empty data range")); }
            let (data_at, data_len) = (h.data.offset as u64 * SS, h.data.size as u64 * SS);
            let dh = parse_level_data_header(&self.iso.read_bytes(data_at, SS.min(data_len))?)?;
            let mut members: Vec<(String, ByteRange)> = vec![
                ("overlay".into(), dh.overlay), ("sound_bank".into(), dh.sound_bank), ("core_index".into(), dh.core_index),
                ("gs_ram".into(), dh.gs_ram), ("hud_header".into(), dh.hud_header),
            ];
            members.extend(dh.hud_banks.iter().enumerate().map(|(i, r)| (format!("hud_bank_{i}"), *r)));
            members.push(("core_data".into(), dh.core_data));
            for (name, r) in members {
                if !r.present() { continue; }
                if r.offset as u64 + r.size as u64 > data_len { return bad(format!("level {id}: {name} runs past the data container")); }
                out.push(DiscFile { path: format!("{dir}/{name}.bin"), offset: data_at + r.offset as u64, bytes: r.size as u64 });
            }
            for (name, r) in [("gameplay_ntsc", h.gameplay_ntsc), ("gameplay_pal", h.gameplay_pal), ("occlusion", h.occlusion)] {
                if r.offset == 0 && r.size == 0 { continue; }
                if r.offset < 0 || r.size < 0 { return bad(format!("level {id}: negative {name} range")); }
                out.push(DiscFile { path: format!("{dir}/{name}.bin"), offset: r.offset as u64 * SS, bytes: r.size as u64 * SS });
            }
            for l in self.level_stream_lumps(id as u32)? {
                out.push(DiscFile { path: format!("{dir}/{}.bin", l.name), offset: l.sector as u64 * SS, bytes: l.bytes });
            }
        }
        let end = self.iso.sector_count() as u64 * SS;
        if let Some(f) = out.iter().find(|f| f.offset.checked_add(f.bytes).is_none_or(|e| e > end)) {
            return bad(format!("{} ({:#x}+{:#x}) lies beyond the end of the image", f.path, f.offset, f.bytes));
        }
        Ok(out)
    }

    /// The global `save_game` lump (TOC +0x10, whole sectors = `extracted/global/save_game.bin`): the
    /// memory-card icon files and the blank save template (`rc_formats::save_game::SaveGameLump`).
    pub fn save_game_lump(&self) -> Result<Vec<u8>> {
        let r = toc::global_sector_range(&self.toc.raw, toc::SAVE_GAME_FIELD)?;
        match self.sector_range(r)? { Some(b) => Ok(b), None => bad("TOC: empty save_game range") }
    }

    /// The audio and scene lumps of level `id` (VAG / WAD sizes probed from their headers).
    pub fn level_stream_lumps(&self, id: u32) -> Result<Vec<StreamLump>> {
        let h = self.toc_level(id)?.header;
        toc::level_stream_lumps(&h, |sector| Ok(toc::probe_lump_size(&self.iso.read_sectors(sector, 1)?).0))
    }

    /// The bytes of one lump from `level_stream_lumps` (`extracted/levels/NN/<name>.bin`).
    pub fn read_lump(&self, l: &StreamLump) -> Result<Vec<u8>> { self.iso.read_bytes(l.sector as u64 * SS, l.bytes) }

    /// Scene `k`'s region file of level `id` (`levels/NN/scene/KK_ntsc.bin` / `_pal.bin`: every chunk
    /// WAD plus the sentinel sector; `crate::scene::Scene::load`). None when the region is empty.
    pub fn scene_region(&self, id: u32, k: usize, pal: bool) -> Result<Option<Vec<u8>>> {
        let h = self.toc_level(id)?.header;
        let Some(rec) = h.scenes.get(k) else { return bad(format!("no scene record {k}")) };
        match rec.region_sectors(pal) {
            Some((first, n)) => Ok(Some(self.iso.read_bytes(first as u64 * SS, n as u64 * SS)?)),
            None => Ok(None),
        }
    }

    /// Scene `k`'s speech VAG of level `id` in language `lang` (0x15ed88 index; `levels/NN/speech/KK_<lang>.bin`).
    pub fn scene_speech(&self, id: u32, k: usize, lang: usize) -> Result<Option<Vec<u8>>> {
        let h = self.toc_level(id)?.header;
        let Some(&s) = h.scenes.get(k).and_then(|r| r.speech.get(lang)) else { return bad(format!("no scene record {k} / language {lang}")) };
        if s <= 0 { return Ok(None); }
        let bytes = toc::probe_lump_size(&self.iso.read_sectors(s as u32, 1)?).0;
        Ok(Some(self.iso.read_bytes(s as u64 * SS, bytes)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iso9660::tests::mini_iso;
    use std::io::Cursor;

    /// Adds a TOC at sector 1500 with one level (id 3) to the synthetic image.
    fn disc_image() -> (Vec<u8>, Vec<u8>) {
        const HDR: usize = 1510;   // level header sector (5 sectors), data follows
        let (mut img, elf) = mini_iso(1600);
        let put = |img: &mut Vec<u8>, at: usize, v: i32| img[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let toc = TOC_SECTOR as usize * SECTOR_SIZE;
        put(&mut img, toc, 1);
        put(&mut img, toc + 4, toc::TOC_SIZE as i32);
        put(&mut img, toc + toc::LEVEL_TABLE_OFFSET + 8, HDR as i32);   // table slot 1
        put(&mut img, toc + toc::LEVEL_TABLE_OFFSET + 12, 1);
        let h = HDR * SECTOR_SIZE;
        put(&mut img, h, 3);
        put(&mut img, h + 4, LEVEL_HEADER_SIZE as i32);
        put(&mut img, h + 8, HDR as i32 + 5);   // data: 2 sectors
        put(&mut img, h + 12, 2);
        put(&mut img, h + 16, HDR as i32 + 7);  // gameplay_ntsc: 1 sector
        put(&mut img, h + 20, 1);
        put(&mut img, h + 0x148, HDR as i32 + 8);   // music[0]: a VAG of 0x30 + 0x20 bytes
        let d = (HDR + 5) * SECTOR_SIZE;
        // ByteRange table: overlay, sound_bank, core_index, gs_ram, hud_header, hud_banks[5], core_data.
        let ranges: [(i32, i32); 11] = [(0x80, 0x10), (-1, 0), (0x100, 0x44), (0x200, 0x300), (-1, 0), (-1, 0), (-1, 0), (-1, 0), (-1, 0), (-1, 0), (0x600, 0x123)];
        for (i, (o, s)) in ranges.iter().enumerate() { put(&mut img, d + i * 8, *o); put(&mut img, d + i * 8 + 4, *s); }
        for i in 0x80..0x1000 { img[d + i] = (i * 13) as u8; }
        let v = (HDR + 8) * SECTOR_SIZE;
        img[v..v + 4].copy_from_slice(b"VAGp");
        img[v + 0x0c..v + 0x10].copy_from_slice(&0x20u32.to_be_bytes());
        (img, elf)
    }

    #[test]
    fn reads_toc_boot_elf_and_level_lumps_from_a_synthetic_disc() {
        let (img, elf) = disc_image();
        let disc = Disc::new(IsoImage::new(Cursor::new(img.clone())).unwrap()).unwrap();
        assert_eq!(disc.toc().raw.len(), toc::TOC_SIZE);
        assert_eq!(disc.level_ids(), [3]);
        assert_eq!(disc.toc().levels[0].table_index, 1);
        assert_eq!(disc.boot_elf_path().unwrap(), "/SCUS_971.99");
        assert_eq!(disc.boot_elf().unwrap(), elf);

        let l = disc.level(3).unwrap();
        let d = 1515 * SECTOR_SIZE;
        assert_eq!(l.overlay.as_deref(), Some(&img[d + 0x80..d + 0x90]));
        assert_eq!(l.core_index.as_deref(), Some(&img[d + 0x100..d + 0x144]));
        assert_eq!(l.gs_ram.as_deref(), Some(&img[d + 0x200..d + 0x500]));
        assert_eq!(l.core_data.as_deref(), Some(&img[d + 0x600..d + 0x723]));
        assert!(l.sound_bank.is_none() && l.hud_header.is_none() && l.hud_banks.iter().all(Option::is_none));
        assert_eq!(l.gameplay_ntsc.as_deref(), Some(&img[1517 * SECTOR_SIZE..1518 * SECTOR_SIZE]));
        assert!(l.gameplay_pal.is_none() && l.occlusion.is_none());
        let names: Vec<String> = l.files().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["level_header.bin", "overlay.bin", "core_index.bin", "gs_ram.bin", "core_data.bin", "gameplay_ntsc.bin"]);
        assert_eq!(l.file("gs_ram.bin").unwrap().len(), 0x300);

        let streams = disc.level_stream_lumps(3).unwrap();
        assert_eq!(streams, [StreamLump { name: "music/000".into(), sector: 1518, bytes: 0x50 }]);
        assert_eq!(disc.read_lump(&streams[0]).unwrap(), &img[1518 * SECTOR_SIZE..1518 * SECTOR_SIZE + 0x50]);
        assert!(disc.level(4).is_err());
    }

    #[test]
    fn global_lumps_and_archive_plan_follow_the_cpp_unpack_rules() {
        let (mut img, _) = disc_image();
        let put = |img: &mut Vec<u8>, at: usize, v: i32| img[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let toc = TOC_SECTOR as usize * SECTOR_SIZE;
        put(&mut img, toc + 0x10, 1520);            // save_game: SectorRange, 2 sectors
        put(&mut img, toc + 0x14, 2);
        put(&mut img, toc + 0xf00, 1541);           // qwark_boss_audio[0]: Sector32, neither VAG nor WAD -> 1 sector
        put(&mut img, toc + 0x17f8 + 3 * 8, 1530);  // mpegs[3]: SectorByteRange, 100 bytes
        put(&mut img, toc + 0x17f8 + 3 * 8 + 4, 100);
        put(&mut img, toc + 0x1ab8 + 5 * 4, 1540);  // help_audio[5]: Sector32 VAG of 0x30 + 0x40 bytes
        let v = 1540 * SECTOR_SIZE;
        img[v..v + 4].copy_from_slice(b"VAGp");
        img[v + 0x0c..v + 0x10].copy_from_slice(&0x40u32.to_be_bytes());
        let disc = Disc::new(IsoImage::new(Cursor::new(img)).unwrap()).unwrap();

        let g = disc.global_lumps().unwrap();
        let names: Vec<(&str, u32, u64)> = g.iter().map(|l| (l.name.as_str(), l.sector, l.bytes)).collect();
        assert_eq!(names, [("save_game", 1520, 0x1000), ("qwark_boss_audio/000", 1541, 0x800), ("mpegs/003", 1530, 100), ("help_audio/005", 1540, 0x70)]);
        assert_eq!(disc.save_game_lump().unwrap().len(), 0x1000);

        let plan = disc.archive_files().unwrap();
        let paths: Vec<&str> = plan.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, [
            "boot/DATA/A.BIN", "boot/SCUS_971.99", "boot/SYSTEM.CNF", "toc.bin",
            "global/save_game.bin", "global/qwark_boss_audio/000.bin", "global/mpegs/003.bin", "global/help_audio/005.bin",
            "levels/03/level_header.bin", "levels/03/overlay.bin", "levels/03/core_index.bin", "levels/03/gs_ram.bin",
            "levels/03/core_data.bin", "levels/03/gameplay_ntsc.bin", "levels/03/music/000.bin",
        ]);
        // Every planned byte range reproduces what the per-member readers return.
        let l = disc.level(3).unwrap();
        for f in &plan {
            let bytes = disc.iso().read_bytes(f.offset, f.bytes).unwrap();
            if let Some(name) = f.path.strip_prefix("levels/03/") {
                if let Some(m) = l.file(name) { assert_eq!(bytes, m, "{}", f.path); }
            }
        }
        assert_eq!(plan[3], DiscFile { path: "toc.bin".into(), offset: TOC_SECTOR * 2048, bytes: toc::TOC_SIZE as u64 });
        assert_eq!(plan[9].offset, 1515 * 2048 + 0x80);
    }
}
