//! Enemy projectiles: the engine pieces every shot uses (level01 addresses; the particle spawns are engine code in all
//! 19 overlays, `tools/ghidra/names/clusters.tsv`), and the pool records of the particle types the enemies spawn that
//! the particle system does not simulate yet.
//!
//! **Shots.** A projectile is a moby whose update moves it by its pvar velocity and sweeps the step: the segment
//! last → new position through `CollLine_Fix` with a hit template (the shooter ignored), then, on a miss, a small
//! sphere through `coll_sphere_mobys` with the same template ([`sweep`]). The template writes the hit record into the
//! moby that was hit (`services::line_hit_in` / `sphere_mobys_in`, `mode & 0x4000` mobys only); Ratchet's record goes
//! through the tick to P2's intake like any other hit. [`template`] is the record the enemy shots build on their stack
//! (`dir` = the push, `attacker` = the projectile moby, the type bytes, the damage). The class code (459's shot 722,
//! the gunship's shell 686) keeps its own flight rule and effects.
//!
//! **Their particles.** Types 16 (smoke `PartType16Spawn` 0x280f30), 22 (rising puff `PartType22Spawn` 0x281f30)
//! and 26 (glow sprite on a moby `PartType26Spawn` 0x282b00) are `crate::particles::type16` / `type22` / `type26`;
//! [`part16`], [`part22`], [`part26`] make the spawner's draws (with a record, or alone without a particle system)
//! and count the spawn. Types 4, 8 and 15 are the callers' `fx::part04` / `part08` / `part15`.

use super::V;
use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pf as to_pf, pv, HitTemplate, World};

/// `PartType16Spawn(size, pos, vel, c1, c2, life, kind)`: the frame-load throttle's draws, then the record (one
/// `rand()` with it). True when a record was taken.
pub fn part16(w: &mut World, a: &crate::particles::type16::Spawn) -> Option<usize> {
    let load = f32::from_bits(w.svc.frame_load[0].0);
    if crate::particles::type16::throttled(w.rng, a.life, load) { return None; }
    *w.svc.fx.part_spawns.entry(16).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return None;
    };
    let r = crate::particles::type16::spawn(sys, w.rng, a);
    if r.is_none() { w.svc.fx.part_failed += 1; }
    r
}

/// `PartType22Spawn(size, pos, vel, c1, c2, life)`: the record (one `rand()` with it).
pub fn part22(w: &mut World, a: &crate::particles::type22::Spawn) {
    *w.svc.fx.part_spawns.entry(22).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    if crate::particles::type22::spawn(sys, w.rng, a).is_none() { w.svc.fx.part_failed += 1; }
}

/// `PartType26Spawn(size, moby, rgba, life, −1)`: a glow on `moby`'s position (one `rand()` with a record).
pub fn part26(w: &mut World, size: f32, moby: MobyId, rgba: u32, life: i32) {
    *w.svc.fx.part_spawns.entry(26).or_default() += 1;
    let p = w.m(moby).position;
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    if crate::particles::type26::spawn(sys, w.rng, size, moby, rgba, life, -1, [p[0], p[1], p[2]]).is_none() { w.svc.fx.part_failed += 1; }
}

/// `PartType26Spawn(size, moby, rgba, life, list)` with a joint list: the glow follows the list's point, starting 0.4
/// from it toward the camera (0x167240).
pub fn part26_joint(w: &mut World, size: f32, moby: MobyId, rgba: u32, life: i32, list: i16) {
    *w.svc.fx.part_spawns.entry(26).or_default() += 1;
    let j = w.joint_point(moby, list as usize);
    let cam = w.camera_point();
    let d = super::set_len3([cam[0] - j[0], cam[1] - j[1], cam[2] - j[2], 0.0], 0.4);
    let at = [j[0] + d[0], j[1] + d[1], j[2] + d[2]];
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    if crate::particles::type26::spawn(sys, w.rng, size, moby, rgba, life, list, at).is_none() { w.svc.fx.part_failed += 1; }
}

/// The hit template the enemy projectiles build (`+0x00` push, `+0x10` attacker, `+0x14` flags, `+0x18`/`+0x19` type
/// / subtype, `+0x1a` class, `+0x1c` damage, `+0x20`).
pub fn template(w: &World, shot: MobyId, dir: V, flags: u32, kind: (u8, u8), damage: f32, w20: u32) -> HitTemplate {
    HitTemplate { dir: pv(dir), attacker: Some(shot), flags, b18: kind.0, b19: kind.1, h1a: w.m(shot).o_class as u16, damage: to_pf(damage), w20 }
}

/// Is `p` inside the world box [2, 1021]³ (the bound every flight tests)?
pub fn in_world(p: V) -> bool { (0..3).all(|k| (2.0..=1021.0).contains(&p[k])) }

/// The sweep of one step `from → to` (module doc): `CollLine_Fix(from, to, flags, ignore, tmpl)`, the hit point on
/// a hit; else `coll_sphere_mobys(r, to, 0x10, ignore, tmpl)`, `to` when it touched a moby; `None` when the step is
/// clear.
pub fn sweep(w: &mut World, from: V, to: V, flags: u32, r: f32, ignore: Option<MobyId>, tmpl: &HitTemplate) -> Option<V> {
    let h = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(from), pv(to), flags, ignore, tmpl);
    if let Some(h) = h { return Some([h.point[0], h.point[1], h.point[2], to[3]]); }
    (w.sphere_mobys(to_pf(r), pv(to), 0x10, ignore, Some(tmpl)) > 0).then_some(to)
}
