//! **Package P3 — ledges and the wall jump.** The ledge probes, the ledge states 0x18..0x1b, the ledge climb 0x1c
//! and the wall jump 0x11, as the game builds them: one set of probes over the level collision (world mesh, then the
//! mobys the query sees), shared by the fall, the jumps, the glide and the ledge states themselves. There is no
//! per-location ledge data anywhere: a ledge is whatever flat top, wall below it and free edge the probes find.
//! Addresses level01 (Novalis compiles every P3 state; level00 is the same code). Spec: docs/plan/hero_states.md
//! §1.2 and "P3". New code in standard `f32` (the hero block stays [`Pf`]; `Pf::f` / `to_f32` at the boundary, as
//! `swim.rs` does); no hardware modelling.
//!
//! **Probes** (they share the "ledge ahead" core; `CollType` = [`CollOutput::surface_id`]):
//! * `HeroWallLedgeCheckB` 0x22d090 ([`wall_ledge_probe_b`], every tick after the surface reaction, and twice per
//!   tick in the shimmy physics): clears 0x13f838, then (only on foot, falling (group 2), in a jump that descends or
//!   rises slower than 3.5 u/s, in the glide 8 or the shimmy 0x1a / 0x1b; not before frame 37 of the double jump;
//!   not while the regrab lockout 0x13f500 runs; nothing at all in gravity mode 2) predicts the feet 2 ticks ahead
//!   (`pos + 2·eff`, z − 3·g with the jump's or the group's gravity), steps `r + 0.075·k` (k = 1..4) ahead along the
//!   facing and drops a line from 1.65 to 1.15 above the predicted feet (flags 4, Ratchet ignored): the first hit
//!   on a face within 20° of flat, not surface 0 / 9 / 0xc, is the ledge top. A moby top needs the moby's ledge flag
//!   ([`moby_ledge_ok`]). Rejected when the top is more than 1.5 above the feet, when a line at top + 0.05 from the
//!   hero to the top point hits something, or when the top is less than 1.8 above the ground under the hero. The
//!   wall: five horizontal lines (flags 2) from the hero at top − 0.7 … top − 0.14 to 2·r past the top point; its
//!   normal's yaw is the ledge yaw 0x13f834 (out of the wall). The hero must face the wall within 65°. The edge:
//!   from the top point out along the wall normal in 0.03 steps (up to 2.4), the first spot where a vertical line
//!   (top ± 0.5) finds no top; the hang point 0x13f820 is 0.015 back, then 0.45 out along the normal, 1.43 below the
//!   top. A capsule (r 0.45, h 0.6) 0.6 above the top point records "blocked above" in 0x13f844 bit 0.
//! * `HeroWallLedgeCheckC` 0x22d838 ([`probe_c`], the hang / shimmy transitions and probe A): the same ledge test
//!   from a given point and yaw, without side effects: surface 0 allowed, no moby / clear-line / capsule tests, edge
//!   steps 0.07, and the wall must face the given yaw within 30°.
//! * `HeroWallLedgeCheckA` 0x22c9a0 ([`wall_ledge_probe_a`], end of the jump-group physics and the glide's): at
//!   least 0.4 above the ground and not falling faster than 20 u/s; if the fall / jump would reach a grabbable ledge
//!   20 ticks ahead (probe C at the parabola point) the wall window closes (0x13f540 = 22); otherwise a wall 0.3 above
//!   the feet within 1.0 ahead, and at 1.7 (in 0x11, or while 0x13f5a5 && !0x13f5a4) / 3.0 above the feet a wall
//!   (flags 2, not surface 10 / 0xc, ≥ 75° steep, facing the hero within 50°) within 0.72 (0.95 in 0x11) opens
//!   the wall-jump window 0x13f504 = 6 ticks (7 in 0x11) when the hero is in or just left the jump group. A second
//!   wall jump needs a wall at least 170° from the one it pushed off (0x141604 / 0x13f7cc).
//!
//! **States.** 0x18 grab (from the fall 6, the jumps but the flip 0xb, the glide 8, whenever 0x13f838 is set):
//! turn to face the wall (TurnTo 0.04 / 0.2 / 360°/s), pull toward the hang point at ≤ 4 u/s, anim 0x20; hang 0x19
//! after key time 13.5 (anim 0x21 once 0x20 wraps). Hang: probe C at the feet each tick (lost → fall 6, regrab
//! lockout 10 ticks); stick back + ✕ or R1 / R2 → drop (fall 6, lockout 10); ✕ within 17 ticks → climb 0x1c; stick
//! (> 0.5) within 45° of a side and probe C 0.3 to that side → shimmy 0x1a (left) / 0x1b (right). Shimmy: each tick
//! probe B at the feet and 0.3 to the side; both found and within 30° → move toward the midpoint of the two hang
//! points at `table[key time]·0.5` (0x1c4130) and turn; otherwise stand still. Stick released or the ledge ends →
//! hang 0x19 when the anim wraps; stick back + ✕ → fall (lockout 40); ✕ → climb. Climb 0x1c: jump group (h 2.0,
//! takeoff 10, g 25·dt², anim 0x22), forward speed ramps to 2 u/s over key time 13..27 and back to 0 by 30. Wall jump
//! 0x11: jump group (h 3.3, takeoff 9, g 29·dt², anim 0x23 from key 2); from tick 9 it flies along the wall normal
//! at 4.9 u/s, turning to it.
//!
//! **Camera** (`CamType0HeroStateTweaks` 0x3111d8, in the camera's update): while 0x1415d4 = 0xd (the ledge states)
//! the camera auto-yaws behind the hero at 12°/tick: [`Hero::ledge_camera_yaw`] computes the game's value, which
//! the follow camera's type-0 update takes (`follow_camera.rs`, the tweaks right after its platform carry).
//!
//! The climb's voice line (0x1c physics, `0x236738(4, 0)` at key 10 when 0x1404f8 = 3: the Thruster-Pack's; the
//! game has no other) goes through the hero's sound queue (`super::packs::flush_sounds`, after the physics).
//! Not ported: gravity modes ≠ 0
//! in the wall jump
//! (Magneboots walls, P5), `HeroLean`. The platform carry of a hang on a moving ledge (0x13f848) is P1's.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::anim::{AnimCtl, AnimView};
use super::common::blend;
use super::physics::{approach, from_f32x3, set_len2, ticks, to_f32x3, turn_spring, Env, DT, DT2, SCALE64};
use super::states::Ctx;
use super::Hero;
use crate::collision_query::{coll_capsule_m, coll_line_m, CollOutput, QueryFlags};
use crate::pad::button;
use crate::ps2v::Pf;
use crate::rng::Rng;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI as PI_F, TAU};

/// The ledge / wall-jump fields of the hero block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LedgeBlock {
    /// 0x13f7b0 / 0x13f7bc: the wall probe A last accepted (raw face normal) and its yaw.
    pub wall_normal: [f32; 3],
    pub wall_yaw: f32,
    /// 0x13f7c0 / 0x13f7cc: the wall the current wall jump pushed off (copied from 0x13f7b0 at the 0x11 entry).
    pub jump_normal: [f32; 3],
    pub jump_yaw: f32,
    /// 0x141604: set by the wall jump's entry, cleared by every other jump entry (a chained wall jump needs a wall
    /// ≥ 170° from `jump_yaw`).
    pub wall_chain: u8,
    /// 0x13f820: the hang point (feet position while hanging).
    pub point: [f32; 3],
    /// 0x13f830: the ledge top's z.
    pub top_z: f32,
    /// 0x13f834: the ledge yaw (yaw of the wall's normal, pointing out of the wall; the hero faces yaw + π).
    pub yaw: f32,
    /// 0x13f83c: 24·dt² (the jump's gravity when grabbed from a jump) at the grab; 0x13f840: the grab height,
    /// approaching the feet by 3 u/s while hanging (no reader in the level01 hero code).
    pub f83c: f32,
    pub f840: f32,
    /// 0x13f844 bit 0: probe B's capsule above the top point hit something.
    pub f844: u32,
    /// 0x13f848: the moby the ledge top belongs to (None: the level mesh).
    pub moby: Option<usize>,
}

const DTF: f32 = 1.0 / 60.0;
const DT2F: f32 = 1.0 / 3600.0;

/// Shimmy speed by the shimmy anim's key time (0x1c4130, 24 words; ×0.5 in the physics). Beyond the table: 0.
pub const SHIMMY_SPEED: [f32; 24] = [
    -0.0, -0.0, 0.025, 0.05, 0.075, 0.162, 0.162, 0.107, 0.037, 0.037, 0.037, -0.017, -0.017, -0.035, -0.035, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];

fn f(x: Pf) -> f32 { x.to_f32() }
fn p(x: f32) -> Pf { Pf::f(x) }

/// An angle wrapped into [−π, π) (`fast_add_rotations` / `fast_subtract_rotations`).
fn wrap(a: f32) -> f32 {
    let r = (a + PI_F).rem_euclid(TAU) - PI_F;
    if r < -PI_F { r + TAU } else { r }
}
fn add_rot(a: f32, b: f32) -> f32 { wrap(a + b) }
/// `FastDiffRots`: the unsigned angle between two yaws.
fn diff_rots(a: f32, b: f32) -> f32 { wrap(a - b).abs() }
/// `FastArcTan(x, y)` = atan2(y, x).
fn arctan(x: f32, y: f32) -> f32 { y.atan2(x) }
fn dir(yaw: f32, s: f32) -> [f32; 2] { [yaw.cos() * s, yaw.sin() * s] }
/// The body's dimensions in the ledge probes B and C (L00 0x20bed0 / 0x20c758: Ratchet's, and Clank's in body 1,
/// `super::bodies::clank::LEDGE`): the highest top above the feet (`top`; the probe's line runs from `top + 0.15` down to
/// `low`), the steps ahead (`step`·0.25), the least height of the top above the ground under the hero, the wall lines'
/// start below the top, the hang point below the top and out along the wall normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgeDims {
    pub top: f32,
    pub low: f32,
    pub step: f32,
    pub min_above: f32,
    pub wall_from: f32,
    pub hang: f32,
    pub out: f32,
}

/// Ratchet's.
pub const RATCHET_LEDGE: LedgeDims = LedgeDims { top: 1.5, low: 1.15, step: 0.3, min_above: 1.8, wall_from: -0.7, hang: -1.43, out: 0.45 };

/// The dimensions of the body now (Clank's in body 1).
fn dims(h: &Hero) -> LedgeDims { if h.mode == 1 { super::bodies::clank::LEDGE } else { RATCHET_LEDGE } }

/// Angle of a raw normal from straight up (`0x2493c8`, gravity mode 0).
fn slope(n: [f32; 3]) -> f32 { arctan(n[2], (n[0] * n[0] + n[1] * n[1]).sqrt()) }

/// `CollLine_Fix(a, b, flags, ignore, 0)`: the world mesh, then the mobys of [`Env::mobys`]; `hero` passes Ratchet's
/// moby as the ignored one (the game's `0x1413d0`), otherwise none.
fn line(env: &Env, a: [f32; 3], b: [f32; 3], flags: u32, hero: bool) -> Option<CollOutput> {
    coll_line_m(env.coll, env.mobys, a, b, QueryFlags(flags), if hero { env.hero_moby } else { None })
}

/// Probe B's moby branch (0x22d3a0): a ledge top on a moby counts only when the moby is a carrier (mode 0x20) whose
/// pvar record (`**(moby+0x78)`, `FUN_002711f8`) has bit 0 of +0x1e set, or whose platform block (`FUN_00275290`)
/// has bit 0 of +0x3c set. Every other moby (crates, enemies, props: no mode 0x20) is rejected. Both come through
/// [`Env::world`] (`HeroWorld::moby_ledge_flag`, `HeroWorld::carrier`).
fn moby_ledge_ok(env: &Env, moby: usize) -> bool {
    env.world.is_some_and(|w| w.moby_ledge_flag(moby) || w.carrier(moby).is_some_and(|c| c.delta.flags & 1 != 0))
}

/// The "ledge ahead" core of probes B and C: the ledge top ahead of `feet` along the hero's facing (4 vertical lines
/// from 1.65 to 1.15 above `feet`, `r + 0.075·k` ahead). `no_water`: probe B also rejects surface 0.
fn top_ahead(h: &Hero, env: &Env, feet: [f32; 3], no_water: bool) -> Option<CollOutput> {
    let d = dims(h);
    let yaw = f(h.rot[2]);
    let r = f(h.cap_radius);
    let mut q = feet;
    q[0] += yaw.cos() * r;
    q[1] += yaw.sin() * r;
    for _ in 0..4 {
        q[0] += yaw.cos() * d.step * 0.25;
        q[1] += yaw.sin() * d.step * 0.25;
        let a = [q[0], q[1], q[2] + (d.top + 0.15)];
        let b = [q[0], q[1], q[2] + d.low];
        if let Some(o) = line(env, a, b, 4, true) {
            let t = o.surface_id();
            if t != 9 && t != 0xc && !(no_water && t == 0) && slope(o.normal) < 0.349_065_84 { return Some(o); }
        }
    }
    None
}

/// The wall under a ledge top (five horizontal lines, flags 2, from `from` at top − 0.7 … top − 0.14 to 2·r past
/// the top point): the yaw of its normal.
fn wall_below(h: &Hero, env: &Env, from: [f32; 3], top: [f32; 3]) -> Option<f32> {
    let yaw_to = arctan(top[0] - from[0], top[1] - from[1]);
    let d = dir(yaw_to, f(h.cap_radius) + f(h.cap_radius));
    let from_z = dims(h).wall_from;
    (0..5).find_map(|i| {
        let z = top[2] + (1.0 - i as f32 / 5.0) * from_z;
        line(env, [from[0], from[1], z], [top[0] + d[0], top[1] + d[1], z], 2, false).map(|w| arctan(w.normal[0], w.normal[1]))
    })
}

/// The edge search: from the top point out along the ledge yaw in `step`s (< 2.4), the first point with no top under
/// it (vertical line top ± 0.5, flags 4, Ratchet ignored).
fn edge_out(env: &Env, top: [f32; 3], yaw: f32, step: f32) -> Option<[f32; 3]> {
    let mut d = 0.0f32;
    loop {
        let q = [top[0] + yaw.cos() * d, top[1] + yaw.sin() * d, top[2]];
        if line(env, [q[0], q[1], q[2] + 0.5], [q[0], q[1], q[2] - 0.5], 4, true).is_none() { return Some(q); }
        d += step;
        if !(d < 2.4) { return None; }
    }
}

/// `HeroWallLedgeCheckC(point, &yaw)` 0x22d838: is there a grabbable ledge ahead of `feet` for a hero facing `yaw`
/// (the probe steps ahead along the hero's own facing)? No side effects.
pub(super) fn probe_c(h: &Hero, env: &Env, feet: [f32; 3], yaw: f32) -> bool {
    if h.f658 == 2 { return false; }
    let Some(o) = top_ahead(h, env, feet, false) else { return false };
    let top = o.point;
    let dm = dims(h);
    if feet[2] + dm.top < top[2] { return false; }
    if top[2] - f(h.ground_z) < dm.min_above { return false; }
    let Some(ly) = wall_below(h, env, feet, top) else { return false };
    if !(diff_rots(yaw, add_rot(ly, PI_F)) < 1.134_464) { return false; }
    if edge_out(env, top, ly, 0.07).is_none() { return false; }
    !(FRAC_PI_6 < diff_rots(add_rot(ly, PI_F), yaw))
}

/// `HeroWallLedgeCheckB` 0x22d090, every hero tick after the surface reaction 0x22cd48 (and in the shimmy physics):
/// sets 0x13f838 and the hang point / ledge yaw when a grabbable ledge is ahead. `anim`: Ratchet's anim fields (the
/// double jump's frame gate reads the key time 0x13fdf8).
pub(super) fn wall_ledge_probe_b(h: &mut Hero, env: &Env, anim: &AnimView) {
    if h.f658 == 2 { return; }
    h.f838 = 0;
    if h.f500 != 0 { return; }
    let mut on = false;
    if h.mode == 0 {
        on = (h.group == 4 && (h.jump.descending != 0 || f(h.vel[2]) < DTF * 3.5))
            || h.group == 2
            || matches!(h.state, 8 | 0x1a | 0x1b);
        if h.state == 0xe && anim.frame < 37.0 { on = false; }
    }
    // Clank (L00 0x20bed0, body 1): in a jump that descends or rises slower than 2.5 u/s, falling, or in his glide 0x4f /
    // shimmy 0x4d / 0x4e (super::bodies::clank).
    if h.mode == 1 {
        on = (h.group == 4 && (h.jump.descending != 0 || f(h.vel[2]) < DTF * 2.5)) || h.group == 2 || matches!(h.state, 0x4d..=0x4f);
    }
    if !on { return; }
    let dm = dims(h);
    // The feet 2 ticks ahead (0x26e488: pos + n·eff, z − ((n² + n) / 2)·g).
    let g = f(if h.group == 4 { h.jump.g } else { h.group_gravity });
    let pos = to_f32x3(h.pos);
    let eff = to_f32x3(h.eff);
    let n = ticks(2);
    let k = (n * n + n) >> 1;
    let feet = [pos[0] + eff[0] * n as f32, pos[1] + eff[1] * n as f32, pos[2] + eff[2] * n as f32 - k as f32 * g];
    let Some(o) = top_ahead(h, env, feet, true) else { return };
    if let Some(m) = o.moby {
        if !moby_ledge_ok(env, m) { return; }
    }
    h.ledge_blk.moby = o.moby;
    let top = o.point;
    if pos[2] + dm.top < top[2] { return; }
    // Nothing between the hero and the top point just above the top.
    if line(env, [pos[0], pos[1], top[2] + 0.05], [top[0], top[1], top[2] + 0.05], 4, true).is_some() { return; }
    let blocked = coll_capsule_m(env.coll, env.mobys, [top[0], top[1], top[2] + 0.6], 0.6, 0.45, QueryFlags(4), env.hero_moby).is_some();
    if blocked { h.ledge_blk.f844 |= 1 } else { h.ledge_blk.f844 &= !1 }
    h.ledge_blk.top_z = top[2];
    if top[2] - f(h.ground_z) < dm.min_above { return; }
    let Some(ly) = wall_below(h, env, pos, top) else { return };
    h.ledge_blk.yaw = ly;
    if !(diff_rots(f(h.rot[2]), add_rot(ly, PI_F)) < 1.134_464) { return; }
    let Some(e) = edge_out(env, top, ly, 0.03) else { return };
    let back = dir(ly, -(0.5 * 0.03));
    let out = dir(ly, dm.out);
    h.ledge_blk.point = [e[0] + back[0] + out[0], e[1] + back[1] + out[1], top[2] + dm.hang];
    h.f838 = 1;
}

/// `HeroWallLedgeCheckA` 0x22c9a0, at the end of the jump-group physics (and of the glide 8's): the wall-jump window
/// 0x13f504 (see the module doc).
pub(super) fn wall_ledge_probe_a(h: &mut Hero, env: &Env) {
    if f(h.height) < 0.4 { return; }
    if f(h.vel[2]) < DTF * -20.0 { return; }
    if h.f838 != 0 {
        h.ledge = 0;
        return;
    }
    // A ledge the fall reaches in 20 ticks takes precedence.
    let n = ticks(20);
    let t = n as f32;
    let pos = to_f32x3(h.pos);
    let feet = [
        pos[0] + f(h.disp[0]) * t,
        pos[1] + f(h.disp[1]) * t,
        (pos[2] + f(h.vel[2]) * t) - f(h.jump.g) * t * t * 0.5,
    ];
    if probe_c(h, env, feet, f(h.rot[2])) {
        h.ledge = 0;
        h.f540 = (n + 2) as i16;
        return;
    }
    // 0x22b628(0.3, 1.0): a wall 0.3 above the feet, between r − 0.02 and 1.0 ahead.
    let r = f(h.cap_radius);
    if line(env, local(h, r - 0.02, 0.3), local(h, 1.0, 0.3), 2, false).is_none() { return; }
    let up = if h.state == 0x11 || (h.f5a5 != 0 && h.f5a4 == 0) { 1.7 } else { 3.0 };
    let a = local(h, 0.0, up);
    let Some(o) = line(env, a, local(h, r + 1.4, up), 2, false) else { return };
    let t = o.surface_id();
    if t == 10 || t == 0xc { return; }
    if slope(o.normal) < 1.308_997 { return; }
    // The normal in the hero's frame (the transposed rows 0x13f390) must point back at him within 50°.
    let rows = h.rows.map(to_f32x3);
    let dot = |r: [f32; 3]| r[0] * o.normal[0] + r[1] * o.normal[1] + r[2] * o.normal[2];
    if 0.872_664_6 < diff_rots(arctan(dot(rows[0]), dot(rows[1])), PI_F) { return; }
    let max = if h.state == 0x11 { 0.95 } else { 0.72 };
    let d2 = ((o.point[0] - a[0]).powi(2) + (o.point[1] - a[1]).powi(2)).sqrt();
    if max < d2 { return; }
    h.ledge_blk.wall_normal = o.normal;
    h.ledge_blk.wall_yaw = arctan(o.normal[0], o.normal[1]);
    if !(h.group == 4 || h.prev_group == 4 || h.prev_prev_group == 4) { return; }
    if h.ledge_blk.wall_chain != 0 && diff_rots(h.ledge_blk.jump_yaw, h.ledge_blk.wall_yaw) < 2.967_059_6 { return; }
    h.ledge = ticks(if h.state == 0x11 { 7 } else { 6 });
}

/// `0x248c60(fwd, 0, up)`: a point `fwd` ahead and `up` above the feet in the hero's frame (rows 0x13f350).
fn local(h: &Hero, fwd: f32, up: f32) -> [f32; 3] {
    let r = h.rows.map(to_f32x3);
    let pos = to_f32x3(h.pos);
    std::array::from_fn(|k| (r[0][k] * fwd + r[2][k] * up) + pos[k])
}

/// The wall-jump test of the jump transitions 0x242930 (jump case): the window 0x13f504 open, in the air, in 7 / 9
/// (or 0x11 after 25 ticks), ✕ pressed within 7 ticks and after entering the state → 0x11.
pub(super) fn wall_jump(h: &mut Hero, c: &mut Ctx) -> bool {
    if h.ledge == 0 || h.jump.landed != 0 || !matches!(h.state, 7 | 9 | 0x11) { return false; }
    if h.state == 0x11 && !(ticks(25) < h.timer) { return false; }
    match c.env.pad.pressed_within(button::CROSS, ticks(7)) {
        Some(ago) if ago < h.timer => h.set_state(c, 0x11, true),
        _ => false,
    }
}

// ------------------------------------------------------------------------------------------------
// SetState, physics and transitions of 0x11, 0x18..0x1c.

/// SetState entry of 0x11, 0x18..0x1c (0x23cf98). 0x11 / 0x1c are jump-group ids: the shared jump entry with their
/// parameters (`jump.rs`).
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, old_sub: i32) -> Option<bool> {
    match id {
        0x11 | 0x1c => return h.jump_group_entry(c, id, play, old_sub),
        0x18 => {
            h.ledge_blk.f83c = DT2F * 24.0;
            h.group = 3;
            h.f15d4 = 0xd;
            h.ledge_blk.f840 = f(h.pos[2]);
            if h.prev_group == 4 {
                h.ledge_blk.f83c = f(h.jump.g);
                h.ledge_blk.f840 = f(h.jump.ref_z);
            }
            h.items.f13f7 = 1;
            if play {
                if f(h.eff_len_xy) <= DTF * 3.5 {
                    h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(9) + 2), 0x20, 1);
                } else {
                    h.set_anim(c.anim, c.rng, blend(9), 0x20, 0);
                }
            }
        }
        0x19 => {
            h.group = 3;
            h.idle.blink_period = 0x68;
            h.f15d4 = 0xd;
            h.items.f13f7 = 1;
            if play { h.set_anim(c.anim, c.rng, blend(6), 0x21, 0); }
        }
        0x1a | 0x1b => {
            h.items.f13f7 = 1;
            h.group = 3;
            h.f15d4 = 0xd;
            if play { h.set_anim(c.anim, c.rng, blend(6), if id == 0x1a { 0x24 } else { 0x25 }, 0); }
        }
        _ => {}
    }
    None
}

/// Per-state physics (0x2370b8 cases 0x11 / 0x1c (the jump case), 0x18 / 0x19, 0x1a / 0x1b).
pub(super) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, _rng: &mut Rng) -> bool {
    match h.state {
        0x11 | 0x1c => {
            // The jump case's first test: climbing (0x1c) with Clank shown and the Thruster-Pack as the back item
            // (0x1404f8 = 3), the anim passing frame 10 → the climb's voice `0x236738(4, 0)` (the Heli-Pack or no pack:
            // no voice).
            let v = anim.view();
            if h.state == 0x1c && h.back_slot.clank_hidden == 0 && h.back_slot.slot.id == 3 && super::boots::passed(&v, 10.0) {
                h.packs.sounds.push(super::packs::SoundCmd::Voice { index: 4, flags: 0 });
            }
            h.phys_jump_anim(env, v.frame)
        }
        0x18 | 0x19 => hang_physics(h),
        0x1a | 0x1b => shimmy_physics(h, env, &anim.view()),
        _ => return false,
    }
    true
}

/// 0x18 / 0x19: face the wall, pull to the hang point (≤ 4 u/s).
fn hang_physics(h: &mut Hero) {
    h.target_yaw = p(add_rot(h.ledge_blk.yaw, PI_F));
    h.turn_to(SCALE64 * p(0.04), SCALE64 * p(0.2), DT * p(TAU));
    // 0x270ac0: Euler y back to 0 at 270°/s.
    let mut y = f(h.rot[1]);
    let step = DTF * 4.712_389;
    y = if y.abs() <= step { 0.0 } else { y - step * y.signum() };
    h.rot[1] = p(y);
    let pos = to_f32x3(h.pos);
    let pt = h.ledge_blk.point;
    let mut v = [pt[0] - pos[0], pt[1] - pos[1], pt[2] - pos[2]];
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let max = DTF * 4.0;
    if max < l { v = v.map(|x| x / l * max); }
    set_vel3(h, v);
    let mut z = h.ledge_blk.f840;
    let s = DTF * 3.0;
    z += (pos[2] - z).clamp(-s, s);
    h.ledge_blk.f840 = z;
}

/// 0x1a / 0x1b: probe B at the feet and 0.3 to the side; move toward the midpoint of the two hang points.
fn shimmy_physics(h: &mut Hero, env: &Env, anim: &AnimView) {
    set_vel3(h, [0.0; 3]);
    let saved = h.pos;
    let side = if h.state == 0x1a { FRAC_PI_2 } else { -FRAC_PI_2 };
    let o = dir(add_rot(f(h.rot[2]), side), 0.3);
    let pos = to_f32x3(h.pos);
    let there = [pos[0] + o[0], pos[1] + o[1], pos[2]];
    wall_ledge_probe_b(h, env, anim);
    if h.f838 == 0 { return; }
    let p1 = h.ledge_blk.point;
    let y1 = add_rot(h.ledge_blk.yaw, PI_F);
    h.pos = [p(there[0]), p(there[1]), p(there[2]), saved[3]];
    wall_ledge_probe_b(h, env, anim);
    h.pos = saved;
    if h.f838 == 0 { return; }
    let p2 = h.ledge_blk.point;
    let y2 = add_rot(h.ledge_blk.yaw, PI_F);
    if FRAC_PI_6 < diff_rots(y1, y2) { return; }
    // The game's blend: y1 + (y1 − y2)/2 (0x222040 then 0x221ff8, as the instructions order them).
    h.target_yaw = p(add_rot(wrap(y1 - y2) * 0.5, y1));
    h.turn_to(SCALE64 * p(0.04), SCALE64 * p(0.2), DT * p(TAU));
    let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5, (p1[2] + p2[2]) * 0.5];
    set_vel3(h, [mid[0] - pos[0], mid[1] - pos[1], mid[2] - pos[2]]);
    let key = anim.frame as i32;
    let s = usize::try_from(key).ok().and_then(|i| SHIMMY_SPEED.get(i)).copied().unwrap_or(0.0);
    h.vel = set_len2(h.vel, p(s * 0.5));
}

fn set_vel3(h: &mut Hero, v: [f32; 3]) {
    let w = h.vel[3];
    h.vel = from_f32x3(v);
    h.vel[3] = w;
}

/// `HeroJumpHorizontal` 0x234b40, the 0x1c and 0x11 branches (the jump physics calls it for them from tick 0).
/// `key`: Ratchet's key time 0x13fdf8.
pub(super) fn jump_horizontal(h: &mut Hero, env: &Env, key: f32) {
    let _ = env;
    if h.state == 0x1c {
        if ticks(40) < h.timer {
            h.turn_to(SCALE64 * p(0.04), SCALE64 * p(0.2), DT * p(15.009_831));
            h.air_accel(DT2 * p(20.0));
        }
        if (13.0..=30.0).contains(&key) {
            let t = if 27.0 < key { 0.0 } else { DTF + DTF };
            let mut s = h.jump.speed7ac;
            approach(p(t), p(DT2F * 8.0), &mut s);
            h.jump.speed7ac = s;
        }
        let (yaw, s) = (f(h.rot[2]), f(h.jump.speed7ac));
        h.vel[0] = p(yaw.cos() * s);
        h.vel[1] = p(yaw.sin() * s);
        return;
    }
    // 0x11: along the wall normal, turning to it after the windup.
    let ly = h.ledge_blk.jump_yaw;
    h.target_yaw = p(ly);
    let n = h.ledge_blk.jump_normal;
    h.stick_world = [p(n[0]), p(n[1]), p(n[2]), h.stick_world[3]];
    if ticks(9) < h.timer {
        let (mut yaw, mut v) = (h.rot[2], h.yaw_vel);
        h.yaw_residual = turn_spring(h.target_yaw, SCALE64 * p(0.042), SCALE64 * p(0.2), DT * p(11.868_238), &mut yaw, &mut v, -1);
        h.rot[2] = yaw;
        h.yaw_vel = v;
    }
    if ticks(9) <= h.timer {
        // Gravity mode 0 (the Magneboots frame of modes 1 / 2 is P5's).
        let d = dir(ly, DTF * 4.9);
        h.vel[0] = p(d[0]);
        h.vel[1] = p(d[1]);
    }
    h.speed = h.eff_len_xy;
}

/// Per-state transitions (0x242930 cases 0x18, 0x19, 0x1a / 0x1b; 0x11 / 0x1c share the jump case).
pub(super) fn transitions(h: &mut Hero, c: &mut Ctx) {
    match h.state {
        0x11 | 0x1c => h.tr_jump(c),
        0x18 => {
            if 13.5 < c.anim.view().frame { h.set_state(c, 0x19, false); }
        }
        0x19 => tr_hang(h, c),
        0x1a | 0x1b => tr_shimmy(h, c),
        _ => {}
    }
}

/// Fall 6 with the regrab lockout 0x13f500.
fn drop_off(h: &mut Hero, c: &mut Ctx, lockout: i32) {
    h.set_state(c, 6, true);
    h.f500 = ticks(lockout) as i16;
}

fn side_point(h: &Hero, side: f32) -> [f32; 3] {
    let pos = to_f32x3(h.pos);
    let o = dir(add_rot(f(h.rot[2]), side), 0.3);
    [pos[0] + o[0], pos[1] + o[1], pos[2]]
}

/// 0x19: hang.
fn tr_hang(h: &mut Hero, c: &mut Ctx) {
    let v = c.anim.view();
    if v.flags & 2 != 0 && v.seq_b == 0x20 { h.set_anim(c.anim, c.rng, blend(6), 0x21, 0); }
    h.stick_target(c.env, Pf::ONE);
    let yaw = f(h.rot[2]);
    if !probe_c(h, c.env, to_f32x3(h.pos), yaw) {
        drop_off(h, c, 10);
        return;
    }
    let pad = c.env.pad;
    let stick = f(h.stick_mag);
    let ty = f(h.target_yaw);
    if 0.5 < stick && pad.pressed_within(button::CROSS, ticks(9)).is_some() && 2.356_194_5 < diff_rots(ty, yaw) {
        drop_off(h, c, 10);
        return;
    }
    if pad.pressed & button::CROUCH != 0 {
        drop_off(h, c, 10);
        return;
    }
    if pad.pressed_within(button::CROSS, ticks(17)).is_some() {
        h.set_state(c, 0x1c, true);
        return;
    }
    if !(0.5 < stick) { return; }
    for (side, to) in [(FRAC_PI_2, 0x1a), (-FRAC_PI_2, 0x1b)] {
        if diff_rots(ty, add_rot(yaw, side)) < FRAC_PI_4 {
            if probe_c(h, c.env, side_point(h, side), yaw) { h.set_state(c, to, true); }
            return;
        }
    }
}

/// 0x1a / 0x1b: shimmy.
fn tr_shimmy(h: &mut Hero, c: &mut Ctx) {
    let side = if h.state == 0x1b { -FRAC_PI_2 } else { FRAC_PI_2 };
    let there = side_point(h, side);
    let yaw = f(h.rot[2]);
    h.stick_target(c.env, Pf::ONE);
    let stick = f(h.stick_mag);
    let ty = f(h.target_yaw);
    let going = 0.5 <= stick && !(FRAC_PI_4 < diff_rots(ty, add_rot(yaw, side))) && probe_c(h, c.env, there, yaw);
    if !going && c.anim.view().flags & 2 != 0 {
        h.set_state(c, 0x19, true);
        return;
    }
    let pad = c.env.pad;
    if 0.5 < stick && pad.pressed_within(button::CROSS, ticks(9)).is_some() && 2.356_194_5 < diff_rots(ty, yaw) {
        drop_off(h, c, 40);
        return;
    }
    if pad.pressed_within(button::CROSS, ticks(9)).is_some() { h.set_state(c, 0x1c, true); }
}

impl Hero {
    /// The ledge branch of the camera's hero-state tweaks (`CamType0HeroStateTweaks` 0x3111d8): while 0x1415d4 = 0xd
    /// (the ledge states set it) the camera's yaw rate D+0x1bc = 12° (`0x313af0(0.2094, 0, dir)`), its scripted yaw
    /// input D+0x1c4 from `0x313888` and the look flag D+0x116 = 1 (`0x313820`); the camera's own reset 0x311010
    /// clears them again each tick. `0x313888(0, cam, dir)`: with `u` the camera up (0x1672b0), `c` = the camera
    /// offset D+0x130 and `d` = dir(ledge yaw + π) (the hero's facing into the wall), both with their `u` part
    /// removed, θ = the angle between them (`FastArcSin` of the normalised dot), `a = π − θ` (0 with the camera
    /// straight behind), `s` = +1 when `(u × d)·c ≥ 0` else −1 (the game's `FastVecCross(out, d, u)` is u × d);
    /// D+0x1c4 = `s·(2t − t²)` with t = min(a / (π/2), 1) (`0x26cc00(−1, 0, 1, 0, t)`). Returns (yaw rate, scripted yaw input) or None outside the ledge states. Not
    /// modelled: the camera data's +0x230 = 0x14d exception and the camera's script lock (+0x86).
    pub fn ledge_camera_yaw(&self, cam_off: [f32; 3], up: [f32; 3]) -> Option<(f32, f32)> {
        if self.f15d4 != 0xd { return None; }
        let rate = 0.209_439_52;
        let fy = add_rot(self.ledge_blk.yaw, PI_F);
        let d = [fy.cos(), fy.sin(), 0.0];
        // `0x313888(0, cam, dir)`: the camera's scripted yaw input with tolerance 0 (a degenerate direction writes
        // nothing: the camera's reset left D+0x1c4 at 0).
        Some((rate, crate::follow_camera::yaw_input_toward(d, cam_off, up, 0.0).unwrap_or(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{cam_x, cell, mesh};
    use super::super::{hero_update, HeroTick};
    use super::*;
    use crate::moby_runtime::Moby;
    use crate::pad::{PadInput, PadState};
    use rc_formats::collision::{Collision, CollisionCell};
    use std::collections::BTreeMap;

    /// A looping stand-in for Ratchet's animation: `set_anim` starts `seq` at key `frame` (blend counted down),
    /// the key time advances by the playback speed and wraps at `LEN` (flags bit 1), as the ledge states need.
    #[derive(Default)]
    struct LoopAnim {
        v: AnimView,
        blend_left: f32,
    }
    /// Key-time length per sequence (the climb 0x22 runs to ~30, the ledge loops ~16).
    fn len(seq: u8) -> f32 { if seq == 0x22 { 36.0 } else { 16.0 } }
    impl AnimCtl for LoopAnim {
        fn set_anim(&mut self, blend: Pf, seq: u8, frame: i32) {
            self.v.seq_a = 0xff;
            self.v.seq_b = seq;
            self.v.frame = frame as f32;
            self.v.frame_count_b = 16;
            self.v.rate = 1.0;
            self.v.frame_b_rate = 1.0;
            self.blend_left = if Pf::ZERO < blend { blend.to_f32() } else { 1.0 };
        }
        fn advance(&mut self, speed: Pf) {
            self.v.flags = 0;
            if self.v.seq_a != self.v.seq_b {
                self.blend_left -= 1.0;
                if self.blend_left <= 0.0 { self.v.seq_a = self.v.seq_b; }
            }
            self.v.frame += speed.to_f32();
            if len(self.v.seq_b) <= self.v.frame {
                self.v.frame -= len(self.v.seq_b);
                self.v.flags |= 2;
            }
        }
        fn view(&self) -> AnimView { self.v }
        fn frame_count(&self, _seq: u8) -> u8 { 16 }
        fn set_loop(&mut self, _start: i32, _end: i32) {}
        fn clear_loop(&mut self) {}
    }

    struct Sim {
        hero: Hero,
        moby: Moby,
        pad: PadState,
        anim: LoopAnim,
        rng: Rng,
        states: Vec<i32>,
    }

    impl Sim {
        fn new(pos: [f32; 3], yaw: f32) -> Sim {
            let hero = Hero::spawn(pos, yaw);
            let mut moby = Moby::zeroed();
            moby.position = hero.pos.map(Pf::to_f32);
            Sim { hero, moby, pad: PadState::default(), anim: LoopAnim::default(), rng: Rng::new(), states: Vec::new() }
        }
        fn tick(&mut self, coll: &Collision, input: PadInput) {
            self.pad.update(Some(&input.bytes()), false);
            let (cam_rows, cam_yaw) = cam_x();
            let env = Env { coll, pad: &self.pad, cam_yaw, cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: None };
            let r = hero_update(&mut self.hero, &mut self.moby, &env, &mut self.anim, &mut self.rng);
            assert_eq!(r, HeroTick::Ran, "hero stopped in state {:#x}", self.hero.state);
            self.states.push(self.hero.state);
        }
        fn run(&mut self, coll: &Collision, input: PadInput, n: usize) { for _ in 0..n { self.tick(coll, input); } }
        fn run_until(&mut self, coll: &Collision, input: PadInput, n: usize, state: i32) -> bool {
            for _ in 0..n {
                self.tick(coll, input);
                if self.hero.state == state { return true; }
            }
            false
        }
        fn pos(&self) -> [f32; 3] { self.hero.position() }
    }

    fn dedup(v: &[i32]) -> Vec<i32> {
        let mut o: Vec<i32> = Vec::new();
        for &s in v { if o.last() != Some(&s) { o.push(s); } }
        o
    }

    /// Quads grouped into the collision cells holding their centroids (callers split them at the 4-unit grid). Type
    /// 0x2f: surface 0xf, which no level's surface reaction handles (the testkit floor's surface 1 burns).
    fn cells_of(quads: &[[[f32; 3]; 4]]) -> Vec<CollisionCell> {
        let mut by: BTreeMap<[i16; 3], Vec<[[f32; 3]; 4]>> = BTreeMap::new();
        for q in quads {
            let c: [f32; 3] = std::array::from_fn(|k| (q[0][k] + q[1][k] + q[2][k] + q[3][k]) / 4.0);
            by.entry(c.map(|v| (v / 4.0).floor() as i16)).or_default().push(*q);
        }
        by.into_iter()
            .map(|(k, qs)| {
                let verts: Vec<[f32; 3]> = qs.iter().flatten().copied().collect();
                let idx: Vec<([u8; 4], u8)> = (0..qs.len()).map(|i| { let b = (4 * i) as u8; ([b, b + 1, b + 2, b + 3], 0x2f) }).collect();
                cell(k, &verts, &idx)
            })
            .collect()
    }

    /// `[a, b)` split at multiples of 4.
    fn spans(a: f32, b: f32) -> Vec<(f32, f32)> {
        let mut v = Vec::new();
        let mut x = a;
        while x < b {
            let n = ((x / 4.0).floor() + 1.0) * 4.0;
            let e = n.min(b);
            v.push((x, e));
            x = e;
        }
        v
    }

    /// A box `[x0, x1] × [y0, y1] × [z0, z1]` with outward faces (no bottom), split at the cell grid.
    /// A floor at `z` over `[x0, x1) × [y0, y1)`.
    fn flat(z: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> Vec<[[f32; 3]; 4]> {
        let mut q = Vec::new();
        for (a, b) in spans(x0, x1) {
            for (c, d) in spans(y0, y1) { q.push([[a, c, z], [a, d, z], [b, d, z], [b, c, z]]); }
        }
        q
    }

    fn block(x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32) -> Vec<[[f32; 3]; 4]> {
        let mut q = Vec::new();
        for (a, b) in spans(x0, x1) {
            for (c, d) in spans(y0, y1) { q.push([[a, c, z1], [a, d, z1], [b, d, z1], [b, c, z1]]); }
        }
        for (za, zb) in spans(z0, z1) {
            for (c, d) in spans(y0, y1) {
                q.push([[x0, c, za], [x0, d, za], [x0, d, zb], [x0, c, zb]]); // −x
                q.push([[x1, c, za], [x1, c, zb], [x1, d, zb], [x1, d, za]]); // +x
            }
            for (a, b) in spans(x0, x1) {
                q.push([[a, y0, za], [a, y0, zb], [b, y0, zb], [b, y0, za]]); // −y
                q.push([[a, y1, za], [b, y1, za], [b, y1, zb], [a, y1, zb]]); // +y
            }
        }
        q
    }

    /// The step's top (3.25 above the floor) and the hang height below it.
    const TOP: f32 = 103.25;
    const HANG_Z: f32 = TOP - 1.43;

    /// Floor at z = 100 over x 400..420, y 396..424, and a step 3.25 high (a held jump clears 2.5): x 410..418,
    /// y 405..411 (its front wall at x = 410 faces −x).
    fn step() -> Collision {
        let mut q = flat(100.0, 400.0, 420.0, 396.0, 424.0);
        q.extend(block(410.0, 418.0, 405.0, 411.0, 100.0, TOP));
        mesh(cells_of(&q))
    }

    fn fwd() -> PadInput { PadInput::neutral().stick(0.0, -1.0) }

    /// Jump at the step and hang: returns the sim hanging (0x19) at y ≈ 408.
    fn hanging(coll: &Collision) -> Sim {
        let mut s = Sim::new([408.8, 408.0, 100.0], 0.0);
        s.run(coll, PadInput::neutral(), 2);
        for _ in 0..30 {
            s.tick(coll, fwd().press(button::CROSS));
            if std::env::var("LEDGE_DEBUG").is_ok() { eprintln!("{:#x} t{} {:?} vz {} desc {} f838 {} h {}", s.hero.state, s.hero.timer, s.pos(), f(s.hero.vel[2]) * 60.0, s.hero.jump.descending, s.hero.f838, f(s.hero.height)); }
            if s.hero.state == 0x18 { break; }
        }
        for _ in 0..60 {
            s.tick(coll, fwd());
            if std::env::var("LEDGE_DEBUG").is_ok() { eprintln!("{:#x} t{} {:?} vz {} desc {} f838 {} h {}", s.hero.state, s.hero.timer, s.pos(), f(s.hero.vel[2]) * 60.0, s.hero.jump.descending, s.hero.f838, f(s.hero.height)); }
            if s.hero.state == 0x19 { break; }
        }
        assert!(s.hero.state == 0x19, "no hang: {:x?}", dedup(&s.states));
        s
    }

    #[test]
    fn probes_see_the_step() {
        let coll = step();
        let s = Sim::new([409.55, 408.0, HANG_Z], 0.0);
        let pad = PadState::default();
        let (cam_rows, cam_yaw) = cam_x();
        let env = Env { coll: &coll, pad: &pad, cam_yaw, cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: None };
        let mut h = s.hero.clone();
        h.ground_z = Pf::f(100.0);
        assert!(probe_c(&h, &env, [409.55, 408.0, HANG_Z], 0.0));
        assert!(!probe_c(&h, &env, [409.55, 408.0, HANG_Z], 1.0), "facing 57° off the wall");
        assert!(!probe_c(&h, &env, [409.55, 411.2, HANG_Z], 0.0), "past the end");
        assert!(!probe_c(&h, &env, [409.55, 408.0, HANG_Z - 0.3], 0.0), "top more than 1.5 up");
        // B, as in a fall: the hang point 0.45 out from the edge, 1.43 below the top.
        h.group = 2;
        h.group_gravity = Pf::ZERO;
        let anim = AnimView::default();
        wall_ledge_probe_b(&mut h, &env, &anim);
        assert_eq!(h.f838, 1);
        let p = h.ledge_blk.point;
        assert!((p[0] - 409.55).abs() < 0.03 && (p[1] - 408.0).abs() < 1e-4 && (p[2] - HANG_Z).abs() < 1e-4, "{p:?}");
        assert!((diff_rots(h.ledge_blk.yaw, PI_F)) < 1e-5, "ledge yaw {}", h.ledge_blk.yaw);
        assert_eq!(h.ledge_blk.f844 & 1, 0);
        // The regrab lockout keeps it off.
        h.f500 = 3;
        wall_ledge_probe_b(&mut h, &env, &anim);
        assert_eq!(h.f838, 0);
    }

    #[test]
    fn grab_from_a_jump_and_hang() {
        let coll = step();
        let s = hanging(&coll);
        let st = dedup(&s.states);
        assert!(st.windows(3).any(|w| w == [7, 0x18, 0x19]), "{st:x?}");
        assert_eq!(s.anim.calls_seq(), 0x20);
        let mut s = s;
        s.run(&coll, PadInput::neutral(), 40);
        assert_eq!(s.hero.state, 0x19);
        let p = s.pos();
        assert!((p[0] - 409.55).abs() < 0.05 && (p[2] - HANG_Z).abs() < 0.01, "hangs at {p:?}");
        assert!(f(s.hero.rot[2]).abs() < 1e-3, "faces the wall: {}", f(s.hero.rot[2]));
        assert_eq!(s.hero.group, 3);
        assert_eq!(s.anim.v.seq_b, 0x21, "hang anim after the grab anim wrapped");
    }

    #[test]
    fn shimmy_both_ways_and_stop_at_the_ends() {
        let coll = step();
        let mut s = hanging(&coll);
        s.run(&coll, PadInput::neutral(), 30);
        let y0 = s.pos()[1];
        // Stick left (camera along +x): shimmy 0x1a toward +y until the ledge ends at y = 411.
        let left = PadInput::neutral().stick(-1.0, 0.0);
        assert!(s.run_until(&coll, left, 5, 0x1a), "{:x?}", dedup(&s.states));
        s.run(&coll, left, 400);
        let y1 = s.pos()[1];
        assert!(y1 > y0 + 2.0 && y1 < 411.0, "left end at {y1} (from {y0})");
        assert!(matches!(s.hero.state, 0x19 | 0x1a), "{:#x}", s.hero.state);
        s.run(&coll, PadInput::neutral(), 40);
        assert_eq!(s.hero.state, 0x19);
        assert!((s.pos()[1] - y1).abs() < 0.2, "stopped");
        // Right: 0x1b toward −y to the other end (y = 405).
        let right = PadInput::neutral().stick(1.0, 0.0);
        assert!(s.run_until(&coll, right, 5, 0x1b));
        s.run(&coll, right, 600);
        let y2 = s.pos()[1];
        assert!(y2 < y0 - 2.0 && y2 > 405.0, "right end at {y2}");
        assert!((s.pos()[2] - HANG_Z).abs() < 0.02 && (s.pos()[0] - 409.55).abs() < 0.05, "still on the ledge: {:?}", s.pos());
        eprintln!("shimmy: {y0} → {y1} → {y2}");
    }

    #[test]
    fn climb_up() {
        let coll = step();
        let mut s = hanging(&coll);
        s.run(&coll, PadInput::neutral(), 20);
        s.tick(&coll, PadInput::neutral().press(button::CROSS));
        assert_eq!(s.hero.state, 0x1c);
        assert_eq!(s.anim.v.seq_b, 0x22);
        for _ in 0..120 {
            s.tick(&coll, PadInput::neutral());
            if std::env::var("LEDGE_DEBUG").is_ok() { eprintln!("{:#x} t{} {:?} vz {} desc {} f838 {} h {} key {}", s.hero.state, s.hero.timer, s.pos(), f(s.hero.vel[2]) * 60.0, s.hero.jump.descending, s.hero.f838, f(s.hero.height), s.anim.v.frame); }
        }
        let p = s.pos();
        eprintln!("climbed: {:x?} at {p:?}", dedup(&s.states));
        assert!((p[2] - TOP).abs() < 1e-3 && p[0] > 410.1, "on top: {p:?}");
        assert!(matches!(s.hero.state, 0 | 3), "{:#x}", s.hero.state);
    }

    #[test]
    fn drop_off() {
        let coll = step();
        // R1 → fall; the 10-tick lockout keeps him from regrabbing; he lands on the floor.
        let mut s = hanging(&coll);
        s.run(&coll, PadInput::neutral(), 20);
        s.tick(&coll, PadInput::neutral().press(button::R1));
        assert_eq!(s.hero.state, 6);
        assert_eq!(s.hero.f500, 10);
        s.run(&coll, PadInput::neutral(), 60);
        assert_eq!(s.pos()[2], 100.0);
        assert!(!s.states[s.states.len() - 60..].contains(&0x18));
        // Stick back + ✕ also lets go.
        let mut s = hanging(&coll);
        s.run(&coll, PadInput::neutral(), 20);
        s.tick(&coll, PadInput::neutral().stick(0.0, 1.0));
        s.tick(&coll, PadInput::neutral().stick(0.0, 1.0).press(button::CROSS));
        assert_eq!(s.hero.state, 6, "{:x?}", dedup(&s.states));
    }

    #[test]
    fn wall_jump_off_a_tall_wall() {
        let mut q = flat(100.0, 400.0, 420.0, 396.0, 424.0);
        q.extend(block(410.0, 414.0, 400.0, 416.0, 100.0, 112.0));
        let coll = mesh(cells_of(&q));
        let mut s = Sim::new([408.0, 408.0, 100.0], 0.0);
        s.run(&coll, PadInput::neutral(), 2);
        s.run(&coll, fwd().press(button::CROSS), 12);
        let mut opened = None;
        for t in 0..40 {
            s.tick(&coll, fwd().press(button::CROSS));
            if s.hero.ledge != 0 && opened.is_none() { opened = Some(t); }
            if opened.is_some() { break; }
        }
        assert!(opened.is_some(), "no wall window: {:x?} at {:?}", dedup(&s.states), s.pos());
        s.tick(&coll, fwd());
        s.tick(&coll, fwd().press(button::CROSS));
        assert_eq!(s.hero.state, 0x11, "{:x?}", dedup(&s.states));
        assert_eq!(s.anim.v.seq_b, 0x23);
        assert!(diff_rots(s.hero.ledge_blk.jump_yaw, PI_F) < 1e-4);
        let x0 = s.pos()[0];
        s.run(&coll, PadInput::neutral(), 30);
        let p = s.pos();
        eprintln!("wall jump: {:x?} {x0} → {p:?}", dedup(&s.states));
        assert!(p[0] < x0 - 1.0, "pushed off the wall: {x0} → {}", p[0]);
        assert!(f(s.hero.rot[2]).abs() > 2.5, "turned to the wall normal: {}", f(s.hero.rot[2]));
    }

    impl LoopAnim {
        fn calls_seq(&self) -> u8 { self.v.seq_b }
    }
}
