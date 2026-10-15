//! Truth test for the tie and shrub lighting ports (`rc_formats::tie_light`, `rc_formats::shrub_light`;
//! docs/plan/tie_lighting.md, shrub_lighting.md): compares our lit colour tables with what `LightTies` and
//! `LightShrubs` left in EE RAM.
//!
//! Where the game keeps them (level01 `InitLevelRenderGlobals` 0x255958; the pointer globals are read from
//! RAM, and every record is checked by content before it is compared):
//! * ties: count `0x160fcc`, 0x20-byte run-time records at `*0x160fc0` (+0x10 pointer to the 0x1c0 record,
//!   +0x1a class index, +0x1c light selector, +0x1e point-light nibbles), 0x1c0 records at `*0x160fc8`
//!   (+0x40 the 64 lit RGBA words, +0x140 the 64 ambient RGBA5551 copied from the instance);
//! * shrubs: count `0x160490`, run-time records `*0x160494` + i·0x20 (+0x1c selector, +0x1e point
//!   nibbles), matrix blocks `*0x16049c` + i·0x40, palettes `*0x1604a0` + i·0x60 (24 RGBA words).
//!
//! The comparison lights with the light bank read from RAM (0x180340 / 0x180740) and the selector and
//! point list read from each live run-time record, so a mismatch is in the lighting maths or the
//! class/instance parse, not in the inputs.

use crate::ee::EeImage;
use anyhow::{bail, Context, Result};
use rc_formats::shrub::{self, LevelShrubClass, ShrubInstance};
use rc_formats::shrub_light::{self, ShrubPointLights, PALETTE};
use rc_formats::tfrag_light::{DirLightSet, LightBank, PointLight, BANK_SETS, POINT_LIGHT_SLOTS};
use rc_formats::tie::{self, LevelTieClass, TieInstance};
use rc_formats::tie_light::{self, TiePointLights, SLOTS};
use rc_formats::{level, wad};
use std::collections::HashMap;
use std::path::Path;

/// Level01 globals (docs/plan/tie_lighting.md §2, shrub_lighting.md §1).
pub const TIE_COUNT: u32 = 0x0016_0fcc;
pub const TIE_RT_RECORDS: u32 = 0x0016_0fc0;
pub const TIE_BIG_RECORDS: u32 = 0x0016_0fc8;
pub const SHRUB_COUNT: u32 = 0x0016_0490;
pub const SHRUB_RT_RECORDS: u32 = 0x0016_0494;
pub const SHRUB_PALETTES: u32 = 0x0016_04a0;
pub const DIR_BANK: u32 = 0x0018_0340;
pub const POINT_BANK: u32 = 0x0018_0740;

fn f32s<const N: usize>(b: &[u8]) -> [f32; N] { std::array::from_fn(|k| f32::from_le_bytes(b[4 * k..4 * k + 4].try_into().unwrap())) }

/// The directional bank (16 sets) and the point bank as they are in RAM.
pub fn ram_banks(ee: &EeImage, count: usize) -> Result<(LightBank, [PointLight; POINT_LIGHT_SLOTS])> {
    let raw = ee.bytes(DIR_BANK, 0x40 * BANK_SETS)?;
    let mut bank = LightBank { count, ..Default::default() };
    for (s, c) in bank.sets.iter_mut().zip(raw.chunks(0x40)) {
        *s = DirLightSet { color_a: f32s(&c[0..]), dir_a: f32s(&c[0x10..]), color_b: f32s(&c[0x20..]), dir_b: f32s(&c[0x30..]) };
    }
    let p = ee.bytes(POINT_BANK, 0x20 * POINT_LIGHT_SLOTS)?;
    let points = std::array::from_fn(|k| PointLight { color: f32s(&p[0x20 * k..]), pos: f32s(&p[0x20 * k + 0x10..]) });
    Ok((bank, points))
}

pub struct Inputs {
    pub disc_bank: LightBank,
    pub ties: Vec<TieInstance>,
    pub tie_classes: Vec<LevelTieClass>,
    pub shrubs: Vec<ShrubInstance>,
    pub shrub_classes: Vec<LevelShrubClass>,
}

impl Inputs {
    pub fn load(extracted: &Path, lvl: u32) -> Result<Inputs> {
        let dir = extracted.join(format!("levels/{lvl:02}"));
        let rd = |n: &str| std::fs::read(dir.join(n)).with_context(|| format!("reading {}/{n}", dir.display()));
        let gameplay = wad::decompress(&rd("gameplay_ntsc.bin")?)?;
        let core_data = wad::decompress(&rd("core_data.bin")?)?;
        let core = level::parse_level_core(&rd("core_index.bin")?, core_data.len())?;
        Ok(Inputs {
            disc_bank: rc_formats::tfrag_light::parse_light_bank(&gameplay)?,
            ties: tie::parse_tie_instances(&gameplay)?,
            tie_classes: tie::parse_level_ties(&core, &core_data)?,
            shrubs: shrub::parse_shrub_instances(&gameplay)?,
            shrub_classes: shrub::parse_level_shrubs(&core, &core_data)?,
        })
    }
}

/// One differing colour entry: (instance, slot, ours, RAM, selector, point list).
pub type EntryDiff = (usize, usize, [u8; 4], [u8; 4], u16, u16);

/// Per-kind result.
#[derive(Default, Debug)]
pub struct KindReport {
    pub instances: usize,
    /// Instances whose RAM record passed the content check (the disc bytes the loader copies).
    pub located: usize,
    pub instances_equal: usize,
    pub entries: u64,
    pub entries_equal: u64,
    pub bytes_equal: u64,
    /// Instances with a live point-light list / a blended selector.
    pub point_lit: usize,
    pub blended: usize,
    pub blended_equal: usize,
    pub point_lit_equal: usize,
    /// Per channel r,g,b,a: histogram of ours − RAM for −8..=8, and |d| > 8.
    pub hist: [[u64; 17]; 4],
    pub hist_far: [u64; 4],
    pub diffs: Vec<EntryDiff>,
    pub notes: Vec<String>,
}

impl KindReport {
    fn add(&mut self, i: usize, ours: &[[u8; 4]], ram: &[u8], sel: u16, plist: u16) {
        let mut all = true;
        for (j, o) in ours.iter().enumerate() {
            let m: [u8; 4] = ram[4 * j..4 * j + 4].try_into().unwrap();
            self.entries += 1;
            for k in 0..4 {
                let d = o[k] as i32 - m[k] as i32;
                if (-8..=8).contains(&d) { self.hist[k][(d + 8) as usize] += 1 } else { self.hist_far[k] += 1 }
                self.bytes_equal += (d == 0) as u64;
            }
            if *o == m { self.entries_equal += 1 } else {
                all = false;
                self.diffs.push((i, j, *o, m, sel, plist));
            }
        }
        self.instances_equal += all as usize;
        if sel & 0xff00 != 0 { self.blended += 1; self.blended_equal += all as usize; }
        if plist != 0xffff { self.point_lit += 1; self.point_lit_equal += all as usize; }
    }

    pub fn print(&self, name: &str, max: usize) {
        let pct = |a: u64, b: u64| if b == 0 { "-".to_string() } else { format!("{:.3}%", 100.0 * a as f64 / b as f64) };
        println!("  {name}: {} instances, {} located by content; {} fully equal ({}); entries {}/{} equal ({}); bytes {}",
            self.instances, self.located, self.instances_equal, pct(self.instances_equal as u64, self.located as u64),
            self.entries_equal, self.entries, pct(self.entries_equal, self.entries), pct(self.bytes_equal, 4 * self.entries));
        println!("    blended selector: {} ({} equal); live point-light list: {} ({} equal)", self.blended, self.blended_equal, self.point_lit, self.point_lit_equal);
        for n in &self.notes { println!("    note: {n}"); }
        if self.entries_equal != self.entries {
            for (k, ch) in ["r", "g", "b", "a"].iter().enumerate() {
                let cells: Vec<String> = (0..17).filter(|&j| self.hist[k][j] > 0 && j != 8).map(|j| format!("{:+}:{}", j as i32 - 8, self.hist[k][j])).collect();
                println!("    {ch}: equal {:>8}  {}  far {}", self.hist[k][8], cells.join(" "), self.hist_far[k]);
            }
            println!("    first mismatches (instance slot: ours / RAM | selector points):");
            for d in self.diffs.iter().take(max) { println!("      {:5} {:2}: {:?} / {:?} | {:#06x} {:#06x}", d.0, d.1, d.2, d.3, d.4, d.5); }
        }
    }
}

/// `LightTies` output vs RAM.
pub fn compare_ties(ee: &EeImage, inp: &Inputs, bank: &LightBank, points: &[PointLight; POINT_LIGHT_SLOTS]) -> Result<KindReport> {
    let mut r = KindReport { instances: inp.ties.len(), ..Default::default() };
    let n = ee.u32(TIE_COUNT)? as usize;
    if n != inp.ties.len() { bail!("tie count in RAM {n} != disc {}", inp.ties.len()); }
    let rt = ee.u32(TIE_RT_RECORDS)?;
    let big = ee.u32(TIE_BIG_RECORDS)?;
    let classes: HashMap<i32, &LevelTieClass> = inp.tie_classes.iter().map(|c| (c.o_class, c)).collect();
    let mut bad_ptr = 0;
    for (i, inst) in inp.ties.iter().enumerate() {
        let rec = ee.bytes(rt + 0x20 * i as u32, 0x20)?;
        let p = u32::from_le_bytes(rec[0x10..0x14].try_into().unwrap());
        if p != big + 0x1c0 * i as u32 { bad_ptr += 1; }
        let b = ee.bytes(p, 0x1c0)?;
        // Content check: the ambient copy and the matrix columns' xyz.
        let amb_ok = (0..SLOTS).all(|j| u16::from_le_bytes([b[0x140 + 2 * j], b[0x141 + 2 * j]]) == inst.ambient_rgbas[j]);
        let mat_ok = (0..4).all(|c| (0..3).all(|k| f32::from_le_bytes(b[16 * c + 4 * k..16 * c + 4 * k + 4].try_into().unwrap()).to_bits() == inst.matrix[c][k].to_bits()));
        if !(amb_ok && mat_ok) { continue; }
        r.located += 1;
        let Some(class) = classes.get(&inst.o_class) else { r.notes.push(format!("tie {i}: class {} not parsed", inst.o_class)); continue };
        let sel = u16::from_le_bytes([rec[0x1c], rec[0x1d]]);
        let plist = u16::from_le_bytes([rec[0x1e], rec[0x1f]]);
        let mut live = *inst;
        live.directional_lights = sel as i32;
        let tp = TiePointLights { bank: points, list: plist };
        let ours = tie_light::light_tie_instance(&class.class, &live, bank, (plist != 0xffff).then_some(&tp));
        r.add(i, &ours, &b[0x40..0x140], sel, plist);
    }
    if bad_ptr > 0 { r.notes.push(format!("{bad_ptr} run-time records do not point at *0x160fc8 + i*0x1c0")); }
    let sel_changed = inp.ties.iter().enumerate().filter(|(i, t)| ee.u16(rt + 0x20 * *i as u32 + 0x1c).ok() != Some(t.directional_lights as u16)).count();
    if sel_changed > 0 { r.notes.push(format!("{sel_changed} run-time selectors (+0x1c) differ from the disc's directional_lights")); }
    Ok(r)
}

/// `LightShrubs` output vs RAM.
pub fn compare_shrubs(ee: &EeImage, inp: &Inputs, bank: &LightBank, points: &[PointLight; POINT_LIGHT_SLOTS]) -> Result<KindReport> {
    let mut r = KindReport { instances: inp.shrubs.len(), ..Default::default() };
    let n = ee.u32(SHRUB_COUNT)? as usize;
    if n != inp.shrubs.len() { bail!("shrub count in RAM {n} != disc {}", inp.shrubs.len()); }
    let rt = ee.u32(SHRUB_RT_RECORDS)?;
    let pal = ee.u32(SHRUB_PALETTES)?;
    let mats = ee.u32(SHRUB_RT_RECORDS + 8)?; // 0x16049c
    let classes: HashMap<i32, &LevelShrubClass> = inp.shrub_classes.iter().map(|c| (c.o_class, c)).collect();
    for (i, inst) in inp.shrubs.iter().enumerate() {
        let rec = ee.bytes(rt + 0x20 * i as u32, 0x20)?;
        // Content check: the matrix block's columns xyz = the instance matrix.
        let m = ee.bytes(mats + 0x40 * i as u32, 0x40)?;
        let mat_ok = (0..4).all(|c| (0..3).all(|k| f32::from_le_bytes(m[16 * c + 4 * k..16 * c + 4 * k + 4].try_into().unwrap()).to_bits() == inst.matrix[c][k].to_bits()));
        if !mat_ok { continue; }
        r.located += 1;
        let Some(class) = classes.get(&inst.o_class) else { r.notes.push(format!("shrub {i}: class {} not parsed", inst.o_class)); continue };
        let sel = u16::from_le_bytes([rec[0x1c], rec[0x1d]]);
        let plist = u16::from_le_bytes([rec[0x1e], rec[0x1f]]);
        let mut live = *inst;
        live.dir_lights = sel as i32;
        let sp = ShrubPointLights { bank: points, list: plist };
        let ours = shrub_light::light_shrub_instance_with_points(&class.class, &live, bank, (plist != 0xffff).then_some(&sp));
        let ram = ee.bytes(pal + 0x60 * i as u32, 4 * PALETTE)?;
        r.add(i, &ours, ram, sel, plist);
        // The loader's average colour in column 1's w.
        let avg = shrub_light::average_colour(&ours);
        let w = u32::from_le_bytes(m[0x1c..0x20].try_into().unwrap());
        let ours_w = avg[0] as u32 | (avg[1] as u32) << 8 | (avg[2] as u32) << 16;
        if w & 0x00ff_ffff != ours_w && r.notes.len() < 8 { r.notes.push(format!("shrub {i}: matrix col1.w {w:#010x} vs our average colour {ours_w:#08x}")); }
    }
    Ok(r)
}
