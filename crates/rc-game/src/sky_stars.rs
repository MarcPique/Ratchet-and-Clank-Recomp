//! Sky stars: the star sprites the level sky dispatch generates on its first frame and updates every frame
//! (docs/plan/sky_render_notes.md "Stars in the port", docs/plan/particles.md §8).
//!
//! Only levels 00, 02, 05, 06, 07, 13, 15 and 17 have stars. Their sky dispatch (level00 overlay copies, which
//! match each level's own overlay instruction for instruction apart from relocated addresses) draws the first
//! shells, then — while sky header +8 (the star count) is 0, i.e. on the first frame after a level load — fills
//! the 0x20-byte records at sky header +0x1c, updates every record, calls `SkySpriteProc` (level00 0x28bd58,
//! boot 0x22bba0) and draws the remaining shells. Generation and update run in the **frame render**, after the
//! frame's game tick, and draw from the one shared `rand` stream (no `srand` anywhere in this code; the last
//! seed is the level init's `srand(1234)`).
//!
//! | Level | Dispatch (level00 / own overlay) | Records | Drawn before shell |
//! |---|---|---|---|
//! | 00 | 0x289330 / – | generic 0x288ec0(244, 12): 244 kind 1 + 12 kind 0 | 2 |
//! | 02 | 0x2895a0 / 0x28a448 | inline: 246 kind 3 (textures 2/3) + 10 kind 0 (texture 3) | 2 |
//! | 05 | 0x289bd8 / 0x2b3760 | generic 0x2b32e8(120, 8) | 1 |
//! | 06 | 0x289cb8 / 0x297d20 | inline: 234 kind 3 + 6 kind 2 (fixed) + 16 kind 0 | 1 |
//! | 07 | 0x28a208 / 0x2b1d60 | generic 0x2b18e0(120, 8) | 1 |
//! | 13 | 0x28a920 / 0x2954d0 | generic 0x295040(120, 8) | 2 |
//! | 15 | 0x28aa98 / 0x279000 | generic 0x278b68(120, 8) | 1 |
//! | 17 | 0x28ac88 / 0x283008 | inline: 240 kind 1 + 16 kind 0 | 1 |
//! | the title world ([`TITLE_LEVEL`]) | boot `update_sky_effects` 0x22ae70 | inline, after `srand(12345)` (`set_global_state_slot(0x3039)`): 246 kind 1 (texture 1) in a band + 10 kind 0 (texture 1) | 2 |
//!
//! **Record** (0x20 bytes, the particle record's first half): byte 0 kind (0 moving, 1/3 twinkle, 2 fixed),
//! byte 1 "drawn" flag (written by `SkySpriteProc`), byte 2 sky texture index (header +0x10 table; < 0 = not
//! drawn), byte 3 ALPHA_1 (0x48 on every star: `Cs·As + Cd`), +4 RGBA drawn, +8 f32 rotation (radians),
//! +0xc per kind (twinkle: base RGBA; moving: two u16 angle counters; fixed: table index, phase group, phase),
//! +0x10 position (x, y, z; game axes, camera-centred), +0x1c f32 size (also the position's w lane).
//!
//! **Generation** (per record index i, `rand` calls in this order; `r16` = `rand() >> 16`):
//! * twinkle (kind 1, or 3 on 02/06): `randi(256)` → +0xc (overwritten below); `r16 & 1` → texture
//!   (+2 on level 02); rotation = `rand_angle`; size = `(randi(48) + 32)·2⁻⁸`; a = `rand_angle`, b =
//!   `rand_angle`; x = cos a·sin b·50, y = sin a·sin b·50; z = cos²b·36 + 16 (generic, 02: a dome from 16 to 52
//!   units up) or cos b·50 (06, 17: the whole sphere); g = `randi(24)` (+8 on 06/17), A = `randi(32)`, then
//!   `r16 & 1` picks the tinted channel: base = `(A << 24) + (g << 16) + 0x30505050` (blue) or
//!   `((A << 24) + (g << 8) + 0x30505050) | g` (red and green; the game ORs g into the red byte, so red =
//!   0x50 | g, not 0x50 + g).
//! * moving (kind 0): +0xc = `r16`, +0xe = `r16` (u16 angle counters); texture 1 (3 on 02); size 0.16 (0.18
//!   on 06/17, where a third `r16 & 3` picks the RGBA from the overlay table gp−0x6700 = 0x160500).
//! * fixed (06 only, i = 234..239, no `rand` except the rotation): k = i − 234, j = k >> 1, p = k & 1;
//!   texture 2, rotation = `rand_angle`, RGBA = overlay 0x1604f0[j], position and size = overlay 0x1bdf30[j]
//!   (vec4), +0xc = j, +0xd = p, +0xe = 24·p (two sprites per star, half a pulse apart).
//!
//! **Update** (every frame, including the generating one, in index order):
//! * kind 0: both counters +1 (u16); a = ((c₁ & 0xfff) − 0x800)·2π/4096, b likewise from c₂;
//!   position = (cos a·sin b·50, sin a·sin b·50, |cos b|·50) (one lap per 4096 frames, upper hemisphere);
//!   RGBA = 0x702020f0 while (c₁ & 0x3f) < 8 else 0x202020f0 (a red light blinking 8 frames in 64); on 06/17
//!   only the alpha changes: 0x70 / 0x24 over the table colour.
//! * twinkle (kind 1 generic/17, kind 3 on 02/06): u = `r16`; RGBA = base − 0x202020 + 4·(u & 0x1f) +
//!   (4·((u >> 4) & 0x1f) << 8) + (4·((u >> 8) & 0x1f) << 16): each channel jitters by −32..+92 every frame,
//!   alpha constant (0x30..0x4f). The field overlap (bits 4..8, 8..12) correlates neighbouring channels. It
//!   is noise, not a periodic twinkle.
//! * kind 2 (06): phase e = (e + 1) mod 48; when e wraps to 0, rotation = (32·p + (r16 & 0xf) − 8)·2π/256;
//!   alpha = 4e, folded to 0xc0 − 4e above 0x60 (a 48-frame triangle 0 → 0x60 → 0); u = `r16`;
//!   RGBA = table[j] + (u & 0xf) + ((u >> 4 & 0xf) << 8) + ((u >> 8 & 0xf) << 16) + (alpha << 24).
//!
//! Float operations follow the game's order on the PS2 FPU/VU0 model ([`crate::ps2v`]); `fast_cos`/`fast_sin`
//! (level00 0x1ff7e8/0x1ff800) are VU0 `vcallms 0x190/0x192` = [`vu0_sin_cos`].

use crate::ps2v::{self, F};
use crate::rng::Rng;
use rc_formats::moby_light::vu0_sin_cos;

/// Bytes per star record.
pub const RECORD: usize = 0x20;
/// Kind (byte 0) of a moving star.
pub const KIND_MOVING: u8 = 0;
/// Kind of a twinkling star (generic dispatch and level 17).
pub const KIND_TWINKLE: u8 = 1;
/// Kind of a level-06 fixed star.
pub const KIND_FIXED: u8 = 2;
/// Kind of a twinkling star on levels 02 and 06.
pub const KIND_TWINKLE_B: u8 = 3;

/// ALPHA_1 byte every generator writes: `Cs·As + Cd` (additive).
pub const ALPHA_ADDITIVE: u8 = 0x48;

const FIFTY: F = 0x4248_0000;
const THIRTY_SIX: F = 0x4210_0000;
const SIXTEEN: F = 0x4180_0000;
/// 2⁻⁸ (0x3b800000).
const INV256: F = 0x3b80_0000;
/// The key of the title world's sky (boot `update_sky_effects`; not a level number).
pub const TITLE_LEVEL: u32 = u32::MAX;
/// The title's `srand(12345)` before it generates (`set_global_state_slot(0x3039)`: newlib's rand state).
pub const TITLE_SEED: u32 = 0x3039;
/// −3.0 (0xc0400000), 0.2, 0.09, 1.2: the title twinklers' band (azimuth −3 ± 0.2·π, polar 1.2 + 0.09·[−π, π)).
const MINUS_THREE: F = 0xc040_0000;
const POINT_TWO: F = 0x3e4c_cccd;
const POINT_09: F = 0x3db8_51ec;
const ONE_POINT_TWO: F = 0x3f99_999a;
/// 0.16 (0x3e23d70a): moving-star size, generic dispatch and 02.
const SIZE_MOVING: F = 0x3e23_d70a;
/// 0.18 (0x3e3851ec): moving-star size on 06 and 17.
const SIZE_MOVING_B: F = 0x3e38_51ec;
/// 2π/4096 (0x3ac90fdb).
const TURN_4096: F = 0x3ac9_0fdb;
/// 2π/256 (0x3cc90fdb).
const TURN_256: F = 0x3cc9_0fdb;
/// Twinkle base colour 0x30505050: RGB 0x50, A 0x30.
const BASE_RGBA: u32 = 0x3050_5050;

/// Level-overlay data the level-06 and level-17 dispatches read (not in the sky block).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OverlayTables {
    /// 06: 0x1bdf30, 3 × (x, y, z, size) as f32 bits.
    pub fixed_pos: [[F; 4]; 3],
    /// 06: gp−0x6710 = 0x1604f0, 3 × RGBA.
    pub fixed_rgba: [u32; 3],
    /// 06/17: gp−0x6700 = 0x160500, 4 × RGBA of the moving stars.
    pub moving_rgba: [u32; 4],
}

impl OverlayTables {
    /// Overlay virtual addresses and byte lengths of the three tables.
    pub const FIXED_POS: (u32, usize) = (0x001b_df30, 48);
    pub const FIXED_RGBA: (u32, usize) = (0x0016_04f0, 12);
    pub const MOVING_RGBA: (u32, usize) = (0x0016_0500, 16);

    /// Builds the tables with `read(vaddr, len)` over the level's overlay memory image.
    pub fn read(mut read: impl FnMut(u32, usize) -> Option<Vec<u8>>) -> Option<OverlayTables> {
        let words = |b: Vec<u8>| b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect::<Vec<u32>>();
        let pos = words(read(Self::FIXED_POS.0, Self::FIXED_POS.1)?);
        let fixed = words(read(Self::FIXED_RGBA.0, Self::FIXED_RGBA.1)?);
        let moving = words(read(Self::MOVING_RGBA.0, Self::MOVING_RGBA.1)?);
        let mut t = OverlayTables::default();
        for j in 0..3 {
            t.fixed_pos[j] = [pos[4 * j], pos[4 * j + 1], pos[4 * j + 2], pos[4 * j + 3]];
            t.fixed_rgba[j] = fixed[j];
        }
        t.moving_rgba.copy_from_slice(&moving[..4]);
        Some(t)
    }
}

/// Which generator/update pair a level's dispatch uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// level00 0x288ec0 / 0x289108 (levels 00, 05, 07, 13, 15).
    Generic,
    /// Level 02 inline (kind 3 twinklers on textures 2/3).
    L02,
    /// Level 06 inline (kind 3 twinklers, kind 2 fixed stars, table-coloured moving stars).
    L06,
    /// Level 17 inline (full-sphere twinklers, table-coloured moving stars).
    L17,
    /// The title world (boot 0x22ae70): twinklers in a band of the sky on texture 1, moving stars on texture 1.
    Title,
}

/// A level's star step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelStars {
    pub variant: Variant,
    /// Record count written to sky header +8.
    pub count: usize,
    /// Twinkling records (indices `0..twinkle`).
    pub twinkle: usize,
    /// Fixed records (06: indices `twinkle..twinkle + fixed`).
    pub fixed: usize,
    /// The star step runs just before this shell (after all lower-index shells).
    pub before_shell: usize,
}

/// The star step of `level`'s sky dispatch, `None` for the eleven levels without stars.
pub fn level_stars(level: u32) -> Option<LevelStars> {
    let s = |variant, twinkle, fixed, moving, before_shell| LevelStars { variant, count: twinkle + fixed + moving, twinkle, fixed, before_shell };
    Some(match level {
        0 => s(Variant::Generic, 0xf4, 0, 0xc, 2),
        2 => s(Variant::L02, 0xf6, 0, 10, 2),
        5 | 7 | 15 => s(Variant::Generic, 0x78, 0, 8, 1),
        13 => s(Variant::Generic, 0x78, 0, 8, 2),
        6 => s(Variant::L06, 0xea, 6, 16, 1),
        17 => s(Variant::L17, 0xf0, 0, 16, 1),
        TITLE_LEVEL => s(Variant::Title, 0xf6, 0, 10, 2),
        _ => return None,
    })
}

/// One star record, raw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Star(pub [u8; RECORD]);

impl Default for Star {
    fn default() -> Self { Star([0; RECORD]) }
}

impl Star {
    pub fn kind(&self) -> u8 { self.0[0] }
    /// Byte 2: the sky texture index (signed; negative = not drawn).
    pub fn texture(&self) -> i8 { self.0[2] as i8 }
    /// Byte 3: ALPHA_1.
    pub fn alpha_reg(&self) -> u8 { self.0[3] }
    fn u16(&self, o: usize) -> u16 { u16::from_le_bytes([self.0[o], self.0[o + 1]]) }
    fn u32(&self, o: usize) -> u32 { u32::from_le_bytes(self.0[o..o + 4].try_into().unwrap()) }
    fn set_u16(&mut self, o: usize, v: u16) { self.0[o..o + 2].copy_from_slice(&v.to_le_bytes()) }
    fn set_u32(&mut self, o: usize, v: u32) { self.0[o..o + 4].copy_from_slice(&v.to_le_bytes()) }
    /// +4: the RGBA drawn (R low byte, 0x80 = 1.0).
    pub fn rgba(&self) -> u32 { self.u32(4) }
    /// +8: rotation in radians (f32 bits).
    pub fn rotation(&self) -> F { self.u32(8) }
    /// +0xc: the twinkle base colour.
    pub fn base_rgba(&self) -> u32 { self.u32(0xc) }
    /// +0x10: position (f32 bits, game axes, camera-centred).
    pub fn position(&self) -> [F; 3] { [self.u32(0x10), self.u32(0x14), self.u32(0x18)] }
    /// +0x1c: size (f32 bits).
    pub fn size(&self) -> F { self.u32(0x1c) }
    pub fn position_f32(&self) -> [f32; 3] { self.position().map(ps2v::to_f32) }
    pub fn size_f32(&self) -> f32 { ps2v::to_f32(self.size()) }
    pub fn rotation_f32(&self) -> f32 { ps2v::to_f32(self.rotation()) }
    fn set_position(&mut self, p: [F; 3]) {
        for (k, v) in p.into_iter().enumerate() { self.set_u32(0x10 + 4 * k, v); }
    }
}

/// `r16`: `rand() >> 16` (0..0x7fff).
fn r16(rng: &mut Rng) -> u32 { (rng.rand() >> 16) as u32 }

fn cos(a: F) -> F { vu0_sin_cos(a).1 }
fn sin(a: F) -> F { vu0_sin_cos(a).0 }

/// The twinkle generator body shared by every variant (from `randi(256)` to the base colour).
fn gen_twinkle(r: &mut Star, rng: &mut Rng, kind: u8, tex_add: u8, sphere: bool, g_add: i32) {
    r.0[0] = kind;
    let v = rng.randi(0x100);
    r.set_u16(0xc, v as u16);
    r.0[2] = (r16(rng) & 1) as u8 + tex_add;
    r.0[3] = ALPHA_ADDITIVE;
    let rot = rng.rand_angle_bits();
    r.set_u32(8, rot);
    let s = rng.randi(0x30);
    r.set_u32(0x1c, ps2v::mul(ps2v::itof0(s + 0x20), INV256));
    let a = rng.rand_angle_bits();
    let b = rng.rand_angle_bits();
    let x = ps2v::mul(ps2v::mul(cos(a), sin(b)), FIFTY);
    let y = ps2v::mul(ps2v::mul(sin(a), sin(b)), FIFTY);
    let z = if sphere {
        ps2v::mul(cos(b), FIFTY)
    } else {
        ps2v::add(ps2v::mul(ps2v::mul(cos(b), cos(b)), THIRTY_SIX), SIXTEEN)
    };
    r.set_position([x, y, z]);
    let g = (rng.randi(0x18) + g_add) as u32;
    let alpha = (rng.randi(0x20) as u32) << 24;
    let base = if r16(rng) & 1 != 0 {
        alpha.wrapping_add((g << 16).wrapping_add(BASE_RGBA))
    } else {
        alpha.wrapping_add((g << 8).wrapping_add(BASE_RGBA)) | g
    };
    r.set_u32(0xc, base);
}

/// The title's twinkle generator (boot 0x22ae70): kind 1, `randi(256)` → +0xc, texture 1, ALPHA 0x48, rotation =
/// `rand_angle`, size = (`randi(24)` + 32)·2⁻⁸, a = `fast_add_rotations(−3, rand_angle·0.2)`, b = `rand_angle·0.09 + 1.2`;
/// x = cos a·sin b·50, y = sin a·sin b·50, z = cos b·50; g = `randi(24)`, A = `randi(32)`, `r16 & 1` picks the tint as
/// [`gen_twinkle`] does.
fn gen_title_twinkle(r: &mut Star, rng: &mut Rng) {
    r.0[0] = KIND_TWINKLE;
    let v = rng.randi(0x100);
    r.0[2] = 1;
    r.0[3] = ALPHA_ADDITIVE;
    r.set_u16(0xc, v as u16);
    let rot = rng.rand_angle_bits();
    r.set_u32(8, rot);
    let s = rng.randi(0x18);
    r.set_u32(0x1c, ps2v::mul(ps2v::itof0(s + 0x20), INV256));
    let ra = rng.rand_angle_bits();
    let a = crate::moby_update::creature::add_rot(ps2v::to_f32(MINUS_THREE), ps2v::to_f32(ps2v::mul(ra, POINT_TWO))).to_bits();
    let rb = rng.rand_angle_bits();
    let b = ps2v::add(ps2v::mul(rb, POINT_09), ONE_POINT_TWO);
    let x = ps2v::mul(ps2v::mul(cos(a), sin(b)), FIFTY);
    let y = ps2v::mul(ps2v::mul(sin(a), sin(b)), FIFTY);
    let z = ps2v::mul(cos(b), FIFTY);
    r.set_position([x, y, z]);
    let g = rng.randi(0x18) as u32;
    let alpha = (rng.randi(0x20) as u32) << 24;
    let base = if r16(rng) & 1 != 0 {
        alpha.wrapping_add((g << 16).wrapping_add(BASE_RGBA))
    } else {
        alpha.wrapping_add((g << 8).wrapping_add(BASE_RGBA)) | g
    };
    r.set_u32(0xc, base);
}

/// Moving-star generator body: the two counters, then texture/size (and on 06/17 the table colour).
fn gen_moving(r: &mut Star, rng: &mut Rng, tex: u8, tables: Option<&OverlayTables>) {
    r.0[0] = KIND_MOVING;
    let c = r16(rng);
    r.set_u16(0xc, c as u16);
    let e = r16(rng);
    r.set_u16(0xe, e as u16);
    r.0[2] = tex;
    r.0[3] = ALPHA_ADDITIVE;
    match tables {
        None => r.set_u32(0x1c, SIZE_MOVING),
        Some(t) => {
            r.set_u32(0x1c, SIZE_MOVING_B);
            // `sra v0, rand, 14; andi 0xc` = byte offset of entry (rand >> 16) & 3.
            let i = ((rng.rand() >> 14) & 0xc) as usize / 4;
            r.set_u32(4, t.moving_rgba[i]);
        }
    }
}

/// Generates `level`'s records exactly as its dispatch does on the first frame (`initial` = the sky block's
/// sprite scratch; the game keeps every byte a generator does not write). The first update is *not* applied.
pub fn generate_over(level: u32, rng: &mut Rng, tables: &OverlayTables, initial: &[Star]) -> Vec<Star> {
    let Some(ls) = level_stars(level) else { return Vec::new() };
    let mut out: Vec<Star> = (0..ls.count).map(|i| initial.get(i).copied().unwrap_or_default()).collect();
    for (i, r) in out.iter_mut().enumerate() {
        match ls.variant {
            Variant::Generic => {
                if i < ls.twinkle { gen_twinkle(r, rng, KIND_TWINKLE, 0, false, 0) } else { gen_moving(r, rng, 1, None) }
            }
            Variant::L02 => {
                if i < ls.twinkle { gen_twinkle(r, rng, KIND_TWINKLE_B, 2, false, 0) } else { gen_moving(r, rng, 3, None) }
            }
            Variant::L17 => {
                if i < ls.twinkle { gen_twinkle(r, rng, KIND_TWINKLE, 0, true, 8) } else { gen_moving(r, rng, 1, Some(tables)) }
            }
            Variant::Title => {
                if i < ls.twinkle { gen_title_twinkle(r, rng) } else { gen_moving(r, rng, 1, None) }
            }
            Variant::L06 => {
                if i >= ls.twinkle + ls.fixed {
                    gen_moving(r, rng, 1, Some(tables));
                } else if i >= ls.twinkle {
                    let k = i - ls.twinkle;
                    let (j, p) = (k >> 1, (k & 1) as u8);
                    r.0[2] = 2;
                    r.0[3] = ALPHA_ADDITIVE;
                    let rot = rng.rand_angle_bits();
                    r.set_u32(8, rot);
                    r.0[0] = KIND_FIXED;
                    r.set_u32(4, tables.fixed_rgba[j]);
                    let [x, y, z, w] = tables.fixed_pos[j];
                    r.set_position([x, y, z]);
                    r.set_u32(0x1c, w);
                    r.0[0xc] = j as u8;
                    r.0[0xd] = p;
                    r.0[0xe] = p * 24;
                } else {
                    gen_twinkle(r, rng, KIND_TWINKLE_B, 0, true, 8);
                }
            }
        }
    }
    out
}

/// [`generate_over`] from zeroed records (the sky block's sprite scratch is zero on the disc).
pub fn generate(level: u32, rng: &mut Rng, tables: &OverlayTables) -> Vec<Star> { generate_over(level, rng, tables, &[]) }

/// Kind-0 position update from the two counters.
fn move_star(r: &mut Star) {
    let c = r.u16(0xc).wrapping_add(1);
    r.set_u16(0xc, c);
    let e = r.u16(0xe).wrapping_add(1);
    r.set_u16(0xe, e);
    let a = ps2v::mul(ps2v::itof0((c & 0xfff) as i32 - 0x800), TURN_4096);
    let b = ps2v::mul(ps2v::itof0((e & 0xfff) as i32 - 0x800), TURN_4096);
    let x = ps2v::mul(ps2v::mul(cos(a), sin(b)), FIFTY);
    let y = ps2v::mul(ps2v::mul(sin(a), sin(b)), FIFTY);
    let z = ps2v::mul(cos(b) & !ps2v::SIGN, FIFTY);
    r.set_position([x, y, z]);
}

/// Twinkle colour: base − 0x202020 plus three overlapping 5-bit fields of `r16`, ×4.
fn twinkle(r: &mut Star, rng: &mut Rng) {
    let u = r16(rng);
    let v = r
        .base_rgba()
        .wrapping_add(((u & 0x1f00) << 10).wrapping_add(0xffdf_dfe0))
        .wrapping_add((u & 0x1f0) << 6)
        .wrapping_add((u & 0x1f) << 2);
    r.set_u32(4, v);
}

/// One frame of `level`'s star update over all records, in index order.
pub fn update(level: u32, stars: &mut [Star], rng: &mut Rng, tables: &OverlayTables) {
    let Some(ls) = level_stars(level) else { return };
    for r in stars.iter_mut() {
        match (ls.variant, r.kind()) {
            (_, KIND_MOVING) => {
                move_star(r);
                let blink = r.u16(0xc) & 0x3f < 8;
                let v = match ls.variant {
                    Variant::Generic | Variant::L02 | Variant::Title => if blink { 0x7020_20f0 } else { 0x2020_20f0 },
                    Variant::L06 | Variant::L17 => (r.rgba() & 0x00ff_ffff) | if blink { 0x7000_0000 } else { 0x2400_0000 },
                };
                r.set_u32(4, v);
            }
            (Variant::Generic, KIND_TWINKLE) | (Variant::L02 | Variant::L06, KIND_TWINKLE_B) => twinkle(r, rng),
            // 0x28ac88 / the title's 0x22ae70: every non-zero kind twinkles (only kind 1 exists there).
            (Variant::L17 | Variant::Title, _) => twinkle(r, rng),
            (Variant::L06, KIND_FIXED) => {
                let e = (r.0[0xe] as u32 + 1) % 0x30;
                r.0[0xe] = e as u8;
                if e == 0 {
                    let u = r16(rng);
                    let k = r.0[0xd] as i32 * 32 + (u & 0xf) as i32 - 8;
                    r.set_u32(8, ps2v::mul(ps2v::itof0(k), TURN_256));
                }
                let mut alpha = e * 4;
                if alpha >= 0x61 { alpha = 0xc0 - alpha; }
                let u = r16(rng);
                let v = tables.fixed_rgba[r.0[0xc] as usize]
                    .wrapping_add((u & 0xf00) << 8)
                    .wrapping_add((u & 0xf0) << 4)
                    .wrapping_add(u & 0xf)
                    .wrapping_add(alpha << 24);
                r.set_u32(4, v);
            }
            _ => {}
        }
    }
}

/// `ticks` frames of [`update`].
pub fn tick(level: u32, stars: &mut [Star], ticks: u32, rng: &mut Rng, tables: &OverlayTables) {
    for _ in 0..ticks { update(level, stars, rng, tables); }
}

/// A level's stars as the dispatch keeps them: generated on the first frame (header +8 == 0), then updated.
#[derive(Clone, Debug, Default)]
pub struct SkyStars {
    pub level: u32,
    pub tables: OverlayTables,
    /// The sky block's sprite scratch as loaded (the bytes generators leave alone).
    pub initial: Vec<Star>,
    /// The live records; empty until the first frame.
    pub stars: Vec<Star>,
}

impl SkyStars {
    pub fn new(level: u32, tables: OverlayTables, initial: Vec<Star>) -> Self { SkyStars { level, tables, initial, stars: Vec::new() } }

    /// One frame of the star step: generate if this is the first frame, then update.
    pub fn frame(&mut self, rng: &mut Rng) {
        if level_stars(self.level).is_none() { return; }
        if self.stars.is_empty() {
            // The title's generator reseeds the one rand stream first (`set_global_state_slot(0x3039)`).
            if self.level == TITLE_LEVEL { rng.srand(TITLE_SEED); }
            self.stars = generate_over(self.level, rng, &self.tables, &self.initial);
        }
        update(self.level, &mut self.stars, rng, &self.tables);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::LEVEL_SEED;

    fn seeded() -> Rng {
        let mut r = Rng::new();
        r.srand(LEVEL_SEED);
        r
    }

    fn tables() -> OverlayTables {
        let f = |x: f32| x.to_bits();
        OverlayTables {
            fixed_pos: [[f(12.0), f(27.0), f(8.0), f(0.35)], [f(-20.0), f(-15.0), f(10.0), f(0.35)], [f(15.0), f(-25.0), f(1.0), f(0.35)]],
            fixed_rgba: [0x0038_6070; 3],
            moving_rgba: [0x8020_20f0, 0x8020_f0f0, 0x80f0_2080, 0x8020_f020],
        }
    }

    fn kinds(s: &[Star]) -> [usize; 4] {
        let mut k = [0; 4];
        for r in s { k[r.kind() as usize] += 1; }
        k
    }

    /// Counts per level (kinds 0..3) and the levels without stars.
    #[test]
    fn counts_per_level() {
        let want: [(u32, [usize; 4]); 8] = [
            (0, [12, 244, 0, 0]),
            (2, [10, 0, 0, 246]),
            (5, [8, 120, 0, 0]),
            (6, [16, 0, 6, 234]),
            (7, [8, 120, 0, 0]),
            (13, [8, 120, 0, 0]),
            (15, [8, 120, 0, 0]),
            (17, [16, 240, 0, 0]),
        ];
        for (level, k) in want {
            let s = generate(level, &mut seeded(), &tables());
            assert_eq!(kinds(&s), k, "level {level}");
            assert!(s.iter().all(|r| r.alpha_reg() == ALPHA_ADDITIVE && r.texture() >= 0));
        }
        for level in [1, 3, 4, 8, 9, 10, 11, 12, 14, 16, 18] {
            assert!(level_stars(level).is_none());
            assert!(generate(level, &mut seeded(), &tables()).is_empty());
        }
    }

    /// `rand` draws per record: twinkle 9, moving 2 (3 on 06/17), fixed 1.
    #[test]
    fn stream_use_matches_the_generators() {
        let draws = |level: u32| {
            let mut a = seeded();
            generate(level, &mut a, &tables());
            let mut b = seeded();
            (0..).find(|_| { b.rand(); b == a }).unwrap() + 1
        };
        assert_eq!(draws(0), 244 * 9 + 12 * 2);
        assert_eq!(draws(5), 120 * 9 + 8 * 2);
        assert_eq!(draws(2), 246 * 9 + 10 * 2);
        assert_eq!(draws(6), 234 * 9 + 6 + 16 * 3);
        assert_eq!(draws(17), 240 * 9 + 16 * 3);
    }

    /// Same seed → identical records and stream; a different seed → different stars.
    #[test]
    fn deterministic_given_a_seed() {
        for level in [0, 2, 6, 17] {
            let (mut a, mut b) = (seeded(), seeded());
            let (mut sa, mut sb) = (generate(level, &mut a, &tables()), generate(level, &mut b, &tables()));
            tick(level, &mut sa, 100, &mut a, &tables());
            tick(level, &mut sb, 100, &mut b, &tables());
            assert_eq!(sa, sb);
            assert_eq!(a, b);
            let mut c = Rng::new();
            c.srand(12345);
            assert_ne!(generate(level, &mut c, &tables()), generate(level, &mut seeded(), &tables()));
        }
    }

    /// Record contents: the generic twinkle geometry (dome 16..52 up at radius ≤ 50), sizes, colours.
    #[test]
    fn generic_records() {
        let s = generate(0, &mut seeded(), &tables());
        for r in &s[..244] {
            let [x, y, z] = r.position_f32();
            assert!((16.0..=52.01).contains(&z), "{z}");
            assert!((x * x + y * y).sqrt() <= 50.01);
            let size = r.size_f32() * 256.0;
            assert!(size.fract() == 0.0 && (32.0..80.0).contains(&size));
            assert!(r.texture() == 0 || r.texture() == 1);
            let [cr, cg, cb, ca] = r.base_rgba().to_le_bytes();
            assert!((0x30..0x50).contains(&ca));
            // One channel tinted (red+green share g), the other(s) at 0x50.
            assert!((cb == 0x50 && cr == 0x50 | (cg - 0x50)) || (cr == 0x50 && cg == 0x50), "{cr:x} {cg:x} {cb:x}");
        }
        for r in &s[244..] {
            assert_eq!((r.texture(), r.size()), (1, SIZE_MOVING));
        }
    }

    /// Kind 1 twinkle: the alpha is constant, RGB = base − 0x20 + 4·field, fresh every frame (noise, no period).
    #[test]
    fn twinkle_is_per_frame_noise() {
        let mut rng = seeded();
        let mut s = generate(0, &mut rng, &tables());
        let mut seen = Vec::new();
        for _ in 0..64 {
            update(0, &mut s, &mut rng, &tables());
            let r = s[0];
            let (b, c) = (r.base_rgba().to_le_bytes(), r.rgba().to_le_bytes());
            assert_eq!(b[3], c[3]);
            for k in 0..3 {
                let d = c[k] as i32 - b[k] as i32;
                assert!((-32..=92).contains(&d) && d % 4 == 0, "{d}");
            }
            seen.push(r.rgba());
        }
        seen.sort();
        seen.dedup();
        assert!(seen.len() > 32, "only {} distinct colours in 64 frames", seen.len());
    }

    /// Kind 2 (06): alpha is a 48-frame triangle 0 → 0x60 → 0, the pair half a period apart.
    #[test]
    fn fixed_star_pulse_period() {
        let t = tables();
        let mut rng = seeded();
        let mut s = generate(6, &mut rng, &t);
        assert_eq!((s[234].0[0xe], s[235].0[0xe]), (0, 24));
        let mut alphas = Vec::new();
        for _ in 0..96 {
            update(6, &mut s, &mut rng, &t);
            alphas.push((s[234].rgba() >> 24, s[235].rgba() >> 24));
        }
        assert_eq!(alphas[0].0, 4);
        assert_eq!(alphas[23].0, 0x60);
        assert_eq!(alphas[24].0, 0xc0 - 25 * 4);
        assert_eq!((alphas[46].0, alphas[47].0), (4, 0));
        for i in 0..48 { assert_eq!(alphas[i], alphas[i + 48]); }
        for i in 0..48 { assert_eq!(alphas[i].0, alphas[(i + 24) % 48].1); }
        assert!(alphas.iter().all(|a| a.0 <= 0x60));
        assert_eq!(s[234].position_f32(), [12.0, 27.0, 8.0]);
        assert_eq!(s[234].size_f32(), 0.35);
    }

    /// Kind 0: one lap in 4096 frames, blink 8 of 64 frames, upper hemisphere.
    #[test]
    fn moving_star_lap_and_blink() {
        let t = tables();
        let mut rng = seeded();
        let mut s = generate(0, &mut rng, &t);
        let i = 250;
        update(0, &mut s, &mut rng, &t);
        let p0 = s[i].position();
        let mut lit = 0;
        for f in 1..4096 {
            update(0, &mut s, &mut rng, &t);
            let [x, y, z] = s[i].position_f32();
            assert!(z >= 0.0 && ((x * x + y * y + z * z).sqrt() - 50.0).abs() < 0.01);
            if f <= 64 && s[i].rgba() >> 24 == 0x70 { lit += 1; }
        }
        assert_eq!(lit, 8);
        update(0, &mut s, &mut rng, &t);
        assert_eq!(s[i].position(), p0);
    }
}
