//! The shared creature and enemy layer of the game's engine code (level01 addresses; every function here is in
//! the ~1375 engine functions each level overlay links, `tools/ghidra/names/clusters.tsv`). The per-class creature updates
//! (`classes::critter` 577, `classes::amoeboid` 572/865/866, …) are thin state machines over these, exactly as the
//! game's class code calls its shared helpers. Spec and the class status: `docs/plan/creatures.md`.
//!
//! | module | game functions | what |
//! |---|---|---|
//! | [`target`] | `0x274b78`, `0x274df8` | target acquisition: Ratchet (or a decoy nearer than the range), none in his cutscene states |
//! | [`turn`] | `SpringTurn2` 0x26d058, `0x270cc0` / 0x270ac0 / 0x2709f8, `Approach` 0x270728 | turning towards a heading, eased approach |
//! | [`walker`] | `SeedJumpPattern` 0x26d930, `0x26d9a8`, `0x26d8b0` / `0x26d610`, `0x26d1d0`, `0x26de80`, `0x26d270`, level00 `0x261630` | ground walking: speed ramp, ledge probe, moby-vs-world move with step-up and gravity, the walk toward a point, the free ground move, the random wander |
//! | [`knock`] | `0x271418`, `0x271558`, `0x26fa48`, `0x26faf0` | knockback / thrown / death flight: ballistic move, landing, bounce, crate hits, anim timing; `0x250a78` the ballistic arc |
//! | [`flame`] | level02 `0x264e70` / `0x264e40` / `0x2651d0` (level16 `0x25eab8` / `0x25ea88` / `0x25ee18`) | the fire-spraying creatures' flame emitter: type-12 flames, kept flames' hits, smoke sparks |
//! | [`legs`] | level04 `0x2922d0`..`0x295fe8` (L01 `0x2b5160`..) | the leg walker: planted feet, hips, ground fit, gaits from the step ahead |
//! | [`damage`] | `0x26f378` (+ `MobyGetHitMessage` 0x26f320 in `services`) | the hit resolver: per-attack-kind cooldown, damage tables, the wrench's push redirect |
//! | [`attack`] | `0x26eaa8`, `0x26e090`, `0x26e830`, level00 `0x2599e8` | hits dealt (to Ratchet through his moby's hit message, P2's intake), group alert, the sphere hit ahead of a moby (at a joint) |
//! | [`flash`] | `0x272318` / `0x2723f8` | the hit flash on the moby's ambient colour |
//! | [`fx`] | `0x273f50`, `SpawnBeamExplosion` 0x273310, `BreakFxB` 0x278ad8, `0x2efbf8`, `0x2f3570` | death explosion, explosion effect, break pieces, rate limiter, explosion light |
//! | [`projectile`] | `CollLine_Fix` / `coll_sphere_mobys` with a template, `PartType04/08/15/16/22/26Spawn` | enemy shots' sweep and hit template; particle records of the unported types |
//! | [`region`] | `0x26e6c0`, `0x276640`, `ClampToPath` 0x276820, `0x276a48`, `0x276c40`, `LineOfSightTest` 0x276fe8 | arena polygons and waypoint graphs on the level paths |
//! | [`react`] | the class reaction table (`lvl.vtbl` word 3), `0x304168` / `0x304390` / `0x3044a0` / `0x304690` / `0x305260` / `0x3051a8` / `0x304798` | how the Suck Cannon (and only it) acts on a class; the damage record's lure |
//! | [`ground`] | `GroundHeight` 0x26e618 + `CollType` 0x2151d8, `0x2765b0`, `MobyAnimKeyTime` 0x263920 | ground probe with surface id, anim-frame crossing |
//!
//! Bolt drops are `SetDeathBits` 0x26c250 → `BoltBurst` 0x275988 ([`crate::moby_update::classes::crate_::set_death_bits`],
//! reused); hits reach Ratchet through the moby hit log and P2's `hit_intake` (`crate::hero::damage`); debris flashes are
//! [`crate::moby_update::classes::debris::flash_spawn`]; the TNT-style sparks are `PartType11Spawn` ([`World::part11`]).
//!
//! **The creature pvar header.** Classes with mode 0x20 start their pvar block with self-relative pointers (pointer
//! fixups; the port keeps them as offsets into the block, `rc_formats::gameplay::parse_pvars`): +0x00 the damage
//! record (`FUN_002711f8`; its f32 +0 is the health), +0x0c the hit flash, +0x10 the knockback record
//! (`FUN_00271228`), +0x14 an extra record, +0x18 the walker. [`header`] reads them.
//!
//! Arithmetic: standard `f32` throughout (no PS2 float model); formulas, constants and operation order are the
//! game's. The rand draws are the game's, at the game's points.

pub mod attack;
pub mod damage;
pub mod flash;
pub mod flame;
pub mod fx;
pub mod ground;
pub mod knock;
pub mod legs;
pub mod projectile;
pub mod react;
pub mod region;
pub mod target;
pub mod turn;
pub mod walker;

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar, World};
use std::f32::consts::PI;

/// `0x15ed6c` dt (NTSC) and `0x15ed70` dt².
pub const DT: f32 = 1.0 / 60.0;
pub const DT2: f32 = DT * DT;
/// `0x15ed60`: the speed multiplier (1.0 NTSC).
pub const SPEED: f32 = 1.0;

/// Game globals the creature layer keeps ([`crate::moby_update::Services::creatures`]).
#[derive(Clone, Debug, Default)]
pub struct Globals {
    /// `0x161a80` s32[3]: the critter death explosions' busy-until ticks ([`fx::rate_slot`]).
    pub death_fx_slots: [i32; 3],
    /// Class header +0x30 (bounding sphere, packed units) by class, for the body pieces' collision sphere
    /// (`BreakFxB`); the loader fills it from the class blobs. A class missing here uses its sequence 0 sphere.
    pub class_spheres: std::collections::HashMap<i16, [f32; 4]>,
    /// Explosion lights created (stats).
    pub lights: u64,
    /// `0x15f404`: an enemy's scripted sequence is running (set by the gunship 688's fly-by and a trooper's camera
    /// cuboid, cleared at their end); the gunship's shells then hit mobys and make fewer particles.
    pub cutscene: bool,
    /// The scripted sequences' calls into the camera and the hero, in the order the game makes them (the game calls
    /// `CameraScript` / `SetState` directly): the cutscene system and the hero read and clear this queue.
    pub scripts: Vec<ScriptRequest>,
    /// The weapons' reaction layer ([`react::Globals`]: the Suck Cannon's slots, the reaction tables, the chickens).
    pub react: react::Globals,
}

/// A call an enemy's scripted sequence makes into the camera and the hero ([`Globals::scripts`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScriptRequest {
    /// `SetState(hero_state, 0)` on Ratchet, then `CameraScript(cuboid centre, cuboid Euler, 0, 0, 0)`: the camera
    /// placed at cuboid `cuboid` (its centre, looking along its Euler angles) for `ticks` ticks, by moby `moby`.
    Start { moby: MobyId, hero_state: u8, cuboid: i32, centre: [f32; 3], euler: [f32; 3], ticks: i32 },
    /// `CameraScript2(0)` (the camera back to Ratchet) and `SetState(0, 1)`.
    End { moby: MobyId },
}

/// Mode bit 0x20: the pvar block starts with the header pointers (`FUN_002711f8` / `FUN_00275290` test it).
pub const PVAR_HEADER: u16 = 0x20;

/// The creature pvar header (mode 0x20; module doc): the pvar offsets its pointers hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub damage: Option<usize>,
    pub flash: Option<usize>,
    pub knock: Option<usize>,
    pub extra: Option<usize>,
    pub walker: Option<usize>,
}

/// The header of moby `id` (all `None` without mode 0x20 or with a zero pointer).
pub fn header(w: &World, id: MobyId) -> Header {
    let m = w.m(id);
    if m.mode & PVAR_HEADER == 0 || m.pvars.len() < 0x1c { return Header::default(); }
    let p = |o: usize| -> Option<usize> { let v = pvar::u32(&m.pvars, o) as usize; (v != 0 && v < m.pvars.len()).then_some(v) };
    Header { damage: p(0), flash: p(0xc), knock: p(0x10), extra: p(0x14), walker: p(0x18) }
}

// -------------------------------------------------------------------------------------------------
// Math on f32 (the engine's VU0 / FPU helpers, level01 0x2210f0..0x222100)

/// `FastArcTan(a, b)` 0x2217c0 = `atan2(b, a)` (0 for the zero vector).
pub fn atan(a: f32, b: f32) -> f32 { if a == 0.0 && b == 0.0 { 0.0 } else { b.atan2(a) } }

fn wrap_once(s: f32) -> f32 {
    let below = s < -PI;
    if s >= PI || s.is_nan() { return (s - PI) - PI; }
    if below { return (s + PI) + PI; }
    s
}
/// `fast_add_rotations` 0x221ff8: `a + b` wrapped once into [−π, π).
pub fn add_rot(a: f32, b: f32) -> f32 { wrap_once(a + b) }
/// `fast_subtract_rotations` 0x222040: `a − b` wrapped once.
pub fn sub_rot(a: f32, b: f32) -> f32 { wrap_once(a - b) }
/// `0x25b750` (level00; cluster a3aa13a8e1c8 on levels 00, 04, 06, 10, 12, 14, 18, not linked on level 01): the angle
/// `t` of the way from `a` to `b`, `add_rot(a, sub_rot(b, a)·t)` ([`walker::wander`]'s turn away from Ratchet).
pub fn lerp_rot(a: f32, b: f32, t: f32) -> f32 { add_rot(a, sub_rot(b, a) * t) }
/// `FastDiffRots` 0x222100: the unsigned angle between `a` and `b` (`|a − b|`, `2π − ` it when ≥ π).
pub fn diff_rots(a: f32, b: f32) -> f32 {
    let d = (a - b).abs();
    if PI <= d { 2.0 * PI - d } else { d }
}

pub type V = [f32; 4];

pub fn add(a: V, b: V) -> V { [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]] }
pub fn sub(a: V, b: V) -> V { [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]] }
/// `VecScale` 0x221210: every lane times `s`.
pub fn scale(v: V, s: f32) -> V { v.map(|x| x * s) }
pub fn dot3(a: V, b: V) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
/// `FastVecLength` 0x2212e8.
pub fn len3(v: V) -> f32 { dot3(v, v).sqrt() }
/// `fun_001f9b20` 0x221318: the xy length.
pub fn len2(v: V) -> f32 { (v[0] * v[0] + v[1] * v[1]).sqrt() }
/// `VecDistance` 0x221360.
pub fn dist3(a: V, b: V) -> f32 { len3(sub(a, b)) }
/// `VecDistance2` 0x221398: the xy distance.
pub fn dist2(a: V, b: V) -> f32 { len2(sub(a, b)) }
/// `FastVecNormalize(l, out, v)` 0x221410: `v.xyz` scaled to 3-D length `l` (`vmulq.xyz`), w kept; the zero vector
/// gives xyz = `vf0 + vf0` = 0, w kept.
pub fn set_len3(v: V, l: f32) -> V {
    let n = len3(v);
    if n == 0.0 { return [0.0, 0.0, 0.0, v[3]]; }
    let q = l / n;
    [v[0] * q, v[1] * q, v[2] * q, v[3]]
}
/// `fun_001f9c48(l, out, v)` 0x221460: `v.xy` scaled so its xy length is `l` (`vmulq.xy`), z and w kept; zero xy
/// gives xy = 0, z and w kept (a knocked flight whose drag stopped it still falls).
pub fn set_len2(v: V, l: f32) -> V {
    let n = len2(v);
    if n == 0.0 { return [0.0, 0.0, v[2], v[3]]; }
    let q = l / n;
    [v[0] * q, v[1] * q, v[2], v[3]]
}
/// `FUN_002745f0(max, v)`: `v` scaled down to 3-D length `max` when longer.
pub fn clamp_len3(v: V, max: f32) -> V { if max < len3(v) { set_len3(v, max) } else { v } }
/// `(cos a, sin a)`.
pub fn cs(a: f32) -> (f32, f32) { (a.cos(), a.sin()) }

/// `ticks(n)` 0x220e30 (NTSC: `n`).
pub fn ticks(w: &World, n: i32) -> i32 { w.ticks(n) }

/// `FastDecTimer__FRi` 0x220e78 on an i32: 1 when it is 0, else `t = max(t, 1) − 1` and 2 when that is ≤ 0, 0
/// otherwise (the s16 variant is `services::fast_dec_timer_s16`).
pub fn dec_timer_i32(t: &mut i32) -> i32 {
    if *t == 0 { return 1; }
    let n = (*t).max(1) - 1;
    *t = n;
    if n < 1 { 2 } else { 0 }
}

/// A pvar i32 timer through [`dec_timer_i32`].
pub fn dec_timer_pvar_i32(w: &mut World, id: MobyId, o: usize) -> i32 {
    let p = &mut w.mm(id).pvars;
    let mut t = pvar::i32(p, o);
    let r = dec_timer_i32(&mut t);
    pvar::set_i32(p, o, t);
    r
}

/// A pvar s16 timer through `FastDecTimer__FRs` 0x220ea8.
pub fn dec_timer_pvar_s16(w: &mut World, id: MobyId, o: usize) -> i32 {
    let p = &mut w.mm(id).pvars;
    let mut t = pvar::i16(p, o);
    let r = crate::moby_update::services::fast_dec_timer_s16(&mut t);
    pvar::set_i16(p, o, t);
    r
}

/// Moby position / Euler as native vectors.
pub fn pos(w: &World, id: MobyId) -> V { w.m(id).position }
pub fn set_pos(w: &mut World, id: MobyId, p: V) { w.mm(id).position = p; }
pub fn yaw(w: &World, id: MobyId) -> f32 { w.m(id).rotation[2] }
pub fn set_yaw(w: &mut World, id: MobyId, a: f32) { w.mm(id).rotation[2] = a; }

/// Pvar f32 / vector shorthands on moby `id`.
pub fn pf(w: &World, id: MobyId, o: usize) -> f32 { pvar::ff(&w.m(id).pvars, o) }
pub fn set_pf(w: &mut World, id: MobyId, o: usize, x: f32) { pvar::set_ff(&mut w.mm(id).pvars, o, x) }
pub fn pv4(w: &World, id: MobyId, o: usize) -> V { pvar::v4f(&w.m(id).pvars, o) }
pub fn set_pv4(w: &mut World, id: MobyId, o: usize, x: V) { pvar::set_v4f(&mut w.mm(id).pvars, o, x) }
pub fn pi32(w: &World, id: MobyId, o: usize) -> i32 { pvar::i32(&w.m(id).pvars, o) }
pub fn set_pi32(w: &mut World, id: MobyId, o: usize, x: i32) { pvar::set_i32(&mut w.mm(id).pvars, o, x) }
pub fn pi16(w: &World, id: MobyId, o: usize) -> i16 { pvar::i16(&w.m(id).pvars, o) }
pub fn set_pi16(w: &mut World, id: MobyId, o: usize, x: i16) { pvar::set_i16(&mut w.mm(id).pvars, o, x) }
pub fn pu8(w: &World, id: MobyId, o: usize) -> u8 { w.m(id).pvars[o] }
pub fn set_pu8(w: &mut World, id: MobyId, o: usize, x: u8) { w.mm(id).pvars[o] = x; }

/// `MobyAnimBlend(m, seq, frame, ticks)` guarded by the game's idiom `if (m+0x53 != seq)`.
pub fn blend_to(w: &mut World, id: MobyId, seq: u8, frame: i32, ticks: i32) {
    if w.m(id).anim.seq_b != seq { w.anim_blend(id, seq, frame, ticks); }
}

/// `fun_00212ed8(m, seq, frame)` (L01 0x26c5a8), the hard cut ([`rc_formats::moby_anim::hard_cut`]), then the loop
/// sound of the new sequence (`update_moby_animation_state`). Nothing for a class without that sequence.
pub fn hard_cut(w: &mut World, id: MobyId, seq: u8, frame: i32) {
    let (o, classes) = (w.m(id).o_class, w.classes);
    let Some(class) = classes.anim(o) else { return };
    if rc_formats::moby_anim::hard_cut(&mut w.table.mobys[id].anim, class, seq, frame) {
        crate::moby_update::anim_sound::after_sequence_change(&mut w.table.mobys[id], class);
    }
}

#[cfg(test)]
mod tests;
