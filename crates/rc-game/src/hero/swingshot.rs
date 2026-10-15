//! **Package P6 — the Swingshot.** Item 12 (owned flag 0x13d4cc, hand class 0xd0, its hook moby class 0xd1) and
//! its two target classes: **758** (0x2f6, pull targets) and **803** (0x323, swing targets). The states exist on
//! every level but Novalis; the reference is **level00** (SetState 0x2223f8, physics 0x217970, transitions
//! 0x229b70, the weapon check 0x227fa0 with the target searches 0x222158 / 0x221ee0); the hand item's update is
//! level01 0x2dbdc0 (every level has it) and the targets' update level03 0x2d0bd8 ([`crate::moby_update::classes::
//! swing_target`]). docs/plan/hero_states.md "P6". New code in standard `f32` (`Pf::f` / `to_f32` at the
//! hero-block boundary, as `boots.rs`); no hardware modelling.
//!
//! **Targets.** Each target's pvar record ([`Record`], `moby+0x78`): +0x00 kind (0 pull, 1 swing), +0x04 the rope /
//! hang length (7 u), +0x08 the pull mode (0 → 0x25, else 0x26; 0 on every disc instance), +0x0c the swing's speed
//! cap (u/tick, 22 u/s), +0x10 the value SetState(0x2c) copies to +0x14 (else +0x0c), +0x1c a cuboid in which the
//! target is not offered (−1 none), +0x20 the reach (0: 30 pull / 16 swing), +0x24 / +0x28 / +0x2c the rope spring
//! k / d / max (0: 0.025, 0.33, 11·dt), +0x30 a mission that must be done (−1 none), +0x34 turn the swing yaw to
//! the target's facing, +0x38 its turn velocity, +0x3c a moby that must be dead (−1 none). The hero sees the
//! targets of this tick's moby run list (`0x15ffe4`, [`Targets`], through [`super::platform::HeroWorld::
//! swing_targets`]).
//!
//! **Target searches** (every tick the Swingshot is ready in hand, from the weapon check [`super::gadgets`]):
//! * pull `0x222158` (skipped in group 0xd, which keeps the target): class 758, kind 0, not hidden, within the
//!   reach (3-D), scored by [`score`] (range 30 or the reach, ±30° of the facing — the grind rail's yaw in group
//!   0xf —, 5° / 5° in the look stance), not within 4 (2-D), +10 when not drawn last frame, not with the hero in
//!   its cuboid; the best score wins (0x13fcb4, its +0x08 → 0x13fcc8, +0x04 → 0x13fcd0, score 0x13fcd4); usable
//!   (0x13fcb8) when a line (flags 2) from 0.5 above the feet reaches it.
//! * swing `0x221ee0`: class 803, kind ≠ 0, not hidden, the hero at most 3 above it, within the reach, range 16 or
//!   the reach, ±45° / 45° (±70° / 65° in 0x2d, 5° in the look stance), +10 when not drawn, the cuboid; 0x13fce0 =
//!   the best (or none), usable (0x13fcee, score 0x13fce8) with a clear line.
//!
//! **Firing** (the weapon check's case 0xc, [`fire`]): in groups 0, 1, 2, 4, 5, 0xf or state 0x1e, ○ pressed
//! within 5 ticks (8 in the air), or held after 15 ticks in hand: a usable swing target (the better score when
//! both are usable) → **0x2c**; else a usable pull target → **0x24**; both need 0x13f536 = 0 (40 ticks after a
//! swing starts).
//!
//! **States.**
//! * **0x24 fire** (group 0xd): faces the target (TurnTo 0.02 / 0.15 / 360°/s), the ground case's braking at 12.6
//!   u/s², anim 0x2f looping 10..14; back to idle when the hand item is not the Swingshot. The hand item's update
//!   (below) flies the hook and, once it holds, sets **0x25** (0x26 for mode ≠ 0) or drops him (blocked: fall 6
//!   / idle; ○ released: idle).
//! * **0x25 pull** (group 0xd, anim 0x30; 0x1413fc = 1): speed 0x13fcb0 → 27 u/s at 80 u/s²; the flight aims the
//!   left hand (0.57 left) at the target; from the first tick at which braking at 42 u/s² stops 1.9 short of it
//!   (the roots of `−21·dt²·t² + v·t − (d − 1.9)`) the speed brakes (substate 1); the body pitches toward the
//!   target (370°/s) and levels out (220°/s) in the last 20 ticks; within 30 ticks of arrival it sinks by
//!   `29·dt²·(30 − eta)`. Anim 10 in the last 22 ticks. ○ released or braked below 4 u/s: fall 6 (gravity 29,
//!   42·dt²) or idle below 0.3, speed ≤ 7 u/s.
//! * **0x26 arrive** (mode ≠ 0): the hang length 0x13fccc → the record's +0x04 at 10 u/s, a pendulum about the
//!   target (0.9 damping, 15·dt² gravity); ○ released → fall 6 above 0.4, else idle.
//! * **0x2c swing** (group 0xe, anim 0x34 then 0x35; 0x1413fc = 1): rope length 0x13fcf4 from the entry distance,
//!   springing to the record's +0x04 (k / d / max 0x13fd14..0x13fd1c, the shortening pulls him in); the body leans
//!   toward the target (0.04 / 0.2 / 550°/s); once the hook holds (0x13fcec) gravity 40·dt² (57 more while rising
//!   on the first swing), air drag 0.015·dt, the stick pumps (0.21·dt·|stick| ≤ 5·dt below the target after 40
//!   ticks), the speed cap +0x0c (15·dt² approach), the rope constraint on the right hand (0.15), the first swing's
//!   kick toward the target (14 u/s spring); at each bottom of the arc the anim 0x35 is re-timed to half a
//!   pendulum period `π·√(L / g)`. ○ released after 10 ticks → **0x2d** (gravity 27, 7·dt²).
//! * **0x2d** the fall after a swing (group 2): the fall's physics and transitions (air.rs), the body rolling back
//!   upright (0.02 / 0.3 / 150°/s; pitched 25° forward above 4 u), substate 1 above 1 u.
//! * The Euler straightening 0x236520 skips 0x25 / 0x26 / 0x2c / 0x2d (swim.rs).
//!
//! **The hand item** (level01 0x2dbdc0, [`item_update`], after the slot loop): it creates its hook (class 0xd1,
//! kept here in [`Hook`]), which rides the item's joint 0; in 0x24 the hook flies to the pull target at 24 u/s (48
//! after a fall / jump from above 2 u; + the grind speed), in 0x2c to the swing target in 7 ticks (5 in the first
//! 30); the rope (drawn by `0x2dba30`, [`Rope`]) waves with an amplitude 0.28 that decays ×0.9 (pull) / ×0.4
//! (swing) per tick; below 0.168 the hook holds (pull: SetState 0x25; swing: 0x13fcec = 1). Afterwards it
//! retracts at 80 u/s. Item anims 1 (idle), 3 (fire), 4 (hooked); class sounds 0 (fire), 1 (hit), 2 (pull), queued
//! in [`SwingItem::sounds`] and played right after the item's update (`super::gadgets::flush_item_sounds`). In the look
//! stance it turns 1 into 0x1e and registers its screen markers (`0x20fb60`: the red centre, the green target).
//!
//! The targets' glints are type-60 particles (`moby_update::classes::swing_target`). **Not ported:**
//! their camera hint `0x2eb4c0` (the camera's look-at record 0x167100); the targeted swing from a grind rail
//! (0x13f904); the stats / help counters 0x1416e0..0x1416e4 (0x13fcd8 is kept); the look-stance aiming beams.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::anim::AnimCtl;
use super::items::ItemData;
use super::physics::*;
use super::states::Ctx;
use super::Hero;
use crate::collision_query::{coll_line_m, QueryFlags};
use crate::pad::fast_diff_rots;
use crate::ps2v::Pf;
use crate::rng::Rng;

/// Item id (`0x13d4c0 + 12`).
pub const SWINGSHOT: i32 = 12;
/// The look stance's markers: the red centre one and the green target one (R low), FX 0x24.
const MARKER_RED: u32 = 0xff00_00ff;
const MARKER_GREEN: u32 = 0xff00_ff00;
const MARKER_FX: usize = 0x24;

/// The hand item's class (item definition 12 `+0x10`) and the hook's (`CreateMoby(0xd1)` in `0x2dcbf0`).
pub const SWINGSHOT_CLASS: i16 = 0xd0;
pub const HOOK_CLASS: i16 = 0xd1;
/// The target classes.
pub const PULL_CLASS: i16 = 758;
pub const SWING_CLASS: i16 = 803;

const DTF: f32 = 1.0 / 60.0;
const DT2F: f32 = 1.0 / 3600.0;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
const PI: f32 = std::f32::consts::PI;
const TAU: f32 = std::f32::consts::TAU;

type V3 = [f32; 3];
fn f3(v: V4) -> V3 { to_f32x3(v) }
fn p4(v: V3, w: Pf) -> V4 { [Pf::f(v[0]), Pf::f(v[1]), Pf::f(v[2]), w] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scl(a: V3, k: f32) -> V3 { [a[0] * k, a[1] * k, a[2] * k] }
fn len(a: V3) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
fn len_xy(a: V3) -> f32 { (a[0] * a[0] + a[1] * a[1]).sqrt() }
fn set_len(a: V3, l: f32) -> V3 {
    let n = len(a);
    if n == 0.0 { [0.0; 3] } else { scl(a, l / n) }
}
fn dist(a: V3, b: V3) -> f32 { len(sub(a, b)) }
fn dist_xy(a: V3, b: V3) -> f32 { len_xy(sub(a, b)) }
fn pf(x: f32) -> Pf { Pf::f(x) }
fn yaw_to(from: V3, to: V3) -> f32 { (to[1] - from[1]).atan2(to[0] - from[0]) }
fn wrap(a: f32) -> f32 {
    let mut x = a;
    while x >= PI { x -= TAU; }
    while x < -PI { x += TAU; }
    x
}
/// `FastDiffRots`: |a − b| wrapped into [0, π].
fn diff_rots(a: f32, b: f32) -> f32 { fast_diff_rots(pf(a), pf(b)).to_f32() }
/// `0x270ac0(target, step, &x)`: x turns toward the target by at most `step` (shortest way).
fn approach_rot(target: f32, step: f32, x: &mut f32) {
    let d = wrap(target - *x).clamp(-step, step);
    *x = wrap(*x + d);
}
/// `Approach(t, step, &x)` 0x270728.
fn approach(t: f32, step: f32, x: &mut f32) { *x += (t - *x).clamp(-step, step); }
/// `Spring(t, k, d, max, &x, &v)` 0x270780 (velocity clamped to ±max and ±|t − x|, a snap within 1 % of max).
fn spring(t: f32, k: f32, d: f32, max: f32, x: &mut f32, v: &mut f32) {
    let e = t - *x;
    let mut nv = *v + (k * e - d * *v);
    if 0.0 < max { nv = nv.clamp(-max, max); }
    nv = nv.clamp(-e.abs(), e.abs());
    *v = nv;
    *x += nv;
    if (t - *x).abs() < max * 0.01 {
        *x = t;
        *v = 0.0;
    }
}
fn blend(n: i32) -> Pf { Pf::from_i32(ticks(n)) }

// ------------------------------------------------------------------------------------------------
// Targets.

/// A target's pvar record (`moby+0x78`, 0x40 bytes). See the module doc.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Record {
    pub kind: i32,
    pub length: f32,
    pub mode: i32,
    pub max_speed: f32,
    pub f10: f32,
    pub cuboid: i32,
    pub range: f32,
    pub k: f32,
    pub d: f32,
    pub max: f32,
    pub mission: i32,
    pub face: i32,
    pub link: i32,
}

impl Record {
    /// The record out of a pvar block (None: shorter than 0x40 bytes).
    pub fn parse(pv: &[u8]) -> Option<Record> {
        if pv.len() < 0x40 { return None; }
        let i = |o: usize| i32::from_le_bytes(pv[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_bits(i(o) as u32);
        Some(Record {
            kind: i(0),
            length: f(4),
            mode: i(8),
            max_speed: f(0xc),
            f10: f(0x10),
            cuboid: i(0x1c),
            range: f(0x20),
            k: f(0x24),
            d: f(0x28),
            max: f(0x2c),
            mission: i(0x30),
            face: i(0x34),
            link: i(0x3c),
        })
    }
}

/// One target moby as the hero sees it this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub id: usize,
    pub o_class: i16,
    /// +0x10 position; +0x48 facing (the swing target turns to the hero).
    pub pos: V3,
    pub yaw: f32,
    /// +0x31 drawn last frame; mode bit 1 hidden.
    pub visible: bool,
    pub hidden: bool,
    pub rec: Record,
    /// The cuboid of `rec.cuboid` (the level's cuboid section, `0x1600ec`).
    pub cuboid: Option<rc_formats::volumes::Shape>,
}

/// The targets of this tick's moby run list in its order, and the camera the look-stance scoring reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Targets {
    pub list: Vec<Target>,
    /// 0x167240 camera position, 0x167258 yaw, 0x167254 pitch.
    pub camera: V3,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
}

impl Targets {
    /// The Swingshot targets among `order` (the moby run list: `0x15ffe4`), with their records and cuboids.
    pub fn collect(table: &crate::moby_runtime::MobyTable, order: &[usize], volumes: Option<&rc_formats::volumes::Volumes>, camera: V3, cam_yaw: f32, cam_pitch: f32) -> Targets {
        let list = order
            .iter()
            .filter_map(|&id| {
                let m = table.mobys.get(id)?;
                if m.o_class != PULL_CLASS && m.o_class != SWING_CLASS || m.state >= 0x80 { return None; }
                let rec = Record::parse(&m.pvars)?;
                let cuboid = volumes.and_then(|v| v.cuboids.get(usize::try_from(rec.cuboid).ok()?).copied());
                Some(Target {
                    id,
                    o_class: m.o_class,
                    pos: [m.position[0], m.position[1], m.position[2]],
                    yaw: m.rotation[2],
                    visible: m.visible != 0,
                    hidden: m.mode & crate::moby_runtime::mode::HIDDEN != 0,
                    rec,
                    cuboid,
                })
            })
            .collect();
        Targets { list, camera, cam_yaw, cam_pitch }
    }

    pub fn get(&self, id: Option<usize>) -> Option<&Target> { self.list.iter().find(|t| Some(t.id) == id) }
}

fn targets<'a>(env: &'a Env) -> Option<&'a Targets> { env.world.and_then(|w| w.swing_targets()) }

/// `0x22dff0(yaw, range, max_yaw, max_pitch, moby, &reject)`: the target's score (distance weighted by the
/// angle off the facing), or None when out of range or outside the cone. In the look stance (1 / 0x1e) the
/// angles are the camera's.
fn score(h: &Hero, ts: &Targets, t: &Target, yaw: f32, range: f32, max_yaw: f32, max_pitch: f32) -> Option<f32> {
    let mut p = f3(h.pos);
    if h.state == 0x14 { p[2] = h.ground_z.to_f32(); }
    let d = dist(p, t.pos);
    let mut reject = range < d;
    let mut a = diff_rots(yaw_to(p, t.pos), yaw);
    let mut worst = a;
    let mut b = diff_rots((t.pos[2] - p[2]).atan2(dist_xy(p, t.pos)), 0.0);
    if h.state == 1 || h.state == 0x1e {
        let c = ts.camera;
        let ay = diff_rots(yaw_to(c, t.pos), ts.cam_yaw);
        b = diff_rots((t.pos[2] - c[2]).atan2(dist_xy(c, t.pos)), -ts.cam_pitch);
        a = ay + b;
        worst = ay.max(b);
    }
    if 0.0 < max_yaw && max_yaw < worst { reject = true; }
    if 0.0 < max_pitch && max_pitch < b { reject = true; }
    if reject { return None; }
    // `0x273278`: +7 for a crate (classes 500..540): never a target class.
    let crate_ = (500..=540).contains(&t.o_class);
    Some(d + a * d + if crate_ { 7.0 } else { 0.0 })
}

/// `0x274820(p, cuboid)`: `p` inside the target's cuboid.
fn in_cuboid(t: &Target, p: V3) -> bool {
    if t.rec.cuboid < 0 { return false; }
    let Some(c) = &t.cuboid else { return false };
    let l = c.local(p);
    (-1.0..=1.0).contains(&l[0]) && (-1.0..=1.0).contains(&l[1]) && (-1.0..=1.0).contains(&l[2])
}

/// `CollLine_Fix(feet + 0.5 up, target, 2, 0, 0)` hits nothing.
fn clear_line(env: &Env, h: &Hero, to: V3) -> bool {
    let mut a = f3(h.pos);
    a[2] += 0.5;
    coll_line_m(env.coll, env.mobys, a, to, QueryFlags(2), None).is_none()
}

/// The pull target search `0x222158`; returns 0x13fcb8.
pub(super) fn pull_search(h: &mut Hero, env: &Env) -> bool {
    if h.group == 0xd { return h.swing.pull_ok; }
    h.swing.pull_ok = false;
    let Some(ts) = targets(env) else { return false };
    let pos = f3(h.pos);
    let (mut best, mut sel) = (1e8f32, None);
    for t in &ts.list {
        if t.o_class != PULL_CLASS || t.hidden || t.rec.kind != 0 { continue; }
        if t.rec.range != 0.0 && t.rec.range < dist(pos, t.pos) { continue; }
        let (mut ma, mut mp) = (std::f32::consts::FRAC_PI_6, -1.0);
        if h.state == 1 || h.state == 0x1e { (ma, mp) = (0.087_266_46, 0.087_266_46); }
        let yaw = if h.group == 0xf { h.boots.yaw } else { h.rot[2].to_f32() };
        let range = if t.rec.range == 0.0 { 30.0 } else { t.rec.range };
        let Some(mut s) = score(h, ts, t, yaw, range, ma, mp) else { continue };
        if dist_xy(pos, t.pos) < 4.0 { continue; }
        if !t.visible { s += 10.0; }
        if in_cuboid(t, pos) { continue; }
        if s < best {
            best = s;
            sel = Some(t);
        }
    }
    if let Some(t) = sel {
        let s = &mut h.swing;
        s.pull = Some(t.id);
        s.pull_pos = t.pos;
        s.pull_mode = t.rec.mode;
        s.pull_score = best;
        s.pull_len = t.rec.length;
        let to = t.pos;
        if clear_line(env, h, to) { h.swing.pull_ok = true; }
    }
    h.swing.pull_ok
}

/// The swing target search `0x221ee0` (0x13fce0, 0x13fcee).
pub(super) fn swing_search(h: &mut Hero, env: &Env) {
    h.swing.swing_ok = false;
    let Some(ts) = targets(env) else {
        h.swing.swing = None;
        return;
    };
    let pos = f3(h.pos);
    let (mut best, mut sel) = (1e8f32, None);
    for t in &ts.list {
        if t.o_class != SWING_CLASS || t.rec.kind == 0 || t.hidden { continue; }
        if 3.0 < pos[2] - t.pos[2] { continue; }
        if t.rec.range != 0.0 && t.rec.range < dist(pos, t.pos) { continue; }
        let (mut ma, mut mp) = if h.state == 0x2d { (1.221_730_5, 1.134_464) } else { (std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4) };
        if h.state == 1 || h.state == 0x1e { (ma, mp) = (0.087_266_46, 0.087_266_46); }
        let range = if t.rec.range == 0.0 { 16.0 } else { t.rec.range };
        let Some(mut s) = score(h, ts, t, h.rot[2].to_f32(), range, ma, mp) else { continue };
        if !t.visible { s += 10.0; }
        if in_cuboid(t, pos) { continue; }
        if s < best {
            best = s;
            sel = Some(t);
        }
    }
    h.swing.swing = sel.map(|t| t.id);
    if let Some(t) = sel {
        h.swing.swing_pos = t.pos;
        let to = t.pos;
        if clear_line(env, h, to) {
            h.swing.swing_score = best;
            h.swing.swing_ok = true;
        }
    }
}

/// The weapon check's case 0xc (level00 0x227fa0): both searches, then 0x2c / 0x24 on ○.
pub(super) fn fire(h: &mut Hero, c: &mut Ctx) {
    swing_search(h, c.env);
    pull_search(h, c.env);
    let n = if h.air_ticks != 0 { ticks(8) } else { ticks(5) };
    let g = h.group;
    if !(g <= 1 || g == 4 || g == 2 || g == 0xf || g == 5 || h.state == 0x1e) { return; }
    let mask = h.items.slot.fire_mask;
    let pad = c.env.pad;
    let go = pad.pressed_within(mask, n).is_some() || (pad.held & mask != 0 && ticks(15) < h.items.slot.ticks_ready);
    if !go { return; }
    let s = &mut h.swing;
    if s.swing_ok {
        if s.pull_ok {
            if s.swing_score < s.pull_score { s.pull_ok = false; } else { s.swing_ok = false; }
        }
        if s.swing_ok && h.f536 == 0 && h.state != 0x2c {
            h.set_state(c, 0x2c, true);
            return;
        }
    }
    if h.swing.pull_ok && h.f536 == 0 { h.set_state(c, 0x24, true); }
}

// ------------------------------------------------------------------------------------------------
// The hero-block fields and the hand item.

/// The hook moby (class 0xd1, created by the hand item `0x2dcbf0`; the port keeps it with the item, as the
/// hand item itself).
#[derive(Clone, Debug, PartialEq)]
pub struct Hook {
    /// +0x10 position; +0xc0.. rows (copied from the hand item while in hand).
    pub pos: V3,
    pub rows: [[f32; 4]; 3],
    /// Its pvar +0x04: the flight speed (u/tick).
    pub speed: f32,
    /// Its state byte +0x20 and animation (the claws: [`hook_update`]), with the blend's pose snapshot.
    pub state: u8,
    pub anim: rc_formats::moby_anim::AnimState,
    pub snapshot: Option<rc_formats::moby_anim::MobyFrame>,
}

impl Default for Hook {
    fn default() -> Self {
        let anim = rc_formats::moby_anim::AnimState { seq_a: 0, frame_a: 0, seq_b: 0, frame_b: 0, t: 0.0, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
        Hook { pos: [0.0; 3], rows: [[0.0; 4]; 3], speed: 0.0, state: 0, anim, snapshot: None }
    }
}

/// The hook's update `0x2dccd8` (after its `MobyAnimAdvance`): its claws follow the hand item's state (+0x20). 0: the
/// fold-out (sequence 0) → at its wrap sequence 1 (open, idle), 1; 1: the item fired (2) → sequence 3, 3, or put away
/// (8) → sequence 2, 2; 2: at the wrap the hook is gone (deleted; the item makes a new one); 3: at the wrap sequence 4, 4;
/// 4: the item in 3..5 → sequence 5, 5; 5: at the wrap its speed +0x58 = 0 (held on the last frame), the item in 6..7 →
/// speed 1, sequence 7, 7; 7: at the wrap sequence 1, 1. Every change blends over `ticks(10)`.
fn hook_update(hook: &mut Hook, item_state: u8, class: &rc_formats::moby_anim::MobyAnimClass) -> bool {
    use rc_formats::moby_anim::{advance, set_sequence};
    advance(&mut hook.anim, class);
    let wrapped = hook.anim.flags & 2 != 0;
    let blend = super::physics::ticks(10);
    let to = |hook: &mut Hook, seq: u8, state: u8| {
        if hook.anim.seq_b != seq { set_sequence(&mut hook.anim, class, seq, 0, blend, &mut hook.snapshot); }
        hook.state = state;
    };
    match hook.state {
        0 => {
            if wrapped { to(hook, 1, 1); hook.anim.speed = 1.0; }
        }
        1 => match item_state {
            2 => to(hook, 3, 3),
            8 => { to(hook, 2, 2); hook.anim.speed = 1.0; }
            _ => {}
        },
        2 => return !wrapped,
        3 => if wrapped { to(hook, 4, 4); },
        4 => if (3..=5).contains(&item_state) { to(hook, 5, 5); },
        5 => {
            if wrapped { hook.anim.speed = 0.0; }
            if (6..=7).contains(&item_state) {
                hook.anim.speed = 1.0;
                to(hook, 7, 7);
            }
        }
        6 => {}
        _ => if wrapped || hook.state != 7 { to(hook, 1, 1); },
    }
    true
}

/// The rope the hand item draws this tick (`0x2dba30`, registered by `0x21afe0`): from the item's joint 0 to
/// the hook, a camera-facing strip of 0.2-long, 0.1-wide quads (effect texture 0xf, colour 0x80808080) displaced
/// sideways by `sin(π·s/L) · sin(π·s/wavelength) · cos(phase) · amplitude`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rope {
    pub from: V3,
    pub to: V3,
    pub phase: f32,
    pub wavelength: f32,
    pub amplitude: f32,
}

/// The hand item's pvars (`moby+0x78`: +0x14 the hook, +0x18 phase, +0x1c wavelength, +0x20 amplitude) and
/// state byte (+0x20), with what it drew and played this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwingItem {
    /// The item exists (its pvars are fresh on creation).
    pub alive: bool,
    pub state: u8,
    pub hook: Option<Hook>,
    pub phase: f32,
    pub wavelength: f32,
    pub amplitude: f32,
    /// The rope drawn this tick.
    pub rope: Option<Rope>,
    /// Class sounds queued this tick (`fun_0022da68(index, 0, item)`).
    pub sounds: Vec<i32>,
}

/// The Swingshot fields of the hero block (0x13fcb0..0x13fd1c) and the hand item's.
#[derive(Clone, Default, PartialEq)]
pub struct Swing {
    /// 0x13fcb0 pull speed; 0x13fcb4 pull target (+ its position); 0x13fcb8 usable; 0x13fcbc ticks to arrive;
    /// 0x13fcc0 distance; 0x13fcc4 the final 20 ticks; 0x13fcc8 mode; 0x13fccc hang length; 0x13fcd0 its
    /// target (+0x04); 0x13fcd4 score; 0x13fcd8 the help flag.
    pub pull_speed: f32,
    pub pull: Option<usize>,
    pub pull_pos: V3,
    pub pull_ok: bool,
    pub pull_eta: f32,
    pub pull_dist: f32,
    pub pull_near: i32,
    pub pull_mode: i32,
    pub hang: f32,
    pub pull_len: f32,
    pub pull_score: f32,
    pub help: i32,
    /// 0x13fce0 swing target (+ position); 0x13fce4 the one being swung on; 0x13fce8 score; 0x13fcec hooked;
    /// 0x13fcee usable.
    pub swing: Option<usize>,
    pub swing_pos: V3,
    pub on: Option<usize>,
    pub on_pos: V3,
    pub swing_score: f32,
    pub hooked: bool,
    pub swing_ok: bool,
    /// 0x13fcf0 rope target length; 0x13fcf4 rope length; 0x13fcf8 its spring velocity; 0x13fcfc yaw to the
    /// target; 0x13fd00 gravity; 0x13fd04 kick speed; 0x13fd08 yaw velocity; 0x13fd0c anim re-time (1 / 2);
    /// 0x13fd0e first swing; 0x13fd14 / 18 / 1c rope spring k, d, max.
    pub rope_target: f32,
    pub rope: f32,
    pub rope_vel: f32,
    pub yaw: f32,
    pub g: f32,
    pub kick: f32,
    pub yaw_vel: f32,
    pub retime: i16,
    pub first: bool,
    pub k: f32,
    pub d: f32,
    pub max: f32,
    /// 0x13f904: a targeted swing from a grind rail (armed by no ported class; cleared by the item).
    pub f904: i32,
    /// The hand item.
    pub item: SwingItem,
}

impl std::fmt::Debug for Swing {
    /// `0` while untouched (the hero digest drops new zero fields), else the fields.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self == Swing::default() { return write!(f, "0"); }
        f.debug_struct("Swing")
            .field("pull", &(self.pull, self.pull_ok, self.pull_speed, self.pull_eta, self.pull_dist, self.pull_near, self.hang))
            .field("swing", &(self.swing, self.swing_ok, self.on, self.hooked, self.rope, self.rope_target, self.rope_vel, self.yaw, self.g))
            .field("anim", &(self.retime, self.first, self.kick))
            .field("item", &self.item)
            .finish()
    }
}

/// The target's position now (the moby, when the hero sees it this tick), else the one remembered.
fn target_pos(env: &Env, id: Option<usize>, fallback: V3) -> V3 { targets(env).and_then(|t| t.get(id)).map_or(fallback, |t| t.pos) }

// ------------------------------------------------------------------------------------------------
// SetState 0x2223f8 (level00).

/// SetState entry of 0x24..0x26, 0x2c, 0x2d. `None`: continue with SetState's epilogue.
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, _old_sub: i32) -> Option<bool> {
    match id {
        0x24 => {
            h.group = 0xd;
            h.f15d4 = 7;
            let t = target_pos(c.env, h.swing.pull, h.swing.pull_pos);
            h.target_yaw = pf(yaw_to(f3(h.pos), t));
            if play {
                h.set_anim(c.anim, c.rng, blend(5), 0x2f, 2);
                c.anim.set_loop(10, 14);
            }
        }
        0x25 | 0x26 => {
            h.items.f13fc = 1;
            h.group = 0xd;
            h.f15d4 = 7;
            h.swing.pull_speed = 0.0;
            h.swing.pull_near = 0;
            if id == 0x26 {
                let t = target_pos(c.env, h.swing.pull, h.swing.pull_pos);
                h.swing.hang = dist(f3(h.pos), t);
            }
            if play {
                let b = if id == 0x25 { 10 } else { 5 };
                h.set_anim(c.anim, c.rng, blend(b), 0x30, 1);
            }
        }
        0x2c => swing_entry(h, c, play),
        0x2d => {
            h.f15d4 = 7;
            h.group = 2;
            h.swim.euler_vel = [Pf::ZERO; 2];
            h.vel = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(15), 0xb, 0); }
        }
        _ => {}
    }
    None
}

fn swing_entry(h: &mut Hero, c: &mut Ctx, play: bool) {
    h.group = 0xe;
    h.f15d4 = 7;
    let s = &mut h.swing;
    s.first = true;
    s.rope_vel = 0.0;
    h.items.f13fc = 1;
    s.on = s.swing;
    s.on_pos = s.swing_pos;
    s.hooked = false;
    s.retime = 0;
    s.kick = 0.0;
    s.yaw_vel = 0.0;
    let t = targets(c.env).and_then(|t| t.get(s.on)).cloned();
    let rec = t.as_ref().map(|t| t.rec).unwrap_or_default();
    if let Some(t) = &t { s.on_pos = t.pos; }
    s.rope_target = rec.length;
    // (The record's +0x14 = +0x10, or +0x0c when 0: a write into the target's pvars nothing reads.)
    s.k = 0.025;
    s.d = 0.33;
    s.max = DTF * 11.0;
    h.swim.euler_vel = [Pf::ZERO; 2];
    h.f536 = ticks(40) as i16;
    let s = &mut h.swing;
    if rec.k != 0.0 { s.k = rec.k; }
    if rec.d != 0.0 { s.d = rec.d; }
    if rec.max != 0.0 { s.max = rec.max; }
    let pos = f3(h.pos);
    s.yaw = yaw_to(pos, s.on_pos);
    s.rope = dist(pos, s.on_pos);
    s.g = DT2F * 27.0;
    if h.prev_group == 4 {
        s.g = h.jump.g.to_f32();
    } else if h.prev_group == 2 {
        s.g = h.group_gravity.to_f32();
    }
    if play {
        h.set_anim(c.anim, c.rng, blend(11), 0x34, 0);
        h.anim_speed = pf(0.7);
    }
}

// ------------------------------------------------------------------------------------------------
// Physics 0x217970 (level00).

/// Per-state physics; false = not ported (the hero freezes).
pub(super) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) -> bool {
    match h.state {
        0x24 => phys_fire(h, env),
        0x25 => phys_pull(h, env),
        0x26 => phys_arrive(h, env),
        0x2c => phys_swing(h, env, anim, rng),
        0x2d => {
            phys_swing_fall(h);
            h.phys_fall(env);
        }
        _ => return false,
    }
    true
}

/// 0x24: the ground case's braking, facing the target.
fn phys_fire(h: &mut Hero, env: &Env) {
    let k = DT2 * Pf::b(0x4149_999a); // 12.6·dt²
    h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
    h.target_speed = Pf::ZERO;
    h.speed_step(k, k);
    h.vel = V0;
    h.momentum_decay(k);
    if h.air_ticks == 0 {
        let z = h.vel[2];
        h.gravity_from(z, DT2 * Pf::f(54.0));
    } else {
        let z = h.eff_v[2];
        h.gravity_from(z, DT2 * Pf::f(24.0));
        h.climb_check(env);
    }
    if h.swing.pull.is_none() { return; }
    let t = target_pos(env, h.swing.pull, h.swing.pull_pos);
    h.target_yaw = pf(yaw_to(f3(h.pos), t));
    h.turn_to(pf(0.02), pf(0.15), pf(DTF * TAU));
}

/// 0x25: the flight to the pull target.
fn phys_pull(h: &mut Hero, env: &Env) {
    let t = target_pos(env, h.swing.pull, h.swing.pull_pos);
    let pos = f3(h.pos);
    let s = &mut h.swing;
    s.pull_dist = dist(pos, t);
    s.pull_eta = ticks(60) as f32;
    if 0.0 < s.pull_speed { s.pull_eta = s.pull_dist / h.eff_len.to_f32(); }
    if h.substate == 0 {
        approach(DTF * 27.0, DT2F * 80.0, &mut s.pull_speed);
        h.target_yaw = pf(yaw_to(pos, t));
        h.turn_to(pf(0.03), pf(0.15), pf(DTF * 7.330_383));
    } else if h.substate == 1 {
        approach(0.0, DT2F * 42.0, &mut h.swing.pull_speed);
    }
    let s = &mut h.swing;
    let (n, r) = super::common::quadratic(pf(-(DT2F * 42.0) * 0.5), pf(s.pull_speed), pf(-(s.pull_dist - 1.9)));
    if 0 < n && 0.0 < r.to_f32() {
        h.substate = 1;
        s.pull_eta = r.to_f32();
    }
    if s.pull_eta < ticks(20) as f32 { s.pull_near = 1; }
    let mut pitch = h.rot[1].to_f32();
    if s.pull_near == 0 {
        let elev = (t[2] - pos[2]).atan2(dist_xy(pos, t));
        approach_rot(wrap(HALF_PI - elev), DTF * 6.457_718, &mut pitch);
    } else {
        approach_rot(0.0, DTF * 3.839_724, &mut pitch);
    }
    h.rot[1] = pf(pitch);
    // The left hand, 0.57 to the side, flies at the target.
    let yaw = h.rot[2].to_f32() - HALF_PI;
    let hand = add([yaw.cos() * -0.57, yaw.sin() * -0.57, 0.0], pos);
    let v = set_len(sub(t, hand), h.swing.pull_speed);
    h.vel = p4(v, h.vel[3]);
    let eta = h.swing.pull_eta;
    if !((ticks(30) as f32) <= eta) {
        h.vel[2] = pf(h.vel[2].to_f32() - DT2F * 29.0 * (ticks(30) as f32 - eta));
    }
}

/// 0x26: the pendulum about the pull target.
fn phys_arrive(h: &mut Hero, env: &Env) {
    let t = target_pos(env, h.swing.pull, h.swing.pull_pos);
    let s = &mut h.swing;
    approach(s.pull_len, DTF * 10.0, &mut s.hang);
    let mut v = scl(f3(h.disp), 0.9);
    v[2] -= DT2F * 15.0;
    let pos = f3(h.pos);
    s.pull_dist = dist(pos, t);
    let d = set_len(sub(add(pos, v), t), s.hang);
    h.vel = p4(sub(add(t, d), pos), h.vel[3]);
}

/// 0x2c: the swing.
fn phys_swing(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) {
    let v0 = anim.view();
    if v0.flags & 2 != 0 && v0.seq_b == 0x34 {
        h.set_anim(anim, rng, blend(12), 0x35, 10);
        h.anim_speed = pf(0.3);
    }
    let vz0 = h.vel[2].to_f32();
    let tg = targets(env).and_then(|t| t.get(h.swing.on)).cloned();
    let t = tg.as_ref().map_or(h.swing.on_pos, |t| t.pos);
    let rec = tg.as_ref().map(|t| t.rec);
    // The swing's yaw turns to the target's facing (record +0x34; never set on the disc).
    if let (Some(tg), Some(r)) = (&tg, rec) {
        if r.face != 0 {
            let mut a = tg.yaw;
            if HALF_PI < diff_rots(a, h.rot[2].to_f32()) { a = wrap(a + PI); }
            let (mut y, mut v) = (pf(h.swing.yaw), pf(h.swing.yaw_vel));
            turn_spring(pf(a), pf(0.02), pf(0.3), Pf::ZERO, &mut y, &mut v, 0);
            h.swing.yaw = y.to_f32();
            h.swing.yaw_vel = v.to_f32();
        }
    }
    h.target_yaw = pf(h.swing.yaw);
    h.turn_to(pf(0.02), pf(0.3), pf(DTF * 4.712_389));
    // The body leans toward the target, in the hero's yaw frame.
    let pos = f3(h.pos);
    let mut d = set_len(sub(t, pos), 1.0);
    if d[2] < 0.23 { d[2] = 0.23; }
    let yaw = h.rot[2].to_f32();
    let (sy, cy) = yaw.sin_cos();
    let l = [cy * d[0] + sy * d[1], -sy * d[0] + cy * d[1], d[2]];
    let roll = -(l[1].atan2((l[0] * l[0] + l[2] * l[2]).sqrt()));
    let pitch = l[0].atan2(l[2]);
    for (k, target) in [(1usize, pitch), (0, roll)] {
        let (mut a, mut v) = (h.rot[k], h.swim.euler_vel[k]);
        turn_spring(pf(target), pf(0.04), pf(0.2), pf(DTF * 9.599_311), &mut a, &mut v, 0);
        h.rot[k] = a;
        h.swim.euler_vel[k] = v;
    }
    let below = pos[2] < t[2];
    let mut vel = f3(h.vel);
    if h.swing.hooked {
        if below && ticks(40) < h.timer && 0.25 < h.stick_mag.to_f32() {
            h.stick_target(env, Pf::ONE);
            let a = (DTF * 0.21 * h.stick_mag.to_f32()).min(DTF * 5.0);
            let ty = h.target_yaw.to_f32();
            vel = add(vel, [ty.cos() * a, ty.sin() * a, 0.0]);
        }
        if dist_xy(pos, t) < h.swing.rope_target * 1.57 || 1.5 < h.height.to_f32() {
            vel[2] = h.disp[2].to_f32() - DT2F * 40.0;
            if h.swing.first && 0.0 < vel[2] { vel[2] -= DT2F * 57.0; }
        }
        let sp = len(vel);
        vel = add(vel, set_len(scl(vel, -1.0), sp * DTF * 0.015));
    }
    if let Some(r) = rec {
        let mut sp = len(vel);
        if r.max_speed < sp {
            approach(r.max_speed, DT2F * 15.0, &mut sp);
            vel = set_len(vel, sp);
        }
    }
    // The rope: the right hand (0.15 to the side) stays on the sphere of the rope's length.
    let side = yaw - HALF_PI;
    let hand = add([side.cos() * 0.15, side.sin() * 0.15, 0.0], pos);
    let r = set_len(sub(add(hand, vel), t), h.swing.rope);
    vel = sub(add(t, r), hand);
    let old = h.swing.rope;
    let s = &mut h.swing;
    spring(s.rope_target, s.k, s.d, s.max, &mut s.rope, &mut s.rope_vel);
    let delta = s.rope - old;
    let mut pos = pos;
    if s.rope < len(r) {
        pos = add(pos, set_len(r, delta));
        h.pos = p4(pos, h.pos[3]);
    }
    let s = &mut h.swing;
    if s.first && below {
        let target = DTF * 14.0;
        let mut xy = len_xy(vel);
        if xy < target {
            spring(target, 0.047, 0.3, 0.0, &mut xy, &mut s.kick);
            vel = add(vel, [s.yaw.cos() * s.kick, s.yaw.sin() * s.kick, 0.0]);
        }
    }
    h.vel = p4(vel, h.vel[3]);
    if !h.swing.hooked || h.air_ticks == 0 { return; }
    // At the bottom of the arc (rising again) the anim 0x35 is re-timed to half a pendulum period.
    if !((vz0 < 0.0 && 0.0 < vel[2]) || h.swing.retime != 0) { return; }
    h.swing.first = false;
    let half = (h.swing.rope_target / h.swing.g).sqrt();
    let fwd = if h.swing.retime == 0 {
        let y = h.rot[2].to_f32();
        let d = f3(h.disp);
        0.0 < d[0] * y.cos() + d[1] * y.sin()
    } else {
        h.swing.retime == 1
    };
    let v = anim.view();
    let frame = if fwd {
        if v.seq_b != 0x35 {
            h.set_anim(anim, rng, blend(17), 0x35, 6);
            h.swing.retime = 1;
            return;
        }
        42.0
    } else {
        if v.seq_b != 0x35 {
            h.set_anim(anim, rng, blend(9), 0x35, 0x15);
            h.swing.retime = 2;
            return;
        }
        12.0
    };
    if v.blending() { return; }
    h.anim_speed_for_ticks(pf(frame), pf(half * PI), pf(0.5), Pf::b(0xbf80_0000), &v);
    h.swing.retime = 0;
}

/// 0x2d's own part (before the fall case): the body rolls back upright about the point 0.6 up (`0x22a8d8`).
fn phys_swing_fall(h: &mut Hero) {
    let pitch = if 4.0 < h.height.to_f32() { 0.436_332_3 } else { 0.0 };
    let max = pf(DTF * 2.617_994);
    let (mut y, mut vy) = (h.rot[1], h.swim.euler_vel[1]);
    turn_spring(pf(pitch), pf(0.02), pf(0.3), max, &mut y, &mut vy, 0);
    let (mut x, mut vx) = (h.rot[0], h.swim.euler_vel[0]);
    turn_spring(Pf::ZERO, pf(0.02), pf(0.3), max, &mut x, &mut vx, 0);
    h.swim.euler_vel = [vx, vy];
    turn_body(h, vx.to_f32(), vy.to_f32(), 0.0);
    if 1.0 < h.height.to_f32() { h.substate = 1; }
}

/// `0x22a8d8(ax, ay, az)`: the body turned about the point 0.6 up ([`Hero::turn_about`]).
fn turn_body(h: &mut Hero, ax: f32, ay: f32, az: f32) { h.turn_about([ax, ay, az], [0.0, 0.0, 0.6]); }

// ------------------------------------------------------------------------------------------------
// Transitions 0x229b70 (level00).

/// Per-state transitions.
pub(super) fn transitions(h: &mut Hero, c: &mut Ctx) {
    let held = c.env.pad.held & h.items.slot.fire_mask != 0;
    match h.state {
        0x24 => {
            if h.items.slot.id != SWINGSHOT { h.set_state(c, 0, true); }
        }
        0x25 => {
            let v = c.anim.view();
            h.anim_speed_for_ticks(pf(10.0), pf(h.swing.pull_eta), pf(0.5), Pf::b(0xbf80_0000), &v);
            if h.swing.pull_eta < ticks(22) as f32 && h.swing.pull_eta < ticks(30) as f32 { h.set_anim(c.anim, c.rng, blend(16), 10, 0); }
            let mut release = false;
            if !held {
                if (ticks(25) as f32) < h.swing.pull_eta { h.swing.help = 1; }
                release = true;
            }
            if h.substate == 1 && h.swing.pull_speed < DTF * 4.0 { release = true; }
            if !release { return; }
            h.speed = pf(h.swing.pull_speed.min(DTF * 7.0));
            if h.height.to_f32() < 0.3 {
                h.set_state(c, 0, false);
                let seq = h.idle_seq();
                h.set_anim(c.anim, c.rng, blend(30), seq, 0);
                return;
            }
            let wrapped10 = c.anim.view().seq_b == 10;
            h.set_state(c, 6, false);
            if !wrapped10 { h.set_anim(c.anim, c.rng, blend(17), 10, 0); }
            h.fc94 = DT2 * Pf::f(42.0);
            h.f15d4 = 0;
            h.group_gravity = DT2 * Pf::f(29.0);
        }
        0x26 => {
            if held { return; }
            h.speed = pf(h.swing.pull_speed.min(DTF * 7.0));
            if 0.4 <= h.height.to_f32() {
                h.set_state(c, 6, true);
                h.f15d4 = 0;
            } else {
                h.set_state(c, 0, true);
            }
        }
        0x2c => {
            if held || h.timer <= ticks(10) { return; }
            if h.timer < ticks(40) { h.swing.help = 1; }
            h.set_state(c, 0x2d, true);
            h.fc94 = DT2 * Pf::f(7.0);
            h.group_gravity = DT2 * Pf::f(27.0);
        }
        0x2d => {
            // The fall's first step with 0x2d's 0x1415d4 = 7; the rest is the fall's (air.rs).
            if h.substate == 0 && (ticks(18) <= h.timer || Pf::b(0x3fe0_0000) < h.height) {
                h.set_anim(c.anim, c.rng, blend(16), 0xb, 0);
                h.substate = 1;
                h.f15d4 = 7;
            }
            h.tr_fall(c);
        }
        _ => {}
    }
}

// ------------------------------------------------------------------------------------------------
// The hand item's update (level01 0x2dbdc0), run with the hero's context after the slot loop.

/// The world point of the hand item's joint 0 (`FUN_002645a8(item, 0, out)`); the item's origin without its
/// class data.
fn item_joint(h: &Hero, data: &ItemData) -> V3 {
    let Some(it) = h.items.slot.item.as_ref() else { return f3(h.pos) };
    let class = data.class(it.o_class);
    let t = match class.and_then(|c| c.chains.first().filter(|ch| !ch.is_empty()).map(|ch| (c, ch))) {
        Some((c, ch)) => rc_formats::moby_anim::evaluate_chain(&c.anim, &it.anim, it.snapshot.as_ref(), ch)[3],
        None => [0.0, 0.0, 0.0, 1.0],
    };
    let k = it.scale * (1.0 / 1024.0);
    let q = [t[0] * k, t[1] * k, t[2] * k];
    let r: [[f32; 4]; 3] = it.rows.map(|row| row.map(f32::from_bits));
    std::array::from_fn(|l| r[0][l] * q[0] + r[1][l] * q[1] + r[2][l] * q[2] + it.position[l])
}

/// `fun_00212f90(item, seq, 0, ticks(10))` unless the item already plays `seq`.
fn item_anim(h: &mut Hero, data: &ItemData, seq: u8) {
    let Some(it) = h.items.slot.item.as_mut() else { return };
    if it.anim.seq_b != seq { super::items::blend_item(it, data, seq, 0, ticks(10)); }
}

fn item_wrapped(h: &Hero) -> bool { h.items.slot.item.as_ref().is_some_and(|it| it.anim.flags & 2 != 0) }

/// `CollLine_Fix(a, b, 2, 0, 0)` hits something.
fn blocked(c: &Ctx, a: V3, b: V3) -> bool { coll_line_m(c.env.coll, c.env.mobys, a, b, QueryFlags(2), None).is_some() }

/// Level01 0x2dbdc0, the Swingshot's `(*moby+0x74)(moby)` (module doc). `data`: the hand classes.
pub(super) fn item_update(h: &mut Hero, c: &mut Ctx, data: &ItemData) {
    if h.items.slot.item.is_none() { return; }
    if !h.swing.item.alive { h.swing.item = SwingItem { alive: true, ..SwingItem::default() }; }
    h.swing.item.rope = None;
    h.swing.item.sounds.clear();
    if h.state == 1 { h.set_state(c, 0x1e, true); }
    let joint = item_joint(h, data);
    // The hook's own update (`0x2dccd8`, a moby of the run list: before the hero update, on the item's state of the
    // last tick): its claws' animation; gone at the end of its put-away.
    let item_state = h.swing.item.state;
    if let (Some(k), Some(cls)) = (h.swing.item.hook.as_mut(), data.class(HOOK_CLASS)) {
        if !hook_update(k, item_state, &cls.anim) { h.swing.item.hook = None; }
    }
    // `0x2dcbf0`: the hook, created once, at the item's joint 0 with its rows, speed 24·dt; `CreateMoby` starts its
    // sequence 0 (the claws folding out).
    if h.swing.item.hook.is_none() {
        let rows = h.items.slot.item.as_ref().map(|it| it.rows.map(|r| r.map(f32::from_bits))).unwrap_or_default();
        let mut k = Hook { pos: joint, rows, speed: DTF * 24.0, ..Default::default() };
        if let Some(cls) = data.class(HOOK_CLASS) { rc_formats::moby_anim::set_sequence(&mut k.anim, &cls.anim, 0, 0, 0, &mut k.snapshot); }
        k.anim.speed = 1.0;
        h.swing.item.hook = Some(k);
    }
    // The look stance's markers (0x2dbdc0 after the hook, in 1 / 0x1e from its second tick): the red one at the screen
    // centre (`FUN_0020fb60(1.0, 0, 90°, item, 0xff0000ff, none, 0x24, −1, 4)`), then a green half-size one on the
    // usable pull target, else on the swing target.
    if (h.state == 1 || h.state == 0x1e) && 2 <= h.timer {
        use crate::targeting::Marker;
        let tick = h.gadgets.pending_tick;
        h.weapons.markers.register(tick, Marker { size: 1.0, angle: 0.0, rgba: MARKER_RED, at: None, fx: MARKER_FX });
        let green = |at: V3| Marker { size: 0.5, angle: 0.0, rgba: MARKER_GREEN, at: Some(at), fx: MARKER_FX };
        if h.swing.pull_ok && h.swing.pull.is_some() {
            let at = target_pos(c.env, h.swing.pull, h.swing.pull_pos);
            h.weapons.markers.register(tick, green(at));
        } else if h.swing.swing.is_some() {
            let at = target_pos(c.env, h.swing.swing, h.swing.swing_pos);
            h.weapons.markers.register(tick, green(at));
        }
    }
    let pull_t = target_pos(c.env, h.swing.pull, h.swing.pull_pos);
    let on_t = target_pos(c.env, h.swing.on, h.swing.on_pos);
    let rope = |h: &mut Hero| {
        let it = &mut h.swing.item;
        let hook = it.hook.as_ref().map_or(joint, |k| k.pos);
        it.rope = Some(Rope { from: joint, to: hook, phase: it.phase, wavelength: it.wavelength, amplitude: it.amplitude });
    };
    let st = h.swing.item.state;
    let next = match st {
        0 | 1 => {
            if item_wrapped(h) {
                item_anim(h, data, 1);
                h.swing.item.state = 1;
            }
            let rows = h.items.slot.item.as_ref().map(|it| it.rows.map(|r| r.map(f32::from_bits))).unwrap_or_default();
            if let Some(k) = h.swing.item.hook.as_mut() {
                k.pos = joint;
                k.rows = rows;
            }
            if h.state == 0x24 || (h.group == 0xf && h.swing.f904 != 0) {
                let it = &mut h.swing.item;
                (it.amplitude, it.wavelength, it.phase) = (0.28, 1.0, 0.0);
                if let Some(k) = it.hook.as_mut() { k.speed = DTF * 24.0; }
                item_anim(h, data, 3);
                h.swing.item.sounds.push(0);
                h.swing.item.state = 2;
            }
            if h.state == 0x2c {
                let n = if h.items.slot.ticks_ready < ticks(30) { ticks(5) } else { ticks(7) };
                let it = &mut h.swing.item;
                if let Some(k) = it.hook.as_mut() { k.speed = dist(on_t, k.pos) / n as f32; }
                (it.amplitude, it.wavelength, it.phase) = (0.28, 1.0, 0.0);
                item_anim(h, data, 3);
                h.swing.item.sounds.push(0);
                h.swing.item.state = 9;
            }
            return;
        }
        2 => {
            rope(h);
            if item_wrapped(h) { item_anim(h, data, 4); }
            let hook = h.swing.item.hook.clone().unwrap_or_default();
            let d = sub(pull_t, hook.pos);
            let mut spd = hook.speed;
            if h.group == 0xf {
                spd += h.boots.speed;
            } else if (h.prev_group == 4 || h.prev_group == 2) && 2.0 < h.height.to_f32() {
                spd = DTF * 48.0;
            }
            let mut a = f3(h.pos);
            a[2] += 0.5;
            if !blocked(c, a, pull_t) {
                if dist(hook.pos, pull_t) <= spd {
                    if let Some(k) = h.swing.item.hook.as_mut() { k.pos = pull_t; }
                    h.swing.item.state = 3;
                } else if let Some(k) = h.swing.item.hook.as_mut() {
                    k.pos = add(k.pos, set_len(d, spd));
                }
                return;
            }
            if 0.5 < h.height.to_f32() {
                h.set_state(c, 6, true);
            } else {
                h.set_state(c, 0, false);
                let seq = h.idle_seq();
                h.set_anim(c.anim, c.rng, blend(14), seq, 0);
            }
            6
        }
        3 => {
            h.swing.item.sounds.push(1);
            rope(h);
            if let Some(k) = h.swing.item.hook.as_mut() { k.pos = pull_t; }
            4
        }
        4 => {
            let a = [joint[0], joint[1], h.pos[2].to_f32() + 0.5];
            if blocked(c, a, pull_t) {
                if h.air_ticks == 0 { h.set_state(c, 0, true); } else { h.set_state(c, 6, true); }
                6
            } else {
                if item_wrapped(h) { item_anim(h, data, 4); }
                rope(h);
                let it = &mut h.swing.item;
                it.phase += 0.698_131_7;
                it.amplitude *= 1.0 - 0.1;
                if 0.168 <= it.amplitude { return; }
                h.swing.f904 = 0;
                let near = h.swing.item.hook.as_ref().is_none_or(|k| dist(k.pos, f3(h.pos)) <= 3.5);
                if near {
                    h.set_state(c, 0, true);
                    6
                } else if c.env.pad.held & h.items.slot.fire_mask == 0 {
                    h.swing.help = 1;
                    h.set_state(c, 0, false);
                    let seq = h.idle_seq();
                    h.set_anim(c.anim, c.rng, blend(14), seq, 0);
                    6
                } else {
                    h.swing.item.sounds.push(2);
                    let id = if h.swing.pull_mode == 0 { 0x25 } else { 0x26 };
                    h.set_state(c, id, true);
                    5
                }
            }
        }
        5 => {
            if let Some(k) = h.swing.item.hook.as_mut() { k.pos = pull_t; }
            if item_wrapped(h) { item_anim(h, data, 4); }
            let it = &mut h.swing.item;
            it.phase += 0.698_131_7;
            it.amplitude *= 1.0 - 0.1;
            rope(h);
            if matches!(h.state, 0x25 | 0x26) { return; }
            6
        }
        6 | 0xd => {
            if st == 0xd && item_wrapped(h) { item_anim(h, data, 4); }
            rope(h);
            if let Some(k) = h.swing.item.hook.as_mut() { k.speed = DTF * 80.0; }
            if st == 6 { 7 } else { 0xe }
        }
        7 | 0xe => {
            if st == 0xe && item_wrapped(h) { item_anim(h, data, 4); }
            let hook = h.swing.item.hook.clone().unwrap_or_default();
            if hook.speed < dist(hook.pos, joint) {
                if let Some(k) = h.swing.item.hook.as_mut() { k.pos = add(k.pos, set_len(sub(joint, k.pos), k.speed)); }
                rope(h);
            } else {
                if let Some(k) = h.swing.item.hook.as_mut() { k.pos = joint; }
                if item_wrapped(h) || h.items.slot.item.as_ref().is_some_and(|it| it.anim.seq_b != 1) { item_anim(h, data, 1); }
                h.swing.item.state = 1;
            }
            if st == 7 || h.state != 0x2c { return; }
            let it = &mut h.swing.item;
            if let Some(k) = it.hook.as_mut() { k.speed = dist(on_t, k.pos) / 10.0; }
            (it.wavelength, it.phase, it.amplitude) = (1.0, 0.0, 0.0);
            item_anim(h, data, 3);
            9
        }
        9 => {
            rope(h);
            if item_wrapped(h) { item_anim(h, data, 4); }
            let hook = h.swing.item.hook.clone().unwrap_or_default();
            let d = sub(on_t, hook.pos);
            if len(d) <= hook.speed {
                if let Some(k) = h.swing.item.hook.as_mut() { k.pos = on_t; }
                h.swing.item.sounds.push(1);
                0xb
            } else {
                if let Some(k) = h.swing.item.hook.as_mut() { k.pos = add(k.pos, set_len(d, hook.speed)); }
                return;
            }
        }
        0xa => {
            rope(h);
            0xb
        }
        0xb => {
            if item_wrapped(h) { item_anim(h, data, 4); }
            rope(h);
            let it = &mut h.swing.item;
            it.amplitude *= 1.0 - 0.6;
            it.phase += 0.698_131_7;
            if 0.168 <= it.amplitude { return; }
            h.swing.hooked = true;
            0xc
        }
        0xc => {
            if h.swing.on.is_some() {
                if let Some(k) = h.swing.item.hook.as_mut() { k.pos = on_t; }
            }
            if item_wrapped(h) { item_anim(h, data, 4); }
            let it = &mut h.swing.item;
            it.phase += 0.698_131_7;
            it.amplitude *= 1.0 - 0.6;
            rope(h);
            if h.state == 0x2c { return; }
            0xd
        }
        _ => return,
    };
    h.swing.item.state = next;
}

/// The hand is not the Swingshot (any more): its pvars and hook go with it.
pub(super) fn item_gone(h: &mut Hero) {
    if h.swing.item != SwingItem::default() { h.swing.item = SwingItem::default(); }
}

#[cfg(test)]
mod tests;
