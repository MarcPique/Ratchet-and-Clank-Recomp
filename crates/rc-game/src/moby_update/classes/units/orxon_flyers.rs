//! Orxon's two flying creatures (level 10), census U335 / U336 (2026-09-29): the **path scout** 1196 (level10 `0x2df270`, 7
//! created) and the **swoop flyer** 1199 (level10 `0x2e01a8`, 15 created of 51 placed; the rest fail the load's spawn
//! test on a fresh save). Read from the level10 decomp (the class code) and disassembly where the decompile hides an
//! argument (docs/plan/creatures.md §11).
//!
//! **System or not.** Each class runs its own update and its own tick (`0x2dfc38` / `0x2e17c0`: two functions, two
//! census clusters). The ticks' second half (the alert range, the target search in an area path or a cuboid, the
//! home leash) is the same code but for one compare of the cuboid test (1196 `0 ≤ x`, 1199 `1 ≤ x` on the local x
//! lane): one Rust function [`targeting`] with that lane as its data row. Their hit halves differ in code (1199:
//! the attacker-class filter, class sound 2, the push's heading through `0x26fa48`; 1196: the skill-point branch), so
//! they stay two functions. Everything else is the engine's creature layer (`creature::*`) and the group call of each
//! (level10 `0x2e0138` / `0x2e1cb0`: moby-group loops, per class).
//!
//! **Pvars** (0x2b0): +0x20 the damage record (health, +0x24 s16 1, +0x26 s16 the hit cooldown, +0x28 column, +0x29,
//! +0x30 D+0x10), +0x38 the lure, +0x40 a step vector (1196), +0x60 the knockback record, +0xc0 the target record
//! (+0x100 moby, +0x104 kind), +0x110 the flash, +0x120 the move vector (+0x128 the vertical speed of the rise),
//! +0x130 home, +0x140 the current point, +0x150 path (1196), +0x158 its node, +0x160.. the mobys the path's nodes
//! wake (1196), +0x170 the turn velocity, +0x174 the mode (1196: 1 path; 1199: 0 hover, 1 deleted, 2 cuboid), +0x178
//! the range, +0x17c the area path, +0x180 the range now, +0x184 the alert timer, +0x188 / +0x18c the waiting range
//! and area (state 1), +0x190 the dive's side (1199), +0x194 the cuboid, +0x198 the wait (1199), +0x19c the bob phase
//! (1199) / the retarget timer (1196), +0x1a0 / +0x1a4 the cuboid height offset and its timer (1199), +0x270 the
//! big-head cheat's.
//!
//! ## Coverage: the ticks (`0x2dfc38` 1196, `0x2e17c0` 1199)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | lure +0x38 → +0x184 = `trunc(scale(randf(180, 240)))`, +0x38 = 0; `FastDecTimer(+0x184)` | the Taunter: 6 more range | [`lure`] |
//! | state ≠ death: `MobyGetHitMessage(m, 0x330000, 0)`, `0x26f378(m, hit, +0x20, 5, …, col 4)` | every weapon's hit record through the resolver | [`hit_1196`], [`hit_1199`] (`World::get_hit`, `damage::resolve`) |
//! | 1199: the attacker's class (record +0x20) = its own → ignored | its own dives do not hurt it | [`hit_1199`] |
//! | health ≤ damage: 1196 only: the level word gp−0x4cc8 (Ratchet in the help director's cuboid, `help_orxon::AIR_WORD`) and skill point 0x13d418 not earned → the point, `PlayLevelSoundAtMoby(1, 0, 0)`, `ShowBanner(0x53d6, −1)` | the skill point | ported (`story::award_skill_point`) |
//! | health 0, untargetable, blend 5 (1196) / 4 (1199), state 0xb / 0xd; K +0x3d 3, gravity 26·dt², drag 0.0005, flags 9, speeds 7·dt / 10·dt; 1199: class sound 2 | the death flight | [`hit_1196`], [`hit_1199`] |
//! | heading: record +0x30 & 1 → atan(record dir) else atan(pos − record pos); 1199: then `0x24d658` (= `0x26fa48`) on the dir (the heading and the exact push's speeds) | | `knock::aim` (1199) |
//! | `0x271418(a, m, K, 5 / 4, 1, 0)`; flash 0x78, `0x272318`; `BoltBurst(m, 1, 3, 0, −1)` | | `knock::start`, `flash::start`, `crate_::bolt_burst` (flags 0: the game's returns at once) |
//! | health left: flash 0xfa, health −= damage, +0x26 = `ticks(60)`, `0x272318` | the hit flash | [`hit_1196`], [`hit_1199`] |
//! | +0xa4 = 0xff (1196: with a hit only); `0x2723f8` | | [`update_1196`], [`update_1199`] (`flash::update`) |
//! | the range: mode 2 → 42; alerted → +0x178 + 6; state 1 → +0x188; else +0x178 | | [`targeting`] |
//! | mode 2: `0x2527c8` (= `0x274b78`); a target → Ratchet's feet in cuboid +0x194 (the inverse rows; outside: x ≤ −1, x ≥ 0 (1196) / 1 (1199), \|y\|, \|z\| ≥ 1) → none | | [`targeting`] (`target::acquire`) |
//! | else the area (+0x18c in state 1, else +0x17c): none → `0x274b78`, else `0x252a48` (= `0x274df8`, the path polygon) | | [`targeting`] (`target::acquire_in`) |
//! | a target: beyond the range from home (xy) or not drawn → none; no moby → Ratchet's | the leash | [`targeting`] |
//!
//! ## Coverage: 1196 (`0x2df270`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x256da0(2.5, m, 0, +0x270)` | the big-head cheat manipulator | (`manip::big_head`) |
//! | drawn, within 29 of the camera: `0x24cc30` (= `0x26f020`), +0x7f = 0x17 | the shadow probe | `shadows::probe_down` |
//! | +0x58 = 1 | anim speed | [`update_1196`] |
//! | case 0: D (health 1, +0x24 1, col 0, +0x29 1, D+0x10 0.5), targetable, home +0x130 / +0x140; mode ≠ 1 → deleted; → 1 | init | [`update_1196`] |
//! | case 1: a target → 2 (blend 1); none and mode ≠ 0 → wait | sleep | [`update_1196`] |
//! | case 2: wrapped → 4, move 0 (blend 4) | wake | [`update_1196`] |
//! | case 3: — | | n/a (no case) |
//! | case 4: speed 4; `0x24e480` (= `0x270830`) z to home + 2 (18·dt², 18·dt², 6·dt, +0x128) returns 0 → 5 | rise | [`update_1196`] (`turn::spring`) |
//! | case 5: speed 4; node = path +0x150 point +0x158; `0x24e910` (= `0x270cc0`) toward it (25.13·dt², 25.13·dt², 50.27·dt); facing within 45°: speed spring (20·dt², 20·dt², 20·dt) on \|move\|, move = node − pos clamped to it (`0x252240` = `0x2745f0`), else 0; `0x24b548(0.5, 0, m, move, 0x200, 0)` (= `0x26d8b0` → `0x26d610`); within 1: next node; its w ≤ 0: the end → 10 (blend 4), home = the ground below (`GroundHeight` + the hit point 0x1742e0), +0x140 = pos; w > 0 → 6 (blend 3) | fly the path | [`path_step`] (`turn::turn_toward_pvar`, `walker::move_collide`) |
//! | case 6: turn to the target; wrapped: moby +0x160[w − 1] → `0x2e0138` (its group's mobys in state 1 → 2); the end → 10; else 5 (blend 4) | wake the group | [`wake_group`] |
//! | case 7: — | | n/a (no case) |
//! | case 8: pitch → 0 (`0x24e710` = `0x270ac0`, 12.57·dt), `SpringTurn2(home, 0.02, 0.3, 2π·dt)`, move (6·dt, z to home + 2 within ±6·dt), within 2 → 7 | (entered by no code) | [`update_1196`] |
//! | case 9: `FastDecTimer(+0x19c)` done → `ticks(120)`, a point `randf(0, 3)` from home, `randf(5, 7)` up; → 10 | pick a point | [`update_1196`] |
//! | case 10: `SpringTurn2(point, 0.05, 0.3, 0.2)`; step = point − pos ≤ 4·dt; `0x1e43c8(0.4, pos + step, 0, m)` (the sphere kernel) free → moved, else 9; no step → 9 | hover about | [`update_1196`] (`World::coll_sphere`) |
//! | case 0xb: `0x24f1a8` (= `0x271558`) & 1 → `SetDeathBits(m, 0x200, −1)`, `0x251ef8(0.5, 10, m, pos, −1)` (= `0x2742a8`), deleted | the death: bolts, save bit, the explosion | [`update_1196`] (`crate_::set_death_bits`, `fx::piece_explosion`) |
//! | no attack on Ratchet | | n/a |
//!
//! ## Coverage: 1199 (`0x2e01a8`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x256da0(2.5, m, 1, +0x270)` | the big-head cheat manipulator | (`manip::big_head`) |
//! | shadow probe within 29 (0x17); +0x58 = 1 | | [`update_1199`] |
//! | states ∉ {0, 0xd}: z −= 0.125·cos(+0x19c) first, the tail adds the next one (+0x19c += 360° / `trunc(scale(1.21·60))`) | the bob | [`update_1199`] |
//! | case 0: D+0x10 0.25, health 1, …, targetable, home, `rand_angle` phase, +0x58 8, +0x5a 20; mode 0 → 3, z + 2; 1 → deleted; 2 → 9: z = the cuboid's centre z, pos += (10·cos a, 10·sin b) (two `rand_angle`), +0x1a0 / +0x1a4; blend 3 | init | [`update_1199`], [`cuboid_redraw`] |
//! | case 2: speed 4, z to home + 2 (as 1196's 4) → 3 | (entered by no code: nothing sets state 1) | [`update_1199`] |
//! | case 3: speed 4; the move toward +0x140 (as 1196's case 5); blocked (result 1) → a new point `randf(4, 6)` from home, `randf(0.5, 6)` up; within 0.1 → 4, wait `ticks(randf(30, 90))`; a target beyond 3 (xy) and the moby +0xbc timer out (`0x1f2630` = `0x220ed8`) → the group's +0xbc = `ticks(5)` (`0x2e1cb0`), 6 (blend 1) | hover about home | [`hover`], [`alert_group`] |
//! | case 4: the same attack start; the wait out → a new point, 3 | | [`update_1199`] |
//! | case 5: pitch → 0, `SpringTurn2(home)`, 6·dt, z to home + 2; within 2 → 3 | back home | [`update_1199`] |
//! | case 6: turn to the target; wrapped → 7 (blend 3), side ±15° (`randi(255) & 1`) | wind up | [`update_1199`] |
//! | case 7: turn to the target + side; aim = the target (z: 4× its drop below, not under the ground + 0.5); move 10·dt along the yaw (x = cos·\|v.xy\|, y = sin·\|(x, v.y)\|: the second length reads the new x); pitch toward it; within 3 (xy): within 2 of Ratchet's z (0x13f3d8) → class sound 0, 8 (blend 2 from frame 4), else back (blend 3, 5); the target lost → 5 | the dive | [`update_1199`] |
//! | case 8: turn; pitch to the aim point; key 20 passed (`0x2543e8` = `0x2765b0`): the template (push (cos, sin, 1)·exact, flags 1, type 0 / 1, its class, damage 1) at joint 0 (`0x242240` = `0x2645a8`): `coll_sphere_mobys(0.4, …)` and `CollLine_Fix(pos, joint, 9, m, tmpl)`; wrapped or no target → 5 (blend 3) | the bite on Ratchet | [`update_1199`] (`World::sphere_mobys`, `services::line_hit_in`) |
//! | case 9: turn to +0x130 (π/4·dt), 4·dt, z to the cuboid's z − +0x1a0 (±2·dt; redraw when its timer runs out); a target beyond 3 and +0xbc out → the group call, 10 (blend 3) | cuboid hover | [`update_1199`] |
//! | case 10: the group call; turn to the target (π/2·dt), 4·dt, z to the cuboid's (±4·dt); within 3 and 1 in z → class sound 0, 0xb (blend 2); the target lost → 9 | cuboid charge | [`update_1199`] |
//! | case 0xb: the group call; turn (2π·dt), 6·dt, z ±3·dt; the template (no push, flags 0x10001, damage 1; the type bytes the game leaves as stack: 0 [L]) at joint 0, `coll_sphere_mobys(0.2, …)`; beyond 1, Ratchet's group ≠ 7 and not wrapped → keep (lost → 9); else 0xc (blend 3) | the cuboid bite on Ratchet | [`update_1199`] |
//! | case 0xc: turn to +0x130 (π/3·dt), the move vector (4·dt, ±3·dt) added to the position (`0x1f28f8` = `0x2211a0`, in place, no collision); within 2 of +0x130 → 9, +0xbc = `ticks(120)` | pull back | [`update_1199`] |
//! | case 0xd: `0x24f1a8` & 1 → `SetDeathBits(m, 0x200, −1)`, `0x251ef8(0.5, 10, m, pos, −1)`, deleted | the death | [`update_1199`] |
//! | reaction table level10 0x1dde28 (the level's shared no-op table) | no Suck Cannon reaction | `react::tables_from_overlays` finds none: the Suck Cannon's projectile is a weapon hit |
//!
//! Native `f32`; the rand draws at the game's points. **[L]**: a +0x160 entry of −1 makes the game read the moby slot
//! before the array (its group byte): the port wakes nothing; the ground point after `GroundHeight` misses is the
//! stale collision output in the game, (x, y, 0) here.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::{bolt_burst, set_death_bits};
use crate::moby_update::creature::{self as c, damage, flash, fx, ground, knock, target, turn, walker};
use crate::moby_update::services::{HitTemplate, World};
use crate::ps2v::Pf;

pub const SCOUT_FN: u32 = 0x2d_f270;
pub const SWOOP_FN: u32 = 0x2e_01a8;
pub const REFERENCE_LEVEL: u32 = 10;
pub const SCOUT_CLASSES: [i16; 1] = [1196];
pub const SWOOP_CLASSES: [i16; 1] = [1199];

pub mod pv {
    pub const D: usize = 0x20;
    pub const COOLDOWN: usize = 0x26;
    pub const LURE: usize = 0x38;
    pub const STEP: usize = 0x40;
    pub const K: usize = 0x60;
    pub const TGT: usize = 0xc0;
    pub const TGT_MOBY: usize = 0x100;
    pub const TGT_KIND: usize = 0x104;
    pub const FLASH: usize = 0x110;
    pub const MOVE: usize = 0x120;
    pub const VZ: usize = 0x128;
    pub const HOME: usize = 0x130;
    pub const POINT: usize = 0x140;
    pub const PATH: usize = 0x150;
    pub const NODE: usize = 0x158;
    pub const WAKE: usize = 0x160;
    pub const TURN_V: usize = 0x170;
    pub const MODE: usize = 0x174;
    pub const RANGE: usize = 0x178;
    pub const AREA: usize = 0x17c;
    pub const RANGE_NOW: usize = 0x180;
    pub const ALERT: usize = 0x184;
    pub const WAIT_RANGE: usize = 0x188;
    pub const WAIT_AREA: usize = 0x18c;
    pub const SIDE: usize = 0x190;
    pub const CUBOID: usize = 0x194;
    pub const WAIT: usize = 0x198;
    pub const PHASE: usize = 0x19c;
    pub const HOVER_Z: usize = 0x1a0;
    pub const HOVER_T: usize = 0x1a4;
    pub const SIZE: usize = 0x2b0;
}

/// 1196's states.
pub mod scout {
    pub const INIT: u8 = 0;
    pub const SLEEP: u8 = 1;
    pub const WAKE: u8 = 2;
    pub const RISE: u8 = 4;
    pub const PATH: u8 = 5;
    pub const CALL: u8 = 6;
    pub const PICK: u8 = 9;
    pub const HOVER: u8 = 10;
    pub const DYING: u8 = 0xb;
}

/// 1199's states.
pub mod swoop {
    pub const INIT: u8 = 0;
    pub const RISE: u8 = 2;
    pub const HOVER: u8 = 3;
    pub const WAIT: u8 = 4;
    pub const BACK: u8 = 5;
    pub const WIND_UP: u8 = 6;
    pub const DIVE: u8 = 7;
    pub const BITE: u8 = 8;
    pub const CUBOID: u8 = 9;
    pub const CHARGE: u8 = 10;
    pub const CUBOID_BITE: u8 = 0xb;
    pub const PULL_BACK: u8 = 0xc;
    pub const DYING: u8 = 0xd;
}

/// Level10 `$gp` words (gp = 0x166c00): −0x4e04 3 (the reach), −0x4e00 5 (the group's attack delay), −0x4dfc / −0x4df8
/// 0.5 / 6 (a hover point's height), −0x4df4 0.125 (the bob), −0x4df0 1.21 (the bob's period, seconds), −0x4dec 1.5
/// (the cuboid height offset), −0x4de8 4.43 (its period), −0x4de4 8, −0x4de0 2.5 (+0x58 / +0x5a).
mod k {
    pub const REACH: f32 = 3.0;
    pub const ATTACK_DELAY: i32 = 5;
    pub const POINT_Z: (f32, f32) = (0.5, 6.0);
    pub const BOB: f32 = 0.125;
    pub const BOB_PERIOD: f32 = 1.21;
    pub const HOVER_Z: f32 = 1.5;
    pub const HOVER_PERIOD: f32 = 4.43;
    pub const B58: f32 = 8.0;
    pub const B5A: f32 = 2.5;
}

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;
const TAU: f32 = std::f32::consts::TAU;
/// The dive's side, ±15° (0x3e860a92).
const SIDE: f32 = 0.261_799_4;

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
/// `if (m+0x53 != s) MobyAnimBlend(m, s, frame, ticks(n))`.
fn blend(w: &mut World, id: MobyId, s: u8, frame: i32, n: i32) {
    if seq(w, id) != s {
        let t = w.ticks(n);
        w.anim_blend(id, s, frame, t);
    }
}
fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, pv::TGT_KIND) }
/// The target moby (+0x100: Ratchet's when the search found none).
fn tgt_moby(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::TGT_MOBY) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
/// The target moby's position (`*(+0x100) + 0x10`).
fn tgt_pos(w: &World, id: MobyId) -> c::V { tgt_moby(w, id).map_or([0.0; 4], |m| w.m(m).position) }
fn clamp(x: f32, lim: f32) -> f32 { if x <= lim { if x < -lim { -lim } else { x } } else { lim } }
/// The cuboid's centre z (`0x1600ec[+0x194]` +0x38); 0 without one [L].
fn cuboid_z(w: &World, id: MobyId) -> f32 {
    w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, pv::CUBOID)).map_or(0.0, |s| s.centre()[2])
}
/// `0x26d8b0(0.5, 0, m, +0x120, 0x200, 0)`: the move with collision (radius 0x200 / 1024).
fn fly_move(w: &mut World, id: MobyId) -> u32 {
    let mut mv = c::pv4(w, id, pv::MOVE);
    let r = walker::move_collide(w, id, 0.5, 0.5, 0.0, &mut mv, 0);
    c::set_pv4(w, id, pv::MOVE, mv);
    r
}
/// The move along the yaw at `speed` with the height `dz` clamped to ±`lim` (the cases' shared shape).
fn yaw_move(w: &mut World, id: MobyId, speed: f32, dz: f32, lim: f32) {
    let (cs, sn) = c::cs(c::yaw(w, id));
    c::set_pv4(w, id, pv::MOVE, [cs * speed, sn * speed, clamp(dz, lim), 0.0]);
}

/// Lure (`+0x38`) and the alert timer (module doc).
fn lure(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::LURE) != 0 {
        let f = w.rng.randf(180.0, 240.0);
        let t = w.svc.timing.scale(Pf::f(f)).to_f32() as i32;
        c::set_pi32(w, id, pv::ALERT, t);
        c::set_pi32(w, id, pv::LURE, 0);
    }
    c::dec_timer_pvar_i32(w, id, pv::ALERT);
}

/// The common start of the death flight (the K fields both ticks write).
fn death_k(w: &mut World, id: MobyId) {
    let k = pv::K;
    c::set_pu8(w, id, k + 0x3d, 3);
    c::set_pf(w, id, k + knock::k::GRAVITY, DT2 * 26.0);
    c::set_pf(w, id, k + knock::k::DRAG, f32::from_bits(0x3a03_126f));
    c::set_pi32(w, id, k + knock::k::FLAGS, 9);
    c::set_pf(w, id, k + knock::k::SPEED, DT * 7.0);
    c::set_pf(w, id, k + knock::k::UP, DT * 10.0);
}

/// The heading of a hit: record +0x30 bit 0 → the push's, else away from the record's point.
fn hit_heading(w: &World, id: MobyId, h: &crate::moby_update::services::HitRecord) -> f32 {
    if h.w30 & 1 == 0 {
        let p = c::pos(w, id);
        c::atan(p[0] - h.pos[0].to_f32(), p[1] - h.pos[1].to_f32())
    } else {
        c::atan(h.dir[0].to_f32(), h.dir[1].to_f32())
    }
}

/// The non-lethal hit: flash 0xfa, health −= damage, cooldown `ticks(60)`.
fn hurt(w: &mut World, id: MobyId, dmg: f32) {
    c::set_pu8(w, id, pv::FLASH + 7, 0xfa);
    let hp = c::pf(w, id, pv::D);
    c::set_pf(w, id, pv::D, hp - dmg);
    let t = w.ticks(0x3c);
    c::set_pi16(w, id, pv::COOLDOWN, t as i16);
    flash::start(w, id, pv::FLASH);
}

/// The hit half of `0x2dfc38` (1196).
fn hit_1196(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 5, 4);
    let Some(h) = res.hit else { return };
    let dmg = h.damage.to_f32();
    if c::pf(w, id, pv::D) <= dmg {
        if w.svc.units.word(super::help_orxon::AIR_WORD) != 0 {
            crate::moby_update::story::award_skill_point(w, crate::moby_update::story::skill_index(0x13_d418));
        }
        c::set_pf(w, id, pv::D, 0.0);
        w.mm(id).mode &= !mode::TARGETABLE;
        blend(w, id, 5, 0, 10);
        set_state(w, id, scout::DYING);
        death_k(w, id);
        let a = hit_heading(w, id, &h);
        knock::start(w, id, pv::K, a, 5, 1, 0);
        c::set_pu8(w, id, pv::FLASH + 7, 0x78);
        flash::start(w, id, pv::FLASH);
        bolt_burst(w, id, 1, 3, 0, -1);
    } else {
        hurt(w, id, dmg);
    }
    w.mm(id).hit_slot = 0xff;
}

/// The hit half of `0x2e17c0` (1199).
fn hit_1199(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 5, 4);
    let own = w.m(id).o_class;
    // [L] a record without an attacker: the game reads class word 0 + 0xa6 (not its own class).
    if let Some(h) = res.hit.filter(|h| h.attacker.is_none_or(|a| w.m(a).o_class != own)) {
        let dmg = h.damage.to_f32();
        if c::pf(w, id, pv::D) <= dmg {
            c::set_pf(w, id, pv::D, 0.0);
            w.mm(id).mode &= !mode::TARGETABLE;
            blend(w, id, 4, 0, 10);
            set_state(w, id, swoop::DYING);
            death_k(w, id);
            w.play_sound(2, 0, id);
            let _ = hit_heading(w, id, &h);
            let dir = h.dir.map(|x| x.to_f32());
            let (mut sp, mut up) = (c::pf(w, id, pv::K + knock::k::SPEED), c::pf(w, id, pv::K + knock::k::UP));
            let a = knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, pv::K + knock::k::SPEED, sp);
            c::set_pf(w, id, pv::K + knock::k::UP, up);
            knock::start(w, id, pv::K, a, 4, 1, 0);
            c::set_pu8(w, id, pv::FLASH + 7, 0x78);
            flash::start(w, id, pv::FLASH);
            bolt_burst(w, id, 1, 3, 0, -1);
        } else {
            hurt(w, id, dmg);
        }
    }
    w.mm(id).hit_slot = 0xff;
}

/// The targeting half both ticks share (module doc); `x_out` is the cuboid test's upper bound on the local x lane
/// (1196: 0, 1199: 1).
pub fn targeting(w: &mut World, id: MobyId, x_out: f32) {
    let md = c::pi32(w, id, pv::MODE);
    let range = if md == 2 {
        42.0
    } else if c::pi32(w, id, pv::ALERT) != 0 {
        c::pf(w, id, pv::RANGE) + 6.0
    } else if state(w, id) == 1 {
        c::pf(w, id, pv::WAIT_RANGE)
    } else {
        c::pf(w, id, pv::RANGE)
    };
    c::set_pf(w, id, pv::RANGE_NOW, range);
    let t = if md == 2 {
        target::acquire(w, id, range)
    } else {
        let area = c::pi32(w, id, if state(w, id) == 1 { pv::WAIT_AREA } else { pv::AREA });
        let area = usize::try_from(area).ok().filter(|&p| p < w.svc.splines.len());
        match area {
            None => target::acquire(w, id, range),
            Some(p) => target::acquire_in(w, id, range, Some(p)),
        }
    };
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
    if t.kind != 2 {
        if md == 2 {
            let hp = w.hero_point();
            let out = match w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, pv::CUBOID)) {
                Some(s) => {
                    let l = s.local(hp);
                    l[0] <= -1.0 || x_out <= l[0] || l[1] <= -1.0 || 1.0 <= l[1] || l[2] <= -1.0 || 1.0 <= l[2]
                }
                None => true,
            };
            if out { c::set_pi32(w, id, pv::TGT_KIND, 2); }
        }
        let home = c::pv4(w, id, pv::HOME);
        if c::pf(w, id, pv::RANGE_NOW) < c::dist2(home, t.pos) || w.m(id).visible == 0 { c::set_pi32(w, id, pv::TGT_KIND, 2); }
    }
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let h = w.hero_moby.map_or(0, |m| m as i32 + 1);
        c::set_pi32(w, id, pv::TGT_MOBY, h);
    }
}

/// The shadow probe both updates make (drawn, within 29 of the camera).
fn shadow(w: &mut World, id: MobyId) {
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 29.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x17;
    }
}

/// The D record and the home both inits write.
fn init_common(w: &mut World, id: MobyId, d10: f32) {
    c::set_pf(w, id, pv::D + 0x10, d10);
    c::set_pf(w, id, pv::D, 1.0);
    c::set_pu8(w, id, pv::D + 9, 1);
    c::set_pi16(w, id, pv::D + 4, 1);
    c::set_pu8(w, id, pv::D + 8, 0);
    w.mm(id).mode |= mode::TARGETABLE;
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOME, p);
    c::set_pv4(w, id, pv::POINT, p);
}

/// Steer toward `to` and fly there (1196 case 5, 1199 case 3): the turn, the speed spring when facing it within 45°,
/// the move. Returns (the distance before the move, the move's result bits).
fn steer_to(w: &mut World, id: MobyId, to: c::V) -> (f32, u32) {
    let mut speed = c::len3(c::pv4(w, id, pv::MOVE));
    let p = c::pos(w, id);
    let d = c::dist3(p, to);
    let h = c::atan(to[0] - p[0], to[1] - p[1]);
    turn::turn_toward_pvar(w, id, h, DT2 * 25.132_742, DT2 * 25.132_742, DT * 50.265_484, pv::TURN_V);
    if c::diff_rots(h, c::yaw(w, id)) < std::f32::consts::FRAC_PI_4 {
        let mut x = 0.0;
        turn::spring(d, DT2 * 20.0, DT2 * 20.0, DT * 20.0, &mut x, &mut speed);
        let v = c::clamp_len3(c::sub(to, p), speed);
        c::set_pv4(w, id, pv::MOVE, v);
    } else {
        c::set_pv4(w, id, pv::MOVE, [0.0; 4]);
    }
    (d, fly_move(w, id))
}

/// The ground point below the moby (`GroundHeight(0.5, pos, 0)` then the hit point 0x1742e0; [L] a miss: z 0).
fn ground_point(w: &World, id: MobyId) -> c::V {
    let p = c::pos(w, id);
    let g = ground::ground(w, p, 0.5, 0);
    [p[0], p[1], g.z, p[3]]
}

/// 1196: the end of the path → hover about the ground point below (state 10, blend 4).
fn path_end(w: &mut World, id: MobyId) {
    set_state(w, id, scout::HOVER);
    blend(w, id, 4, 0, 10);
    let g = ground_point(w, id);
    c::set_pv4(w, id, pv::HOME, g);
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::POINT, p);
}

/// 1196 case 5: one step along the path (module doc).
fn path_step(w: &mut World, id: MobyId) {
    w.mm(id).anim.speed = 4.0;
    let Some(pts) = usize::try_from(c::pi32(w, id, pv::PATH)).ok().and_then(|p| w.svc.splines.get(p)).cloned() else { return };
    let n = c::pi32(w, id, pv::NODE);
    let Some(pt) = usize::try_from(n).ok().and_then(|i| pts.get(i)).map(|q| q.map(f32::from_bits)) else { return };
    let (d, _) = steer_to(w, id, pt);
    if 1.0 <= d { return; }
    c::set_pi32(w, id, pv::NODE, n + 1);
    if pt[3] <= 0.0 {
        if n + 1 == pts.len() as i32 { path_end(w, id); }
        return;
    }
    set_state(w, id, scout::CALL);
    blend(w, id, 3, 0, 10);
}

/// `0x2e0138(m)` / `0x2e47d8(m)`: the mobys of `m`'s group in state 1 go to 2 (the sleepers wake).
pub fn wake_group_of(w: &mut World, m: MobyId) {
    let g = w.m(m).group;
    if g < 0 { return; }
    let Some(Some(list)) = w.svc.groups.lists.get(g as usize) else { return };
    for e in list.clone() {
        if let Some(x) = w.table.mobys.get_mut((e & 0x7fff) as usize) {
            if x.state == 1 { x.state = 2; }
        }
    }
}

/// 1196 case 6 (module doc).
fn wake_group(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    let t = tgt_pos(w, id);
    turn::turn_toward_pvar(w, id, c::atan(t[0] - p[0], t[1] - p[1]), DT2 * 25.132_742, DT2 * 25.132_742, DT * 50.265_484, pv::TURN_V);
    if !wrapped(w, id) { return; }
    let pts = usize::try_from(c::pi32(w, id, pv::PATH)).ok().and_then(|p| w.svc.splines.get(p)).cloned().unwrap_or_default();
    let n = c::pi32(w, id, pv::NODE);
    let wv = usize::try_from(n - 1).ok().and_then(|i| pts.get(i)).map_or(0.0, |q| f32::from_bits(q[3]));
    let slot = wv as i32 - 1;
    if (0..4).contains(&slot) {
        let m = c::pi32(w, id, pv::WAKE + 4 * slot as usize);
        if let Some(m) = usize::try_from(m).ok().filter(|&m| m < w.table.mobys.len()) { wake_group_of(w, m); }
    }
    if n == pts.len() as i32 {
        path_end(w, id);
        return;
    }
    set_state(w, id, scout::PATH);
    blend(w, id, 4, 0, 10);
}

/// Level10 `0x2df270`: the path scout 1196 (module doc).
pub fn update_1196(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    lure(w, id);
    if state(w, id) != scout::DYING { hit_1196(w, id); }
    flash::update(w, id, pv::FLASH);
    targeting(w, id, 0.0);
    // 0x256da0(2.5, m, 0, +0x270): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 0, id, 0x270);
    shadow(w, id);
    w.mm(id).anim.speed = 1.0;
    match state(w, id) {
        scout::INIT => {
            init_common(w, id, 0.5);
            if c::pi32(w, id, pv::MODE) != 1 {
                w.delete_moby(id);
                return;
            }
            set_state(w, id, scout::SLEEP);
        }
        scout::SLEEP => {
            if kind(w, id) == 2 && c::pi32(w, id, pv::MODE) != 0 { return; }
            set_state(w, id, scout::WAKE);
            blend(w, id, 1, 0, 10);
        }
        scout::WAKE => {
            if !wrapped(w, id) { return; }
            set_state(w, id, scout::RISE);
            c::set_pv4(w, id, pv::MOVE, [0.0; 4]);
            blend(w, id, 4, 0, 10);
        }
        scout::RISE => {
            w.mm(id).anim.speed = 4.0;
            let to = c::pf(w, id, pv::HOME + 8) + 2.0;
            let (mut z, mut v) = (w.m(id).position[2], c::pf(w, id, pv::VZ));
            let r = turn::spring(to, DT2 * 18.0, DT2 * 18.0, DT * 6.0, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, pv::VZ, v);
            if r == 0.0 { set_state(w, id, scout::PATH); }
        }
        scout::PATH => path_step(w, id),
        scout::CALL => wake_group(w, id),
        8 => {
            let mut pitch = w.m(id).rotation[1];
            turn::approach_rot(0.0, DT * 12.566_371, &mut pitch);
            w.mm(id).rotation[1] = pitch;
            let home = c::pv4(w, id, pv::HOME);
            let p = c::pos(w, id);
            turn::spring_turn2_pvar(w, id, c::atan(home[0] - p[0], home[1] - p[1]), f32::from_bits(0x3ca3_d70a), f32::from_bits(0x3e99_999a), DT * TAU, pv::TURN_V);
            yaw_move(w, id, DT * 6.0, home[2] + 2.0 - p[2], DT * 6.0);
            fly_move(w, id);
            if c::dist2(c::pos(w, id), home) < 2.0 { set_state(w, id, 7); }
        }
        scout::PICK => {
            if c::dec_timer_pvar_i32(w, id, pv::PHASE) == 0 { return; }
            let t = w.ticks(0x78);
            c::set_pi32(w, id, pv::PHASE, t);
            let r = w.rng.randf(0.0, 3.0);
            let a = w.rng.rand_angle();
            let (cs, sn) = c::cs(a);
            let z = w.rng.randf(5.0, 7.0);
            let home = c::pv4(w, id, pv::HOME);
            c::set_pv4(w, id, pv::POINT, c::add([cs * r, sn * r, z, 0.0], home));
            set_state(w, id, scout::HOVER);
        }
        scout::HOVER => {
            let to = c::pv4(w, id, pv::POINT);
            let p = c::pos(w, id);
            turn::spring_turn2_pvar(w, id, c::atan(to[0] - p[0], to[1] - p[1]), f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3e99_999a), f32::from_bits(0x3e4c_cccd), pv::TURN_V);
            let step = c::clamp_len3(c::sub(to, p), DT * 4.0);
            let next = c::add(step, p);
            if w.coll_sphere(next.map(Pf::f), Pf::f(0.4), 0, Some(id)).is_none() {
                c::set_pos(w, id, next);
                c::set_pv4(w, id, pv::STEP, step);
            } else {
                c::set_pv4(w, id, pv::STEP, [0.0; 4]);
                set_state(w, id, scout::PICK);
            }
            if c::len3(c::pv4(w, id, pv::STEP)) == 0.0 { set_state(w, id, scout::PICK); }
        }
        scout::DYING if knock::update(w, id, pv::K) & 1 != 0 => {
            set_death_bits(w, id, 0x200, -1);
            let p = c::pos(w, id);
            fx::piece_explosion(w, 0.5, 10.0, Some(id), p);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// `0x2e1cb0(m)`: every moby of its group gets +0xbc = `ticks(5)` (the next attacker waits).
fn alert_group(w: &mut World, id: MobyId) {
    let g = w.m(id).group;
    if g < 0 { return; }
    let Some(Some(list)) = w.svc.groups.lists.get(g as usize) else { return };
    for e in list.clone() {
        let t = w.ticks(k::ATTACK_DELAY) as u8;
        if let Some(x) = w.table.mobys.get_mut((e & 0x7fff) as usize) { x.cmd = t; }
    }
}

/// `FastDecTimer` on the moby's +0xbc byte (0x220ed8).
fn dec_cmd(w: &mut World, id: MobyId) -> i32 { crate::moby_update::services::fast_dec_timer_u8(&mut w.mm(id).cmd) }

/// The start of an attack from the hover (1199 cases 3 / 4): a target beyond the reach (xy) and the +0xbc delay out.
fn attack_start(w: &mut World, id: MobyId) -> bool {
    if kind(w, id) == 2 || c::dist2(c::pos(w, id), tgt_pos(w, id)) <= k::REACH { return false; }
    if dec_cmd(w, id) == 0 { return false; }
    alert_group(w, id);
    set_state(w, id, swoop::WIND_UP);
    blend(w, id, 1, 0, 10);
    true
}

/// A new hover point about home (1199 cases 3 / 4).
fn new_point(w: &mut World, id: MobyId) {
    let r = w.rng.randf(4.0, 6.0);
    let a = w.rng.rand_angle();
    let (cs, sn) = c::cs(a);
    let z = w.rng.randf(k::POINT_Z.0, k::POINT_Z.1);
    let home = c::pv4(w, id, pv::HOME);
    c::set_pv4(w, id, pv::POINT, c::add([cs * r, sn * r, z, 0.0], home));
}

/// 1199 case 3 (module doc).
fn hover(w: &mut World, id: MobyId) {
    w.mm(id).anim.speed = 4.0;
    let to = c::pv4(w, id, pv::POINT);
    let (d, r) = steer_to(w, id, to);
    if r == 1 {
        new_point(w, id);
    } else if d < 0.1 {
        set_state(w, id, swoop::WAIT);
        let f = w.rng.randf(30.0, 90.0);
        let t = w.ticks(f as i32);
        c::set_pi32(w, id, pv::WAIT, t);
    }
    attack_start(w, id);
}

/// The cuboid hover's height offset and its timer (1199: `randf(0, 1.5)`, `trunc(scale(4.43·60)·randf(0.75, 1.25))`).
fn cuboid_redraw(w: &mut World, id: MobyId) {
    let z = w.rng.randf(0.0, k::HOVER_Z);
    c::set_pf(w, id, pv::HOVER_Z, z);
    let a = w.svc.timing.scale(Pf::f(k::HOVER_PERIOD * 60.0)).to_f32();
    let b = w.rng.randf(0.75, 1.25);
    c::set_pi32(w, id, pv::HOVER_T, (a * b) as i32);
}

/// The bite template of 1199 (case 8: the push along the yaw; case 0xb: none).
fn bite_template(w: &World, id: MobyId, push: bool) -> HitTemplate {
    let (cs, sn) = c::cs(c::yaw(w, id));
    let (dir, flags, b19, h1a) = if push {
        ([Pf::f(cs), Pf::f(sn), Pf::ONE, Pf::b(0x45af_df66)], 1, 1, w.m(id).o_class as u16)
    } else {
        ([Pf::ZERO; 4], 0x1_0001, 0, 0)
    };
    HitTemplate { dir, attacker: Some(id), flags, b18: 0, b19, h1a, damage: Pf::ONE, w20: 1 }
}

/// Turn toward `to` with `SpringTurn2(…, 0.02, 0.3, max)`.
fn face(w: &mut World, id: MobyId, to: c::V, add: f32, max: f32) {
    let p = c::pos(w, id);
    let h = c::add_rot(c::atan(to[0] - p[0], to[1] - p[1]), add);
    turn::spring_turn2_pvar(w, id, h, f32::from_bits(0x3ca3_d70a), f32::from_bits(0x3e99_999a), max, pv::TURN_V);
}

/// Back to the hover (blend 3, state 5): the dive's and the bite's end.
fn back(w: &mut World, id: MobyId) {
    blend(w, id, 3, 0, 0x14);
    set_state(w, id, swoop::BACK);
}

/// The cuboid mode's lost target: back to 9 with a new height.
fn to_cuboid(w: &mut World, id: MobyId) {
    set_state(w, id, swoop::CUBOID);
    cuboid_redraw(w, id);
}

/// Level10 `0x2e01a8`: the swoop flyer 1199 (module doc).
pub fn update_1199(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    lure(w, id);
    if state(w, id) != swoop::DYING { hit_1199(w, id); }
    flash::update(w, id, pv::FLASH);
    targeting(w, id, 1.0);
    // 0x256da0(2.5, m, 1, +0x270): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 1, id, 0x270);
    shadow(w, id);
    w.mm(id).anim.speed = 1.0;
    let s = state(w, id);
    if s != swoop::INIT && s != swoop::DYING {
        let b = k::BOB * c::pf(w, id, pv::PHASE).cos();
        w.mm(id).position[2] -= b;
    }
    let t = tgt_pos(w, id);
    match s {
        swoop::INIT => {
            init_common(w, id, 0.25);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::TURN_V, 0.0);
            c::set_pf(w, id, pv::PHASE, a);
            c::set_pu8(w, id, 0x58, k::B58 as u8);
            c::set_pu8(w, id, 0x5a, (k::B5A * 8.0) as u8);
            match c::pi32(w, id, pv::MODE) {
                1 => {
                    w.delete_moby(id);
                    return;
                }
                0 => {
                    set_state(w, id, swoop::HOVER);
                    w.mm(id).position[2] += 2.0;
                    blend(w, id, 3, 0, 10);
                }
                2 => {
                    let a = w.rng.rand_angle();
                    let x = a.cos() * 10.0;
                    let b = w.rng.rand_angle();
                    let y = b.sin() * 10.0;
                    set_state(w, id, swoop::CUBOID);
                    let z = cuboid_z(w, id);
                    let m = w.mm(id);
                    m.position[2] = z;
                    m.position[0] += x;
                    m.position[1] += y;
                    cuboid_redraw(w, id);
                    blend(w, id, 3, 0, 10);
                }
                _ => return,
            }
        }
        swoop::RISE => {
            w.mm(id).anim.speed = 4.0;
            let to = c::pf(w, id, pv::HOME + 8) + 2.0;
            let (mut z, mut v) = (w.m(id).position[2], c::pf(w, id, pv::VZ));
            let r = turn::spring(to, DT2 * 18.0, DT2 * 18.0, DT * 6.0, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, pv::VZ, v);
            if r == 0.0 { set_state(w, id, swoop::HOVER); }
        }
        swoop::HOVER => hover(w, id),
        swoop::WAIT => {
            w.mm(id).anim.speed = 4.0;
            if !attack_start(w, id) && c::dec_timer_pvar_i32(w, id, pv::WAIT) != 0 {
                new_point(w, id);
                set_state(w, id, swoop::HOVER);
            }
        }
        swoop::BACK => {
            let mut pitch = w.m(id).rotation[1];
            turn::approach_rot(0.0, DT * 12.566_371, &mut pitch);
            w.mm(id).rotation[1] = pitch;
            let home = c::pv4(w, id, pv::HOME);
            face(w, id, home, 0.0, DT * TAU);
            let dz = home[2] + 2.0 - w.m(id).position[2];
            yaw_move(w, id, DT * 6.0, dz, DT * 6.0);
            fly_move(w, id);
            if c::dist2(c::pos(w, id), home) < 2.0 { set_state(w, id, swoop::HOVER); }
        }
        swoop::WIND_UP => {
            face(w, id, t, 0.0, DT * TAU);
            if wrapped(w, id) {
                set_state(w, id, swoop::DIVE);
                blend(w, id, 3, 0, 10);
                let side = if w.rng.randi(0xff) & 1 == 0 { -SIDE } else { SIDE };
                c::set_pf(w, id, pv::SIDE, side);
            }
        }
        swoop::DIVE => {
            let side = c::pf(w, id, pv::SIDE);
            face(w, id, t, side, DT * TAU);
            let p = c::pos(w, id);
            let g = ground::ground(w, p, 0.5, 0).z;
            let fz = if t[2] < p[2] { t[2] - (p[2] - t[2]) * 4.0 } else { p[2] };
            let aim_z = if g <= fz + 0.5 { fz + 0.5 } else { g };
            let v = c::set_len3(c::sub([t[0], t[1], aim_z, t[3]], p), DT * 10.0);
            // x = cos yaw·|v.xy|, then y = sin yaw·|(x, v.y)| (the second length reads the x just written).
            let (cs, sn) = c::cs(c::yaw(w, id));
            let x = cs * c::len2(v);
            let y = sn * c::len2([x, v[1], 0.0, 0.0]);
            c::set_pv4(w, id, pv::MOVE, [x, y, v[2], v[3]]);
            fly_move(w, id);
            let mv = c::pv4(w, id, pv::MOVE);
            let mut pitch = w.m(id).rotation[1];
            turn::approach_rot(-c::atan(c::len2(mv), mv[2]), DT * TAU, &mut pitch);
            w.mm(id).rotation[1] = pitch;
            let p = c::pos(w, id);
            if c::dist2(p, t) < k::REACH {
                if (p[2] - crate::moby_update::classes::units::hero_pos(w)[2]).abs() < 2.0 {
                    w.play_sound(0, 0, id);
                    set_state(w, id, swoop::BITE);
                    c::set_pv4(w, id, pv::MOVE, [0.0; 4]);
                    blend(w, id, 2, 4, 5);
                    return tail_1199(w, id);
                }
                back(w, id);
            } else if kind(w, id) == 2 {
                back(w, id);
            }
        }
        swoop::BITE => {
            face(w, id, t, 0.0, DT * TAU);
            let aim = c::pv4(w, id, pv::TGT + 0x20);
            let p = c::pos(w, id);
            let mut pitch = w.m(id).rotation[1];
            turn::approach_rot(-c::atan(c::dist2(p, aim), aim[2] - p[2]), DT * 12.566_371, &mut pitch);
            w.mm(id).rotation[1] = pitch;
            if ground::passed_frame(w, id, 20.0) {
                let tmpl = bite_template(w, id, true);
                let j = w.joint_point(id, 0);
                w.sphere_mobys(Pf::f(0.4), j.map(Pf::f), 0, Some(id), Some(&tmpl));
                let p = c::pos(w, id);
                crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, p.map(Pf::f), j.map(Pf::f), 9, Some(id), &tmpl);
            }
            if kind(w, id) == 2 || wrapped(w, id) { back(w, id); }
        }
        swoop::CUBOID => {
            let d = c::dist3(c::pos(w, id), t);
            let home = c::pv4(w, id, pv::HOME);
            face(w, id, home, 0.0, DT * std::f32::consts::FRAC_PI_4);
            if c::dec_timer_pvar_i32(w, id, pv::HOVER_T) != 0 { cuboid_redraw(w, id); }
            let dz = cuboid_z(w, id) - w.m(id).position[2] - c::pf(w, id, pv::HOVER_Z);
            yaw_move(w, id, DT * 4.0, dz, DT + DT);
            fly_move(w, id);
            if kind(w, id) != 2 && dec_cmd(w, id) != 0 && k::REACH < d {
                alert_group(w, id);
                set_state(w, id, swoop::CHARGE);
                blend(w, id, 3, 0, 10);
            }
        }
        swoop::CHARGE => {
            let d = c::dist3(c::pos(w, id), t);
            alert_group(w, id);
            face(w, id, t, 0.0, DT * std::f32::consts::FRAC_PI_2);
            let dz = cuboid_z(w, id) - w.m(id).position[2];
            yaw_move(w, id, DT * 4.0, dz, DT * 4.0);
            fly_move(w, id);
            if d < k::REACH && (cuboid_z(w, id) - w.m(id).position[2]).abs() < 1.0 {
                w.play_sound(0, 0, id);
                set_state(w, id, swoop::CUBOID_BITE);
                blend(w, id, 2, 0, 2);
            } else if kind(w, id) == 2 {
                to_cuboid(w, id);
            }
        }
        swoop::CUBOID_BITE => {
            alert_group(w, id);
            face(w, id, t, 0.0, DT * TAU);
            let dz = cuboid_z(w, id) - w.m(id).position[2];
            yaw_move(w, id, DT * 6.0, dz, DT * 3.0);
            fly_move(w, id);
            let tmpl = bite_template(w, id, false);
            let j = w.joint_point(id, 0);
            w.sphere_mobys(Pf::f(0.2), j.map(Pf::f), 0, Some(id), Some(&tmpl));
            let d = c::dist3(c::pos(w, id), t);
            if 1.0 <= d && w.hero.group != 7 && !wrapped(w, id) {
                if kind(w, id) == 2 { to_cuboid(w, id); }
            } else {
                set_state(w, id, swoop::PULL_BACK);
                blend(w, id, 3, 0, 2);
            }
        }
        swoop::PULL_BACK => {
            let home = c::pv4(w, id, pv::HOME);
            face(w, id, home, 0.0, DT * std::f32::consts::FRAC_PI_3);
            let dz = cuboid_z(w, id) - w.m(id).position[2];
            yaw_move(w, id, DT * 4.0, dz, DT * 3.0);
            // `0x1f28f8(_, pos, move)` (= `0x2211a0`, pos += move in place): no collision.
            let mv = c::pv4(w, id, pv::MOVE);
            let p = c::add(c::pos(w, id), mv);
            c::set_pos(w, id, p);
            if c::dist2(c::pos(w, id), home) < 2.0 {
                to_cuboid(w, id);
                w.mm(id).cmd = w.ticks(0x78) as u8;
            }
        }
        swoop::DYING if knock::update(w, id, pv::K) & 1 != 0 => {
            set_death_bits(w, id, 0x200, -1);
            let p = c::pos(w, id);
            fx::piece_explosion(w, 0.5, 10.0, Some(id), p);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    tail_1199(w, id);
}

/// The bob (states ∉ {0, 0xd}): phase += 360° / `trunc(scale(1.21·60))`, z += 0.125·cos.
fn tail_1199(w: &mut World, id: MobyId) {
    let s = state(w, id);
    if s == swoop::INIT || s == swoop::DYING { return; }
    let n = w.svc.timing.scale(Pf::f(k::BOB_PERIOD * 60.0)).to_f32() as i32;
    let a = c::add_rot(c::pf(w, id, pv::PHASE), (360.0 / n as f32) * 0.017_453_292);
    c::set_pf(w, id, pv::PHASE, a);
    w.mm(id).position[2] += k::BOB * a.cos();
}
