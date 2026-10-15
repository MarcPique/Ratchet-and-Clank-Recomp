//! The in-game map (docs/plan/menus.md §13): the map files, the fog mask and its writer, the saved mask, the
//! Map-o-Matic set, the world → map transform. Level01 addresses; every function named here is engine code
//! (boot-hash-matched), compiled into each level overlay with the same per-level tables.
//!
//! **The map files** (global TOC field 0x820, 38 entries, each WAD-compressed): 0..18 the plain maps, 19..37 the
//! Map-o-Matic maps, chosen by owned[33] (the TOC tables 0x1383a0 / 0x138438). A header of 8 offsets: [0] the zone
//! tiles (u32 0x100, u32 0x20, then 256 u16 end offsets and the run-length nibble data of 256 tiles of 32×32 px),
//! [1] the mask run table ({skip, count} byte pairs: the map pixels a saved mask stores), [2] the fogged picture
//! (512×512, 8-bit), [3] the revealed picture, [4..6] three 128×128 PIFs (the globe's layers; [4]'s palette is the
//! map's too [L: the map's CLUT is uploaded to 0x3ff0, the PIFs' to `0x1848b4`; the pictures decode right with it]).
//!
//! **The fog mask** (0x18467c, 512×512 bits, bit set = fogged, row y at byte 64·y, bit x & 7 of byte x / 8): the
//! level entry `FUN_0025a4c0` makes it from the saved mask (chunk 3002 = 0x141ec0 + 0x800·level) when that has one,
//! else from the zone tiles (every pixel of a zone ≠ 0 fogged). The hero update calls the writer `FUN_0025c4f8` every
//! tick; the pause page composes the fogged and revealed pictures through it (`compose_bitmap_from_mask` 0x25af58);
//! the saves pack it back (`fun_00207b08` 0x25deb8 from `memcard_Save` 0x261448 and the Save page 0x29a5c8).

pub mod predicates;

use std::sync::Arc;

/// The map files' TOC field.
pub const FIELD: u32 = 0x820;
/// Levels with a map entry (the TOC tables hold 19 each).
pub const LEVELS: usize = 19;
/// The Map-o-Matic (owned[33], 0x13d4e1): its set is the second 19 entries.
pub const MAP_O_MATIC: usize = 33;
/// The map's side in pixels.
pub const SIZE: usize = 512;
/// The live mask's bytes (512 × 512 bits) and a saved mask's (chunk 3002).
pub const MASK_BYTES: usize = SIZE * SIZE / 8;
pub const SAVE_BYTES: usize = 0x800;

/// The map file index of `level` (`0x1383a0` / `0x138438` + 8·level).
pub fn file_index(level: usize, map_o_matic: bool) -> usize { level + if map_o_matic { LEVELS } else { 0 } }

/// One decompressed map file.
#[derive(Clone, Debug)]
pub struct MapFile {
    pub bytes: Arc<Vec<u8>>,
    pub hdr: [usize; 8],
}

impl MapFile {
    /// The file (already WAD-decompressed); None when its header does not describe it.
    pub fn parse(bytes: Vec<u8>) -> Option<MapFile> {
        let hdr: [usize; 8] = std::array::from_fn(|k| bytes.get(4 * k..4 * k + 4).map_or(usize::MAX, |b| u32::from_le_bytes(b.try_into().unwrap()) as usize));
        let ok = hdr.windows(2).all(|w| w[0] <= w[1])
            && hdr[7] <= bytes.len()
            && hdr[1] >= hdr[0] + 8 + 0x200
            && hdr[3] >= hdr[2] + MASK_BYTES * 8
            && hdr[4] >= hdr[3] + MASK_BYTES * 8;
        ok.then(|| MapFile { bytes: Arc::new(bytes), hdr })
    }

    /// The zone tiles' base (`DAT_00184670` = the copied block + 8): the u16 end offsets, then the data from +0x200.
    fn tiles(&self) -> &[u8] { &self.bytes[self.hdr[0] + 8..self.hdr[1]] }

    /// The mask run table (`DAT_00184684`).
    pub fn runs(&self) -> &[u8] { &self.bytes[self.hdr[1]..self.hdr[2]] }

    /// The fogged / revealed pictures (512 × 512 palette indices).
    pub fn fogged(&self) -> &[u8] { &self.bytes[self.hdr[2]..self.hdr[2] + SIZE * SIZE] }
    pub fn revealed(&self) -> &[u8] { &self.bytes[self.hdr[3]..self.hdr[3] + SIZE * SIZE] }

    /// The 256-entry palette of the pictures (CSM1 order: the first PIF's).
    pub fn palette(&self) -> &[u8] { &self.bytes[self.hdr[4] + 0x20..self.hdr[4] + 0x420] }

    /// The PIF `k` (0..3) of the header's [4 + k].
    pub fn pif(&self, k: usize) -> Option<rc_formats::pif::Pif<'_>> { rc_formats::pif::Pif::parse_at(&self.bytes, *self.hdr.get(4 + k)?).ok() }

    /// `fun_00206860` (0x25c3e0): tile `t`'s 32 × 32 zone numbers (row-major). Each run is 3 nibbles from the tile's
    /// start: the zone, then an 8-bit length (0 = 256); at an even nibble the zone is the low nibble of its byte and
    /// the length the next two nibbles, at an odd one the high nibble and the next byte.
    pub fn tile(&self, t: usize) -> [u8; 1024] {
        let d = self.tiles();
        let u16_at = |i: usize| d.get(2 * i..2 * i + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
        let start = if t == 0 { 0x200 } else { u16_at(t - 1) };
        let end = u16_at(t);
        let runs = (end.saturating_sub(start) * 2) / 3;
        let mut out = [0u8; 1024];
        let mut n = 0usize;
        for k in 0..runs {
            let p = 3 * k;
            let (b0, b1) = (d.get(start + p / 2).copied().unwrap_or(0), d.get(start + p / 2 + 1).copied().unwrap_or(0));
            let (zone, len) = if p & 1 == 0 { (b0 & 0xf, (((b1 & 0xf) as usize) << 4) | (b0 >> 4) as usize) } else { (b0 >> 4, b1 as usize) };
            let len = if len == 0 { 0x100 } else { len };
            for _ in 0..len {
                if n < out.len() { out[n] = zone; }
                n += 1;
            }
        }
        out
    }

    /// The zone number of map pixel (x, y).
    pub fn zone(&self, x: usize, y: usize) -> u8 { self.tile((y >> 5) * 16 + (x >> 5))[(y & 31) * 32 + (x & 31)] }
}

/// A fog mask, 512 × 512 bits (set = fogged).
#[derive(Clone, PartialEq, Eq)]
pub struct Mask(pub Box<[u8]>);

impl std::fmt::Debug for Mask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Mask({} fogged)", self.fogged_count()) }
}

impl Mask {
    pub fn empty() -> Mask { Mask(vec![0; MASK_BYTES].into_boxed_slice()) }
    pub fn get(&self, x: usize, y: usize) -> bool { self.0[y * 64 + (x >> 3)] >> (x & 7) & 1 != 0 }
    pub fn set(&mut self, x: usize, y: usize, v: bool) {
        let b = &mut self.0[y * 64 + (x >> 3)];
        if v { *b |= 1 << (x & 7) } else { *b &= !(1 << (x & 7)) }
    }
    pub fn fogged_count(&self) -> usize { self.0.iter().map(|b| b.count_ones() as usize).sum() }

    /// `fun_00206710` (0x25c290): every pixel of a zone ≠ 0 fogged (the tiles in order, 32 rows of 32 bits each).
    pub fn initial(file: &MapFile) -> Mask {
        let mut m = Mask::empty();
        for t in 0..256 {
            let tile = file.tile(t);
            let (tx, ty) = ((t & 15) * 32, (t >> 4) * 32);
            for (i, &z) in tile.iter().enumerate() {
                if z != 0 { m.set(tx + (i & 31), ty + (i >> 5), true); }
            }
        }
        m
    }

    /// The saved mask `saved` (chunk 3002) back (`fun_00207bb0` 0x25df60): bit 0 of the first byte set → the run
    /// coding over `runs` ([`Mask::unpack_runs`]), clear → the 128 × 128 downsample ([`Mask::unpack_coarse`]).
    pub fn unpack(saved: &[u8], runs: &[u8]) -> Mask {
        if saved.first().is_some_and(|b| b & 1 != 0) { Mask::unpack_runs(saved, runs) } else { Mask::unpack_coarse(saved) }
    }

    /// `fun_00207c28` (0x25dfd8): the run table's pixels in order (skip `runs[2i]`, then `runs[2i + 1]` coded ones);
    /// the coded values alternate from fogged, the first length `saved[0] >> 1`, then one byte each (a 0 length
    /// flips again). Skipped pixels are clear.
    pub fn unpack_runs(saved: &[u8], runs: &[u8]) -> Mask {
        let mut m = Mask::empty();
        let mut src = 1usize;
        let mut left = (saved.first().copied().unwrap_or(0) >> 1) as u32;
        let mut value = true;
        let mut pos = 0usize;
        for pair in runs.as_chunks::<2>().0 {
            pos += pair[0] as usize;
            for _ in 0..pair[1] {
                while left == 0 {
                    left = saved.get(src).copied().unwrap_or(0xff) as u32;
                    src += 1;
                    value = !value;
                }
                if pos < SIZE * SIZE && value { m.set(pos % SIZE, pos / SIZE, true); }
                left -= 1;
                pos += 1;
            }
            if pos >= SIZE * SIZE { break; }
        }
        m
    }

    /// `fun_00207e58` (0x25e208): 128 rows of 16 bytes, each bit a 4 × 4 block (bit k of byte j → pixels
    /// 32j + 4k .. +4 of 4 rows).
    pub fn unpack_coarse(saved: &[u8]) -> Mask {
        let mut m = Mask::empty();
        for row in 0..128 {
            let mut line = [0u8; 64];
            for j in 0..16 {
                let b = saved.get(row * 16 + j).copied().unwrap_or(0);
                for k in 0..8 {
                    if b >> k & 1 != 0 {
                        let bit = 32 * j + 4 * k;
                        for x in bit..bit + 4 { line[x >> 3] |= 1 << (x & 7); }
                    }
                }
            }
            for r in 0..4 { m.0[(row * 4 + r) * 64..(row * 4 + r + 1) * 64].copy_from_slice(&line); }
        }
        m
    }

    /// `fun_00207b08` (0x25deb8): the mask packed into a save's 0x800 bytes: the run coding (`FUN_00222328`) when it
    /// fits, else the downsample (`fun_00208030`). Also returns the run coding's length (−1: it did not fit), which
    /// the game keeps as a statistic in 0x13d560[level].
    pub fn pack(&self, runs: &[u8]) -> ([u8; SAVE_BYTES], i32) {
        let mut out = [0u8; SAVE_BYTES];
        let n = self.pack_runs(runs, &mut out);
        if n < 0 {
            out = [0; SAVE_BYTES];
            self.pack_coarse(&mut out);
        }
        (out, n)
    }

    /// `FUN_00222328` (0x222328): the run table's pixels as alternating lengths from fogged (the first ≤ 0x7f, stored
    /// ·2 + 1; the rest ≤ 0xff, a 0 between two maximal lengths of the same value); −1 when 0x800 bytes are not enough.
    pub fn pack_runs(&self, runs: &[u8], out: &mut [u8; SAVE_BYTES]) -> i32 {
        let bit = |p: usize| self.0[p >> 3] >> (p & 7) & 1;
        let (mut limit, mut count, mut o) = (0x7fu32, 0u32, 0usize);
        let (mut pos, mut left, mut cur) = (0usize, 0u32, 1u8);
        let mut table = runs.as_chunks::<2>().0.iter();
        loop {
            while left != 0 {
                let b = bit(pos);
                pos += 1;
                left -= 1;
                if cur == b {
                    if count == limit {
                        out[o] = count as u8;
                        count = 1;
                        limit = 0xff;
                        if o + 2 == SAVE_BYTES { return -1; }
                        out[o + 1] = 0;
                        o += 2;
                    } else {
                        count += 1;
                    }
                } else {
                    out[o] = count as u8;
                    limit = 0xff;
                    if o + 1 == SAVE_BYTES { return -1; }
                    count = 1;
                    o += 1;
                    cur = b;
                }
            }
            let Some(pair) = table.next() else { break };
            left = pair[1] as u32;
            pos += pair[0] as usize;
            if pos >= SIZE * SIZE { break; }
        }
        if count != 0 {
            out[o] = count as u8;
            o += 1;
        }
        out[0] = out[0].wrapping_mul(2).wrapping_add(1);
        o as i32
    }

    /// `fun_00208030` (0x25e3e0): each 4 × 4 block → one bit, set when ≥ 8 of its pixels are fogged; bit 0 of the
    /// first byte cleared and bit 1 set (the format mark, over the first block's bits).
    pub fn pack_coarse(&self, out: &mut [u8; SAVE_BYTES]) {
        for group in 0..128 {
            let mut counts = [0u32; 128];
            for r in 0..4 {
                let row = &self.0[(group * 4 + r) * 64..(group * 4 + r + 1) * 64];
                for (i, &b) in row.iter().enumerate() {
                    counts[2 * i] += (b & 0xf).count_ones();
                    counts[2 * i + 1] += (b >> 4).count_ones();
                }
            }
            for j in 0..16 {
                out[group * 16 + j] = (0..8).fold(0u8, |acc, k| acc | (((counts[8 * j + k] >= 8) as u8) << k));
            }
        }
        out[0] = (out[0] & 0xfe) | 2;
    }
}

/// `compose_bitmap_from_mask` (0x25af58): per pixel the fogged picture where the mask is set, else the revealed one.
pub fn compose(file: &MapFile, mask: &Mask) -> Vec<u8> {
    let (fog, rev) = (file.fogged(), file.revealed());
    (0..SIZE * SIZE).map(|p| if mask.0[p >> 3] >> (p & 7) & 1 != 0 { fog[p] } else { rev[p] }).collect()
}

/// The world → map transforms (level entry 0x25a4c0 from the reference points 0x182c90: per level world points A,
/// B and their map pixels; `u = a + b·x`, `v = c + d·y` in pixels of the 512 map).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
}

/// The reference points' table (level01 address).
pub const TRANSFORM_POINTS: u32 = 0x182c90;

impl Transform {
    /// From the 8 floats `{wx_a, wy_a, wx_b, wy_b, u_a, v_a, u_b, v_b}` (the entry's order, 0x25a4c0).
    pub fn from_points(p: [f32; 8]) -> Transform {
        let b = (p[4] - p[6]) / (p[0] - p[2]);
        let d = (p[5] - p[7]) / (p[1] - p[3]);
        Transform { a: p[4] - b * p[0], b, c: p[5] - d * p[1], d }
    }

    /// `fun_00208408` (0x25e800): world (x, y) → map (0..1, 0..1).
    pub fn to_map(&self, x: f32, y: f32) -> (f32, f32) { ((self.a + self.b * x) * 0.001953125, (self.c + self.d * y) * 0.001953125) }
}

/// `fun_00208408` with the level-6 alternative (`level + 100` when the level-6 flag gp−0x6e08 is set): the map turned
/// a quarter, `u = (d₆·y + 1053) / 512`, `v = (740 − b₆·x) / 512`.
pub fn world_to_map(t: &[Transform; LEVELS], level: i32, alt: bool, x: f32, y: f32) -> (f32, f32) {
    let l = if (0..LEVELS as i32).contains(&level) { level as usize } else { 0 };
    if l == 6 && alt {
        let t6 = t[6];
        return ((t6.d * y + 1053.0) * 0.001953125, (740.0 - t6.b * x) * 0.001953125);
    }
    t[l].to_map(x, y)
}

/// One reveal zone (0x183020 + 0x100·level + 0x10·zone): the hero's z range (equal = any), the rule flags and the
/// flag 0x80's argument (an index into the map zone flags 0x184928).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Zone {
    pub z_min: f32,
    pub z_max: f32,
    pub flags: u32,
    pub arg: i32,
}

/// The zone table (level01 address).
pub const ZONES: u32 = 0x183020;
/// The per-level predicate table (level01 address; 19 × 8 function pointers, [`predicates::address`]).
pub const PREDICATES: u32 = 0x184410;

/// Zone rule flags (`FUN_0025c4f8`).
pub mod zf {
    /// Only while riding (movement group 0x11 / 0x12, or 0x140634 == 1).
    pub const RIDING: u32 = 1;
    /// Only while not riding.
    pub const NOT_RIDING: u32 = 2;
    pub const GROUP_10: u32 = 4;
    pub const NOT_GROUP_10: u32 = 8;
    pub const GROUP_F: u32 = 0x10;
    pub const NOT_GROUP_F: u32 = 0x20;
    /// Only on a magnetic floor (0x13f658 ≠ 0).
    pub const MAGNETIC: u32 = 0x40;
    /// Only while the map zone flag `arg` (0x184928) is set.
    pub const ZONE_FLAG: u32 = 0x80;
    /// 0x100 << k: the level's predicate k (0x184410 + 0x20·level + 4k) must hold.
    pub const PREDICATE0: u32 = 0x100;
}

/// The reveal brush 0x184814 (level entry): 32 × 32 bits, set outside the circle of radius 16 about (15.5, 15.5).
pub fn brush_open(bx: usize, by: usize) -> bool {
    let (dx, dy) = (15.5 - bx as f32, 15.5 - by as f32);
    dx * dx + dy * dy <= 256.0
}

/// What the fog writer reads besides the mask and the tables.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FogInput {
    /// The hero position 0x13f3d0 (x, y, z) and its moby's z (`*(0x1413d0) + 0x18`, the same point).
    pub pos: [f32; 3],
    /// 0x15ed84 (the current level).
    pub level: i32,
    /// gp−0x6e08: level 6's alternative map.
    pub alt: bool,
    /// 0x1413dc (the hero's movement group), 0x140634, 0x13f658.
    pub group: i32,
    pub f0634: u8,
    pub magnetic: i16,
}

/// Pixel (x, y) of a map from its (0..1) coordinates: `(int)(u·512)`.
pub fn pixel(u: f32, v: f32) -> (i32, i32) { ((u * 512.0) as i32, (v * 512.0) as i32) }

/// The map system's state for the loaded level (0x184670..0x184940): the level's map file, its zone numbers, the
/// live mask, the tables, and the pause page's per-level view (zoom 0x184724, pan 0x184774 / 0x1847c4).
#[derive(Clone, Debug)]
pub struct MapState {
    /// 0x184698 (a map exists) = `file.is_some()`; the file loaded at the level entry.
    pub file: Option<MapFile>,
    /// The zone number of every pixel (the tile cache 0x1846a0 / 0x184678 decoded once).
    zones_px: Arc<Vec<u8>>,
    /// 0x18467c.
    pub mask: Mask,
    /// The level the state was made for (0x15ed84 at the entry).
    pub level: i32,
    pub transforms: Arc<[Transform; LEVELS]>,
    pub zones: Arc<[[Zone; 16]; LEVELS]>,
    /// 0x184928: the map zone flags the level classes set (flag 0x80's argument, the predicates' `zf`).
    pub zone_flags: [i32; 16],
    /// 0x1848a0: the loaded picture set is the Map-o-Matic's (owned[33] at the level entry, or picked up since and
    /// the page opened).
    pub map_o_matic: bool,
    /// gp−0x6e08 (level 6's quarter-turned map) and gp−0x6e04 (level 3's predicate).
    pub alt: bool,
    pub g15fdfc: i32,
    /// The run coding's length at the last pack (0x13d560[level], a statistic nothing reads).
    pub packed_len: i32,
    pub view: [View; LEVELS],
}

/// The pause page's view of one level's map (0x184724 zoom, 0x184774 / 0x1847c4 pan in 1/0x8000 pixels of the
/// 0x2000-unit picture, i.e. fixed point 16.16 of 0..0x2000·...; the defaults 0x184370 per level, zoom 0.65).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub pan: [i32; 2],
}

impl Default for View {
    fn default() -> View { View { zoom: f32::from_bits(0x3f26_6666), pan: [0, 0] } }
}

/// The default pans (level01 address; 19 × {x, y}).
pub const DEFAULT_PANS: u32 = 0x184370;

impl Default for MapState {
    fn default() -> MapState {
        MapState {
            file: None,
            zones_px: Arc::new(Vec::new()),
            mask: Mask::empty(),
            level: 0,
            transforms: Arc::new([Transform::default(); LEVELS]),
            zones: Arc::new([[Zone::default(); 16]; LEVELS]),
            zone_flags: [0; 16],
            map_o_matic: false,
            alt: false,
            g15fdfc: 0,
            packed_len: 0,
            view: [View::default(); LEVELS],
        }
    }
}

/// The tables the map reads from the level overlay (the same in every overlay: see `tests/ui/map_levels.rs`).
#[derive(Clone, Debug)]
pub struct Tables {
    pub transforms: [Transform; LEVELS],
    pub zones: [[Zone; 16]; LEVELS],
    pub pans: [[i32; 2]; LEVELS],
}

impl Tables {
    /// From the overlay at the level01 labels [`TRANSFORM_POINTS`], [`ZONES`], [`DEFAULT_PANS`] (relocated).
    pub fn read(ov: &crate::menus::Overlay) -> Option<Tables> {
        let f = |a: u32| ov.u32(a).map(f32::from_bits);
        let tp = ov.at(TRANSFORM_POINTS);
        let zt = ov.at(ZONES);
        let pt = ov.at(DEFAULT_PANS);
        let mut t = Tables { transforms: [Transform::default(); LEVELS], zones: [[Zone::default(); 16]; LEVELS], pans: [[0; 2]; LEVELS] };
        for l in 0..LEVELS {
            let base = tp + 32 * l as u32;
            let p: Option<Vec<f32>> = (0..8).map(|k| f(base + 4 * k)).collect();
            t.transforms[l] = Transform::from_points(p?.try_into().ok()?);
            for z in 0..16 {
                let a = zt + 0x100 * l as u32 + 0x10 * z as u32;
                t.zones[l][z] = Zone { z_min: f(a)?, z_max: f(a + 4)?, flags: ov.u32(a + 8)?, arg: ov.i32(a + 12)? };
            }
            t.pans[l] = [ov.i32(pt + 8 * l as u32)?, ov.i32(pt + 8 * l as u32 + 4)?];
        }
        Some(t)
    }
}

impl MapState {
    /// The level entry's map part (`FUN_0025a4c0`): the views reset to the default pans and zoom 0.65; the level's
    /// map file (the Map-o-Matic set when owned[33]), its mask from the level's saved mask (chunk 3002) when that has
    /// one, else from the zone tiles. `file` None: no map for the level (0x184698 = 0, the page says "no map").
    pub fn enter(tables: &Tables, level: i32, map_o_matic: bool, file: Option<MapFile>, saved: &[u8]) -> MapState {
        let mut s = MapState { transforms: Arc::new(tables.transforms), zones: Arc::new(tables.zones), level, map_o_matic, ..MapState::default() };
        for (v, p) in s.view.iter_mut().zip(tables.pans) { *v = View { zoom: f32::from_bits(0x3f26_6666), pan: p }; }
        if let Some(f) = file {
            let mut px = vec![0u8; SIZE * SIZE];
            for t in 0..256 {
                let tile = f.tile(t);
                let (tx, ty) = ((t & 15) * 32, (t >> 4) * 32);
                for (i, &z) in tile.iter().enumerate() { px[(ty + (i >> 5)) * SIZE + tx + (i & 31)] = z; }
            }
            s.zones_px = Arc::new(px);
            s.mask = if saved.first().is_some_and(|&b| b != 0) { Mask::unpack(saved, f.runs()) } else { Mask::initial(&f) };
            s.file = Some(f);
        }
        s
    }

    /// The zone number of pixel (x, y) (0 outside the map or without one).
    pub fn zone(&self, x: usize, y: usize) -> u8 { self.zones_px.get(y * SIZE + x).copied().unwrap_or(0) }

    /// `fun_00207b08`: the mask packed for the save (all zero without a map).
    pub fn pack(&mut self) -> [u8; SAVE_BYTES] {
        let Some(f) = &self.file else { return [0; SAVE_BYTES] };
        let (out, n) = self.mask.pack(f.runs());
        if self.packed_len < n { self.packed_len = n; }
        out
    }

    /// Which of the level's 16 zones the hero reveals now (`FUN_0025c4f8`'s first loop; zone 0 always).
    pub fn zones_open(&self, inp: &FogInput, flags: &[u8], px: i32, py: i32) -> [bool; 16] {
        let l = inp.level.clamp(0, LEVELS as i32 - 1) as usize;
        let l = if inp.level > 0x12 { 0 } else { l };
        let riding = (inp.group - 0x11) as u32 <= 1 || inp.f0634 == 1;
        let (g10, gf) = (inp.group == 0x10, inp.group == 0xf);
        let env = predicates::Env {
            x: inp.pos[0],
            y: inp.pos[1],
            z: inp.pos[2],
            px,
            py,
            flags,
            zone_flags: &self.zone_flags,
            group: inp.group,
            f0634: inp.f0634,
            g15fdfc: self.g15fdfc,
        };
        let mut open = [true; 16];
        for (z, o) in open.iter_mut().enumerate().skip(1) {
            let zone = self.zones[l][z];
            if zone.z_min != zone.z_max && !(zone.z_min <= inp.pos[2] && inp.pos[2] <= zone.z_max) {
                *o = false;
                continue;
            }
            let f = zone.flags;
            if f == 0 { continue; }
            let fail = (f & zf::RIDING != 0 && !riding)
                || (f & zf::NOT_RIDING != 0 && riding)
                || (f & zf::GROUP_10 != 0 && !g10)
                || (f & zf::NOT_GROUP_10 != 0 && g10)
                || (f & zf::GROUP_F != 0 && !gf)
                || (f & zf::NOT_GROUP_F != 0 && gf)
                || (f & zf::MAGNETIC != 0 && inp.magnetic == 0)
                || (f & zf::ZONE_FLAG != 0 && self.zone_flags.get(zone.arg.max(0) as usize).is_none_or(|&v| v == 0))
                || (0..8).any(|k| f & (zf::PREDICATE0 << k) != 0 && predicates::eval(inp.level.max(0) as usize, k, &env) == Some(false));
            if fail { *o = false; }
        }
        open
    }

    /// `FUN_0025c4f8` (0x25c4f8): the hero's map pixel; within the map, every fogged pixel of the 32 × 32 square about
    /// it (x, y from pixel − 15 to < min(pixel + 17, 511)) inside the brush circle and in an open zone is revealed.
    /// Nothing without a map. Returns the number of pixels revealed.
    pub fn reveal(&mut self, inp: &FogInput, flags: &[u8]) -> usize {
        if self.file.is_none() { return 0; }
        let (u, v) = world_to_map(&self.transforms, inp.level, inp.alt, inp.pos[0], inp.pos[1]);
        let (hx, hy) = pixel(u, v);
        let (cx, cy) = (hx + 1, hy + 1);
        if !(0..SIZE as i32).contains(&cx) || !(0..SIZE as i32).contains(&cy) { return 0; }
        let open = self.zones_open(inp, flags, cx, cy);
        let (x0, y0) = ((hx - 15).max(0), (hy - 15).max(0));
        let (x1, y1) = ((hx + 17).min(0x1ff), (hy + 17).min(0x1ff));
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let (xu, yu) = (x as usize, y as usize);
                if !self.mask.get(xu, yu) { continue; }
                let (bx, by) = (x - cx + 16, y - cy + 16);
                if !(0..32).contains(&bx) || !(0..32).contains(&by) || !brush_open(bx as usize, by as usize) { continue; }
                if open[self.zone(xu, yu) as usize & 15] {
                    self.mask.set(xu, yu, false);
                    n += 1;
                }
            }
        }
        n
    }
}
