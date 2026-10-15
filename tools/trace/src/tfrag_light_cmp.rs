//! Truth test for the tfrag lighting port (`rc_formats::tfrag_light`, docs/plan/tfrag_lighting.md):
//! compares our lit RGBA with the RGBA blocks the game's `LightTfrags` wrote into EE RAM.
//!
//! Locating the data (docs/plan/trace_harness.md §3):
//! * The level core data is decompressed to the address in the level ELF's global
//!   `DAT_00174294` (level01; `InitMemSlots` = `0x17428c + 0x94000`, where `0x17428c` depends on
//!   a mode variable, so the base is read from RAM, not assumed). The tfrags block sits at
//!   `base + LevelCoreHeader.tfrags`; its header table at `block + table_offset`.
//! * At load, `fun_002040e0` (level01 `0x255470`) rewrites every header's `data` (0x10) from
//!   "relative to the table" to an absolute pointer, and `LightTfrags` overwrites `data + rgba_ofs`.
//! * The locator does not depend on the level-specific globals: it searches RAM for the header table
//!   by its bounding spheres and the load-invariant header fields, then follows the relocated `data`
//!   pointers. The globals are only a cross-check.

use crate::ee::EeImage;
use anyhow::{bail, Context, Result};
use rc_formats::level::{self, LevelCore};
use rc_formats::tfrag::{self, Tfrag, TfragRgba};
use rc_formats::tfrag_light::{self, DirLightSet, LightBank, NormalTable, PointLight, PointLights, VertexLight, BANK_SETS, BOOT_NORMAL_TABLE_VADDR, POINT_LIGHT_SLOTS};
use rc_formats::{wad, Buf};
use std::path::Path;

/// Everything the port needs from the extracted disc to light one level's tfrags.
pub struct LevelInputs {
    pub level: u32,
    pub core: LevelCore,
    /// The tfrags block as stored on disc (decompressed core data slice).
    pub block: Vec<u8>,
    /// Offset of the tfrags block in the decompressed core data (`LevelCoreHeader.tfrags`).
    pub block_offset: usize,
    pub table_offset: usize,
    pub tfrags: Vec<Tfrag>,
    pub bank: LightBank,
    pub normals: NormalTable,
}

impl LevelInputs {
    pub fn load(extracted: &Path, level: u32) -> Result<LevelInputs> {
        let dir = extracted.join(format!("levels/{level:02}"));
        let rd = |n: &str| std::fs::read(dir.join(n)).with_context(|| format!("reading {}/{n} (extract the game data first)", dir.display()));
        let gameplay = wad::decompress(&rd("gameplay_ntsc.bin")?)?;
        let bank = tfrag_light::parse_light_bank(&gameplay)?;
        let core_data = wad::decompress(&rd("core_data.bin")?)?;
        let core = level::parse_level_core(&rd("core_index.bin")?, core_data.len())?;
        let block = tfrag::tfrag_block(&core, &core_data)?.to_vec();
        let block_offset = core.header.tfrags as usize;
        let table_offset = Buf(&block).i32(0)? as usize;
        let tfrags = tfrag::parse_tfrags(&block)?;
        let boot = std::fs::read(extracted.join("boot/SCUS_971.99")).context("reading extracted/boot/SCUS_971.99")?;
        let normals = NormalTable::from_elf(&boot, BOOT_NORMAL_TABLE_VADDR)?;
        Ok(LevelInputs { level, core, block, block_offset, table_offset, tfrags, bank, normals })
    }

    /// The directional sets the gameplay file provides, as the loader copies them (count x 0x40 bytes).
    pub fn bank_bytes(&self) -> Vec<u8> { bank_to_bytes(&self.bank.sets[..self.bank.count]) }

    /// Our lit RGBA for tfrag `i` with the disc's header light fields and no point lights.
    pub fn lit(&self, i: usize) -> Vec<TfragRgba> { tfrag_light::light_tfrag(&self.tfrags[i], &self.bank, &self.normals, None) }
}

fn bank_to_bytes(sets: &[DirLightSet]) -> Vec<u8> {
    sets.iter().flat_map(|s| [s.color_a, s.dir_a, s.color_b, s.dir_b]).flatten().flat_map(f32::to_le_bytes).collect()
}

fn f32s<const N: usize>(b: &[u8]) -> [f32; N] { std::array::from_fn(|k| f32::from_le_bytes(b[4 * k..4 * k + 4].try_into().unwrap())) }

/// EE addresses known for one level ELF (each level is its own executable with its own data layout;
/// see work/decomp/level01.elf). Only level01 has been read so far.
#[derive(Clone, Copy, Debug)]
pub struct KnownAddrs {
    /// Global holding the decompressed core-data base (`InitMemSlots`: `DAT_00174294`).
    pub core_data_ptr: u32,
    /// Directional light bank (16 x 0x40), `FastMemZero16(0x180340, 0x400)` in `FUN_00255958`.
    pub dir_bank: u32,
}

pub fn known_addrs(level: u32) -> Option<KnownAddrs> {
    match level {
        1 => Some(KnownAddrs { core_data_ptr: 0x0017_4294, dir_bank: 0x0018_0340 }),
        _ => None,
    }
}

/// Point-light bank follows the directional bank (level01: 0x180740 = 0x180340 + 0x400). **[inferred for other levels]**
pub const POINT_BANK_OFFSET: u32 = 0x400;

/// Header bytes the load path and the per-frame passes leave alone: bsphere, the offsets/sizes,
/// msphere count/ofs, light ofs, cube ofs, vert/tri count, mip distance. Excluded: `data` (0x10,
/// relocated at load), `occl_index_stash` (0x2b) and `flags` (0x2d) (unknown writers), the
/// light fields 0x34..0x38 (written by the level loader and `LightTfrags`), and `occl_index`
/// (0x3a; the Novalis savestate of 2026-09-26 shows the loader renumbers it: disc 0 for every
/// tfrag, RAM a per-tfrag value in 969/1004 hi bytes).
const STABLE: [(usize, usize); 6] = [(0x00, 0x10), (0x14, 0x2b), (0x2c, 0x2d), (0x2e, 0x34), (0x38, 0x3a), (0x3c, 0x40)];
/// Volatile header bytes reported individually.
const VOLATILE: [(usize, &str); 8] = [
    (0x2b, "occl_index_stash"), (0x2d, "flags"), (0x34, "dir_lights_one"), (0x35, "dir_lights_upd"),
    (0x36, "point_lights lo"), (0x37, "point_lights hi"), (0x3a, "occl_index lo"), (0x3b, "occl_index hi"),
];

fn stable_eq(a: &[u8], b: &[u8]) -> bool { STABLE.iter().all(|&(s, e)| a[s..e] == b[s..e]) }

pub struct Location {
    /// EE (physical) address of the tfrag header table.
    pub table: u32,
    /// Candidate addresses for tfrag 0's bounding sphere.
    pub candidates: usize,
    /// Headers equal to the disc on every load-invariant byte.
    pub headers_matching: usize,
    /// Per tfrag: EE address of its data block.
    pub data: Vec<u32>,
    /// Headers whose `data` field is the absolute pointer `table + disc data` (load relocation done).
    pub relocated: usize,
    /// Headers whose `data` field still holds the disc's relative value.
    pub unrelocated: usize,
    /// Per volatile header byte: how many tfrags differ from the disc.
    pub volatile_diffs: Vec<(usize, &'static str, usize)>,
    /// Level-global cross-check: (global address, value read, block address it implies, agrees).
    pub core_ptr_check: Option<(u32, u32, u32, bool)>,
}

pub fn locate(ee: &EeImage, lvl: &LevelInputs) -> Result<Location> {
    let n = lvl.tfrags.len();
    let hdr = |i: usize| &lvl.block[lvl.table_offset + 0x40 * i..lvl.table_offset + 0x40 * (i + 1)];
    let cands = ee.find(&hdr(0)[..0x10], 4);
    let count_matches = |table: usize| (0..n).filter(|&i| ee.ram.get(table + 0x40 * i..table + 0x40 * (i + 1)).is_some_and(|r| stable_eq(r, hdr(i)))).count();
    let best = cands.iter().map(|&c| (count_matches(c as usize), c)).max();
    let Some((headers_matching, table)) = best.filter(|&(m, _)| m * 2 > n) else {
        bail!(
            "tfrag header table of level {:02} not found in EE RAM ({} candidate(s) for tfrag 0's bounding sphere). \
             Is this level loaded in the dump?",
            lvl.level,
            cands.len()
        );
    };
    let (mut relocated, mut unrelocated, mut data) = (0, 0, Vec::with_capacity(n));
    let mut vol = vec![0usize; VOLATILE.len()];
    for i in 0..n {
        let ram = ee.bytes(table + 0x40 * i as u32, 0x40)?;
        let disc_rel = lvl.tfrags[i].header.data as u32;
        let expect = table.wrapping_add(disc_rel);
        let v = u32::from_le_bytes(ram[0x10..0x14].try_into().unwrap());
        if ee.phys(v) == Some(expect as usize) { relocated += 1 } else if v == disc_rel { unrelocated += 1 }
        data.push(expect);
        for (k, &(o, _)) in VOLATILE.iter().enumerate() { if ram[o] != hdr(i)[o] { vol[k] += 1; } }
    }
    let volatile_diffs = VOLATILE.iter().zip(vol).map(|(&(o, name), c)| (o, name, c)).collect();
    let block_addr = table - lvl.table_offset as u32;
    let core_ptr_check = known_addrs(lvl.level).and_then(|k| {
        let v = ee.u32(k.core_data_ptr).ok()?;
        let implied = v.wrapping_add(lvl.block_offset as u32);
        Some((k.core_data_ptr, v, implied, ee.phys(implied) == Some(block_addr as usize)))
    });
    Ok(Location { table, candidates: cands.len(), headers_matching, data, relocated, unrelocated, volatile_diffs, core_ptr_check })
}

/// Where the directional bank was found and what it holds.
pub struct BankInRam {
    pub addr: u32,
    /// How it was found: "search" (disc bank bytes found) or "known address" (level global table).
    pub how: &'static str,
    pub bank: LightBank,
    pub equals_disc: bool,
    pub points: [PointLight; POINT_LIGHT_SLOTS],
}

pub fn locate_bank(ee: &EeImage, lvl: &LevelInputs) -> Option<BankInRam> {
    let want = lvl.bank_bytes();
    let found = if want.is_empty() { Vec::new() } else { ee.find(&want, 16) };
    let known = known_addrs(lvl.level).map(|k| k.dir_bank);
    let (addr, how) = match (known, found.as_slice()) {
        (Some(k), f) if f.contains(&k) => (k, "search + known address"),
        (_, [f, ..]) => (*f, "search"),
        (Some(k), []) => (k, "known address (disc bank bytes not found in RAM)"),
        (None, []) => return None,
    };
    let raw = ee.bytes(addr, 0x40 * BANK_SETS).ok()?;
    let mut bank = LightBank { count: lvl.bank.count, ..Default::default() };
    for (s, c) in bank.sets.iter_mut().zip(raw.chunks(0x40)) {
        *s = DirLightSet { color_a: f32s(&c[0..]), dir_a: f32s(&c[0x10..]), color_b: f32s(&c[0x20..]), dir_b: f32s(&c[0x30..]) };
    }
    let praw = ee.bytes(addr + POINT_BANK_OFFSET, 0x20 * POINT_LIGHT_SLOTS).ok()?;
    let points = std::array::from_fn(|k| PointLight { color: f32s(&praw[0x20 * k..]), pos: f32s(&praw[0x20 * k + 0x10..]) });
    let equals_disc = bank_to_bytes(&bank.sets) == bank_to_bytes(&lvl.bank.sets);
    Some(BankInRam { addr, how, bank, equals_disc, points })
}

/// Vertex classes that exercise different parts of the float model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexClass {
    /// One light set: mul/add/max only.
    Single,
    /// Two sets blended by `sel >> 8`: adds `rsqrt` normalisation.
    Blended,
    /// Tfrag has active point lights: adds `div`/`sqrt`.
    PointLit,
}

#[derive(Clone, Debug)]
pub struct VertexDiff {
    pub tfrag: usize,
    pub vertex: usize,
    pub rgba_addr: u32,
    pub ours: [u8; 4],
    pub ram: [u8; 4],
    pub disc: [u8; 4],
    pub light: VertexLight,
    pub class: VertexClass,
}

#[derive(Default, Clone, Debug)]
pub struct ClassStats { pub vertices: u64, pub equal: u64 }

#[derive(Default, Debug)]
pub struct Report {
    pub tfrags: usize,
    pub tfrags_equal: usize,
    pub vertices: u64,
    pub vertices_equal: u64,
    pub bytes: u64,
    pub bytes_equal: u64,
    /// Vertices whose RAM colour still equals the disc placeholder (the pass did not run there).
    pub ram_equals_disc: u64,
    /// Vertices where our lit colour equals the disc placeholder (baseline for the above).
    pub ours_equals_disc: u64,
    pub single: ClassStats,
    pub blended: ClassStats,
    pub point_lit: ClassStats,
    /// Tfrags with an active point-light list (RAM header 0x36 != 0xffff).
    pub tfrags_point_lit: usize,
    /// Tfrags skipped because they have point lights but the point bank was not found.
    pub tfrags_skipped: usize,
    /// Per channel r,g,b,a: histogram of `ours - ram` for -8..=8 (index d + 8); outside that range in `hist_far`.
    pub hist: [[u64; 17]; 4],
    pub hist_far: [u64; 4],
    /// Padding entries (`vert_count..rgba_size*4`): how many equal the disc bytes in RAM (scratchpad leftovers otherwise).
    pub padding: u64,
    pub padding_equal_disc: u64,
    pub diffs: Vec<VertexDiff>,
}

fn px(c: TfragRgba) -> [u8; 4] { [c.r, c.g, c.b, c.a] }

/// Compares every tfrag's lit RGBA with RAM. `bank` is the directional bank to light with (the disc's,
/// or the one read from RAM); `points` the RAM point-light bank, used for tfrags whose RAM header
/// has an active point-light list.
pub fn compare(ee: &EeImage, lvl: &LevelInputs, loc: &Location, bank: &LightBank, points: Option<&[PointLight; POINT_LIGHT_SLOTS]>) -> Result<Report> {
    let mut r = Report { tfrags: lvl.tfrags.len(), ..Default::default() };
    for (i, t0) in lvl.tfrags.iter().enumerate() {
        let hram = ee.bytes(loc.table + 0x40 * i as u32, 0x40)?;
        let mut t = t0.clone();
        // Inputs the game takes from the live header: whole-tfrag set select and point-light list.
        t.header.dir_lights_one = hram[0x34];
        t.header.point_lights = u16::from_le_bytes([hram[0x36], hram[0x37]]);
        let plist = t.header.point_lights;
        let pl;
        let point_arg = if plist != 0xffff {
            r.tfrags_point_lit += 1;
            match points {
                Some(b) => {
                    pl = PointLights { bank: b, list: plist, origin: t.origin };
                    Some(&pl)
                }
                None => {
                    r.tfrags_skipped += 1;
                    continue;
                }
            }
        } else {
            None
        };
        let ours = tfrag_light::light_tfrag(&t, bank, &lvl.normals, point_arg);
        let h = t.header;
        let rgba_addr = loc.data[i] + h.rgba_ofs as u32;
        let ram = ee.bytes(rgba_addr, h.rgba_size as usize * 16)?;
        let nv = (h.vert_count as usize).min(ours.len());
        let mut all_eq = true;
        for v in 0..nv {
            let (o, d) = (px(ours[v]), px(t.rgba[v]));
            let m: [u8; 4] = ram[4 * v..4 * v + 4].try_into().unwrap();
            let light = VertexLight::from_record(&t.lights[v]);
            let class = if plist != 0xffff { VertexClass::PointLit } else if light.select & 0xff00 != 0 && (h.dir_lights_one as i8) < 0 { VertexClass::Blended } else { VertexClass::Single };
            let cs = match class { VertexClass::Single => &mut r.single, VertexClass::Blended => &mut r.blended, VertexClass::PointLit => &mut r.point_lit };
            cs.vertices += 1;
            r.vertices += 1;
            r.bytes += 4;
            r.ram_equals_disc += (m == d) as u64;
            r.ours_equals_disc += (o == d) as u64;
            for k in 0..4 {
                let dd = o[k] as i32 - m[k] as i32;
                if (-8..=8).contains(&dd) { r.hist[k][(dd + 8) as usize] += 1 } else { r.hist_far[k] += 1 }
                r.bytes_equal += (dd == 0) as u64;
            }
            if o == m {
                r.vertices_equal += 1;
                cs.equal += 1;
            } else {
                all_eq = false;
                r.diffs.push(VertexDiff { tfrag: i, vertex: v, rgba_addr: rgba_addr + 4 * v as u32, ours: o, ram: m, disc: d, light, class });
            }
        }
        for v in nv..h.rgba_size as usize * 4 {
            r.padding += 1;
            r.padding_equal_disc += (ram[4 * v..4 * v + 4] == px(t.rgba[v])) as u64;
        }
        r.tfrags_equal += all_eq as usize;
    }
    Ok(r)
}

fn pct(a: u64, b: u64) -> String { if b == 0 { "-".into() } else { format!("{:.3}%", 100.0 * a as f64 / b as f64) } }

impl Report {
    pub fn all_equal(&self) -> bool { self.vertices > 0 && self.vertices == self.vertices_equal && self.tfrags_skipped == 0 }

    pub fn print(&self, max_diffs: usize) {
        println!("  tfrags        {:>8} fully equal {:>8} ({})", self.tfrags, self.tfrags_equal, pct(self.tfrags_equal as u64, self.tfrags as u64));
        println!("  vertices      {:>8} equal       {:>8} ({})", self.vertices, self.vertices_equal, pct(self.vertices_equal, self.vertices));
        println!("  bytes         {:>8} equal       {:>8} ({})", self.bytes, self.bytes_equal, pct(self.bytes_equal, self.bytes));
        for (name, c) in [("single-set", &self.single), ("blended", &self.blended), ("point-lit", &self.point_lit)] {
            if c.vertices > 0 { println!("    {name:<11} {:>8} equal       {:>8} ({})", c.vertices, c.equal, pct(c.equal, c.vertices)); }
        }
        println!("  RAM == disc placeholder: {} vertices (ours == placeholder: {}); a pass that ran leaves few of these", self.ram_equals_disc, self.ours_equals_disc);
        if self.tfrags_point_lit > 0 {
            println!("  tfrags with active point lights: {} (skipped, point bank unknown: {})", self.tfrags_point_lit, self.tfrags_skipped);
        }
        println!("  padding entries (not lit by the game, scratchpad leftovers): {} , equal to disc: {}", self.padding, self.padding_equal_disc);
        if self.vertices_equal != self.vertices {
            println!("  ours - RAM per channel (d = -8..8; far = |d| > 8):");
            for (k, ch) in ["r", "g", "b", "a"].iter().enumerate() {
                let cells: Vec<String> = (0..17).filter(|&j| self.hist[k][j] > 0 && j != 8).map(|j| format!("{:+}:{}", j as i32 - 8, self.hist[k][j])).collect();
                println!("    {ch}: equal {:>8}  {}  far {}", self.hist[k][8], cells.join(" "), self.hist_far[k]);
            }
            println!("  first mismatches (tfrag vertex @addr: ours / RAM / disc | select az el base555):");
            for d in self.diffs.iter().take(max_diffs) {
                println!(
                    "    {:5} {:3} @{:#09x}: {:?} / {:?} / {:?} | {:#06x} {:3} {:3} {:#06x} {:?}",
                    d.tfrag, d.vertex, d.rgba_addr, d.ours, d.ram, d.disc, d.light.select, d.light.azimuth, d.light.elevation, d.light.color, d.class
                );
            }
        }
    }

    pub fn write_csv(&self, path: &Path) -> Result<()> {
        let mut s = String::from("tfrag,vertex,addr,class,select,azimuth,elevation,base555,ours_r,ours_g,ours_b,ours_a,ram_r,ram_g,ram_b,ram_a,disc_r,disc_g,disc_b,disc_a\n");
        for d in &self.diffs {
            s += &format!(
                "{},{},{:#x},{:?},{:#x},{},{},{:#x},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                d.tfrag, d.vertex, d.rgba_addr, d.class, d.light.select, d.light.azimuth, d.light.elevation, d.light.color,
                d.ours[0], d.ours[1], d.ours[2], d.ours[3], d.ram[0], d.ram[1], d.ram[2], d.ram[3], d.disc[0], d.disc[1], d.disc[2], d.disc[3]
            );
        }
        crate::write_output(path, s).with_context(|| format!("writing {}", path.display()))
    }
}

/// Builds a synthetic EE RAM image of `lvl` "as loaded": the tfrags block at `core_base +
/// LevelCoreHeader.tfrags`, header `data` fields relocated to absolute pointers (as `fun_002040e0`
/// does), the directional bank at its level address, the core-data global set, and, if `lit`, our
/// lit RGBA written over each RGBA block (as `LightTfrags` does). Used by the tests and `rc-trace synth`.
pub fn synthesize(lvl: &LevelInputs, core_base: u32, lit: bool) -> EeImage {
    let mut ram = vec![0u8; crate::ee::EE_RAM_SIZE];
    let block_addr = core_base as usize + lvl.block_offset;
    ram[block_addr..block_addr + lvl.block.len()].copy_from_slice(&lvl.block);
    let table = block_addr + lvl.table_offset;
    for (i, t) in lvl.tfrags.iter().enumerate() {
        let h = table + 0x40 * i;
        let data = (table as u32).wrapping_add(t.header.data as u32);
        ram[h + 0x10..h + 0x14].copy_from_slice(&data.to_le_bytes());
        if lit {
            let rgba = lvl.lit(i);
            let a = data as usize + t.header.rgba_ofs as usize;
            for (k, c) in rgba.iter().enumerate() { ram[a + 4 * k..a + 4 * k + 4].copy_from_slice(&px(*c)); }
        }
    }
    let k = known_addrs(lvl.level).unwrap_or(KnownAddrs { core_data_ptr: 0x0017_4294, dir_bank: 0x0018_0340 });
    ram[k.core_data_ptr as usize..k.core_data_ptr as usize + 4].copy_from_slice(&core_base.to_le_bytes());
    let b = lvl.bank_bytes();
    ram[k.dir_bank as usize..k.dir_bank as usize + b.len()].copy_from_slice(&b);
    EeImage::new(ram, format!("synthetic level {:02} at core base {core_base:#x}{}", lvl.level, if lit { ", lit" } else { ", unlit" }))
}
