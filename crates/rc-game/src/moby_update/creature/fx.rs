//! Death and break effects shared by the creatures (level01 addresses; all engine code linked into every level):
//!
//! * [`spark_burst`] ([`SparkBurst`]): `0x273f50(size, light, moby, pos, sound)` ([`death_explosion`]), `0x2742a8`
//!   ([`piece_explosion`]) and `0x2fa1b0` (the path enemies' glob): three spark pairs (type 11), two flashes, a
//!   camera shake when in view, the moby's sound, an explosion light ([`light_spawn`], class 639).
//! * [`beam_explosion`] `SpawnBeamExplosion` 0x273310: the general explosion (damage sphere, type-15 streaks,
//!   type-11 sparks by camera distance, type-8 puffs, flashes, shake, sound, light).
//! * [`break_piece`] `BreakFxB` 0x278ad8 and [`piece_update`] `FxGroupUpdate` 0x30cd18: a body piece thrown off with a
//!   random velocity and spin, bouncing on the world, exploding (or fading, flag 1) when its timer runs out.
//! * [`light_spawn`] 0x2f3570 and [`light_update`] 0x2f3748: the explosion light moby (class 0x27f = 639).
//! * [`rate_slot`] `0x2efbf8`: a small "busy until tick" table that rate-limits the death explosions.
//!
//! * [`muzzle_smoke`] level03 `0x250ae8` (and its copies on 10 / 15 / 18): a gun's smoke ring and sparks.
//!
//! The particle spawners the effects call ([`part02`], [`part04`], [`part08`], [`part15`], [`part21`], [`part44`],
//! [`part52`]) fill the game's
//! records (`crate::particles::type02` …) and make the spawners' own draws at the game's point, and are counted in
//! `FxStats::part_spawns`. Without a particle system (unit tests) the draws are made as if a record was free. The
//! explosion light takes one of the eight point-light slots (`WritePointLight_B` 0x252750, [`crate::point_lights`]).

use super::{add, cs, len3, scale, set_len3, sub, V};
use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::debris::flash_spawn;
use crate::moby_update::services::{pf as to_pf, pv, pvar, HitTemplate, World};
use crate::ps2v::Pf;

/// `0x20a090` / `0x20a0a8`: the spark colours the explosions pick from (`randi(6)` each). The other explosions' copies
/// hold the same words (level01 0x20a980 / 0x20a998 the Bomb Glove, 0x20b820 / 0x20b838 the Suck Cannon burst, 0x20aa50
/// / 0x20aa68 the Devastator, 0x20b4f0 / 0x20b508 the glob, and the crates' and the Visibomb's): one table here.
pub const SPARK_A: [u32; 6] = [0x4f00_8fff, 0x4f00_8fff, 0x4f00_7fff, 0x4f00_6fff, 0x2fff_ffff, 0x2fff_ffff];
pub const SPARK_B: [u32; 6] = [0x2f00_5f7f, 0x2f00_4f7f, 0x2f00_3f7f, 0x2f00_004f, 0x2f00_0000, 0x3f00_0000];

/// The explosion light class (0x27f) and its update's level01 address.
pub const LIGHT_CLASS: i16 = 0x27f;
pub const LIGHT_UPDATE_FN: u32 = 0x2f3748;
/// `FxGroupUpdate` 0x30cd18 and the level01 classes it runs (the body pieces of 577 are 1747–1749).
pub const PIECE_UPDATE_FN: u32 = 0x30cd18;
pub const PIECE_CLASSES: [i16; 13] = [1736, 1737, 1738, 1747, 1748, 1749, 1761, 1762, 1763, 1770, 1814, 1815, 1817];

pub(crate) fn frame_load(w: &World) -> (f32, f32) { (f32::from_bits(w.svc.frame_load[0].0), f32::from_bits(w.svc.frame_load[1].0)) }

/// A record of an unported particle type (the pool slot the game takes) and its count.
pub fn part_unported(w: &mut World, ty: u8) -> bool {
    *w.svc.fx.part_spawns.entry(ty).or_default() += 1;
    let Some(p) = w.particles.as_deref_mut() else { return true };
    let ok = p.create_part(ty).is_some();
    if !ok { w.svc.fx.part_failed += 1; }
    ok
}

/// `PartType28Spawn(spread, pos, 0)` 0x282ef0 (the mote ring, no frame: `crate::particles::type28`).
pub fn part28(w: &mut World, spread: f32, p: V) -> bool {
    let Some(sys) = w.particles.as_deref_mut() else { return part_unported(w, 28) };
    *w.svc.fx.part_spawns.entry(28).or_default() += 1;
    let ok = crate::particles::type28::spawn(sys, w.rng, spread, p, None).is_some();
    if !ok { w.svc.fx.part_failed += 1; }
    ok
}

/// `PartType23Spawn(jitter, grow_lo, grow_hi, size, pos, spin, vel, rgba)` 0x282060 as is (`crate::particles::type23`).
#[allow(clippy::too_many_arguments)]
pub fn part23(w: &mut World, [jitter, lo, hi, size]: [f32; 4], p: V, spin: i32, vel: V, rgba: u32) -> bool {
    let Some(sys) = w.particles.as_deref_mut() else { return part_unported(w, 23) };
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let ok = crate::particles::type23::spawn(sys, w.rng, jitter, lo, hi, size, p, spin, vel, rgba).is_some();
    if !ok { w.svc.fx.part_failed += 1; }
    ok
}

/// `0x26cae0(out, lo, hi)` (level14 `0x260200`): two random angles, then a length `randf(lo, hi)`, as
/// `polar(len, a, b)` (w 0).
pub fn rand_vec_ab(w: &mut World, lo: f32, hi: f32) -> V {
    let a = w.rng.rand_angle();
    let b = w.rng.rand_angle();
    let l = w.rng.randf(lo, hi);
    let v = crate::targeting::polar(l, a, b);
    [v[0], v[1], v[2], 0.0]
}

/// `PartType69Spawn(pos, vel, alpha, moby)` (level14 `0x27c5d8`, [`crate::particles::type69`]): the record's pool index.
pub fn part69(w: &mut World, p: V, vel: V, alpha: i32, moby: Option<MobyId>) -> Option<usize> {
    let Some(sys) = w.particles.as_deref_mut() else {
        part_unported(w, 69);
        return None;
    };
    *w.svc.fx.part_spawns.entry(69).or_default() += 1;
    let i = crate::particles::type69::spawn(sys, w.rng, p, vel, alpha, moby.map_or(0, |m| m as u32 + 1));
    if i.is_none() { w.svc.fx.part_failed += 1; }
    i
}

/// `PartType71Spawn(pos, v1, v2, c1, c2, A, B, C, moby)` (level15 `0x2647f0`, [`crate::particles::type71`]): the
/// record's pool index.
pub fn part71(w: &mut World, s: &crate::particles::type71::Spawn) -> Option<usize> {
    let Some(sys) = w.particles.as_deref_mut() else {
        part_unported(w, 71);
        return None;
    };
    *w.svc.fx.part_spawns.entry(71).or_default() += 1;
    let i = crate::particles::type71::spawn(sys, w.rng, s);
    if i.is_none() { w.svc.fx.part_failed += 1; }
    i
}

/// [`part23`] returning the record's pool index for the caller's patch (`None`: no record, or no particle system).
#[allow(clippy::too_many_arguments)]
pub fn part23_rec(w: &mut World, [jitter, lo, hi, size]: [f32; 4], p: V, spin: i32, vel: V, rgba: u32) -> Option<usize> {
    let Some(sys) = w.particles.as_deref_mut() else {
        part_unported(w, 23);
        return None;
    };
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let i = crate::particles::type23::spawn(sys, w.rng, jitter, lo, hi, size, p, spin, vel, rgba);
    if i.is_none() { w.svc.fx.part_failed += 1; }
    i
}

/// The record `i` of the particle pool (after [`part23_rec`]).
pub fn rec_mut<'a>(w: &'a mut World, i: usize) -> Option<&'a mut crate::particles::Record> { w.particles.as_deref_mut().and_then(|s| s.pool.recs.get_mut(i)) }

/// One type-23 puff of [`jet_puffs`]: `PartType23Spawn(jitter, grow_lo, grow_hi, size, pos, spin, vel, rgba)` 0x282060
/// with the callers' patch: timer +0x0a = `life`, byte 9 = 4 + 0x40, the rotation byte `randi(255)` when asked (drawn
/// after the spawn, only with a record), phase 2 (+0x24) fading from +0x2a = `a0` over +0x2b = the timer's low byte.
/// Without a particle system only the pool slot is counted.
#[allow(clippy::too_many_arguments)]
pub fn puff23(w: &mut World, [jitter, lo, hi, size]: [f32; 4], p: V, spin: i32, vel: V, rgba: u32, life: i32, rotation: bool, a0: u8) -> bool {
    let Some(sys) = w.particles.as_deref_mut() else { return part_unported(w, 23) };
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let Some(i) = crate::particles::type23::spawn(sys, w.rng, jitter, lo, hi, size, p, spin, vel, rgba) else {
        w.svc.fx.part_failed += 1;
        return false;
    };
    let rot = rotation.then(|| w.rng.randi(0xff) as u8);
    let r = &mut w.particles.as_deref_mut().unwrap().pool.recs[i];
    use crate::particles::rec;
    rec::set_i16(r, 0xa, life as i16);
    r[9] = 4 + 0x40;
    if let Some(b) = rot { r[8] = b; }
    rec::set_u32(r, 0x24, 2);
    r[0x2a] = a0;
    r[0x2b] = r[0xa];
    true
}

/// `FUN_00278810(glow, core, a, b, vel)` 0x278810 (the same code on other levels: level18 `0x266140`, cluster with 5
/// copies): the jet glow of a moving point. Two glow puffs at `a` (`randi(16)` spin, its sign `randi(2)`; jitter 0.1,
/// growth 1 → 0.9, size `glow`, colour 0x7f204080, life `ticks(30)`) and three white cores at `b` (jitter 0.05, growth
/// 1 → 0.97, size `core`, spin 16, −16, 16, colour 0x7fffffff, life `ticks(6)`, a rotation byte), all moving at `vel`.
/// Callers: the cutscene ships' trail (`classes::cutscene_fx`, `vel` zero), the boss 1422's jet (`units::veldin_boss`).
pub fn jet_puffs(w: &mut World, glow: f32, core: f32, a: V, b: V, vel: V) {
    for _ in 0..2 {
        let r = w.rng.randi(0x10);
        let spin = if w.rng.randi(2) == 0 { r } else { -r };
        let life = w.ticks(0x1e);
        puff23(w, [0.1, 1.0, 0.9, glow], a, spin, vel, 0x7f20_4080, life, false, 0x7f);
    }
    let mut spin = 0x10;
    for _ in 0..3 {
        let life = w.ticks(6);
        puff23(w, [0.05, 1.0, f32::from_bits(0x3f78_51ec), core], b, spin, vel, 0x7fff_ffff, life, true, 0x7f);
        spin = -spin;
    }
}

/// `PartType74Spawn(m, p, v, desc, additive)` 0x28a7a8 (`particles::type74`): a puff carried in moby `id`'s frame; one
/// `rand()` with a record (or without the particle system). Callers: Gemlik's ship 69, the Summoner's mouse 1818.
pub fn part74(w: &mut World, id: MobyId, p: V, v: [f32; 3], d: crate::particles::type74::Desc, additive: bool) {
    *w.svc.fx.part_spawns.entry(74).or_default() += 1;
    let m = w.m(id);
    let (mp, rows) = ([m.position[0], m.position[1], m.position[2]], [0, 1, 2].map(|k| [m.rows[k][0], m.rows[k][1], m.rows[k][2]]));
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    if crate::particles::type74::spawn(sys, w.rng, id, mp, rows, p, v, d, additive).is_none() { w.svc.fx.part_failed += 1; }
}

/// `PartType05Spawn(grow, size, pos, r, g, b, life)` 0x27e750 (`particles::type05`, no draws): life 0 makes no call
/// to `CreatePart` (nothing counted). The decoy's and the Glove of Doom canister's pops, the morph's flash.
pub fn part05(w: &mut World, grow: f32, size: f32, pos: [f32; 4], rgb: [u32; 3], life: i32) {
    if life == 0 { return; }
    *w.svc.fx.part_spawns.entry(crate::particles::type05::TYPE).or_default() += 1;
    let Some(p) = w.particles.as_deref_mut() else { return };
    if crate::particles::type05::spawn(p, grow, size, pos, rgb[0], rgb[1], rgb[2], life).is_none() { w.svc.fx.part_failed += 1; }
}

/// A spawner call of type `ty`: `f` fills the record (and makes the spawner's draws) when there is a particle
/// system; `draws` makes the same draws when there is none. Counted in `FxStats`; false when the pool was full.
fn spawn_part(w: &mut World, ty: u8, draws: impl FnOnce(&mut crate::rng::Rng), f: impl FnOnce(&mut crate::particles::Particles, &mut crate::rng::Rng) -> Option<usize>) -> bool {
    *w.svc.fx.part_spawns.entry(ty).or_default() += 1;
    let Some(p) = w.particles.as_deref_mut() else {
        draws(w.rng);
        return true;
    };
    let ok = f(p, w.rng).is_some();
    if !ok { w.svc.fx.part_failed += 1; }
    ok
}

/// `PartType19Spawn(pos, vel)` 0x281748 (the streak; `crate::particles::type19`): one `randi(6)` with a record.
pub fn part19(w: &mut World, p: V, vel: V) -> bool {
    spawn_part(w, 19, |r| { r.randi(6); }, |s, r| crate::particles::type19::spawn19(s, r, p, vel))
}

/// `PartType02Spawn` 0x27dc98 (the trail blob; `crate::particles::type02`): one `randf(0, 255)` with a record.
pub fn part02(w: &mut World, a: &crate::particles::type02::Spawn) -> bool {
    spawn_part(w, 2, |r| { r.randf(0.0, 255.0); }, |p, r| crate::particles::type02::spawn(p, r, a))
}

/// [`part02`] keeping the record index (the classes that hit-test their own puffs). None without particles.
pub fn part02_rec(w: &mut World, a: &crate::particles::type02::Spawn) -> Option<usize> {
    *w.svc.fx.part_spawns.entry(2).or_default() += 1;
    let Some(p) = w.particles.as_deref_mut() else {
        w.rng.randf(0.0, 255.0);
        return None;
    };
    let r = crate::particles::type02::spawn(p, w.rng, a);
    if r.is_none() { w.svc.fx.part_failed += 1; }
    r
}

/// `PartType04Spawn` 0x27e538 (the smoke / fire puff; `crate::particles::type04`): one raw `rand()` with a record.
pub fn part04(w: &mut World, a: &crate::particles::type04::Spawn) -> bool {
    spawn_part(w, 4, |r| { r.rand(); }, |p, r| crate::particles::type04::spawn(p, r, a))
}

/// `PartType21Spawn(size, pos, vel, c1, c2, life, split)` 0x281c10 (the splitting spark; `crate::particles::type21`):
/// nothing for life 0; one raw `rand()` (its rotation) with a record.
#[allow(clippy::too_many_arguments)]
pub fn part21(w: &mut World, size: f32, p: V, vel: V, c1: u32, c2: u32, life: i32, split: i16) -> bool {
    if life == 0 { return false; }
    spawn_part(w, 21, |r| { r.rand(); }, |s, r| crate::particles::type21::spawn_rng(s, r, size, p, vel, c1, c2, life, split))
}

/// `PartType27Spawn` 0x282d80 (the gun spark; `crate::particles::type27`): one raw `rand()` (its rotation) with a record.
pub fn part27(w: &mut World, size: f32, p: V, vel: V, rgba: u32, life: i32) -> bool {
    spawn_part(w, 27, |r| { r.rand(); }, |s, r| crate::particles::type27::spawn_rng(s, r, size, p, vel, rgba, life))
}

/// `PartType44Spawn` 0x286450 (the drifting smoke puff; `crate::particles::type44`): one `randi(0xff)` (its rotation)
/// with a record.
pub fn part44(w: &mut World, a: &crate::particles::type44::Spawn) -> bool {
    spawn_part(w, 44, |r| { r.randi(0xff); }, |s, r| crate::particles::type44::spawn_rng(s, r, a))
}

/// Level03 `0x24e650(angle, &out, v, axis)` (cluster with level10's copy): `v` turned by `angle` about the normalised
/// `axis` (`build_quaternion_from_axis_angle`, then the quaternion rotation `fun_00214800`); `v` itself when
/// `|angle| < 1e-5`. Rodrigues' form of the same rotation [L: the sense of the turn; every caller turns through a full
/// ring].
pub fn turn_about(angle: f32, v: V, axis: V) -> V {
    if angle.abs() < 1e-5 { return v; }
    let k = set_len3(axis, 1.0);
    let (s, c) = angle.sin_cos();
    let kv = super::dot3(k, v);
    let x = [k[1] * v[2] - k[2] * v[1], k[2] * v[0] - k[0] * v[2], k[0] * v[1] - k[1] * v[0]];
    std::array::from_fn(|i| if i == 3 { v[3] } else { v[i] * c + x[i] * s + k[i] * kv * (1.0 - c) })
}

/// The muzzle smoke ring, level03 `0x250ae8(moby, &pos, add)` (one function on 03 / 10 / 15 / 18: L10 0x255ab0, L15
/// 0x251d00, L18 0x264c10, census cluster of 720 bytes; no level-01 copy): around the moby's x axis (its rows +0xc0), the
/// moby's z row (+0xe0) turned through a ring makes the directions.
/// * `truncate(24.0)` = 24 smoke puffs: `i·15° + randf_sym(0, 7.5°)`, speed `randf(0.05, 0.1)` a tick, spin
///   `rand_range(0, 3)` negated on `randi(2) ≠ 0`, life `rand_range(ticks(30), ticks(90))`, `+ add` when given;
///   `PartType44Spawn(60000, 3000, 0.85, −0.001, 0.85, pos, v, life, 0x1e, 0xffffff, spin)`.
/// * `truncate(6.0)` = 6 sparks: `i·60° + randf(0, 45°)`, speed `randf(0.05, 0.1)`, life `rand_range(ticks(20),
///   ticks(60))`, `+ add`; `PartType21Spawn(10000, pos, v, 0x4f007fff, 0x1fffffff, life, 1)`.
pub fn muzzle_smoke(w: &mut World, id: MobyId, pos: V, add_v: Option<V>) {
    let rows = w.m(id).rows;
    let (x_axis, z_row) = (rows[0], rows[2]);
    for i in 0..24 {
        let j = w.rng.randf_sym(0.0, f32::from_bits(0x3e06_0a92));
        let mut v = turn_about(i as f32 * 0.261_799_4 + j, z_row, x_axis);
        let sp = w.rng.randf(0.05, 0.1);
        v = set_len3(v, sp);
        let mut spin = w.rng.rand_range(0, 3);
        if w.rng.randi(2) != 0 { spin = -spin; }
        let (lo, hi) = (w.ticks(0x1e), w.ticks(0x5a));
        let life = w.rng.rand_range(lo, hi);
        if let Some(a) = add_v { v = add(v, a); }
        let s = crate::particles::type44::Spawn { size: 60000.0, growth: 3000.0, damp: 0.85, fall: f32::from_bits(0xba83_126f), w: 0.85, pos: [pos[0], pos[1], pos[2]], vel: [v[0], v[1], v[2]], life, alpha: 0x1e, rgb: 0xff_ffff, spin };
        part44(w, &s);
    }
    for i in 0..6 {
        let j = w.rng.randf(0.0, std::f32::consts::FRAC_PI_4);
        let mut v = turn_about(i as f32 * std::f32::consts::FRAC_PI_3 + j, z_row, x_axis);
        let sp = w.rng.randf(0.05, 0.1);
        v = set_len3(v, sp);
        let (lo, hi) = (w.ticks(0x14), w.ticks(0x3c));
        let life = w.rng.rand_range(lo, hi);
        if let Some(a) = add_v { v = add(v, a); }
        part21(w, 10000.0, pos, v, 0x4f00_7fff, 0x1fff_ffff, life, 1);
    }
}

/// `PartType08Spawn(size, pos, vel, c1, c2, life)` 0x27f2b0 (the explosion puff): nothing for life 0; no draws.
pub fn part08(w: &mut World, size: f32, p: V, vel: V, c1: u32, c2: u32, life: i32) -> bool {
    if life == 0 { return false; }
    spawn_part(w, 8, |_| {}, |s, _| crate::particles::type08::spawn(s, size, p, vel, c1, c2, life))
}

/// `PartType15Spawn` 0x280bd0 (the streak): nothing for life 0; one raw `rand()` with a record.
pub fn part15(w: &mut World, a: &crate::particles::type15::Spawn) -> bool {
    if a.life == 0 { return false; }
    spawn_part(w, 15, |r| { r.rand(); }, |p, r| crate::particles::type15::spawn(p, r, a))
}

/// `PartType52Spawn(s1, s2, pos, c1, c2, life)` 0x287158 (the flat goo drip): one `randi(255)` with a record.
pub fn part52(w: &mut World, s1: f32, s2: f32, p: V, c1: u32, c2: u32, life: i32) -> bool {
    spawn_part(w, 52, |r| { r.randi(0xff); }, |s, r| crate::particles::type52::spawn(s, r, s1, s2, p, c1, c2, life))
}

/// `0x277b50(speed, a, b, out)`: `speed·(cos a·cos b, sin a·cos b, sin b)`.
pub fn polar(speed: f32, a: f32, b: f32) -> V { [a.cos() * speed * b.cos(), a.sin() * speed * b.cos(), b.sin() * speed, 0.0] }

/// `0x2efbf8(table, n, dur)`: the first slot (from the last) whose busy-until tick is before now gets `now + dur`;
/// returns its index, −1 when all are busy.
pub fn rate_slot(slots: &mut [i32], now: i32, dur: i32) -> i32 {
    for i in (0..slots.len()).rev() {
        if slots[i] < now {
            slots[i] = now + dur;
            return i as i32;
        }
    }
    -1
}

/// Is the sphere `(pos, r)` in view (`FastBSphereCheck(far, …) ≠ −1`)? No view: in view.
pub fn in_view(w: &World, far: f32, p: V, r: f32) -> bool {
    match w.view {
        Some(v) => !v.culled(far, [p[0], p[1], p[2], r]),
        None => true,
    }
}

/// One code, three copies (docs/plan/explosions.md §C): `0x273f50` (the death explosion), `0x2742a8` (a broken
/// prop's piece) and `0x2fa1b0` (the path enemies' glob), as data: three type-11 spark pairs, two flashes (0x2c20e0)
/// when there is a moby, the camera shake, the moby's class sound, the explosion light.
#[derive(Clone, Copy, Debug)]
pub struct SparkBurst {
    /// The spark sprite per unit of size (`size · sprite`).
    pub sprite: f32,
    /// The colour shift (`0x270fa8` on the sparks, `0x270f48` on the flashes; `0x2742a8` passes 1: red ↔ green).
    pub shift: u8,
    /// The two flashes: size factor, colour and alpha, duration (ticks).
    pub flashes: [(f32, [u8; 4], i32); 2],
    /// `FastBSphereCheck(10, (pos, 2))` → the shake along up `size·0.1` for `ticks(20)`.
    pub shake: bool,
    /// The moby's class sound (when it is not deleted and `sound ≠ −1`).
    pub sound: bool,
}

/// `0x273f50`: the death explosion.
pub const DEATH_BURST: SparkBurst = SparkBurst { sprite: 400000.0, shift: 0, flashes: [(4.0, [0x7f, 0x40, 0, 0x30], 20), (3.0, [0x60, 0x20, 0, 0x20], 0x1d)], shake: true, sound: true };
/// `0x2742a8`: the quiet explosion of a broken prop's flag-2 pieces (`crate::moby_update::classes::breakables`).
pub const PIECE_BURST: SparkBurst = SparkBurst { sprite: 500000.0, shift: 1, flashes: [(4.0, [0x7f, 0x40, 0, 0x30], 20), (3.0, [0x60, 0x20, 0, 0x20], 0x1d)], shake: false, sound: false };
/// `0x2fa1b0`: the path enemies' glob hitting (level01 only; its tables 0x20b4f0 / 0x20b508 and light template
/// 0x1e3440 hold 0x20a090 / 0x20a0a8 / 0x1b0770's words).
pub const SHOT_BURST: SparkBurst = SparkBurst { sprite: 400000.0, shift: 0, flashes: [(2.0, [0x7f, 0, 0x40, 0x30], 20), (1.5, [0x20, 0, 0x20, 0], 0x1d)], shake: false, sound: true };

/// `0x273f50(size, light, moby, pos, sound)`: the death explosion ([`DEATH_BURST`]).
pub fn death_explosion(w: &mut World, size: f32, light: f32, moby: Option<MobyId>, p: V, sound: i32) {
    spark_burst(w, &DEATH_BURST, size, light, moby, p, sound);
}

/// `0x2742a8(size, light, moby, pos)`: [`PIECE_BURST`] (sprites 500000·size, the colours with red and green
/// swapped, no shake, no sound).
pub fn piece_explosion(w: &mut World, size: f32, light: f32, moby: Option<MobyId>, p: V) {
    spark_burst(w, &PIECE_BURST, size, light, moby, p, -1);
}

/// The spark burst ([`SparkBurst`]): per pair `randf(8, 10)`, `randi(6)` ×2, `rand_range(ticks 15, 20)` /
/// `(ticks 25, 30)` and the type-11 spawn's own draws (sprite `size·sprite`, speed `randf·dt·size`); the flashes (3
/// draws each); the shake; the sound; the light (none for 0, radius 13 for a negative size; template 0x1b0770).
pub fn spark_burst(w: &mut World, row: &SparkBurst, size: f32, light: f32, moby: Option<MobyId>, p: V, sound: i32) {
    for _ in 0..3 {
        let sp = w.rng.randf(8.0, 10.0) * super::DT;
        let a = w.rng.randi(6) as usize;
        let b = w.rng.randi(6) as usize;
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let life = w.rng.rand_range(t15, t20);
        let (t25, t30) = (w.ticks(25), w.ticks(30));
        let t1 = w.rng.rand_range(t25, t30);
        w.part11(to_pf(size * row.sprite), to_pf(sp * size), pv(p), [Pf::ZERO; 4], colour_shift(SPARK_A[a], row.shift), colour_shift(SPARK_B[b], row.shift), life, t1, 0, 0);
    }
    if let Some(m) = moby {
        for (k, [r, g, b, a], t) in row.flashes {
            let c = colour_shift(r as u32 | (g as u32) << 8 | (b as u32) << 16, row.shift);
            let t = w.ticks(t);
            flash_spawn(w, size * k, m, p, [0.0; 4], t, c as u8, (c >> 8) as u8, (c >> 16) as u8, a);
        }
    }
    if row.shake && in_view(w, 10.0, p, 2.0) {
        let t = w.ticks(20);
        w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp: size * 0.1, ticks: t });
    }
    if let Some(m) = moby {
        let s = w.m(m).state;
        if row.sound && s != 0xfe && s != 0xfd && sound != -1 { w.play_sound(sound, 0, m); }
    }
    explosion_light(w, light, p);
}

/// The explosion light of the spark bursts: none for 0, radius 13 for a negative size.
fn explosion_light(w: &mut World, light: f32, p: V) {
    if light != 0.0 {
        let l = if light <= 0.0 { 13.0 } else { light };
        let mut t = LIGHT_DEATH;
        t.radius = [l, l, l];
        light_spawn(w, &t, p);
    }
}

/// The parameters of `SpawnBeamExplosion` 0x273310 (in the game's order).
#[derive(Clone, Copy, Debug)]
pub struct Beam {
    /// `param_1` / `param_2`: damage sphere radius and damage (flags 0x810001, type 2 / subtype 1, the moby's class).
    pub damage_r: f32,
    pub damage: f32,
    /// `param_3` / `param_4`: the two flash sizes; `param_5`: the camera distance beyond which the extra flashes show.
    pub flash: f32,
    pub flash2: f32,
    pub flash_dist: f32,
    /// `param_6`: the effect scale; `param_7`: the light radius (0 none).
    pub scale: f32,
    pub light: f32,
    /// `param_11` type-15 streaks, `param_12` type-11 spark pairs, `param_13` type-8 puffs, `param_16` debris.
    pub streaks: i32,
    pub sparks: i32,
    pub puffs: i32,
    pub debris: i32,
    /// `param_14` the moby's class sound (−1 none), `param_15` camera shake.
    pub sound: i32,
    pub shake: bool,
}

/// `SpawnBeamExplosion(…, moby, pos, …)` 0x273310 with `param_17` = −1 (throttled when the frame loads sum over 1.7)
/// and `param_18` = 0 (the normal colours). The debris burst (`param_16`) makes the Bomb Glove's fireballs 122
/// (`0x2c4c20`, ported 2026-09-28: hero_gameplay.md §14.1).
pub fn beam_explosion(w: &mut World, b: &Beam, moby: Option<MobyId>, p: V) { beam_explosion_shift(w, b, moby, p, 0) }

/// `FUN_00270fa8(rgba, shift)`: the colour with its channels swapped by the bits of `shift` (1: r ↔ g, 2: g ↔ b,
/// 4: b ↔ r, in that order; `FUN_00270f48`); alpha kept. The gold chicken's burst (`param_18` = 1) and the Bomb
/// Glove's colour shift use it.
pub fn colour_shift(c: u32, shift: u8) -> u32 {
    if shift == 0 { return c; }
    let (mut r, mut g, mut b) = (c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff);
    if shift & 1 != 0 { std::mem::swap(&mut r, &mut g); }
    if shift & 2 != 0 { std::mem::swap(&mut g, &mut b); }
    if shift & 4 != 0 { std::mem::swap(&mut b, &mut r); }
    (c & 0xff00_0000) | b << 16 | g << 8 | r
}

/// [`beam_explosion`] with `param_18` = `shift`: the streaks', sparks' and puffs' colours through [`colour_shift`],
/// the first two flashes 0x46 / 0x46 / 0x3c instead of 0x96 / 0x96 / 0x7f, and the light template 0x1b0720
/// ([`LIGHT_BEAM_GOLD`]) instead of 0x1b06d0 (the gold chicken 270's burst).
pub fn beam_explosion_shift(w: &mut World, b: &Beam, moby: Option<MobyId>, p: V, shift: u8) {
    let (l0, l1) = frame_load(w);
    let throttle = (1.7 < l1 + l0) as i32;
    let base_z = super::DT * 8.0 * b.scale;
    if 0.0 < b.damage_r {
        if let Some(m) = moby {
            let t = HitTemplate { dir: [Pf::ZERO; 4], attacker: Some(m), flags: 0x81_0001, b18: 2, b19: 1, h1a: w.m(m).o_class as u16, damage: to_pf(b.damage), w20: 0 };
            w.sphere_mobys(to_pf(b.damage_r), pv(p), 0x10, Some(m), Some(&t));
        }
    }
    for _ in 0..b.streaks.max(0) {
        let pitch = w.rng.randf(0.2618, 1.3963);
        let ang = w.rng.rand_angle();
        let sp = w.rng.randf(7.0, 10.5);
        let mut v = polar(b.scale * sp * super::DT, ang, pitch);
        v[2] += super::DT * 3.0;
        let (t60, t120) = (w.ticks(60), w.ticks(120));
        let life = w.rng.rand_range(t60, t120) - throttle * 10;
        let a = crate::particles::type15::Spawn { size: b.scale * 40000.0, pos: p, vel: v, c1: colour_shift(0x4f00_7fff, shift), c2: colour_shift(0x1f00_007f, shift), life, split: 1, def: -1, blend: -1 };
        part15(w, &a);
    }
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let dist = len3(sub(cam, p));
    if b.debris != 0 {
        // The debris (`param_16`): the fireballs 122 of `0x2c4c20` (`crate::moby_update::classes::bomb::fireball`, the
        // Bomb Glove's). A random vector; within 14 of the camera one fireball drifting toward it; then `debris − 1`
        // thrown out 15°..75° up at 7..10.5 × scale a second, one in three of the larger kind.
        let r = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0];
        if dist < 14.0 {
            let mut d = sub(cam, p);
            d[2] += dist * 0.5;
            let v = super::set_len3(r, dist / 5.0 * super::DT);
            let d = super::set_len3(d, 2.0 * dist * super::DT);
            let v = super::clamp_len3(add(v, d), super::DT * 10.0);
            let (t60, t90) = (w.ticks(60), w.ticks(90));
            let life = w.rng.rand_range(t60, t90);
            crate::moby_update::classes::bomb::fireball(w, p, v, life, 0);
        }
        for i in 0..(b.debris - 1).max(0) {
            let pitch = w.rng.randf(f32::from_bits(0x3e86_0a92), f32::from_bits(0x3fa7_8d36));
            let yaw = w.rng.rand_angle();
            let sp = w.rng.randf(7.0, 10.5);
            let mut v = polar(b.scale * sp * super::DT, yaw, pitch);
            v[2] += super::DT + super::DT;
            let (t60, t90) = (w.ticks(60), w.ticks(90));
            let life = w.rng.rand_range(t60, t90);
            crate::moby_update::classes::bomb::fireball(w, p, v, life, (i % 3 == 0) as u8);
        }
    }
    let mut n = b.sparks;
    if dist < (b.sparks as f32) * 2.0 { n = (dist as i32) / 2; }
    let near = if dist < 7.0 { 7.0 - dist } else { 0.0 };
    for _ in 0..n.max(0) {
        let sp = w.rng.randf(8.0, 10.0) * super::DT;
        let a = w.rng.randi(6) as usize;
        let c = w.rng.randi(6) as usize;
        let speed = (sp - near * super::DT) * b.scale;
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let life = w.rng.rand_range(t15, t20);
        let (t30, t45) = (w.ticks(30), w.ticks(45));
        let t1 = w.rng.rand_range(t30, t45);
        let size = to_pf(b.scale * 400000.0);
        let basev = [Pf::ZERO, Pf::ZERO, to_pf(base_z), Pf::ZERO];
        w.part11(size, to_pf(speed), pv(p), basev, colour_shift(SPARK_A[a], shift), colour_shift(SPARK_B[c], shift), life - throttle * 3, t1 - throttle * 5, 0, 0);
        let (t5, t10) = (w.ticks(5), w.ticks(10));
        let life2 = w.rng.rand_range(t5, t10);
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let t2 = w.rng.rand_range(t15, t20);
        w.part11(size, to_pf(speed * 0.5), pv(p), basev, 0x7fff_ffff, 0x00ff_ffff, life2 - throttle * 2, t2 - throttle * 3, 0, 0);
    }
    for _ in 0..b.puffs.max(0) {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let s = w.rng.randf(0.0, 3.0);
        let vel = set_len3([x, y, z, 0.0], b.scale * s * super::DT);
        let a = w.rng.randi(6) as usize;
        let c = w.rng.randi(6) as usize;
        let (t30, t45) = (w.ticks(30), w.ticks(45));
        let life = w.rng.rand_range(t30, t45) - throttle * 5;
        part08(w, 200_000.0, p, vel, colour_shift(SPARK_A[a], shift), colour_shift(SPARK_B[c], shift), life);
    }
    // The flashes carry the sparks' base vector (sp+0x1b0: (0, 0, 8·dt·scale)) in their +0x00 (unused by their update).
    let basev_f = [0.0, 0.0, base_z, 0.0];
    if let Some(m) = moby {
        if 0.0 < b.flash {
            let (r0, b0, r1) = if shift != 0 { (0x46, 0x46, 0x3c) } else { (0x96, 0x96, 0x7f) };
            if l0 < 0.95 && b.flash_dist < dist {
                let t = w.ticks(16);
                flash_spawn(w, b.flash, m, p, basev_f, t, r0, 0x96, b0, 0x20);
                let t = w.ticks(22);
                flash_spawn(w, b.flash, m, p, basev_f, t, r1, 0x7f, 0x50, 0x20);
            }
            let t = w.ticks(30);
            flash_spawn(w, b.flash, m, p, basev_f, t, r1, 0x7f, 0, 0x30);
        }
        if 0.0 < b.flash2 {
            let t = w.ticks(27);
            flash_spawn(w, b.flash2, m, p, basev_f, t, 0xff, 0xff, 0xff, 0x20);
        }
    }
    if b.shake && in_view(w, 10.0, p, 2.0) {
        let amp = if dist < 20.0 { 0.4 - dist * 0.0175 } else { 0.050_000_012 };
        let t = w.ticks(25);
        w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp, ticks: t });
    }
    if let (Some(m), true) = (moby, b.sound != -1) { w.play_sound(b.sound, 0, m); }
    if b.light != 0.0 && throttle == 0 && in_view(w, 100.0, p, b.light) {
        // The template's three radii are overwritten (the game writes them into 0x1b06d0 itself); param_18 = 0.
        let l = if b.light <= 0.0 { 15.0 } else { b.light };
        let mut t = if shift != 0 { LIGHT_BEAM_GOLD } else { LIGHT_BEAM };
        t.radius = [l, l, l];
        light_spawn(w, &t, p);
    }
}

// -------------------------------------------------------------------------------------------------
// The amoeboids' goo (0x2ef560 drips, 0x2ef770 bursts; gp block 0x1619b0..0x161a3c)

/// `0x2747a0(r, v)`: `v.xyz += (randf(−r, r), randf(−r, r), randf(−r, r))`, in that order.
pub fn jitter(w: &mut World, r: f32, v: &mut V) {
    for c in v.iter_mut().take(3) { *c += w.rng.randf(-r, r); }
}

/// `0x2ef560`: while the amoeboid was drawn (+0x31), two chances of a flat goo drip (type 52) at a random point within
/// 0.1 of its feet: 1 in 20 (size `randf(0, 1)`·s → `randf(1, 2)`·s, colours 0x30002028 → 0x2020) and 1 in 5 (size
/// `randf(0, 3.5)`·s → `randf(3.5, 4.5)`·s, colours 0x20002020 → 0x2030); life `randf(120, 180)` ticks; s = the
/// amoeboid's size (pvar +0x250). Per drip 6 draws plus the spawner's one.
pub fn goo_drips(w: &mut World, id: MobyId, size: f32) {
    if w.m(id).visible == 0 { return; }
    for (n, lo, hi, c1, c2) in [(20, 1.0f32, 2.0f32, 0x3000_2028u32, 0x0000_2020u32), (5, 3.5, 4.5, 0x2000_2020, 0x0000_2030)] {
        if w.rng.randi(n - 1) != 0 { continue; }
        let a = w.rng.rand_angle();
        let r = w.rng.randf(0.0, 0.1);
        let s1 = w.rng.randf(0.0, lo) * size;
        let s2 = w.rng.randf(lo, hi) * size;
        let life = w.rng.randf(120.0, 180.0) as i32;
        let (cs, sn) = cs(a);
        let q = w.m(id).position;
        let p = [cs * r + q[0], sn * r + q[1], q[2], q[3]];
        let life = w.ticks(life);
        part52(w, s1, s2, p, c1, c2, life);
    }
}

/// `0x2ef770(moby, dir)`: the goo burst of a hit / death: 20 clumps (from the feet jittered by `0.5·size`, 1 up) of 10
/// type-2 blobs each (`crate::particles::type02`). A clump flies off at `randf(1, 8)` horizontally in a random direction
/// and `randf(2, 6)` up (per second; plus `dir`·`randf(0, 0)`), its phase-B velocity falling by 20·dt² per tick of
/// `ticks(30)`; each blob moves the clump's point by `randf(±0.2)` and its phase-B velocity by `randf(±0.5)·dt` (both
/// kept for the next blob), sizes `r·0.125` / `r·0.065` (r = `randf(0.5, 1.5)`), colours tweened between
/// 0x8000eeee / 0x8000ff90 and 0xffee, phases `ticks(10)`, `ticks(30)`, `randf(5, 25)` ticks, texture `def[23]`.
pub fn goo_burst(w: &mut World, id: MobyId, dir: V, size: f32) { goo_burst_k(w, id, dir, size, 0.0) }

/// [`goo_burst`] with the clumps' share of `dir` drawn as `randf(0, k)` (the amoeboids 0; Rilgar's boost pickups 133 1,
/// their copy `0x2db238` with size 0.5).
pub fn goo_burst_k(w: &mut World, id: MobyId, dir: V, size: f32, k: f32) {
    goo_burst_with(w, id, dir, &Goo { spread: 0.5 * size, lift: 1.0, blob_spread: 0.2, k })
}

/// The numbers that differ between the goo bursts' copies (level01 `0x2ef770`: [`goo_burst_k`]; level06 `0x2ea198`,
/// Blarg's 827: spread 0.25, lift 0.25, blob spread 0.125, k 0 from its words gp−0x4fe0..−0x4f9c).
#[derive(Clone, Copy, Debug)]
pub struct Goo {
    /// The clump's jitter about the feet, and its lift.
    pub spread: f32,
    pub lift: f32,
    /// Each blob's jitter of the clump's point.
    pub blob_spread: f32,
    /// The clumps' share of `dir`: `randf(0, k)`.
    pub k: f32,
}

/// [`goo_burst`] with a copy's numbers ([`Goo`]).
pub fn goo_burst_with(w: &mut World, id: MobyId, dir: V, g: &Goo) {
    for _ in 0..20 {
        let mut p = w.m(id).position;
        jitter(w, g.spread, &mut p);
        p[2] += g.lift;
        let a = w.rng.rand_angle();
        let s1 = w.rng.randf(1.0, 8.0) * super::DT;
        let s2 = w.rng.randf(2.0, 6.0) * super::DT;
        let s3 = w.rng.randf(0.0, g.k);
        let mut v1 = scale(dir, s3 * super::DT);
        let (cs_, sn) = cs(a);
        v1[0] += cs_ * s1;
        v1[1] += sn * s1;
        v1[2] += s2;
        let mut v2 = v1;
        let t30 = w.ticks(30);
        v2[2] -= 20.0 * super::DT2 * t30 as f32;
        for _ in 0..10 {
            let r = w.rng.randf(0.5, 1.5);
            v1[3] = r * 0.125;
            v2[3] = r * 0.065;
            jitter(w, g.blob_spread, &mut p);
            jitter(w, 0.5 * super::DT, &mut v2);
            let f = w.rng.randf(0.0, 1.0);
            let c1 = crate::particles::tween_color(f.to_bits(), 0x8000_eeee, 0x8000_ff90);
            let f = w.rng.randf(0.0, 1.0);
            let c2 = crate::particles::tween_color(f.to_bits(), 0x0000_ffee, 0x0000_ffee);
            let (ta, tb) = (w.ticks(10), w.ticks(30));
            let tc = w.svc.timing.scale(to_pf(w.rng.randf(5.0, 25.0))).to_f32() as i32;
            let a = crate::particles::type02::Spawn { pos: p, v1, v2, c1, c2, t: [ta, tb, tc], def: -1 };
            part02(w, &a);
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Body pieces (BreakFxB 0x278ad8, FxGroupUpdate 0x30cd18)

/// The class bounding sphere (class header +0x30) of `o_class`: [`super::Globals::class_spheres`], else the first
/// sequence's sphere.
fn class_sphere(w: &World, o_class: i16) -> [f32; 4] {
    if let Some(s) = w.svc.creatures.class_spheres.get(&o_class) { return *s; }
    w.classes.anim(o_class).and_then(|c| c.sequence(0)).map(|q| q.header.sphere).unwrap_or([0.0; 4])
}

fn euler_rows(e: V) -> [V; 3] {
    let r = rc_formats::moby_light::rotation_rows([e[0], e[1], e[2]]);
    r.map(|row| row.map(f32::from_bits))
}

/// `MatrixMulVec3(out, v, rows)` 0x2215e0: `r0·v.x + r1·v.y + r2·v.z`.
fn apply(rows: &[V; 3], v: V) -> V {
    std::array::from_fn(|k| if k == 3 { 0.0 } else { rows[0][k] * v[0] + rows[1][k] * v[1] + rows[2][k] * v[2] })
}

/// `BreakFxB(g, parent, class, pos, rot, timer, flags, vel, spin, sphere)` 0x278ad8 with the zero vectors the
/// creatures pass (random velocity, spin and the class sphere): [`break_piece_with`].
pub fn break_piece(w: &mut World, parent: MobyId, class: i16, p: V, rot: V, timer: i32, flags: u32) -> Option<MobyId> {
    break_piece_with(w, parent, class, p, rot, timer, flags, [0.0; 4], [0.0; 4], [0.0; 4])
}

/// `BreakFxB(g, parent, class, pos, rot, timer, flags, vel, spin, sphere)` 0x278ad8 (the same code in all 19
/// overlays: masked overlay-diff; on levels 00, 05–07, 13–16, 18 its words hash as cluster 494499ccf189, with two
/// padding words after `jr ra`): a piece moby of `class` at `pos`, rows from `rot`, scale `class scale · parent scale
/// / parent class scale`, the parent's light word and ambient, state 1, update distance 0xff, draw distance 0x80, no
/// collision; pvars: +0x30 timer (`timer`, or `trunc(randf(60, 120))` when 0), +0x34 flags (1 fade instead of
/// exploding, 2 the other explosion), +0x38 gravity `randf(10, 15)·dt²` (the game tests the new block's +0x38, always
/// 0, so `g` is never used), +0x00 velocity (`vel`, or `(cos a, sin a)·randf(2, 4)·dt, randf(5, 8)·dt` with a =
/// `rand_angle` when |vel| = 0), +0x10 spin (`spin`, or `rand_vec(dt·π, dt·2π)` when |spin| = 0), +0x20 the collision
/// sphere (`sphere`, or the class sphere · scale / 1024 when its length and w are 0). 3 to 8 draws.
#[allow(clippy::too_many_arguments)]
pub fn break_piece_with(w: &mut World, parent: MobyId, class: i16, p: V, rot: V, timer: i32, flags: u32, vel: V, spin: V, sphere: V) -> Option<MobyId> {
    let m = w.create_moby(class)?;
    let (pscale, pclass, light, ambient) = { let q = w.m(parent); (q.scale, q.o_class, q.light, q.ambient) };
    let pcs = w.classes.info(pclass).map(|c| c.scale).unwrap_or(1.0);
    let cs_ = w.classes.info(class).map(|c| c.scale).unwrap_or(1.0);
    let t = if timer == 0 {
        let x = w.rng.randf(60.0, 120.0);
        w.svc.timing.scale(to_pf(x)).to_f32() as i32
    } else {
        timer
    };
    let g = w.rng.randf(10.0, 15.0) * super::DT2;
    let v = if super::len3(vel) == 0.0 {
        let a = w.rng.rand_angle();
        let s = w.rng.randf(2.0, 4.0) * super::DT;
        let (c, sn) = cs(a);
        let vz = w.rng.randf(5.0, 8.0) * super::DT;
        [c * s, sn * s, vz, 0.0]
    } else {
        vel
    };
    let sp = if super::len3(spin) == 0.0 {
        let r = w.rng.rand_vec(super::DT * std::f32::consts::PI, super::DT * 2.0 * std::f32::consts::PI);
        [r[0], r[1], r[2], 0.0]
    } else {
        spin
    };
    let class_sph = (super::len3(sphere) == 0.0 && sphere[3] == 0.0).then(|| class_sphere(w, class));
    let mo = w.mm(m);
    mo.visible = 1;
    mo.mode |= mode::KEEP_ROWS;
    mo.scale = cs_ * (pscale / pcs);
    mo.light = light;
    mo.ambient = ambient;
    mo.state = 1;
    mo.position = p;
    let r = euler_rows(rot);
    mo.rows[..3].copy_from_slice(&r);
    mo.update_dist = 0xff;
    mo.draw_dist = 0x80;
    mo.has_collision = false;
    let k = mo.scale * (1.0 / 1024.0);
    let pv_ = &mut mo.pvars;
    pvar::set_i32(pv_, 0x30, t);
    pvar::set_u32(pv_, 0x34, flags);
    pvar::set_ff(pv_, 0x38, g);
    pvar::set_v4f(pv_, 0, v);
    pvar::set_v4f(pv_, 0x10, sp);
    let sph = match class_sph { Some(s) => [s[0] * k, s[1] * k, s[2] * k, s[3] * k], None => sphere };
    pvar::set_v4f(pv_, 0x20, sph);
    w.build_matrix(m);
    Some(m)
}

/// `FxGroupUpdate` 0x30cd18: state 1 flies (spin about the sphere centre, gravity, bounce off the world keeping the
/// speed, the spin re-aimed about `vel × n`) until the timer runs out, then state 2 explodes
/// (`SpawnBeamExplosion(0, 0, r, r/2, 9, r/2, 0, piece, pos, 5, 3, 5, −1, 0, 0)`, or `0x2742a8` with flag 2) and is
/// deleted, or state 3 (flag 1) fades its alpha over `scale(10)` ticks while still flying; state 0 is deleted, the
/// states above 3 fly.
pub fn piece_update(w: &mut World, id: MobyId) {
    let st = w.m(id).state;
    match st {
        1 => {
            if super::dec_timer_pvar_i32(w, id, 0x30) != 0 {
                let f = pvar::u32(&w.m(id).pvars, 0x34);
                w.mm(id).state = if f & 1 != 0 { 3 } else { 2 };
                let t = w.svc.timing.scale(to_pf(10.0)).to_f32() as i32;
                pvar::set_i32(&mut w.mm(id).pvars, 0x30, t);
                return;
            }
        }
        2 => {
            let rows = rows3(w, id);
            let c = add(apply(&rows, super::pv4(w, id, 0x20)), super::pos(w, id));
            let r = super::pf(w, id, 0x2c);
            if pvar::u32(&w.m(id).pvars, 0x34) & 2 != 0 {
                piece_explosion(w, r * 0.5, 0.0, Some(id), c);
            } else {
                let b = Beam { damage_r: 0.0, damage: 0.0, flash: r, flash2: r * 0.5, flash_dist: 9.0, scale: r * 0.5, light: 0.0, streaks: 5, sparks: 3, puffs: 5, debris: 0, sound: -1, shake: false };
                beam_explosion(w, &b, Some(id), c);
            }
            w.delete_moby(id);
            return;
        }
        3 => {
            if super::dec_timer_pvar_i32(w, id, 0x30) != 0 {
                w.delete_moby(id);
                return;
            }
            let d = w.svc.timing.scale(to_pf(10.0)).to_f32();
            let a = ((super::pi32(w, id, 0x30) as f32 / d) * 128.0) as i32;
            w.mm(id).alpha = a as u8;
        }
        // State 0 (a moby of a piece class not made by `BreakFxB`, which starts its pieces in 1): deleted at once. The
        // remains of Blarg's breakables (1656, 1661, 1665) run this update on level 6, so those breaks leave nothing.
        0 => {
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    fly(w, id);
}

fn dec_s16(w: &mut World, id: MobyId, o: usize) -> i32 { super::dec_timer_pvar_s16(w, id, o) }

fn rows3(w: &World, id: MobyId) -> [V; 3] { let r = &w.m(id).rows; [r[0], r[1], r[2]] }

fn fly(w: &mut World, id: MobyId) {
    let spin_e = super::pv4(w, id, 0x10);
    let s = euler_rows(spin_e);
    let sph = super::pv4(w, id, 0x20);
    let old = rows3(w, id);
    let a = apply(&old, sph);
    let new: [V; 3] = old.map(|r| apply(&s, r));
    for k in 0..3 { w.mm(id).rows[k] = [new[k][0], new[k][1], new[k][2], old[k][3]]; }
    let b = apply(&new, sph);
    let mut vel = super::pv4(w, id, 0);
    let p = add(sub(super::pos(w, id), sub(b, a)), vel);
    super::set_pos(w, id, p);
    vel[2] -= super::pf(w, id, 0x38);
    super::set_pv4(w, id, 0, vel);
    let c = add(apply(&new, sph), p);
    let r = super::pf(w, id, 0x2c);
    let Some(o) = w.coll_sphere(pv(c), to_pf(r), 0, Some(id)) else { return };
    let n = set_len3([o.normal[0], o.normal[1], o.normal[2], 0.0], 1.0);
    let l = len3(vel);
    let d = super::dot3(vel, n);
    if d >= 0.0 || d.is_nan() { return; }
    let rf = crate::moby_update::services::reflect(pv(vel), pv(n)).map(|x| f32::from_bits(x.0));
    let v2 = add(rf, scale(n, d * 0.5));
    let v2 = set_len3(v2, l);
    super::set_pv4(w, id, 0, v2);
    // FastVecCross(out, n, vel) = vel × n.
    let ax = [v2[1] * n[2] - v2[2] * n[1], v2[2] * n[0] - v2[0] * n[2], v2[0] * n[1] - v2[1] * n[0], 0.0];
    let ax = set_len3(ax, 1.0);
    let ang = len3(spin_e);
    let q = crate::moby_update::services::axis_angle_quat(to_pf(ang), pv(ax));
    let rows = crate::moby_update::services::quat_rows(q);
    let z = Pf::ZERO;
    let m4 = [rows[0], rows[1], rows[2], [z, z, z, Pf::ONE]];
    let e = crate::moby_update::services::rows_euler(&m4).map(|x| f32::from_bits(x.0));
    super::set_pv4(w, id, 0x10, [e[0], e[1], e[2], spin_e[3]]);
}

// -------------------------------------------------------------------------------------------------
// The explosion light (class 0x27f)

/// An explosion light template (the 0x50-byte records at 0x1b0770 / 0x1b06d0 / 0x1b0720 that 0x2f3570 copies).
#[derive(Clone, Copy, Debug)]
pub struct LightTemplate {
    /// +0x00: offset (w kept).
    pub offset: V,
    /// +0x10 bytes: intensity ×100 start (the light's `+0x24`), then r, g, b triples (start, max, min, up, down per
    /// channel in the game's byte order +0x11..+0x1f).
    pub bytes: [u8; 16],
    /// +0x20 / +0x24 / +0x28: radius start / max / min; +0x2c / +0x30 radius grow / shrink per tick.
    pub radius: [f32; 3],
    pub grow: f32,
    pub shrink: f32,
    /// +0x34: life in ticks (−1 forever); +0x38: flags.
    pub life: i32,
    pub flags: u32,
    /// +0x3c draw distance byte, +0x3d/+0x3e/+0x3f hold counts, +0x42/+0x44/+0x46 periods, +0x48 radius hold,
    /// +0x49 start delay.
    pub draw: u8,
    pub holds: [u8; 3],
    pub periods: [u16; 3],
    pub period_r: u16,
    pub hold_r: u8,
    pub delay: u8,
}

/// `0x1b0770`, the death explosion's light (`0x273f50` writes the three radii).
pub const LIGHT_DEATH: LightTemplate = LightTemplate {
    offset: [0.0, 0.0, 1.3, 0.0],
    bytes: [0x00, 0x00, 0xff, 0x00, 0x13, 0x0f, 0x00, 0xff, 0x00, 0x13, 0x11, 0x00, 0x64, 0x00, 0x13, 0x13],
    radius: [13.0, 13.0, 13.0],
    grow: 0.01,
    shrink: 0.01,
    life: 45,
    flags: 0x0008_0000,
    draw: 0xff,
    holds: [0, 0, 0],
    periods: [0, 0, 0],
    period_r: 0,
    hold_r: 0,
    delay: 0,
};

/// `0x1b06d0`, `SpawnBeamExplosion`'s light (normal colours; 0x1b0720 is the param_18 variant).
pub const LIGHT_BEAM: LightTemplate = LightTemplate {
    offset: [0.0, 0.0, 0.3, 0.0],
    bytes: [0x00, 0x00, 0xff, 0x00, 0x11, 0x0b, 0x00, 0xff, 0x00, 0x11, 0x0d, 0x00, 0x64, 0x00, 0x10, 0x0d],
    radius: [15.0, 15.0, 15.0],
    grow: 0.01,
    shrink: 0.01,
    life: 60,
    flags: 0x0008_0000,
    draw: 0xff,
    holds: [0, 0, 0],
    periods: [0, 0, 0],
    period_r: 0,
    hold_r: 0,
    delay: 0,
};

/// Template 0x1b0720: [`LIGHT_BEAM`] with its colour bytes for a shifted explosion (`param_18` ≠ 0: the gold chicken
/// 270's burst; level01 data).
pub const LIGHT_BEAM_GOLD: LightTemplate = LightTemplate {
    bytes: [0x00, 0x00, 0x80, 0x00, 0x11, 0x0d, 0x00, 0xff, 0x00, 0x11, 0x0b, 0x00, 0x64, 0x00, 0x10, 0x0d],
    ..LIGHT_BEAM
};

/// `0x20a8e0` = `0x20aa00`, the gold weapons' explosion light: the gold Mine Glove's mine (`0x2bfe40`) and the gold
/// Devastator's missile (`0x2c5b70`) (the bomb's layout, other colours).
pub const LIGHT_GOLD: LightTemplate = LightTemplate {
    bytes: [0x00, 0x00, 0x80, 0x00, 0x80, 0x0c, 0x00, 0xff, 0x00, 0x80, 0x09, 0x00, 0x64, 0x00, 0x80, 0x0d],
    ..LIGHT_BOMB
};

/// `0x20a930`, the Bomb Glove explosion's light (copied to the stack by `0x2c3300`, spawned at the explosion).
pub const LIGHT_BOMB: LightTemplate = LightTemplate {
    offset: [0.0, 0.0, 1.0, 0.0],
    bytes: [0x00, 0x00, 0xff, 0x00, 0x80, 0x09, 0x00, 0xff, 0x00, 0x80, 0x0c, 0x00, 0x64, 0x00, 0x80, 0x0d],
    radius: [20.0, 20.0, 20.0],
    grow: 0.01,
    shrink: 0.01,
    life: 70,
    flags: 0x0008_0000,
    draw: 0xff,
    holds: [0, 0, 0],
    periods: [0, 0, 0],
    period_r: 0,
    hold_r: 0,
    delay: 3,
};

/// The light moby's pvar offsets (0x2f3748's record).
mod lp {
    pub const OFFSET: usize = 0x00;
    pub const BASE: usize = 0x10;
    pub const PARENT: usize = 0x20;
    pub const R_MAX: usize = 0x38;
    pub const R_MIN: usize = 0x3c;
    pub const R_UP: usize = 0x40;
    pub const R_DOWN: usize = 0x44;
    pub const LIFE: usize = 0x48;
    pub const FLAGS: usize = 0x4c;
    pub const SLOT: usize = 0x50;
    pub const RGB: usize = 0x54;
    pub const RADIUS: usize = 0x60;
}

/// `0x2f3570(template, pos, 0, 0)`: `CreateMoby(0x27f)` hidden (mode |= 1), update distance 0xff, +0x31 = 1, at `pos`,
/// with the template copied into its pvars; then the matrix.
pub fn light_spawn(w: &mut World, t: &LightTemplate, p: V) -> Option<MobyId> {
    let m = w.create_moby(LIGHT_CLASS)?;
    let mo = w.mm(m);
    mo.update_dist = 0xff;
    mo.visible = 1;
    mo.mode |= 1;
    mo.state = 0;
    mo.cmd = 0;
    mo.position = p;
    mo.draw_dist = t.draw as i16;
    if mo.pvars.len() < 0x80 { mo.pvars.resize(0x80, 0); }
    let pv_ = &mut mo.pvars;
    pvar::set_v4f(pv_, lp::OFFSET, t.offset);
    pvar::set_i32(pv_, lp::PARENT, 0);
    pv_[0x24..0x34].copy_from_slice(&t.bytes);
    pvar::set_ff(pv_, 0x34, t.radius[0]);
    pvar::set_ff(pv_, lp::R_MAX, t.radius[1]);
    pvar::set_ff(pv_, lp::R_MIN, t.radius[2]);
    pvar::set_ff(pv_, lp::R_UP, t.grow);
    pvar::set_ff(pv_, lp::R_DOWN, t.shrink);
    pvar::set_i32(pv_, lp::LIFE, t.life);
    pvar::set_u32(pv_, lp::FLAGS, t.flags);
    pv_[0x68] = t.holds[0];
    pv_[0x69] = t.holds[1];
    pv_[0x6a] = t.holds[2];
    pv_[0x6b] = t.hold_r;
    pvar::set_i16(pv_, 0x6c, t.periods[0] as i16);
    pvar::set_i16(pv_, 0x6e, t.periods[1] as i16);
    pvar::set_i16(pv_, 0x70, t.periods[2] as i16);
    pvar::set_i16(pv_, 0x72, t.period_r as i16);
    pvar::set_i32(pv_, 0x64, -1);
    pv_[0x1c] = t.delay;
    w.build_matrix(m);
    w.svc.creatures.lights += 1;
    Some(m)
}

/// One colour channel of the light (`0x2f3748`'s three identical blocks). `b` = the channel's bytes (start, max, min,
/// up, down; ×100), `bits` = (down, hold, repeat, holding, random). Per tick: the period timer (when non-zero) restarts
/// the ramp (down cleared, the value set to the *min byte* unscaled, a game quirk kept); with the random bit the value is
/// `randf(min, max)/100` (only without a period); else it ramps up by `up/100` to `max/100` (then holds for the hold
/// byte's ticks when the hold bit is set), down by `down/100` to `min/100` (turning up again with the repeat bit when
/// there is no period).
#[allow(clippy::too_many_arguments)]
fn channel(w: &mut World, id: MobyId, flags: &mut u32, v: &mut f32, b: [u8; 5], period: u16, timer_off: usize, hold_off: usize, bits: [u32; 5]) {
    let [_start, max, min, up, down] = b;
    let [b_down, b_hold, b_repeat, b_holding, b_random] = bits;
    if period != 0 {
        let mut t = super::pi16(w, id, timer_off);
        let r = crate::moby_update::services::fast_dec_timer_s16(&mut t);
        super::set_pi16(w, id, timer_off, t);
        if r != 0 {
            let p = w.ticks(period as i32);
            super::set_pi16(w, id, timer_off, p as i16);
            *flags &= !b_down;
            *v = min as f32;
        }
    }
    if *flags & b_random != 0 {
        if period == 0 { *v = w.rng.randf(min as f32 / 100.0, max as f32 / 100.0); }
        return;
    }
    if *flags & (b_hold | b_holding) == b_holding { *flags ^= b_holding; }
    if *flags & b_holding != 0 {
        if super::pu8(w, id, hold_off) == 0 { return; }
        let mut h = super::pu8(w, id, hold_off + 0x14);
        let r = crate::moby_update::services::fast_dec_timer_u8(&mut h);
        super::set_pu8(w, id, hold_off + 0x14, h);
        if r != 0 { *flags ^= b_holding; }
        return;
    }
    if *flags & b_down == 0 {
        *v += (up as f32 / 100.0) * super::SPEED;
        let top = max as f32 / 100.0;
        if top <= *v {
            *v = top;
            if *flags & b_hold == 0 { *flags ^= b_down; } else {
                let hold = super::pu8(w, id, hold_off);
                let t = w.ticks(hold as i32) as u8;
                super::set_pu8(w, id, hold_off + 0x14, t);
                *flags = (*flags | b_holding) ^ b_down;
            }
        }
    } else {
        *v -= (down as f32 / 100.0) * super::SPEED;
        let bot = min as f32 / 100.0;
        if *v <= bot {
            *v = bot;
            if *flags & b_repeat != 0 && period == 0 { *flags ^= b_down; }
        }
    }
}

/// `0x2f3748`: the explosion light (class 0x27f). State 0 keeps its start position, radius and colours (w = −1 slot);
/// then per tick: the start delay (+0xbc), the life timer (deleted when it runs out), the view test
/// (`FastBSphereCheck(255, pos, radius)`; out of view the light slot is freed and nothing else runs), the position
/// (follows the parent, plus the offset: fixed, `randf(0, 1)`-scaled (flag 0x100000) or `randf(±offset)` (0x300000)),
/// the radius (grow/shrink between its limits, or `randf(min, max)` with flag 0x80000) and the colour channels; then
/// its point-light slot (+0x50, −1 none): taken with `WritePointLight_B` 0x252750 when it has none, else rewritten
/// (colour, intensity +0x24 / 100, position, radius); freed on the view cull and at the end of its life
/// ([`crate::point_lights`]).
pub fn light_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x80 { return; }
    if w.m(id).state == 0 {
        let p = super::pos(w, id);
        super::set_pv4(w, id, lp::BASE, p);
        let radius0 = super::pf(w, id, 0x34);
        super::set_pf(w, id, lp::RADIUS, radius0);
        for (k, o) in [0x25usize, 0x2a, 0x2f].iter().enumerate() {
            let v = super::pu8(w, id, *o) as f32 / 100.0;
            super::set_pf(w, id, lp::RGB + 4 * k, v);
        }
        let d = super::pu8(w, id, 0x1c);
        w.mm(id).cmd = d;
        pvar::set_i32(&mut w.mm(id).pvars, lp::SLOT, -1);
        w.mm(id).state = 1;
    }
    let c = w.m(id).cmd;
    if c != 0 {
        w.mm(id).cmd = c - 1;
        return;
    }
    if super::pi32(w, id, lp::LIFE) != -1 && super::dec_timer_pvar_i32(w, id, lp::LIFE) != 0 {
        free_slot(w, id);
        w.delete_moby(id);
        return;
    }
    let mut radius = super::pf(w, id, lp::RADIUS);
    let p = super::pos(w, id);
    if !in_view(w, 255.0, p, radius) {
        free_slot(w, id);
        return;
    }
    let mut flags = pvar::u32(&w.m(id).pvars, lp::FLAGS);
    let base = super::pv4(w, id, lp::BASE);
    if flags & 0x80_0000 == 0 {
        let off = super::pv4(w, id, lp::OFFSET);
        let o = if flags & 0x10_0000 == 0 {
            [off[0], off[1], off[2], 0.0]
        } else if flags & 0x20_0000 == 0 {
            let k = w.rng.randf(0.0, 1.0);
            scale([off[0], off[1], off[2], 0.0], k)
        } else {
            let x = w.rng.randf(-off[0], off[0]);
            let y = w.rng.randf(-off[1], off[1]);
            let z = w.rng.randf(-off[2], off[2]);
            [x, y, z, 0.0]
        };
        let np = add(base, o);
        super::set_pos(w, id, [np[0], np[1], np[2], p[3]]);
    } else {
        super::set_pos(w, id, [base[0], base[1], base[2], p[3]]);
    }
    // Radius period (+0x72, timer +0x7a): restart at the minimum, going up.
    let period_r = super::pi16(w, id, 0x72) as u16;
    let mut reset = false;
    if period_r != 0 && dec_s16(w, id, 0x7a) != 0 {
        let t = w.ticks(period_r as i32);
        super::set_pi16(w, id, 0x7a, t as i16);
        if flags & 4 != 0 {
            reset = true;
            radius = super::pf(w, id, lp::R_MIN);
            flags &= !8;
        }
    }
    // Radius (bits 1 hold, 2 holding, 4 repeat, 8 down, 0x80000 random).
    if flags & 0x8_0000 != 0 {
        if period_r == 0 || reset { radius = w.rng.randf(super::pf(w, id, lp::R_MIN), super::pf(w, id, lp::R_MAX)); }
    } else {
        if flags & 3 == 2 { flags ^= 2; }
        if flags & 2 == 0 {
            if flags & 8 == 0 {
                radius += super::pf(w, id, lp::R_UP) * super::SPEED;
                let mx = super::pf(w, id, lp::R_MAX);
                if mx <= radius {
                    radius = mx;
                    if flags & 1 == 0 { flags ^= 8; } else {
                        let h = super::pu8(w, id, 0x6b);
                        let t = w.ticks(h as i32) as u8;
                        super::set_pu8(w, id, 0x7f, t);
                        flags = (flags | 2) ^ 8;
                    }
                }
            } else {
                radius -= super::pf(w, id, lp::R_DOWN) * super::SPEED;
                let mn = super::pf(w, id, lp::R_MIN);
                if radius <= mn {
                    radius = mn;
                    if flags & 4 != 0 && period_r == 0 { flags ^= 8; }
                }
            }
        } else if super::pu8(w, id, 0x6b) != 0 {
            let mut h = super::pu8(w, id, 0x7f);
            if crate::moby_update::services::fast_dec_timer_u8(&mut h) != 0 { flags ^= 2; }
            super::set_pu8(w, id, 0x7f, h);
        }
    }
    super::set_pf(w, id, lp::RADIUS, radius);
    // Colour channels: r (bits 0x10..0x80, random 0x10000), g (0x100..0x800, 0x20000), b (0x1000..0x8000, 0x40000).
    type Chan = ([usize; 5], u16, usize, usize, [u32; 5]);
    let chans: [Chan; 3] = [
        ([0x25, 0x26, 0x27, 0x28, 0x29], super::pi16(w, id, 0x6c) as u16, 0x74, 0x68, [0x80, 0x10, 0x40, 0x20, 0x1_0000]),
        ([0x2a, 0x2b, 0x2c, 0x2d, 0x2e], super::pi16(w, id, 0x6e) as u16, 0x76, 0x69, [0x800, 0x100, 0x400, 0x200, 0x2_0000]),
        ([0x2f, 0x30, 0x31, 0x32, 0x33], super::pi16(w, id, 0x70) as u16, 0x78, 0x6a, [0x8000, 0x1000, 0x4000, 0x2000, 0x4_0000]),
    ];
    for (k, (offs, period, toff, hoff, bits)) in chans.iter().enumerate() {
        let b = offs.map(|o| super::pu8(w, id, o));
        let mut v = super::pf(w, id, lp::RGB + 4 * k);
        channel(w, id, &mut flags, &mut v, b, *period, *toff, *hoff, *bits);
        super::set_pf(w, id, lp::RGB + 4 * k, v);
    }
    pvar::set_u32(&mut w.mm(id).pvars, lp::FLAGS, flags);
    // The slot: taken or rewritten with this tick's colour, intensity, position and radius.
    let l = crate::point_lights::PointLight {
        color: [0, 1, 2].map(|k| super::pf(w, id, lp::RGB + 4 * k)),
        intensity: super::pu8(w, id, 0x24) as f32 / 100.0,
        pos: { let q = super::pos(w, id); [q[0], q[1], q[2]] },
        radius: super::pf(w, id, lp::RADIUS),
    };
    let slot = super::pi32(w, id, lp::SLOT);
    if slot == -1 {
        let load = f32::from_bits(w.svc.frame_load[1].0);
        let got = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i32);
        pvar::set_i32(&mut w.mm(id).pvars, lp::SLOT, got);
    } else {
        w.svc.point_lights.set(slot as usize, l);
    }
}

/// `FreePointLight` of the light moby's slot (+0x50), which becomes −1.
fn free_slot(w: &mut World, id: MobyId) {
    let slot = super::pi32(w, id, lp::SLOT);
    if slot != -1 {
        w.svc.point_lights.free(slot as usize);
        pvar::set_i32(&mut w.mm(id).pvars, lp::SLOT, -1);
    }
}

/// The melee creatures' death sparks (level10 `0x2e3a90` Orxon's brawler 1202, level12 `0x2e3098` Hoven's burrower
/// 238; the same code, their gp words equal): `clumps` clumps from the position jittered 0.5 and 0.55 up (`dying`:
/// speeds × 0.55, 0.2 lower), `randf(1, 5)` out and `randf(2, 5)` up per second plus half the knockback velocity (the
/// record at pvar `k`), the second velocity falling 20·dt² over `ticks(30)`; 8 type-2 blobs each (r = `randf(0.5,
/// 1.5)`, sizes 0.125·r / 0.065·r, jitter 0.2 / 0.5·dt, colours 0x8000eeee–0x8000ff90 / 0xffee, `ticks(10)`,
/// `ticks(30)`, `scale(randf(5, 25))`).
pub fn clump_sparks(w: &mut World, id: MobyId, k: usize, clumps: i32, dying: bool) {
    let (dt, dt2) = (super::DT, super::DT2);
    let kv = scale(super::pv4(w, id, k), 0.5);
    for _ in 0..clumps {
        let mut p = super::pos(w, id);
        jitter(w, 0.5, &mut p);
        p[2] += 0.55;
        let a = w.rng.rand_angle();
        let mut s1 = w.rng.randf(1.0, 5.0);
        let mut s2 = w.rng.randf(2.0, 5.0);
        if dying {
            s1 *= 0.55;
            s2 *= 0.55;
            p[2] -= 0.2;
        }
        let (c, sn) = cs(a);
        let mut v1 = [c * s1 * dt + kv[0], sn * s1 * dt + kv[1], s2 * dt + 0.0 + kv[2], 0.0];
        let mut v2 = v1;
        let t30 = w.ticks(30);
        v2[2] -= 20.0 * dt2 * t30 as f32;
        for _ in 0..8 {
            let r = w.rng.randf(0.5, 1.5);
            v1[3] = r * 0.125;
            v2[3] = r * 0.065;
            jitter(w, 0.2, &mut p);
            jitter(w, 0.5 * dt, &mut v2);
            let f = w.rng.randf(0.0, 1.0);
            let c1 = crate::particles::tween_color(f.to_bits(), 0x8000_eeee, 0x8000_ff90);
            let f = w.rng.randf(0.0, 1.0);
            let c2 = crate::particles::tween_color(f.to_bits(), 0x0000_ffee, 0x0000_ffee);
            let (ta, tb) = (w.ticks(10), w.ticks(30));
            let tc = w.svc.timing.scale(Pf::f(w.rng.randf(5.0, 25.0))).to_f32() as i32;
            part02(w, &crate::particles::type02::Spawn { pos: p, v1, v2, c1, c2, t: [ta, tb, tc], def: -1 });
        }
    }
}

#[cfg(test)]
mod tests {
    //! The explosion routines' rows (docs/plan/explosions.md §A, §C, §F).
    use super::*;
    use crate::hero::Hero;
    use crate::moby_runtime::{ClassInfo, Moby, MobyTable};
    use crate::moby_update::classes::debris::{flash_spawn_as, FLASH_BOMB, FLASH_SHELL};
    use crate::moby_update::services::{ClassTable, Services};
    use crate::rng::Rng;

    const SRC_CLASS: i16 = 577;

    fn run(f: impl FnOnce(&mut World)) -> (MobyTable, Services) {
        let mut ct = ClassTable::default();
        for (slot, oc) in [(1u8, 0x70i16), (2, 1192), (3, LIGHT_CLASS), (4, 122), (5, SRC_CLASS)] {
            ct.classes.insert(oc, (ClassInfo { slot, scale: 1.0, ..Default::default() }, None));
        }
        let mut src = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        src.o_class = SRC_CLASS;
        src.position = [100.0, 100.0, 50.0, 1.0];
        let mut table = MobyTable::new(vec![src], 400);
        let hero = Hero::new();
        let mut rng = Rng::new();
        let mut svc = Services::new();
        {
            let mut w = World::new(&mut table, &hero, &mut rng, &ct, &mut svc, 1);
            w.camera = [Pf::f(130.0), Pf::f(100.0), Pf::f(50.0), Pf::ONE];
            f(&mut w);
        }
        (table, svc)
    }

    fn flashes(t: &MobyTable) -> Vec<(f32, [u8; 4])> {
        t.mobys.iter().filter(|m| m.o_class == 0x70 && m.state < 0x80).map(|m| (p::ff(&m.pvars, 0x18), [m.ambient[0], m.ambient[1], m.ambient[2], m.alpha])).collect()
    }

    use crate::moby_update::services::pvar as p;

    /// §F: the two flash spawners are one code: `0x2c20e0` (class 0x70) starts `ticks(4)` in (timer T − 4, the size
    /// `4·full/T`), `0x309a68` (class 1192) at T with size 0; both take the whole position and the vector.
    #[test]
    fn flash_kinds() {
        let (t, svc) = run(|w| {
            flash_spawn_as(w, &FLASH_SHELL, 2.0, 0, [1.0, 2.0, 3.0, 0.5], [0.0, 0.0, 0.25, 0.0], 20, 1, 2, 3, 0x30);
            flash_spawn_as(w, &FLASH_BOMB, 2.0, 0, [1.0, 2.0, 3.0, 0.5], [0.0, 0.0, 0.25, 0.0], 20, 1, 2, 3, 0x30);
        });
        let a = t.mobys.iter().find(|m| m.o_class == 0x70 && m.state < 0x80).unwrap();
        let b = t.mobys.iter().find(|m| m.o_class == 1192 && m.state < 0x80).unwrap();
        assert_eq!((p::i16(&a.pvars, 0x1c), a.scale), (16, 4.0 * 2.0 / 20.0));
        assert_eq!((p::i16(&b.pvars, 0x1c), b.scale), (20, 0.0));
        for m in [a, b] {
            assert_eq!((m.position, p::v4f(&m.pvars, 0), p::i32(&m.pvars, 0x14), p::ff(&m.pvars, 0x18), p::i16(&m.pvars, 0x1e)), ([1.0, 2.0, 3.0, 0.5], [0.0, 0.0, 0.25, 0.0], 20, 2.0, 0x30));
            assert_eq!(([m.ambient[0], m.ambient[1], m.ambient[2]], m.alpha, p::i32(&m.pvars, 0x10)), ([1, 2, 3], 0x30, 1));
        }
        assert_eq!(svc.fx.flashes, 2);
    }

    /// §C `0x273f50`: the flashes 4·s (0x7f, 0x40, 0) / 3·s (0x60, 0x20, 0), the shake `s·0.1` for `ticks(20)`, the sound,
    /// the light (radius 13 for a negative size).
    #[test]
    fn spark_burst_death_row() {
        let (t, svc) = run(|w| death_explosion(w, 0.5, -1.0, Some(0), [100.0, 100.0, 50.0, 1.0], 4));
        assert_eq!(flashes(&t), vec![(2.0, [0x7f, 0x40, 0, 0x30]), (1.5, [0x60, 0x20, 0, 0x20])]);
        assert_eq!(svc.camera_shakes.len(), 1);
        assert_eq!((svc.camera_shakes[0].amp, svc.camera_shakes[0].ticks), (0.05, 20));
        assert_eq!(svc.sounds.iter().filter(|s| s.index == 4).count(), 1);
        let l = t.mobys.iter().find(|m| m.o_class == LIGHT_CLASS && m.state < 0x80).expect("the light");
        assert_eq!(p::ff(&l.pvars, 0x34), 13.0);
    }

    /// §C `0x2742a8` (divergence fixed 2026-09-29): its colours go through `0x270fa8` / `0x270f48` with shift **1**
    /// (a literal `li a1, 1` / `li a3, 1`), so its flashes are (0x40, 0x7f, 0) and (0x20, 0x60, 0), not the death
    /// explosion's; no shake, no sound, no light for 0.
    #[test]
    fn spark_burst_piece_row_swaps_red_and_green() {
        let (t, svc) = run(|w| piece_explosion(w, 1.0, 0.0, Some(0), [100.0, 100.0, 50.0, 1.0]));
        assert_eq!(flashes(&t), vec![(4.0, [0x40, 0x7f, 0, 0x30]), (3.0, [0x20, 0x60, 0, 0x20])]);
        assert!(svc.camera_shakes.is_empty() && svc.sounds.is_empty());
        assert!(t.mobys.iter().all(|m| m.o_class != LIGHT_CLASS || m.state >= 0x80));
        assert_eq!(colour_shift(SPARK_A[0], PIECE_BURST.shift), 0x4f00_ff8f, "the sparks: red and green swapped");
    }

    /// §C `0x2fa1b0` (the path enemies' glob): flashes 2·s (0x7f, 0, 0x40, 0x30) / 1.5·s (0x20, 0, 0x20, 0), the
    /// sound, no shake.
    #[test]
    fn spark_burst_shot_row() {
        let (t, svc) = run(|w| spark_burst(w, &SHOT_BURST, 0.5, 0.0, Some(0), [100.0, 100.0, 50.0, 1.0], 0));
        assert_eq!(flashes(&t), vec![(1.0, [0x7f, 0, 0x40, 0x30]), (0.75, [0x20, 0, 0x20, 0])]);
        assert!(svc.camera_shakes.is_empty());
        assert_eq!(svc.sounds.len(), 1);
    }

    /// §A `SpawnBeamExplosion` (divergence fixed 2026-09-29): the flashes take the sparks' base vector
    /// `(0, 0, 8·dt·scale)` (sp+0x1b0) as their +0x00.
    #[test]
    fn beam_flashes_carry_the_base_vector() {
        let b = Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.5, light: 0.0, streaks: 0, sparks: 0, puffs: 0, debris: 0, sound: -1, shake: false };
        let (t, _) = run(|w| beam_explosion(w, &b, Some(0), [100.0, 100.0, 50.0, 1.0]));
        let v: Vec<[f32; 4]> = t.mobys.iter().filter(|m| m.o_class == 0x70 && m.state < 0x80).map(|m| p::v4f(&m.pvars, 0)).collect();
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|x| *x == [0.0, 0.0, super::super::DT * 8.0 * 1.5, 0.0]), "{v:?}");
    }

    /// §A the Novalis crash (`0x30c190`, divergence fixed 2026-09-29): the first `SpawnBeamExplosion` passes
    /// param_16 = 60, so 60 fireballs 122 fly out of it (the one toward the camera only within 14; here 30 away: 59).
    #[test]
    fn crash_beam_throws_sixty_fireballs() {
        use crate::moby_update::classes::cutscene_fx::{CRASH_BEAM, CRASH_BEAM2};
        assert_eq!((CRASH_BEAM.debris, CRASH_BEAM2.debris), (60, 0));
        let (t, _) = run(|w| beam_explosion(w, &CRASH_BEAM, Some(0), [100.0, 100.0, 50.0, 1.0]));
        assert_eq!(t.mobys.iter().filter(|m| m.o_class == 122 && m.state < 0x80).count(), 59);
        let (t, _) = run(|w| { w.camera = [Pf::f(105.0), Pf::f(100.0), Pf::f(50.0), Pf::ONE]; beam_explosion(w, &CRASH_BEAM, Some(0), [100.0, 100.0, 50.0, 1.0]) });
        assert_eq!(t.mobys.iter().filter(|m| m.o_class == 122 && m.state < 0x80).count(), 60);
    }
}
