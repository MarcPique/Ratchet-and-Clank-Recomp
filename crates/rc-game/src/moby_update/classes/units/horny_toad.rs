//! U25: class 749, the Veldin horny toads (levels 00: 16, 18: 90 created instances; level00 `0x2d4610`, level18
//! `0x2e02e8`, the same code). A small ground creature over the shared creature layer: it wanders around its home
//! inside its area path (the random wander [`walker::wander`], level00 `0x261630`), goes for Ratchet (or a decoy)
//! inside that area within 24, bites (a sphere hit at its joint 0), dies from any damaging hit (a death flight, the
//! bolts, a small explosion), can be lured by the Taunter, carried by the Suck Cannon (its reaction table,
//! [`react::VELDIN_749`]), and presses a floor switch 830 it is knocked onto.
//!
//! **Pvars** (0x280; the creature header: damage record +0x20, hit flash +0x60, knockback +0x70, suck record +0xd0):
//! +0x180 the target record (`0x274df8`: position, Euler, aim, body; +0x1c0 the moby, +0x1c4 the kind), +0x1d0 home,
//! +0x1e0 the area path (the target search's polygon, the chase's walls, the wall push), +0x1e4 the arrival area (state
//! 3), +0x1e8 the arrival path (−1 on every instance of 00 and 18), +0x1ec a moby to face (state 1), +0x1f0 a floor
//! switch, +0x1f4 the turn velocity, +0x1f8 the walk speed, +0x1fc the side offset, +0x200 the big-head
//! manipulator, +0x240 the wander record ([`walker::wr`]), +0x270 the vertical speed, +0x274 s16 the arrival node,
//! +0x276 s16 the lure timer, +0x278 the last walk heading.
//!
//! **The tick** (`0x2d5160`, before the states, when the state is not 0): hidden in game mode 2; the hit (mask
//! 0x330000) through the resolver (column 4) — any damage that is not a "no-reaction" result (`out5 ≠ 1`) starts the
//! death flight (0xc: the knockback record `40·dt²` gravity, `14·dt²` drag, `13·dt` / `14·dt` out / up scaled by the
//! push, sequence 6 or 7 (`randi(2)`), apex / landing keys 14 / 25), untargetable, the red flash 0x78, `SetDeathBits`;
//! the flash; the target search (`0x274df8` in the area path, 24; 64 in the arrival area in state 3; none → Ratchet);
//! the lure (+0x38) → the alert timer `randf(180, 240)` ticks, and wandering / going home with the alert on → the chase
//! (10).
//!
//! **States**: 0 init (scale = class scale × 0.75, home, the wander record, → 5, or the arrival path 3, or 1);
//! 1 face moby +0x1ec (gone → 8); 2 / 4 walk the arrival path (→ 5 at its end, → 6 on a target); 3 wait at its start
//! for a target; 5 wander (sequence 4 at a random frame, anim speed `randf(0.92, 1.08)`; a target → 6 with a
//! `randf(−20°, 20°)` side offset); 6 walk to the target, within 2 and facing it within 10° → 7; 7 bite (at key 34 of
//! sequence 5: [`attack::joint_hit`] radius 0.333, damage 1, flags 1, push 1; the anim over: bite again when within
//! 1.5 and facing, else 6, no target → 8); 8 walk home (→ 5 within 1.5, a target → 6); 9 held by the Suck Cannon
//! ([`react::carried`]; landed → 5); 10 the chase (toward the lure / target while 2.5 ahead stays inside the area,
//! else turn to it with sequence 2 / 3); 0xb a knockback flight (→ 5); 0xc the death flight (landed or out: class
//! sound 7, the small explosion [`fx::piece_explosion`]`(0.75, 10)`, deleted).
//!
//! **After the states**: out of 9 / 0xb / 0xc the height eases to the ground (`Approach(GroundHeight, 7·dt)`) and,
//! but in 4, the position is kept 0.25 off the area path's walls ([`crate::path::push_from_walls`]); a floor switch
//! +0x1f0 within 1 (xy) is pressed ([`floor_switch::press`]) and forgotten.
//!
//! Coverage (every call and branch of `0x2d4610`, `0x2d5160`, `0x2d54c8`, `0x2dc9f0`, `0x2d5830`) is in
//! docs/plan/creatures.md §9. Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::floor_switch;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, ground, knock, react, target, turn, walker};
use crate::moby_update::services::World;

/// The update in the level00 class table (the reference overlay).
pub const UPDATE_FN: u32 = 0x2d_4610;
pub const REFERENCE_LEVEL: u32 = 0;
pub const CLASSES: [i16; 1] = [749];
/// The bite's joint point (list 0).
pub const JOINTS: [i16; 1] = [749];

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const SUCK: usize = 0xd0;
    pub const TGT: usize = 0x180;
    pub const TGT_MOBY: usize = 0x1c0;
    pub const TGT_KIND: usize = 0x1c4;
    pub const HOME: usize = 0x1d0;
    pub const AREA: usize = 0x1e0;
    pub const ARRIVE_AREA: usize = 0x1e4;
    pub const ARRIVE: usize = 0x1e8;
    pub const LOOK: usize = 0x1ec;
    pub const SWITCH: usize = 0x1f0;
    pub const TURN_V: usize = 0x1f4;
    pub const SPEED: usize = 0x1f8;
    pub const SIDE: usize = 0x1fc;
    pub const WANDER: usize = 0x240;
    pub const VZ: usize = 0x270;
    pub const NODE: usize = 0x274;
    pub const ALERT_T: usize = 0x276;
    pub const LAST_YAW: usize = 0x278;
    pub const SIZE: usize = 0x280;
}

/// The level00 `.lit` words (gp = 0x166c00): gp−0x5294 0.75 (the scale), −0x5290 40, −0x528c 14, −0x5288 13,
/// −0x5284 14 (the death flight: gravity, drag ×dt², out, up ×dt), −0x5280 2 (the bite range), −0x527c 7 (the
/// bite's start frame), −0x5278 25 / −0x5274 14 (the flight's landing / apex keys).
pub mod k {
    pub const SCALE: f32 = 0.75;
    pub const GRAVITY: f32 = 40.0;
    pub const DRAG: f32 = 14.0;
    pub const OUT: f32 = 13.0;
    pub const UP: f32 = 14.0;
    pub const BITE_RANGE: f32 = 2.0;
    pub const BITE_FRAME: i32 = 7;
    pub const KEY_LAND: f32 = 25.0;
    pub const KEY_APEX: f32 = 14.0;
    /// The bite's key frame (0x42080000).
    pub const BITE_KEY: f32 = 34.0;
    /// 10° (0.17453292).
    pub const FACING: f32 = 0.174_532_92;
    /// The point near which the leash is 0.75 instead of 1.5 ((150, 127), within 5 xy).
    pub const TIGHT_AT: [f32; 2] = [150.0, 127.0];
}

/// The state machine's states (module doc).
pub mod st {
    pub const INIT: u8 = 0;
    pub const FACE: u8 = 1;
    pub const ARRIVE_WALK: u8 = 2;
    pub const ARRIVE_WAIT: u8 = 3;
    pub const ARRIVE: u8 = 4;
    pub const WANDER: u8 = 5;
    pub const GO: u8 = 6;
    pub const BITE: u8 = 7;
    pub const HOME: u8 = 8;
    pub const HELD: u8 = 9;
    pub const CHASE: u8 = 10;
    pub const KNOCKED: u8 = 0xb;
    pub const DYING: u8 = 0xc;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
/// `if (m+0x53 != seq) fun_00212f90(m, seq, 0, ticks(n))`.
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    if w.m(id).anim.seq_b != seq {
        let t = w.ticks(n);
        w.anim_blend(id, seq, 0, t);
    }
}
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, pv::TGT_KIND) }
/// The target moby of the record (the port stores moby + 1; 0 none).
fn target_moby(w: &World, id: MobyId) -> Option<MobyId> {
    let v = c::pi32(w, id, pv::TGT_MOBY);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn path(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn path_point(w: &World, p: usize, i: usize) -> c::V { w.svc.splines[p].get(i).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }
/// A moby of the table by a pvar index (−1 none; gone when deleted).
fn moby_at(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let i = usize::try_from(c::pi32(w, id, o)).ok().filter(|&i| i < w.table.mobys.len())?;
    let s = w.m(i).state;
    (s != crate::moby_runtime::state::DELETED && s != crate::moby_runtime::state::DELETED_STATIC).then_some(i)
}

/// `0x274df8` into the record +0x180 (the target record's layout: position, Euler, aim, body, moby, kind).
fn store_target(w: &mut World, id: MobyId, t: &target::Target) {
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
}

/// `0x2d5160`: the tick before the states (module doc). Not run in state 0.
fn pre(w: &mut World, id: MobyId) {
    if w.svc.game_mode == 2 {
        let m = w.mm(id);
        m.visible = 0;
        m.mode |= mode::HIDDEN;
    } else {
        let m = w.mm(id);
        m.visible = 1;
        m.mode &= !mode::HIDDEN;
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING && res.damage != 0.0 {
        let kr = pv::K;
        c::set_pu8(w, id, kr + 0x3d, 0);
        c::set_pf(w, id, kr + knock::k::GRAVITY, k::GRAVITY * c::DT2);
        c::set_pf(w, id, kr + knock::k::DRAG, k::DRAG * c::DT2);
        c::set_pf(w, id, kr + knock::k::SPEED, k::OUT * c::DT);
        c::set_pf(w, id, kr + knock::k::UP, k::UP * c::DT);
        c::set_pf(w, id, kr + knock::k::ZOFF, 0.75);
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        c::set_pf(w, id, kr + knock::k::AIR_SPEED, c::DT + c::DT);
        c::set_pi32(w, id, kr + knock::k::FLAGS, 0x29);
        c::set_pi32(w, id, kr + knock::k::RADIUS, 0x200);
        w.mm(id).mode &= !mode::TARGETABLE;
        let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
        let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
        let a = knock::aim(dir, &mut sp, &mut up);
        c::set_pf(w, id, kr + knock::k::SPEED, sp);
        c::set_pf(w, id, kr + knock::k::UP, up);
        let s = w.rng.randi(2);
        knock::start(w, id, kr, a, (s + 6) as u8, 1, 0);
        c::set_pf(w, id, kr + knock::k::KEY_APEX, k::KEY_APEX);
        c::set_pf(w, id, kr + knock::k::KEY_LAND, k::KEY_LAND);
        set_state(w, id, st::DYING);
        c::set_pu8(w, id, pv::FLASH + 7, 0x78);
        flash::start(w, id, pv::FLASH);
        set_death_bits(w, id, 0, -1);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    let (range, area) = if state(w, id) == st::ARRIVE_WAIT { (64.0, pv::ARRIVE_AREA) } else { (24.0, pv::AREA) };
    // A path index of −1 makes the game read the word before its path table as the polygon (level00 0x1b04ac:
    // 1.0f); no instance of 00 / 18 has one: the port searches without a polygon then [L].
    let region = path(w, id, area);
    let t = target::acquire_in(w, id, range, region);
    store_target(w, id, &t);
    if t.moby.is_none() {
        let hm = w.hero_moby;
        c::set_pi32(w, id, pv::TGT_MOBY, hm.map_or(0, |m| m as i32 + 1));
        let hp = crate::moby_update::classes::units::hero_pos(w);
        c::set_pv4(w, id, pv::TGT, hp);
    }
    c::dec_timer_pvar_s16(w, id, pv::ALERT_T);
    if c::pi32(w, id, pv::LURE) == 0 {
        c::set_pi32(w, id, pv::LURE, 0);
    } else {
        // `truncate_float_to_s32(multiply_global_scale(randf(180, 240)))`: ticks (NTSC: the value).
        let f = w.rng.randf(180.0, 240.0);
        let t = w.ticks(f as i32);
        c::set_pi16(w, id, pv::ALERT_T, t as i16);
        c::set_pi32(w, id, pv::LURE, 0);
    }
    let s = state(w, id);
    if (s == st::WANDER || s == st::HOME) && c::pi16(w, id, pv::ALERT_T) != 0 {
        set_state(w, id, st::CHASE);
        blend(w, id, 4, 20);
    }
}

/// `0x2d54c8(0.5, moby, point)`: one walking tick toward `point`: the heading to it (plus the side offset while
/// farther than 2), once the blend is done: turn (`0x270cc0`, 4π·dt² / 4π·dt), the speed springs (`0x270830`: from
/// 0.5 toward the distance, 8·dt² up, 12·dt² down, at most 4·dt, never negative), the move (`0x26d610(0.5, 0.5, 0,
/// …, 0x10)`) with the vertical speed (−10·dt² a tick, capped at 4·dt by the height change). Returns the 3-D
/// distance, or 37 while the blend is done and the animation has not wrapped this tick.
fn walk(w: &mut World, id: MobyId, point: c::V) -> f32 {
    let p = c::pos(w, id);
    let mut h = c::atan(point[0] - p[0], point[1] - p[1]);
    let z0 = p[2];
    let d = c::dist3(p, point);
    if 2.0 < d { h = c::add_rot(h, c::pf(w, id, pv::SIDE)); }
    let a = &w.m(id).anim;
    if a.seq_a != a.seq_b { return d; }
    let r = 4.0 * std::f32::consts::PI;
    turn::turn_toward_pvar(w, id, h, c::DT2 * r, c::DT2 * r, c::DT * r, pv::TURN_V);
    let mut x = 0.5;
    let mut v = c::pf(w, id, pv::SPEED);
    turn::spring(d, c::DT2 * 8.0, c::DT2 * 12.0, c::DT * 4.0, &mut x, &mut v);
    if v < 0.0 { v = 0.0; }
    c::set_pf(w, id, pv::SPEED, v);
    let yaw = c::yaw(w, id);
    c::set_pf(w, id, pv::LAST_YAW, yaw);
    let (cy, sy) = c::cs(yaw);
    let mut mv = [cy * v, sy * v, c::pf(w, id, pv::VZ) - c::DT2 * 10.0, 0.0];
    walker::move_collide(w, id, 0.5, 0.5, 0.0, &mut mv, 0x10);
    let dz = c::pos(w, id)[2] - z0;
    c::set_pf(w, id, pv::VZ, dz.min(c::DT * 4.0));
    if !wrapped(w, id) { return 37.0; }
    d
}

/// State 0 (`0x2d4610` case 0). False: deleted.
fn init(w: &mut World, id: MobyId) -> bool {
    let o = w.m(id).o_class;
    w.mm(id).scale = crate::moby_update::classes::units::class_scale(w, o) * k::SCALE;
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOME, p);
    if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 0); }
    c::set_pu8(w, id, pv::D + 9, 1);
    let r = pv::WANDER;
    c::set_pv4(w, id, r + walker::wr::HOME, p);
    c::set_pf(w, id, r + walker::wr::STEP, c::DT + c::DT);
    c::set_pf(w, id, r + walker::wr::TURN, c::DT * 2.967_059_6);
    c::set_pf(w, id, r + walker::wr::LEASH, 1.5);
    if c::dist2(p, [k::TIGHT_AT[0], k::TIGHT_AT[1], 0.0, 0.0]) < 5.0 { c::set_pf(w, id, r + walker::wr::LEASH, 0.75); }
    c::set_pu8(w, id, 0x58, 12);
    c::set_pu8(w, id, 0x5a, 2);
    if c::pi32(w, id, pv::ARRIVE) < 0 {
        set_state(w, id, st::WANDER);
    } else if c::pi32(w, id, pv::ARRIVE_AREA) < 0 {
        if c::pi32(w, id, pv::LOOK) < 0 {
            w.delete_moby(id);
            return false;
        }
        c::set_pv4(w, id, pv::HOME, p);
        set_state(w, id, st::FACE);
        blend(w, id, 2, 10);
    } else if let Some(a) = path(w, id, pv::ARRIVE) {
        let n = w.svc.splines[a].len();
        let last = path_point(w, a, n.saturating_sub(1));
        c::set_pv4(w, id, pv::HOME, last);
        let first = path_point(w, a, 0);
        c::set_pos(w, id, first);
        set_state(w, id, st::ARRIVE_WAIT);
    }
    true
}

fn fast_turn(w: &mut World, id: MobyId, to: c::V) {
    let p = c::pos(w, id);
    let r = 4.0 * std::f32::consts::PI;
    turn::turn_toward_pvar(w, id, c::atan(to[0] - p[0], to[1] - p[1]), c::DT2 * r, c::DT2 * r, c::DT * r, pv::TURN_V);
}

/// `0x2d4610`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    if state(w, id) != st::INIT { pre(w, id); }
    let tm = target_moby(w, id);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 26.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    // 0x263ac8(2.1, moby, 1, +0x200): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.1, id, 1, id, 0x200);
    let tpos = |w: &World| tm.map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
    match state(w, id) {
        st::INIT => {
            if !init(w, id) { return; }
        }
        st::FACE => match moby_at(w, id, pv::LOOK) {
            Some(m) => {
                let q = c::pos(w, m);
                fast_turn(w, id, q);
            }
            None => {
                set_state(w, id, st::HOME);
                blend(w, id, 4, 10);
            }
        },
        st::ARRIVE_WALK => {
            let pt = path(w, id, pv::ARRIVE).map(|a| path_point(w, a, c::pi16(w, id, pv::NODE).max(0) as usize)).unwrap_or([0.0; 4]);
            if walk(w, id, pt) < 1.0 {
                set_state(w, id, st::FACE);
                blend(w, id, 2, 10);
            }
        }
        st::ARRIVE_WAIT => {
            if kind(w, id) != 2 {
                set_state(w, id, st::ARRIVE);
                blend(w, id, 4, 10);
            }
        }
        st::ARRIVE => {
            let a = path(w, id, pv::ARRIVE);
            let n = a.map_or(0, |a| w.svc.splines[a].len()) as i32;
            let node = c::pi16(w, id, pv::NODE);
            let pt = a.map(|a| path_point(w, a, node.max(0) as usize)).unwrap_or([0.0; 4]);
            if walk(w, id, pt) < 1.0 {
                if n - 1 <= node as i32 {
                    set_state(w, id, st::WANDER);
                    blend(w, id, 1, 10);
                } else {
                    c::set_pi16(w, id, pv::NODE, node + 1);
                }
            } else if kind(w, id) != 2 {
                set_state(w, id, st::GO);
                blend(w, id, 4, 10);
            }
        }
        st::WANDER => {
            if w.m(id).anim.seq_b != 4 {
                let f = w.rng.rand_range(0, 5);
                let t = w.ticks(10);
                w.anim_blend(id, 4, f, t);
                let s = w.rng.randf(f32::from_bits(0x3f6b_851f), f32::from_bits(0x3f8a_3d71));
                w.mm(id).anim.speed = s;
            }
            walker::wander(w, id, 0.5, 0.5, pv::WANDER);
            if kind(w, id) != 2 {
                let o = w.rng.randf(-20.0, 20.0);
                c::set_pf(w, id, pv::SIDE, o * 0.017_453_292);
                set_state(w, id, st::GO);
                blend(w, id, 4, 10);
            }
        }
        st::GO => {
            let t = c::pv4(w, id, pv::TGT);
            let d = walk(w, id, t);
            let mut stay = false;
            if d < k::BITE_RANGE {
                let (p, q) = (c::pos(w, id), tpos(w));
                let a = c::atan(q[0] - p[0], q[1] - p[1]);
                if c::diff_rots(c::yaw(w, id), a) < k::FACING {
                    set_state(w, id, st::BITE);
                    if w.m(id).anim.seq_b != 5 {
                        let t = w.ticks(10);
                        w.anim_blend(id, 5, k::BITE_FRAME, t);
                    }
                    stay = true;
                }
            }
            if !stay && kind(w, id) == 2 {
                set_state(w, id, st::HOME);
                blend(w, id, 4, 10);
            }
        }
        st::BITE => {
            if ground::passed_frame(w, id, k::BITE_KEY) {
                attack::joint_hit(w, 0.333, 1.0, id, 0, 1, 0, 1, 0);
            } else if wrapped(w, id) {
                if kind(w, id) == 2 {
                    set_state(w, id, st::HOME);
                    blend(w, id, 4, 10);
                } else {
                    let (p, q) = (c::pos(w, id), tpos(w));
                    let near = c::dist2(p, q) <= 1.5 && c::diff_rots(c::yaw(w, id), c::atan(q[0] - p[0], q[1] - p[1])) <= k::FACING;
                    if !near {
                        set_state(w, id, st::GO);
                        blend(w, id, 4, 10);
                    }
                }
            }
        }
        st::HOME => {
            let h = c::pv4(w, id, pv::HOME);
            if walk(w, id, h) < 1.5 {
                set_state(w, id, st::WANDER);
                blend(w, id, 1, 10);
            } else if kind(w, id) != 2 {
                set_state(w, id, st::GO);
                blend(w, id, 4, 10);
            }
        }
        st::HELD => {
            if react::carried(w, id, pv::K) != 0 {
                set_state(w, id, st::WANDER);
                c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
            }
        }
        st::CHASE => chase(w, id, tm),
        st::KNOCKED => {
            if knock::update(w, id, pv::K) & 0x140 != 0 {
                set_state(w, id, st::WANDER);
                blend(w, id, 1, 10);
            }
        }
        st::DYING if knock::update(w, id, pv::K) & 0x140 != 0 => {
            w.play_sound(7, 0, id);
            let p = c::pos(w, id);
            fx::piece_explosion(w, 0.75, 10.0, Some(id), p);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    after(w, id);
}

/// State 10 (`0x2d4610` case 10): the chase.
fn chase(w: &mut World, id: MobyId, tm: Option<MobyId>) {
    let p = c::pos(w, id);
    let t = c::pv4(w, id, pv::TGT);
    let mut v = c::sub(t, p);
    v[2] = 0.0;
    let q = c::add(c::set_len3(v, 2.5), p);
    let crossed = path(w, id, pv::AREA).is_some_and(|a| c::region::crosses(w, a, p, q));
    if !crossed {
        blend(w, id, 4, 20);
        walk(w, id, t);
    } else {
        let tp = tm.map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
        fast_turn(w, id, tp);
        if !wrapped(w, id) { return; }
        let s = w.m(id).anim.seq_b;
        let k1 = w.rng.randi(2);
        if s as i32 != k1 + 2 {
            let k2 = w.rng.randi(2);
            let t = w.ticks(20);
            w.anim_blend(id, (k2 + 2) as u8, 0, t);
        }
    }
    if wrapped(w, id) {
        if kind(w, id) == 2 {
            if c::pi16(w, id, pv::ALERT_T) == 0 {
                set_state(w, id, st::HOME);
                blend(w, id, 4, 20);
            }
        } else {
            set_state(w, id, st::GO);
            blend(w, id, 4, 10);
        }
    }
}

/// The tail of `0x2d4610` (module doc): the ground ease, the wall push, the floor switch.
fn after(w: &mut World, id: MobyId) {
    let s = state(w, id);
    if s != st::KNOCKED && s != st::DYING && s != st::HELD {
        let mut p = c::pos(w, id);
        let g = ground::ground(w, p, 0.5, 0).z;
        turn::approach(g, c::DT * 7.0, &mut p[2]);
        if state(w, id) != st::ARRIVE {
            if let Some(a) = path(w, id, pv::AREA) {
                if let Some(q) = crate::path::push_from_walls(&w.svc.splines[a], 0.25, p) {
                    p = [q[0], q[1], q[2], p[3]];
                }
            }
        }
        c::set_pos(w, id, p);
    }
    let Some(sw) = usize::try_from(c::pi32(w, id, pv::SWITCH)).ok().filter(|&i| i < w.table.mobys.len()) else { return };
    if c::dist2(c::pos(w, id), c::pos(w, sw)) < 1.0 && w.m(sw).o_class == 0x33e {
        floor_switch::press(w, sw);
        c::set_pi32(w, id, pv::SWITCH, -1);
    }
}
