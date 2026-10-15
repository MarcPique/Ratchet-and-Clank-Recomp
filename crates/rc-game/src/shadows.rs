//! The moby shadows' game side (docs/plan/shadows.md §3.2): the shadow directions (table 0x1af000), Ratchet's
//! shadow gate / pitch / four probes, and the per-class ground slab (moby +0x84 / +0x88) rules the class updates
//! call. The geometry that turns a caster into a shadow volume is [`volume`]. Native `f32` with `std` trig for
//! the game's `fast_sin` / `fast_cos` / `FastArcTan` (no PS2 effect is visible in a shadow direction).
//!
//! | Game function | Here | Called from (game) | Called from (port) |
//! |---|---|---|---|
//! | *UpdateShadowDir* 0x264988 | [`update_shadow_dir`] | end of `GameStateUpdate`, `InLevelFrameUpdate`, `CutsceneModeUpdate` | the engine after each tick (crate `rc-engine` `shadow_render`) |
//! | `fun_0020d510` 0x265130 | [`light_dir`] | the two direction updates | same |
//! | *UpdateHeroShadow* 0x22a260 | [`update_hero_shadow`] | `HeroUpdateAlt` 0x228000, 0x228870 | the engine after each tick |
//! | *ShadowSetGround* 0x26eff8 | [`set_ground`] | `PathEnemyUpdate` | `moby_update::classes::path_enemy` |
//! | *ShadowProbeDown* 0x26f020 | [`probe_down`] | `AmoeboidUpdate`, `GroundCritterUpdate`, `TalkingNpcUpdate` | the three class updates |
//! | `0x26eec8` (scale, moby) (L04 `0x24d018`, L18 `0x25c328`, L09 `0x279ae0`, L13 `0x266070`: one source) | [`blob`] | the blob shadows of small movers (the Summoner's mouse, the Drone Device's drones, the Visibomb, the pods, the ship pickups, Gemlik's ship, the ring shell) | the same classes |
//! | `fun_001f4880` 0x21b260 (`RunDrawCallbacks_2`) | [`blob_quad`] | the blob list's draw | `classes::units` draw row, `rc-engine` fx_draw |
//! | *ShadowProbeAlongDir* 0x26f0e0 | [`probe_along_dir`] | the scene actors (0x2a4080, `CutsceneModeUpdate`, `VendorModeUpdate`) | the engine every scene frame, on the drawn pose (crate `rc-engine` `shadow_render`) |
//!
//! The slab `[lo, hi]` (+0x84, +0x88) is the height band the volume is clipped to; `hi ≤ 0` = no shadow.
//! `CollLine_Fix` with flags 0x22 (skip moby primitives, exclude surface 0 = water) is the probe of every rule.

pub mod volume;

use crate::moby_runtime::{mode, Moby, MobyId};
use crate::moby_update::services::World;

/// Index of the direction every moby but Ratchet uses (+0xbd = 0).
pub const DIR_MOBYS: usize = 0;
/// Ratchet's direction (+0xbd = 1, set by [`update_hero_shadow`]).
pub const DIR_HERO: usize = 1;
/// `CollLine_Fix` flags of every slab probe: 0x2 (no moby primitives) | 0x20 (exclude surface 0, water).
pub const PROBE_FLAGS: u32 = 0x22;

/// The direction table 0x1af000 (4 × vec4; +0xbd selects the entry; entries 2 and 3 are never written).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShadowDirs(pub [[f32; 4]; 4]);

impl ShadowDirs {
    /// The direction of a moby (+0xbd; an index past the table reads entry 0).
    pub fn of(&self, bbd: u8) -> [f32; 3] {
        let d = self.0.get(bbd as usize).copied().unwrap_or(self.0[0]);
        [d[0], d[1], d[2]]
    }
}

/// `fun_0020d510(moby, out)` 0x265130: the moby's first directional light, light A of the light set(s) of its
/// light word (+0x38: byte 0 set A, byte 1 set B, byte 2 cross-fade f): `dir_a[A]·(1 − f/256) + dir_a[B]·f/256`,
/// or `dir_a[A]` when f = 0. `dir_a` = each set's light A direction (0x180340 + 0x40·set + 0x10); a set past the
/// table reads (0, 0, 0).
pub fn light_dir(dir_a: &[[f32; 4]], light_word: u32) -> [f32; 3] {
    let get = |k: u32| dir_a.get(k as usize).map_or([0.0; 3], |d| [d[0], d[1], d[2]]);
    let (a, b, f) = (light_word & 0xff, (light_word >> 8) & 0xff, (light_word >> 16) & 0xff);
    let da = get(a);
    if f == 0 { return da; }
    let t = f as f32 / 256.0;
    let db = get(b);
    std::array::from_fn(|k| da[k] * (1.0 - t) + db[k] * t)
}

/// *UpdateShadowDir* 0x264988: direction 0 = `(0.14·cos φ, 0.14·sin φ, −0.99)` with φ = the horizontal angle of
/// Ratchet's key light ([`light_dir`] of the hero moby): almost straight down, leaning ~8° along the light.
pub fn update_shadow_dir(dirs: &mut ShadowDirs, hero_light: [f32; 3]) {
    let phi = hero_light[1].atan2(hero_light[0]);
    let d = &mut dirs.0[DIR_MOBYS];
    d[0] = phi.cos() * 0.14;
    d[1] = phi.sin() * 0.14;
    d[2] = -0.99;
}

/// The hero-block fields [`update_hero_shadow`] reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeroShadowIn {
    /// 0x1413dc movement group, 0x1413d4 state.
    pub group: i32,
    pub state: i32,
    /// 0x13f65e: air ticks.
    pub air_ticks: i16,
    /// 0x13f628 ground z, 0x13f640 water level, 0x13f644 liquid level.
    pub ground_z: f32,
    pub water_z: f32,
    pub liquid_z: f32,
}

/// Ratchet's probe offsets 0x17c780 (x, y along cos φ / sin φ, z; × his bounding radius). The fourth repeats the
/// third (a data quirk the game keeps: three distinct probes, one of them run twice).
pub const HERO_PROBES: [[f32; 3]; 4] = [[0.7, -0.5, 0.4], [0.7, 0.5, 0.4], [-0.7, -0.5, 0.4], [-0.7, -0.5, 0.4]];
/// The pitch targets (0.97 rad on the ground, 1.5 in the air / group 4) and the step per tick.
pub const HERO_PITCH_GROUND: f32 = 0.97;
pub const HERO_PITCH_AIR: f32 = 1.5;
pub const HERO_PITCH_STEP: f32 = 0.052;

/// Wraps an angle once into [−π, π) (`fast_add_rotations` / `fast_subtract_rotations`).
fn wrap(a: f32) -> f32 {
    use std::f32::consts::PI;
    if a >= PI { a - 2.0 * PI } else if a < -PI { a + 2.0 * PI } else { a }
}

/// *UpdateHeroShadow* 0x22a260, on Ratchet's moby `m` (docs/plan/shadows.md §3.2):
/// * no shadow (mode &= !0x400) in groups 0x12 (water surface), 0x15, 0x16 (Hoverboard) and state 0x7d;
/// * else mode |= 0x400, +0xbd = 1, and the pitch `p` (0x140070) moves by at most 0.052 rad towards 0.97 (1.5 while
///   air ticks ≠ 0 or in group 4); direction 1 = `(cos φ·cos p, sin φ·cos p, −sin p)`;
/// * the slab: seed = the first of ground z, water level, liquid level that is ≥ 1 (the liquid level: > 1); none →
///   `lo = hi = 0`. Four probes (`line`) from `c + (cos φ·x, sin φ·y, z)·r` ([`HERO_PROBES`], c / r = his bounding
///   sphere in world units) along direction 1 down to height `seed − 0.5`; `lo = min(seed, hits) − 0.12`,
///   `hi = min(max(seed, hits) + 0.24, lo + 3)`.
pub fn update_hero_shadow(
    m: &mut Moby,
    pitch: &mut f32,
    dirs: &mut ShadowDirs,
    h: &HeroShadowIn,
    hero_light: [f32; 3],
    mut line: impl FnMut([f32; 3], [f32; 3]) -> Option<f32>,
) {
    if matches!(h.group, 0x12 | 0x15 | 0x16) || h.state == 0x7d {
        m.mode &= !mode::CLASS_F;
        return;
    }
    m.mode |= mode::CLASS_F;
    m.bbd = DIR_HERO as u8;
    let target = if h.air_ticks != 0 || h.group == 4 { HERO_PITCH_AIR } else { HERO_PITCH_GROUND };
    let step = wrap(target - *pitch).clamp(-HERO_PITCH_STEP, HERO_PITCH_STEP);
    *pitch = wrap(*pitch + step);
    let phi = hero_light[1].atan2(hero_light[0]);
    let (c_phi, s_phi) = (phi.cos(), phi.sin());
    let dir = [c_phi * pitch.cos(), s_phi * pitch.cos(), -pitch.sin()];
    dirs.0[DIR_HERO] = [dir[0], dir[1], dir[2], 0.0];
    let seed = if h.ground_z >= 1.0 {
        h.ground_z
    } else if h.water_z >= 1.0 {
        h.water_z
    } else if h.liquid_z > 1.0 {
        h.liquid_z
    } else {
        m.shadow_lo = 0.0;
        m.shadow_hi = 0.0;
        return;
    };
    let (c, r) = ([m.bsphere[0] / 1024.0, m.bsphere[1] / 1024.0, m.bsphere[2] / 1024.0], m.bsphere[3] / 1024.0);
    let (mut lo, mut hi) = (seed, seed);
    for o in HERO_PROBES {
        let s = [c[0] + c_phi * o[0] * r, c[1] + s_phi * o[1] * r, c[2] + o[2] * r];
        // Along the (unit) direction to height seed − 0.5.
        let len = ((s[2] - seed) + 0.5) / -dir[2];
        let e = [s[0] + dir[0] * len, s[1] + dir[1] * len, s[2] + dir[2] * len];
        if let Some(z) = line(s, e) {
            lo = lo.min(z);
            hi = hi.max(z);
        }
    }
    m.shadow_lo = lo - 0.12;
    m.shadow_hi = (hi + 0.24).min(m.shadow_lo + 3.0);
}

/// *ShadowSetGround* 0x26eff8: the slab `[z − 0.2, z + 0.2]` about a ground height the class found itself.
pub fn set_ground(m: &mut Moby, z: f32) {
    m.shadow_hi = z + 0.2;
    m.shadow_lo = z - 0.2;
}

/// One blob shadow of the list 0x16e500 (32 × 0x20; the count 0x15f434 cleared by `ResetDrawGlobals` each frame):
/// +0x00 the point on the ground (z + 0.025), +0x0c its size, +0x10 the ground's normal (raw: the hit's +0x40).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blob {
    pub point: [f32; 3],
    pub size: f32,
    pub normal: [f32; 3],
}

/// The list's capacity.
pub const BLOBS: usize = 32;

/// `0x26eec8(scale, m)`: drawn (+0x31) and room in the list → `GroundHeight(0.5, position, 0)` (a line from 0.5 above
/// down to 0.01); a hit: the point at the hit's z + 0.025, the normal the hit's, size = scale · max(0.25, (8 −
/// \|z − ground\|) / 8). The port keeps the list in `Services::blobs` (stamped with the tick: a new tick starts it
/// empty) and registers the draw for the moby ([`blob_quad`]).
pub fn blob(w: &mut World, scale: f32, id: MobyId) {
    if w.m(id).visible == 0 { return; }
    if w.svc.blobs.0 != w.counter { w.svc.blobs = (w.counter, Vec::new()); }
    if BLOBS <= w.svc.blobs.1.len() { return; }
    let p = w.m(id).position;
    let a = crate::moby_update::services::pv([p[0], p[1], p[2] + 0.5, p[3]]);
    let b = crate::moby_update::services::pv([p[0], p[1], f32::from_bits(0x3c23_d70a), p[3]]);
    let Some(h) = w.coll_line(a, b, 2, None) else { return };
    let g = h.point[2];
    let f = ((8.0 - (p[2] - g).abs()) * 0.125).max(0.25);
    w.svc.blobs.1.push((id, Blob { point: [p[0], p[1], g + 0.025], size: scale * f, normal: h.normal }));
    if let Some(r) = crate::moby_update::classes::units::row(1, BLOB_FN) {
        w.svc.draw_callbacks.register(crate::moby_update::classes::draw_callbacks::Callback::UnitQuads(r), id);
    }
}

/// The blob draw's address (level01 `0x21b260`).
pub const BLOB_FN: u32 = 0x21_b260;

/// `fun_001f4880`'s quad for one blob: the corners (±1, ±1) of 0x16cc60 laid on the ground plane (`c − n·(c·n)` with
/// n the unit normal) at the blob's size; ST (0, 1) (0, 0) (1, 1) (1, 0); colour 0x40808080; FX 0 (`GetEffectTex(0)`),
/// ALPHA 0x44 (blended).
pub fn blob_quad(b: &Blob) -> ([[f32; 3]; 4], [[f32; 2]; 4]) {
    let l = (b.normal[0] * b.normal[0] + b.normal[1] * b.normal[1] + b.normal[2] * b.normal[2]).sqrt();
    let n = if l == 0.0 { [0.0; 3] } else { b.normal.map(|x| x / l) };
    let corner = |x: f32, y: f32| {
        let d = x * n[0] + y * n[1];
        [b.point[0] + (x - n[0] * d) * b.size, b.point[1] + (y - n[1] * d) * b.size, b.point[2] - n[2] * d * b.size]
    };
    ([corner(1.0, -1.0), corner(-1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)], [[0.0, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]])
}

/// *ShadowProbeDown* 0x26f020: a vertical probe from `pos.z + 0.5` down to `max(pos.z − 16, 0.5)`; a hit gives
/// `[hit − 0.2, hit + 0.2]`, a miss no shadow (`[0, 0]`). Returns `(lo, hi)`.
pub fn probe_down_slab(p: [f32; 4], line: impl FnOnce([f32; 3], [f32; 3]) -> Option<f32>) -> (f32, f32) {
    let end = (p[2] - 16.0).max(0.5);
    match line([p[0], p[1], p[2] + 0.5], [p[0], p[1], end]) {
        Some(z) => (z - 0.2, z + 0.2),
        None => (0.0, 0.0),
    }
}

/// [`probe_down_slab`] on a moby.
pub fn probe_down_with(m: &mut Moby, line: impl FnOnce([f32; 3], [f32; 3]) -> Option<f32>) {
    (m.shadow_lo, m.shadow_hi) = probe_down_slab(m.position, line);
}

/// [`probe_down_slab`] on moby `id` inside the moby loop (`CollLine_Fix` against the world and the mobys, flags
/// [`PROBE_FLAGS`], no moby ignored).
pub fn probe_down(w: &mut World, id: MobyId) {
    let v = |p: [f32; 3]| crate::moby_update::services::pv([p[0], p[1], p[2], 0.0]);
    let slab = probe_down_slab(w.m(id).position, |a, b| w.coll_line(v(a), v(b), PROBE_FLAGS, None).map(|o| o.point[2]));
    let m = w.mm(id);
    (m.shadow_lo, m.shadow_hi) = slab;
}

/// *ShadowProbeAlongDir* 0x26f0e0 (the scene actors): with `d` = direction 0 scaled to a horizontal length of 1
/// (0x221460) and c / r = the bounding sphere in world units:
/// 1. a vertical probe 8 units down from `c − d.xy·0.75r` (the side the shadow leans from); a miss: no shadow;
/// 2. from `c + (d.xy·0.75r, r/2)`, along `d` rescaled to a 3-D length of `(start.z − hit₁)/(−d.z)`, i.e. only
///    about 14 % of the way down to hit₁ (the length mixes the horizontal and the 3-D normalisation: a game quirk).
///
/// `lo = min(hits) − 0.25`, `hi = min(max(hits) + 0.25, lo + 4)`.
pub fn probe_along_dir(m: &mut Moby, dir0: [f32; 3], mut line: impl FnMut([f32; 3], [f32; 3]) -> Option<f32>) {
    let hl = (dir0[0] * dir0[0] + dir0[1] * dir0[1]).sqrt();
    let d = if hl > 0.0 { dir0.map(|x| x / hl) } else { [0.0; 3] };
    let (c, r) = ([m.bsphere[0] / 1024.0, m.bsphere[1] / 1024.0, m.bsphere[2] / 1024.0], m.bsphere[3] / 1024.0);
    let s1 = [c[0] - d[0] * 0.75 * r, c[1] - d[1] * 0.75 * r, c[2]];
    let Some(h1) = line(s1, [s1[0], s1[1], s1[2] - 8.0]) else {
        m.shadow_hi = 0.0;
        m.shadow_lo = 0.0;
        return;
    };
    let s2 = [c[0] + d[0] * 0.75 * r, c[1] + d[1] * 0.75 * r, c[2] + r * 0.5];
    let len = (s2[2] - h1) / -d[2];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let e2 = if n > 0.0 { std::array::from_fn(|k| s2[k] + d[k] / n * len) } else { s2 };
    let h2 = line(s2, e2).unwrap_or(h1);
    m.shadow_lo = h1.min(h2) - 0.25;
    let hi = h1.max(h2) + 0.25;
    m.shadow_hi = if m.shadow_lo + 4.0 < hi { m.shadow_lo + 4.0 } else { hi };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool { (a - b).abs() < 1e-5 }

    #[test]
    fn direction_zero_leans_along_the_light() {
        let mut d = ShadowDirs::default();
        update_shadow_dir(&mut d, [1.0, 0.0, -0.3]);
        assert_eq!(d.0[0], [0.14, 0.0, -0.99, 0.0]);
        // Novalis spawn (RAM 0x1af000: −0.12949, 0.05323, −0.99): the light's horizontal angle is the direction's own.
        update_shadow_dir(&mut d, [-0.12949 * 4.0, 0.05323 * 4.0, -0.8]);
        assert!(close(d.0[0][0], -0.12949) && close(d.0[0][1], 0.05323), "{:?}", d.0[0]);
        let len = (d.0[0][0].powi(2) + d.0[0][1].powi(2) + d.0[0][2].powi(2)).sqrt();
        assert!((len - 1.0).abs() < 2e-4, "almost a unit vector: {len}");
    }

    #[test]
    fn light_dir_cross_fades_set_a_into_b() {
        let sets = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]];
        assert_eq!(light_dir(&sets, 0x0000_0100), [1.0, 0.0, 0.0], "f = 0: set A only");
        assert_eq!(light_dir(&sets, 0x0040_0100), [0.75, 0.25, 0.0]);
        assert_eq!(light_dir(&sets, 0x0000_0009), [0.0; 3], "a set past the table");
    }

    fn hero_in(ground: f32) -> HeroShadowIn { HeroShadowIn { ground_z: ground, ..Default::default() } }

    fn ratchet() -> Moby {
        let mut m = Moby::zeroed();
        m.position = [10.0, 20.0, 60.0, 0.0];
        m.bsphere = [10.0 * 1024.0, 20.0 * 1024.0, 60.5 * 1024.0, 0.85 * 1024.0];
        m
    }

    #[test]
    fn hero_slab_from_the_seed_and_the_four_probes() {
        let (mut m, mut p, mut d) = (ratchet(), 0.97f32, ShadowDirs::default());
        // Flat ground at the seed height: lo = 60 − 0.12, hi = 60 + 0.24 (RAM: 59.88 / 60.24 at the Novalis spawn).
        let mut calls = Vec::new();
        update_hero_shadow(&mut m, &mut p, &mut d, &hero_in(60.0), [-0.6, 0.25, -0.7], |a, b| {
            calls.push((a, b));
            Some(60.0)
        });
        assert!(close(m.shadow_lo, 59.88) && close(m.shadow_hi, 60.24), "{} {}", m.shadow_lo, m.shadow_hi);
        assert_eq!((m.mode & mode::CLASS_F, m.bbd), (mode::CLASS_F, 1));
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[2], calls[3], "the fourth probe repeats the third");
        for (_, e) in &calls { assert!(close(e[2], 59.5), "each probe ends 0.5 below the seed: {e:?}"); }
        // Direction 1 at p = 0.97 (RAM 0x1af010: −0.52284, 0.21494, −0.82489).
        let phi = 0.21494f32.atan2(-0.52284);
        update_hero_shadow(&mut m, &mut p, &mut d, &hero_in(60.0), [phi.cos(), phi.sin(), 0.0], |_, _| None);
        assert!(close(d.0[1][0], -0.52284) && close(d.0[1][1], 0.21494) && close(d.0[1][2], -0.82489), "{:?}", d.0[1]);
        // A step down under one probe: lo follows it, hi is clamped to lo + 3.
        let mut k = 0;
        update_hero_shadow(&mut m, &mut p, &mut d, &hero_in(60.0), [1.0, 0.0, 0.0], |_, _| {
            k += 1;
            Some(if k == 2 { 55.0 } else { 60.0 })
        });
        assert!(close(m.shadow_lo, 54.88) && close(m.shadow_hi, 57.88), "{} {}", m.shadow_lo, m.shadow_hi);
        // No seed ≥ 1: no slab.
        update_hero_shadow(&mut m, &mut p, &mut d, &hero_in(0.5), [1.0, 0.0, 0.0], |_, _| Some(60.0));
        assert_eq!((m.shadow_lo, m.shadow_hi), (0.0, 0.0));
    }

    #[test]
    fn hero_pitch_slides_under_him_in_the_air() {
        let (mut m, mut p, mut d) = (ratchet(), HERO_PITCH_GROUND, ShadowDirs::default());
        let air = HeroShadowIn { air_ticks: 1, ..hero_in(60.0) };
        let mut n = 0;
        while p < HERO_PITCH_AIR {
            update_hero_shadow(&mut m, &mut p, &mut d, &air, [1.0, 0.0, 0.0], |_, _| None);
            n += 1;
        }
        // (1.5 − 0.97) / 0.052 = 10.2: 11 ticks, the last one a partial step.
        assert_eq!(n, 11);
        assert!(close(p, 1.5));
        assert!(close(d.0[1][2], -(1.5f32).sin()));
        // Gates: water surface / Hoverboard / level-16 group / state 0x7d switch the shadow off.
        for (g, s) in [(0x12, 0), (0x15, 0), (0x16, 0), (0, 0x7d)] {
            m.mode |= mode::CLASS_F;
            update_hero_shadow(&mut m, &mut p, &mut d, &HeroShadowIn { group: g, state: s, ..hero_in(60.0) }, [1.0, 0.0, 0.0], |_, _| None);
            assert_eq!(m.mode & mode::CLASS_F, 0, "group {g:#x} state {s:#x}");
        }
    }

    #[test]
    fn class_slab_rules() {
        let mut m = ratchet();
        set_ground(&mut m, 12.0);
        assert!(close(m.shadow_lo, 11.8) && close(m.shadow_hi, 12.2));
        // Probe down: from z + 0.5 to max(z − 16, 0.5).
        probe_down_with(&mut m, |a, b| {
            assert_eq!((a[2], b[2]), (60.5, 44.0));
            Some(59.0)
        });
        assert!(close(m.shadow_lo, 58.8) && close(m.shadow_hi, 59.2));
        m.position[2] = 10.0;
        probe_down_with(&mut m, |_, b| {
            assert_eq!(b[2], 0.5);
            None
        });
        assert_eq!((m.shadow_lo, m.shadow_hi), (0.0, 0.0));
        // Along direction 0: a vertical probe 8 down, then a short second probe; lo − 0.25 / hi + 0.25 (≤ lo + 4).
        let mut m = ratchet();
        let dir0 = [0.14, 0.0, -0.99];
        let mut calls = Vec::new();
        probe_along_dir(&mut m, dir0, |a, b| {
            calls.push((a, b));
            Some(if calls.len() == 1 { 59.0 } else { 60.2 })
        });
        assert!(close(m.shadow_lo, 58.75) && close(m.shadow_hi, 60.45), "{} {}", m.shadow_lo, m.shadow_hi);
        let (a1, b1) = calls[0];
        assert!(close(a1[0], 10.0 - 0.75 * 0.85) && close(b1[2], a1[2] - 8.0), "{a1:?} {b1:?}");
        let (a2, b2) = calls[1];
        assert!(close(a2[0], 10.0 + 0.75 * 0.85) && close(a2[2], 60.5 + 0.425));
        let drop = a2[2] - b2[2];
        let n = (1.0f32 + (0.99f32 / 0.14).powi(2)).sqrt();
        assert!(close(drop, (a2[2] - 59.0) / n), "the second probe reaches 1/7.14 = 14 % of the way: {drop}");
        probe_along_dir(&mut m, dir0, |_, _| None);
        assert_eq!((m.shadow_lo, m.shadow_hi), (0.0, 0.0));
    }
}
