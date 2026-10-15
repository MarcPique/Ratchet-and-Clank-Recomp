//! **Rilgar's biters, class 623** (level05 `0x301f48` with its hits `0x303390`, its steering `0x303b08` /
//! `0x303e50`, its group search `0x303f80` and its wall check `0x304058`; 56 placed, 51 made; census U189; the name is
//! descriptive [L]). Rilgar's common creature: it keeps to an arena (its wall path +0x2c0, a waypoint graph +0x2c4,
//! made at init) and idles at home, wandering within 4. When it sees its target (Ratchet, or a decoy, inside its region
//! +0x2cc and within its sight of home) by a clear route it runs at it along the graph, weaving by its own angle
//! (±+0x298°), winds up (35 ticks with a burst of sparks from joint list 0) and bites (damage 1 at key < 1.5, facing
//! within 15°, 3 away and 2 in height; head item 6 spares Ratchet the 0x1 flag). Out of reach it tries the walls'
//! nearest point; lost, it goes home. Two points of health; light hits knock it back (it explodes when the flight
//! ends with no way home), heavy ones kill it (a death flight, an explosion, three body pieces 0x6da..0x6dc). Some wait
//! for a trigger (+0x2b8: that moby's command byte; +0x2bc: Ratchet in a cuboid), some start hidden 20 below (+0x2b4:
//! state 0xc, woken by other code). Hidden in game mode 2. Its glow pulses; its head looks at Ratchet. Read from the
//! level05 decomp and disassembly (the hit line's ends, the particle arguments) and its words gp−0x518c..−0x512c.
//! Native `f32`.
//!
//! **Pvars** (0x2e0; the header: damage +0x20, flash +0x60, knockback +0x70, walker +0xd0): +0x20 the damage record
//! (health 2, meter +0x24 2, column +0x28 1, +0x30 1.0), +0x38 the lure, +0x60 the flash, +0x70 the knockback record,
//! +0xd0 the walker J (J9 +0xf4 the speed), +0x120 the target record (its moby +0x160, here the runtime index + 1;
//! its kind +0x164), +0x170 / +0x1f0 the head's look records (lists 1 / 2), +0x270 home, +0x280 the waypoint, +0x290
//! the turn velocity, +0x294 the weave angle (+0x298 in degrees, its sign re-rolled), +0x29c the sight, +0x2a0 this
//! tick's sight (+20 while alerted), +0x2ac (`ticks(randf(30, 90))`, not read), +0x2b0 the alert timer, +0x2b4 starts
//! hidden, +0x2b8 the trigger moby, +0x2bc the trigger cuboid, +0x2c0 / +0x2c4 the wall path / graph, +0x2c8 a group
//! to hunt (−1 none), +0x2cc the target region, +0x2d0 (s16) the wind-up, +0x2d2 (s16) the wander count, +0x2d4 the
//! wall-check hold, +0x2d8 the glow phase, +0x2dc hidden by game mode 2.
//!
//! | address | what | port |
//! |---|---|---|
//! | top | `0x28dd48(2.5, +0x170)` the big-head scale; game mode 2 → hidden (mode \|= 0x41, once); mode 0 → shown; drawn within 28 of the camera → the shadow probe, +0x7f 0x16; the hits; scale = the class's ×1.5 | [`update`] (`manip::big_head_scale`, `shadows::probe_down`) |
//! | 0 | mode \|= 0x1000; home; health 2, meter 2, column 1, +0x30 1, +0x29 0, alert 0; +0x58 14, +0x5a 11; `SeedJumpPattern(J)`, J2 / J3 2, J0 trunc(1.5·409.6), J9 6·dt; glow phase `random_angle_radians`; `randi(2)` → mirrored; +0x2b4 → 0xc hidden 20 below (draw distance 0, not targetable, no collision); else no trigger → 1, seq 0; else 0xf, not targetable, seq 0; `0x28be78(&walls, 1, graph)` | [`init`] (`region::graph_init`) |
//! | 1 | no target: alerted → 6 (seq 1, `ticks(10)`); else `randi(9)` = 0: a route to the target (`LineOfSightTest(0.6, …)`) → `randi(256)` odd flips +0x298, +0x294 = it in radians, +0x2ac, 4 (seq 1); none → no target; the animation's end: drawn and `randi(255)` odd → wander (a point within 4 of home, count 0, back to this state), else seq 0 | [`idle`] |
//! | 2 | turn to the point (`0x286078`: 2π·dt², 2π·dt², 2π·dt); facing within 90°: the walker step (out ignored), pushed off the walls (`0x28c010`, 0.6); count + 1; there and past `ticks(60)`, or past `ticks(180)` → back (seq 0, `ticks(20)`) | [`wander`] |
//! | 4 | [`steer`] at the target with the weave; within 3 (xy, before the step) and facing within 45° → 7 (wind-up `ticks(35)`, seq 2); no target → 10 | [`update`] |
//! | 5 | not alerted → speed 0, 10 (seq 1, `ticks(20)`); within 3 → 7 (seq 2) | [`update`] |
//! | 6 | speed `Approach`ed to 6·dt by 6·dt²; turn and step as 2; `ClampToPath` toward the target within 1.5 → 5 (seq 4, `ticks(20)`); not alerted → 10; within 3 → 7 | [`hunt`] |
//! | 7 | turn to the target (4π·dt², 4π·dt², 8π·dt); seq 2 done → seq 7 (`ticks(5)`); the wind-up out → 8 (seq 3, `ticks(1)`), 30 + 30 sparks at joint list 0 (type 2, ±0.065; out 6·dt along the facing, sizes 0.43 / 0.63 then ×0.1; the second velocity `randf(±dt)`, `randf(±dt)`, `randf(±dt)`; phases `scale(randf(5, 10))`, `scale(randf(10, 12))`, `scale(randf(60, 72))`; colours 0x2010c020 / 0x1010c020, then 0x8040ff80) | [`wind_up`] |
//! | 8 | key < 1.5, facing within 15°, within 3 and 2 in height → `0x283e60(1, target, m, 0x10001, target + 0.75 z, (cos, sin, 0, 5627.925))` (`0x10000` with head item 6); the animation's end → 9 (seq 4) | [`bite`] (`attack::hit_moby`) |
//! | 9 | the animation's end: within 3 and facing within 15° → 7 (seq 7, `ticks(5)`); else speed 0, 4 (seq 1) | [`update`] |
//! | 10 | [`steer`] home (no weave); there → 1 (seq 0, `ticks(3)`); no target: alerted → 6; a target → 4 (seq 1, `ticks(3)`) | [`update`] |
//! | 0xb | the knockback flight (`0x286910` = `0x271558`); flying: below 2 → deleted; landed: a route home → 4 (seq 1), none → the death explosion (0.5, 13), deleted | [`update`] (`knock::update`, `fx::death_explosion`) |
//! | 0xc | nothing (hidden until other code wakes it) | — |
//! | 0xd | the animation's end → 1 (seq 0) | [`update`] |
//! | 0xe | the death flight; landed → the explosion, `BreakFxB` 0x6da / 0x6db / 0x6dc, deleted; below 0 → deleted | [`update`] |
//! | 0xf | the trigger moby's command byte set, or Ratchet in the cuboid → draw distance 0x40, 1, targetable; else as 1's animation end (drawn or not; the wander count starts at `ticks(90)`) | [`waiting`] |
//! | `0x303390` | the wall-check hold ticks; `MobyGetHitMessage(0x330000)`; a line from 0.5 above it to the hit (no attacker: the record's point; the wrench 0x47: Ratchet's body; a creature or class 0xba: itself; else the attacker) shorter than 24 and blocked → no hit; the resolver (column 4); out5 ≠ 1 outside 0xe: health −= damage (≤ 0 → reaction 1); K radius 614, flags 9, +0x3d 0, gravity 20·dt², +0x28 0.6; reactions 1 / 2: not targetable, keys 5.5 / 11, speed 20·dt, up 6·dt, the aim (`0x284e00`), `0x2867d0(a, m, K, 6, 1, 0)`, 0xe, flash 0xf0, `SetDeathBits(m, 0x800 for a creature's hit else 0, −1)`; 3..8: keys 5 / 10, speed 15·dt, up 5·dt, seq 5, 0xb, flash 0x78; 9 / 10: flash 0xfa; the flash | [`hits`] |
//! | `0x303390` tail | +0xa4 0xff; the flash; the lure → alert `scale(randf(180, 240))`; the alert ticks; sight = +0x29c (+20 alerted); `0x289fc0(sight, m, +0x120, 0, 0, region)` (= `0x274df8`); a target beyond the sight of home or 3 in height → none; a hunt group: its nearest live targetable member within 120 replaces a farther target; none → Ratchet's moby; the glow (phase += 3π·dt, `FastTweenColor` 0x80004000 → 0x80008000); in 4..9 at Ratchet: the head pitch (±30°) and yaw (±90°, mirrored negated) into +0x1d4, +0x1d8 (0.7) and +0x258 (0.3); `0x28cde8(0.04, 0.3, m, +0x170, 1)`, `(…, +0x1f0, 2)` | [`hits`] (`target::acquire_in`, `manip::look`) |
//! | `0x303b08(a, m, p)` | its quarter of the ticks (moby index & 3 = counter & 3): the route to `p` (`LineOfSightTest`) into the waypoint, none → no target; turn to the waypoint (4π·dt², 8π·dt), by `a` more when it is beyond 5, `a ≠ 0` and the weave's line is clear (`0x303e50`); speed `Approach`ed to dt (facing away or within 3) or 6·dt by 6·dt²; the walker step; the wall check held: pushed off the walls (`0x304058`), beyond 2 → held `ticks(5)` | [`steer`] (`region::push_out_dist`) |
//! | `0x303e50(r, m, p)` | the point as far as `p` at the weave heading (+0x294·1.5 off); the line from it to `r` toward `p` crosses no wall | [`weave_clear`] (`region::crosses`) |
//! | `0x303f80(m, g)` | the group's live (state < 0x7f) targetable members, the nearest (3-D) within 120 | [`hunt_target`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, knock, region, target, turn, walker};
use crate::moby_update::manip;
use crate::moby_update::services::{pv as v4, World};
use crate::particles::type02;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x30_1f48;
pub const CLASSES: [i16; 1] = [623];
pub const PIECES: [i16; 3] = [0x6da, 0x6db, 0x6dc];
/// The head item that spares Ratchet the bite's 0x1 flag (0x1404a8).
pub const SPARING_HEAD_ITEM: i32 = 6;
/// gp−0x5168 (1.5): the class's scale; its 0.4 is the walls' radius.
const SCALE: f32 = 1.5;
const RADIUS: f32 = SCALE * 0.4;
/// gp−0x5138 (35): the wind-up.
const WIND_UP: i32 = 0x23;
/// gp−0x518c / −0x5188: the glow.
const GLOW: (u32, u32) = (0x8000_4000, 0x8000_8000);
/// gp−0x5164..−0x513c: the sparks.
const SPARK_COLOURS: [(u32, u32); 2] = [(0x2010_c020, 0x1010_c020), (0x8040_ff80, 0x8040_ff80)];
const SPARK_SIZES: (f32, f32) = (f32::from_bits(0x3edc_28f6), f32::from_bits(0x3f21_47ae));
const BIG_HEAD: f32 = 2.5;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const KEY_APEX: usize = 0xc0;
    pub const KEY_LAND: usize = 0xc4;
    pub const J: usize = 0xd0;
    pub const SPEED: usize = 0xf4;
    pub const TARGET: usize = 0x120;
    pub const TARGET_MOBY: usize = 0x160;
    pub const KIND: usize = 0x164;
    pub const LOOK_A: usize = 0x170;
    pub const LOOK_B: usize = 0x1f0;
    pub const HOME: usize = 0x270;
    pub const WAY: usize = 0x280;
    pub const TURN_V: usize = 0x290;
    pub const WEAVE: usize = 0x294;
    pub const WEAVE_DEG: usize = 0x298;
    pub const SIGHT_BASE: usize = 0x29c;
    pub const SIGHT: usize = 0x2a0;
    pub const W2AC: usize = 0x2ac;
    pub const ALERT: usize = 0x2b0;
    pub const HIDDEN: usize = 0x2b4;
    pub const TRIGGER: usize = 0x2b8;
    pub const CUBOID: usize = 0x2bc;
    pub const WALLS: usize = 0x2c0;
    pub const GRAPH: usize = 0x2c4;
    pub const HUNT: usize = 0x2c8;
    pub const REGION: usize = 0x2cc;
    pub const WIND: usize = 0x2d0;
    pub const COUNT: usize = 0x2d2;
    pub const HOLD: usize = 0x2d4;
    pub const GLOW: usize = 0x2d8;
    pub const MODE2: usize = 0x2dc;
    pub const SIZE: usize = 0x2e0;
}

/// The states (module doc).
pub mod st {
    pub const INIT: u8 = 0;
    pub const IDLE: u8 = 1;
    pub const WANDER: u8 = 2;
    pub const CHASE: u8 = 4;
    pub const AT_WALL: u8 = 5;
    pub const HUNT: u8 = 6;
    pub const WIND_UP: u8 = 7;
    pub const BITE: u8 = 8;
    pub const RECOVER: u8 = 9;
    pub const HOME: u8 = 10;
    pub const KNOCKED: u8 = 0xb;
    pub const HIDDEN: u8 = 0xc;
    pub const SETTLE: u8 = 0xd;
    pub const DYING: u8 = 0xe;
    pub const WAITING: u8 = 0xf;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn done(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
/// `if (m+0x53 != seq) fun_00212f90(m, seq, 0, ticks(t))`.
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) {
    let t = w.ticks(t);
    c::blend_to(w, id, seq, 0, t);
}
fn gscale(w: &World, x: f32) -> f32 { w.svc.timing.scale(Pf::f(x)).to_f32() }
fn walls(w: &World, id: MobyId) -> usize { c::pi32(w, id, pv::WALLS).max(0) as usize }
fn graph(w: &World, id: MobyId) -> usize { c::pi32(w, id, pv::GRAPH).max(0) as usize }
/// The target moby (+0x160, the runtime index + 1).
fn tgt(w: &World, id: MobyId) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, pv::TARGET_MOBY) - 1).ok().filter(|&m| m < w.table.mobys.len())
}
fn tgt_pos(w: &World, id: MobyId) -> c::V { tgt(w, id).map_or([0.0; 4], |m| w.m(m).position) }
fn heading(p: c::V, q: c::V) -> f32 { c::atan(q[0] - p[0], q[1] - p[1]) }
fn no_target(w: &World, id: MobyId) -> bool { c::pi32(w, id, pv::KIND) == 2 }
fn alerted(w: &World, id: MobyId) -> bool { c::pi32(w, id, pv::ALERT) != 0 }
/// `LineOfSightTest(0.6, &walls, 1, graph, from, to, &out)`.
fn route(w: &World, id: MobyId, from: c::V, to: c::V) -> Option<c::V> {
    region::line_of_sight(w, RADIUS, &[walls(w, id)], graph(w, id), from, to)
}
/// `0x286078(atan(p − pos), acc, acc, vmax, &yaw, &+0x290)`.
fn turn_to(w: &mut World, id: MobyId, h: f32, acc: f32, vmax: f32) {
    let (mut y, mut v) = (c::yaw(w, id), c::pf(w, id, pv::TURN_V));
    turn::turn_toward(h, acc, acc, vmax, &mut y, &mut v);
    c::set_yaw(w, id, y);
    c::set_pf(w, id, pv::TURN_V, v);
}
/// The walker step toward `2·(cos yaw, sin yaw)` (`0x282e30(1, m, J, &dir, &out)`).
fn step(w: &mut World, id: MobyId) {
    let (cy, sy) = c::cs(c::yaw(w, id));
    let mut out = [0.0; 4];
    walker::step(w, id, pv::J, 1.0, [cy + cy, sy + sy, 0.0, 0.0], &mut out);
}
/// `0x28c010(0.6, walls, &pos, &pos)`.
fn push_off(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    if let Some(q) = region::push_out(w, RADIUS, walls(w, id), p) { c::set_pos(w, id, q); }
}
/// To 7: the wind-up `ticks(35)` and seq 2 over `ticks(10)`.
fn to_wind_up(w: &mut World, id: MobyId, seq: u8, t: i32) {
    set_state(w, id, st::WIND_UP);
    let wt = w.ticks(WIND_UP);
    c::set_pi16(w, id, pv::WIND, wt as i16);
    blend(w, id, seq, t);
}
/// Within 3 (xy) of the target.
fn in_reach(w: &World, id: MobyId) -> bool { c::dist2(c::pos(w, id), tgt_pos(w, id)) < 3.0 }
fn facing(w: &World, id: MobyId, lim: f32) -> bool {
    c::diff_rots(c::yaw(w, id), heading(c::pos(w, id), tgt_pos(w, id))) < lim
}

/// `0x303e50(r, m, p)` (module doc).
fn weave_clear(w: &World, id: MobyId, r: f32, p: c::V) -> bool {
    let me = c::pos(w, id);
    let h = c::add_rot(heading(me, p), c::pf(w, id, pv::WEAVE) * 1.5);
    let d = c::dist3(p, me);
    let a = c::add([h.cos() * d, h.sin() * d, 0.0, 0.0], me);
    let b = c::add(c::set_len3(c::sub(a, p), r), me);
    !region::crosses(w, walls(w, id), a, b)
}

/// `0x303b08(a, m, p)` (module doc).
fn steer(w: &mut World, id: MobyId, a: f32, p: c::V) {
    if (id as u64 & 3) == (w.counter & 3) {
        match route(w, id, c::pos(w, id), p) {
            Some(q) => c::set_pv4(w, id, pv::WAY, q),
            None => c::set_pi32(w, id, pv::KIND, 2),
        }
    }
    let way = c::pv4(w, id, pv::WAY);
    let me = c::pos(w, id);
    let d = c::dist2(me, way);
    let (acc, vmax) = (c::DT2 * 12.566_371, c::DT * 25.132_742);
    let h = heading(me, way);
    if d <= 5.0 || a == 0.0 || !weave_clear(w, id, RADIUS, way) {
        turn_to(w, id, h, acc, vmax);
    } else {
        turn_to(w, id, c::add_rot(h, a), acc, vmax);
    }
    let off = c::diff_rots(c::yaw(w, id), heading(c::pos(w, id), way));
    let mut v = c::pf(w, id, pv::SPEED);
    if std::f32::consts::FRAC_PI_2 <= off || d <= 3.0 {
        turn::approach(c::DT, c::DT2 * 6.0, &mut v);
    } else {
        turn::approach(c::DT * 6.0, c::DT2 * 6.0, &mut v);
    }
    c::set_pf(w, id, pv::SPEED, v);
    step(w, id);
    if c::pi32(w, id, pv::HOLD) == 0 {
        let p = c::pos(w, id);
        let (near, q) = region::push_out_dist(w, RADIUS, walls(w, id), p);
        if let Some(q) = q { c::set_pos(w, id, q); }
        if 2.0 < near {
            let t = w.ticks(5);
            c::set_pi32(w, id, pv::HOLD, t);
        }
    }
}

/// `0x303f80(m, g)` (module doc).
fn hunt_target(w: &World, id: MobyId, g: i32) -> Option<MobyId> {
    let me = c::pos(w, id);
    let mut best = (120.0f32, None);
    for m in crate::moby_update::scheduler::group_ids(w, i8::try_from(g).unwrap_or(-1)) {
        let o = w.m(m);
        if o.mode & mode::TARGETABLE == 0 || 0x7f <= o.state { continue; }
        let d = c::dist3(o.position, me);
        if d < best.0 { best = (d, Some(m)); }
    }
    best.1
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).mode |= mode::TARGETABLE;
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOME, p);
    c::set_pf(w, id, pv::D, 2.0);
    c::set_pu8(w, id, pv::D + 9, 0);
    c::set_pi32(w, id, pv::ALERT, 0);
    c::set_pi16(w, id, pv::D + 4, 2);
    c::set_pf(w, id, pv::D + 0x10, 1.0);
    c::set_pu8(w, id, pv::D + 8, 1);
    c::set_pu8(w, id, 0x58, 14);
    c::set_pu8(w, id, 0x5a, f32::from_bits(0x4133_3333) as u8);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    c::set_pf(w, id, pv::J + 8, 2.0);
    c::set_pf(w, id, pv::J + 0xc, 2.0);
    c::set_pi32(w, id, pv::J, (SCALE * 409.6) as i32);
    c::set_pf(w, id, pv::SPEED, c::DT * 6.0);
    let a = w.rng.rand_angle();
    c::set_pf(w, id, pv::GLOW, a);
    if w.rng.randi(2) == 0 { w.mm(id).mode &= !mode::MIRROR; } else { w.mm(id).mode |= mode::MIRROR; }
    if c::pi32(w, id, pv::HIDDEN) != 0 {
        set_state(w, id, st::HIDDEN);
        let m = w.mm(id);
        m.draw_dist = 0;
        m.mode &= !mode::TARGETABLE;
        m.position[2] -= 20.0;
        m.visible = 0;
        m.has_collision = false;
    } else if c::pi32(w, id, pv::TRIGGER) == -1 {
        set_state(w, id, st::IDLE);
        c::blend_to(w, id, 0, 0, 0);
    } else {
        set_state(w, id, st::WAITING);
        w.mm(id).mode &= !mode::TARGETABLE;
        c::blend_to(w, id, 0, 0, 0);
    }
    let (wl, g) = (walls(w, id), graph(w, id));
    if g < w.svc.splines.len() { region::graph_init(w, &[wl], g); }
}

/// The animation's end in 1 / 0xf: `randi(255)` odd (in 1 only when drawn) → the wander (count `count0`), else seq 0.
fn idle_end(w: &mut World, id: MobyId, count0: i32, need_drawn: bool) {
    if (!need_drawn || w.m(id).visible != 0) && w.rng.randi(0xff) & 1 != 0 {
        let r = w.rng.rand_vec(0.0, 4.0);
        let home = c::pv4(w, id, pv::HOME);
        let p = [r[0] + home[0], r[1] + home[1], home[2], home[3]];
        c::set_pv4(w, id, pv::WAY, p);
        c::set_pi16(w, id, pv::COUNT, count0 as i16);
        let s = state(w, id);
        w.mm(id).cmd = s;
        set_state(w, id, st::WANDER);
        blend(w, id, 1, 10);
        return;
    }
    blend(w, id, 0, 10);
}

/// State 1 (module doc).
fn idle(w: &mut World, id: MobyId) {
    if !no_target(w, id) && w.rng.randi(9) == 0 {
        match route(w, id, c::pos(w, id), tgt_pos(w, id)) {
            None => c::set_pi32(w, id, pv::KIND, 2),
            Some(_) => {
                if w.rng.randi(0x100) & 1 != 0 {
                    let d = c::pf(w, id, pv::WEAVE_DEG);
                    c::set_pf(w, id, pv::WEAVE_DEG, -d);
                }
                let d = c::pf(w, id, pv::WEAVE_DEG);
                c::set_pf(w, id, pv::WEAVE, d * 0.017_453_292);
                let f = w.rng.randf(30.0, 90.0);
                let t = w.ticks(f as i32);
                c::set_pi32(w, id, pv::W2AC, t);
                set_state(w, id, st::CHASE);
                blend(w, id, 1, 10);
            }
        }
    }
    if no_target(w, id) && alerted(w, id) {
        set_state(w, id, st::HUNT);
        blend(w, id, 1, 10);
        return;
    }
    if done(w, id) { idle_end(w, id, 0, true); }
}

/// State 2 (module doc).
fn wander(w: &mut World, id: MobyId) {
    let way = c::pv4(w, id, pv::WAY);
    let h = heading(c::pos(w, id), way);
    turn_to(w, id, h, c::DT2 * std::f32::consts::TAU, c::DT * std::f32::consts::TAU);
    if c::diff_rots(c::yaw(w, id), heading(c::pos(w, id), way)) < std::f32::consts::FRAC_PI_2 {
        step(w, id);
        push_off(w, id);
    }
    let n = c::pi16(w, id, pv::COUNT).wrapping_add(1);
    c::set_pi16(w, id, pv::COUNT, n);
    let there = c::dist2(c::pos(w, id), way) < 1.0;
    let n = n as i32;
    if (there && w.ticks(60) < n) || w.ticks(180) < n {
        let back = w.m(id).cmd;
        set_state(w, id, back);
        blend(w, id, 0, 20);
    }
}

/// State 6 (module doc).
fn hunt(w: &mut World, id: MobyId) {
    let mut v = c::pf(w, id, pv::SPEED);
    turn::approach(c::DT * 6.0, c::DT2 * 6.0, &mut v);
    c::set_pf(w, id, pv::SPEED, v);
    let t = tgt_pos(w, id);
    let h = heading(c::pos(w, id), t);
    turn_to(w, id, h, c::DT2 * std::f32::consts::TAU, c::DT * std::f32::consts::TAU);
    if facing(w, id, std::f32::consts::FRAC_PI_2) {
        step(w, id);
        push_off(w, id);
    }
    let me = c::pos(w, id);
    let (crossed, at) = region::clamp(w, walls(w, id), me, t);
    if crossed && c::dist3(me, at) < 1.5 {
        set_state(w, id, st::AT_WALL);
        blend(w, id, 4, 20);
        return;
    }
    if !alerted(w, id) {
        set_state(w, id, st::HOME);
    } else if in_reach(w, id) {
        to_wind_up(w, id, 2, 10);
    }
}

/// State 7 (module doc).
fn wind_up(w: &mut World, id: MobyId) {
    let h = heading(c::pos(w, id), tgt_pos(w, id));
    turn_to(w, id, h, c::DT2 * 12.566_371, c::DT * 25.132_742);
    if done(w, id) && w.m(id).anim.seq_b == 2 {
        let t = w.ticks(5);
        w.anim_blend(id, 7, 0, t);
    }
    if c::dec_timer_pvar_s16(w, id, pv::WIND) == 0 { return; }
    set_state(w, id, st::BITE);
    c::set_pi16(w, id, pv::WIND, 0);
    blend(w, id, 3, 1);
    let a = c::add_rot(c::yaw(w, id), 0.0);
    let (ca, sa) = c::cs(a);
    let mut v1 = [ca * 6.0 * c::DT, sa * 6.0 * c::DT, 0.0, SPARK_SIZES.0];
    let mut w2 = SPARK_SIZES.1;
    let base = w.joint_point(id, 0);
    for (k, colours) in SPARK_COLOURS.into_iter().enumerate() {
        if k == 1 {
            v1[3] = SPARK_SIZES.0 * 0.1;
            w2 = SPARK_SIZES.1 * 0.1;
        }
        for _ in 0..30 {
            let j = f32::from_bits(0x3d85_1eb8);
            let mut p = base;
            for q in p.iter_mut().take(3) { *q += w.rng.randf(-j, j); }
            let vx = w.rng.randf(-c::DT, c::DT);
            let vy = w.rng.randf(-c::DT, c::DT);
            let vz = w.rng.randf(-c::DT, c::DT);
            let r0 = w.rng.randf(5.0, 10.0);
            let t0 = gscale(w, r0) as i32;
            let r1 = w.rng.randf(10.0, 12.0);
            let t1 = gscale(w, r1) as i32;
            let r2 = w.rng.randf(60.0, 72.0);
            let t2 = gscale(w, r2) as i32;
            let s = type02::Spawn { pos: p, v1, v2: [vx, vy, vz, w2], c1: colours.0, c2: colours.1, t: [t0, t1, t2], def: -1 };
            fx::part02(w, &s);
        }
    }
}

/// State 8 (module doc).
fn bite(w: &mut World, id: MobyId) {
    let a = c::add_rot(c::yaw(w, id), 0.0);
    let (me, t) = (c::pos(w, id), tgt_pos(w, id));
    if c::ground::key_time(w, id) < 1.5 && c::diff_rots(a, heading(me, t)) < 0.261_799_4 && c::dist2(me, t) < 3.0 && (t[2] - me[2]).abs() < 2.0 {
        if let Some(m) = tgt(w, id) {
            let (cy, sy) = c::cs(c::yaw(w, id));
            let flags = if w.hero.head_slot.id == SPARING_HEAD_ITEM { 0x1_0000 } else { 0x1_0001 };
            let at = [t[0], t[1], t[2] + 0.75, t[3]];
            attack::hit_moby(w, m, id, 1.0, flags, at, [cy, sy, 0.0, crate::hero::damage::EXACT_PUSH_W]);
        }
    }
    if done(w, id) {
        set_state(w, id, st::RECOVER);
        blend(w, id, 4, 10);
    }
}

/// State 0xf (module doc).
fn waiting(w: &mut World, id: MobyId) {
    let trig = c::pi32(w, id, pv::TRIGGER);
    let fired = usize::try_from(trig).ok().and_then(|t| w.table.mobys.get(t)).is_some_and(|m| m.cmd != 0);
    let cub = c::pi32(w, id, pv::CUBOID);
    let h = super::hero_pos(w);
    if fired || (cub != -1 && crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [h[0], h[1], h[2]], cub)) {
        let m = w.mm(id);
        m.draw_dist = 0x40;
        m.state = st::IDLE;
        m.mode |= mode::TARGETABLE;
        return;
    }
    if done(w, id) {
        let n = w.ticks(90);
        idle_end(w, id, n, false);
    }
}

/// `0x303390`: the hits and the tail (module doc).
fn hits(w: &mut World, id: MobyId) {
    c::dec_timer_pvar_i32(w, id, pv::HOLD);
    let mut hit = w.get_hit(id, 0x33_0000, false);
    if let Some(h) = hit {
        let p = c::pos(w, id);
        let from = [p[0], p[1], p[2] + 0.5, p[3]];
        let to = match h.attacker {
            None => h.pos.map(|x| f32::from_bits(x.0)),
            Some(a) if w.m(a).o_class == 0x47 => w.hero.body_point.map(|x| f32::from_bits(x.0)),
            Some(a) => {
                let o = w.m(a).o_class;
                let creature = w.classes.info(o).is_some_and(|i| i.ty == 5);
                if creature || o == 0xba { from } else { w.m(a).position }
            }
        };
        if c::dist3(from, to) < 24.0 && w.coll_line(v4(from), v4(to), 2, Some(id)).is_some() { hit = None; }
    }
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        let reaction = if hp <= 0.0 { 1 } else { res.reaction };
        let kr = pv::K;
        c::set_pi32(w, id, kr + knock::k::RADIUS, (SCALE * 409.6) as i32);
        c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
        c::set_pu8(w, id, kr + 0x3d, 0);
        c::set_pf(w, id, kr + knock::k::GRAVITY, c::DT2 * 20.0);
        c::set_pf(w, id, kr + knock::k::ZOFF, RADIUS);
        let h = res.hit;
        let attacker = h.and_then(|h| h.attacker);
        let dir = h.map_or([0.0; 4], |h| h.dir.map(|x| f32::from_bits(x.0)));
        match reaction {
            1 | 2 => {
                w.mm(id).mode &= !mode::TARGETABLE;
                c::set_pf(w, id, kr + knock::k::GRAVITY, c::DT2 * 20.0);
                c::set_pf(w, id, pv::KEY_APEX, 5.5);
                c::set_pf(w, id, pv::KEY_LAND, 11.0);
                let (mut sp, mut up) = (c::DT * 20.0, c::DT * 6.0);
                // `FastArcTan(pos − attacker)`: a dead store (the aim overwrites it).
                let a = knock::aim(dir, &mut sp, &mut up);
                c::set_pf(w, id, kr + knock::k::SPEED, sp);
                c::set_pf(w, id, kr + knock::k::UP, up);
                knock::start(w, id, kr, a, 6, 1, 0);
                set_state(w, id, st::DYING);
                c::set_pu8(w, id, pv::F + 7, 0xf0);
                let by_creature = attacker.is_some_and(|a| w.classes.info(w.m(a).o_class).is_some_and(|i| i.ty == 5));
                set_death_bits(w, id, if by_creature { 0x800 } else { 0 }, -1);
            }
            3..=8 => {
                c::set_pf(w, id, pv::KEY_APEX, 5.0);
                c::set_pf(w, id, pv::KEY_LAND, 10.0);
                let (mut sp, mut up) = (c::DT * 15.0, c::DT * 5.0);
                let a = knock::aim(dir, &mut sp, &mut up);
                c::set_pf(w, id, kr + knock::k::SPEED, sp);
                c::set_pf(w, id, kr + knock::k::UP, up);
                knock::start(w, id, kr, a, 5, 1, 0);
                set_state(w, id, st::KNOCKED);
                c::set_pu8(w, id, pv::F + 7, 0x78);
            }
            9 | 10 => c::set_pu8(w, id, pv::F + 7, 0xfa),
            _ => {}
        }
        flash::start(w, id, pv::F);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::F);
    if c::pi32(w, id, pv::LURE) != 0 {
        let f = w.rng.randf(180.0, 240.0);
        let t = gscale(w, f) as i32;
        c::set_pi32(w, id, pv::ALERT, t);
        c::set_pi32(w, id, pv::LURE, 0);
    }
    c::dec_timer_pvar_i32(w, id, pv::ALERT);
    let sight = c::pf(w, id, pv::SIGHT_BASE) + if alerted(w, id) { 20.0 } else { 0.0 };
    c::set_pf(w, id, pv::SIGHT, sight);
    let region_path = usize::try_from(c::pi32(w, id, pv::REGION)).ok();
    let t = target::acquire_in(w, id, sight, region_path);
    c::set_pv4(w, id, pv::TARGET, t.pos);
    c::set_pi32(w, id, pv::TARGET_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    let mut kind = t.kind as i32;
    if kind != 2 {
        let home = c::pv4(w, id, pv::HOME);
        if sight < c::dist2(home, t.pos) || 3.0 < (c::pos(w, id)[2] - t.pos[2]).abs() { kind = 2; }
    }
    c::set_pi32(w, id, pv::KIND, kind);
    let g = c::pi32(w, id, pv::HUNT);
    if 0 <= g {
        if let Some(n) = hunt_target(w, id, g) {
            let me = c::pos(w, id);
            let keep = kind != 2 && c::dist2(me, tgt_pos(w, id)) <= c::dist2(me, w.m(n).position);
            if !keep {
                c::set_pi32(w, id, pv::TARGET_MOBY, n as i32 + 1);
                c::set_pi32(w, id, pv::KIND, 1);
            }
        }
    }
    if c::pi32(w, id, pv::TARGET_MOBY) == 0 {
        let h = w.hero_moby.map_or(0, |m| m as i32 + 1);
        c::set_pi32(w, id, pv::TARGET_MOBY, h);
    }
    let ph = c::add_rot(c::pf(w, id, pv::GLOW), c::DT * 9.424_778);
    c::set_pf(w, id, pv::GLOW, ph);
    w.mm(id).glow = crate::particles::tween_color(((ph.sin() + 1.0) * 0.5).to_bits(), GLOW.0, GLOW.1);
    let s = state(w, id);
    if (4..10).contains(&s) && tgt(w, id).is_some() && tgt(w, id) == w.hero_moby {
        let (me, t) = (c::pos(w, id), tgt_pos(w, id));
        let d = c::sub(t, me);
        let mut yaw = c::sub_rot(c::atan(d[0], d[1]), c::yaw(w, id));
        let pitch = -c::atan(c::len2(d), d[2]);
        if w.m(id).mode & mode::MIRROR != 0 { yaw = -yaw; }
        let yaw = yaw.clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
        let pitch = pitch.clamp(-std::f32::consts::FRAC_PI_6, std::f32::consts::FRAC_PI_6);
        c::set_pf(w, id, pv::LOOK_A + 0x64, pitch);
        c::set_pf(w, id, pv::LOOK_A + 0x68, yaw * 0.7);
        c::set_pf(w, id, pv::LOOK_B + 0x68, yaw * 0.3);
    }
    manip::look(w, id, id, pv::LOOK_A, 1, 0.04, 0.3);
    manip::look(w, id, id, pv::LOOK_B, 2, 0.04, 0.3);
}

/// Level05 `0x301f48` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    manip::big_head_scale(w, BIG_HEAD, id, pv::LOOK_A);
    if w.svc.game_mode == 2 {
        if w.m(id).mode & mode::HIDDEN == 0 {
            c::set_pi32(w, id, pv::MODE2, 1);
            w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
        }
    } else if c::pi32(w, id, pv::MODE2) != 0 && w.svc.game_mode == 0 {
        c::set_pi32(w, id, pv::MODE2, 0);
        w.mm(id).mode &= !(mode::HIDDEN | mode::NO_ANIM);
    }
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 28.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x16;
        }
    }
    hits(w, id);
    let o = w.m(id).o_class;
    w.mm(id).scale = super::class_scale(w, o) * SCALE;
    match state(w, id) {
        st::INIT => init(w, id),
        st::IDLE => idle(w, id),
        st::WANDER => wander(w, id),
        st::CHASE => {
            let t = tgt_pos(w, id);
            let near = in_reach(w, id);
            let a = c::pf(w, id, pv::WEAVE);
            steer(w, id, a, t);
            if near && facing(w, id, std::f32::consts::FRAC_PI_4) { to_wind_up(w, id, 2, 10); }
            if no_target(w, id) { set_state(w, id, st::HOME); }
        }
        st::AT_WALL => {
            if !alerted(w, id) {
                c::set_pf(w, id, pv::SPEED, 0.0);
                set_state(w, id, st::HOME);
                blend(w, id, 1, 20);
            } else if in_reach(w, id) {
                to_wind_up(w, id, 2, 10);
            }
        }
        st::HUNT => hunt(w, id),
        st::WIND_UP => wind_up(w, id),
        st::BITE => bite(w, id),
        st::RECOVER => {
            if done(w, id) {
                if in_reach(w, id) && facing(w, id, 0.261_799_4) {
                    to_wind_up(w, id, 7, 5);
                } else {
                    c::set_pf(w, id, pv::SPEED, 0.0);
                    set_state(w, id, st::CHASE);
                    blend(w, id, 1, 10);
                }
            }
        }
        st::HOME => {
            let home = c::pv4(w, id, pv::HOME);
            steer(w, id, 0.0, home);
            if c::dist2(c::pos(w, id), home) < 1.0 {
                set_state(w, id, st::IDLE);
                blend(w, id, 0, 3);
            } else if no_target(w, id) {
                if alerted(w, id) { set_state(w, id, st::HUNT); }
            } else {
                set_state(w, id, st::CHASE);
                blend(w, id, 1, 3);
            }
        }
        st::KNOCKED => {
            if knock::update(w, id, pv::K) & 0x60 == 0 {
                if c::pos(w, id)[2] < 2.0 { w.delete_moby(id); }
                return;
            }
            let home = c::pv4(w, id, pv::HOME);
            if route(w, id, c::pos(w, id), home).is_none() {
                let p = c::pos(w, id);
                fx::death_explosion(w, 0.5, 13.0, Some(id), p, -1);
                w.delete_moby(id);
                return;
            }
            set_state(w, id, st::CHASE);
            blend(w, id, 1, 10);
        }
        st::SETTLE => {
            if done(w, id) {
                set_state(w, id, st::IDLE);
                blend(w, id, 0, 10);
            }
        }
        st::DYING => {
            if knock::update(w, id, pv::K) & 0x60 != 0 {
                let (p, rot) = (c::pos(w, id), w.m(id).rotation);
                fx::death_explosion(w, 0.5, 13.0, Some(id), p, -1);
                for class in PIECES { fx::break_piece(w, id, class, p, rot, 0, 0); }
                w.delete_moby(id);
            } else if c::pos(w, id)[2] < 0.0 {
                w.delete_moby(id);
            }
        }
        st::WAITING => waiting(w, id),
        _ => {}
    }
}
