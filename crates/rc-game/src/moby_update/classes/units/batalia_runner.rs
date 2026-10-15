//! U280 (census 2026-09-30; U277 in older runs): class 452, Batalia's runners (level08 `0x2e2df0`, the only copy; three
//! instances, #341–#343). The name is descriptive [L]. A path creature that waits until Ratchet comes within 24 of its
//! trigger path's first point (+0x220), then runs its path (+0xe4) at 10 u/s and is deleted at its end, or (with a
//! second path, +0x114: #343, path 52) keeps running that path away from him: it stops and turns to him once he is 19
//! away, and runs on (reversing when he is nearer its next node) once he is within 17 or a Devastator missile locks it.
//! Any hit (mask 0x210000) knocks it into a flight (no damage resolver): `SetDeathBits` (its bolts), the flash value
//! 0x78, and the death explosion when the flight lands (deleted below 5 without one).
//!
//! It also keeps a list of up to 50 "members" (+0x158), filled from its ten moby links (+0x130) with the mobys of
//! class 633 (`0x2e2da0`): it collects them (states 2–4: run to a path node whose w is 0, pull them to its joint 0),
//! sends the ones it passes flying (the bolts' fly-off `0x2bcb90` toward it) and releases the held ones when hit. **On
//! the disc this never happens:** its links name class-13 bolts and no level has a class 633 (none in any `lvl.vtbl`),
//! so the list stays empty, and nothing sets its state 2. The code is ported as the game has it.
//!
//! **Pvars** (0x2a0): +0x20 damage record (+0x29 byte), +0x40 zeroed, +0x60 the flash, +0x70 the knockback record,
//! +0xd0 the node, +0xd4 its step (s8), +0xe0 the path (the game: its pointer; the port: the index), +0xe4 the path id,
//! +0x100 / +0x104 / +0x110 / +0x114 the same for the second path, +0x130 ten moby links, +0x158 fifty members (the
//! game: pointers; the port: index + 1, 0 none), +0x220 the trigger path id, +0x224 the turn velocity, +0x22c members
//! taken, +0x260 the big-head cheat's record.
//!
//! The level08 `$gp` word: gp−0x4fb0 10 (the run speed ×dt).
//!
//! ## Coverage
//!
//! **The update** `0x2e2df0`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x26dae0(2.5, m, 1, +0x260)` (= `0x278720`) | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn and within 29 of the camera (0x1675c0): `0x264650` (= `0x26f020`), +0x7f = 0x17 | the shadow probe | [`update`] (`shadows::probe_down`) |
//! | `MobyGetHitMessage(m, 0x210000, 0)` (0x264950) outside 8: held members (3) → 1; K gravity 0.008, flags 1, up 10·dt, drag 0.0005, speed 8·dt, +0x3d 0; the flight away from Ratchet (`0x266a48` = `0x271418`, seq 4, 1 tick); flash value 0x78; `SetDeathBits(m, 0, −1)` (0x261970); 8 | the hit | [`update`] (`knock::start`, `crate_::set_death_bits`) |
//! | +0xa4 = 0xff | | [`update`] |
//! | 0: at the path's first point, node 1; a second path: node 1; mode \|= 0x1000; +0x29 0; Ratchet's moby's light and ambient; +0x40 zeroed; 1, seq 1 (0 ticks) | init | [`update`] |
//! | 1: Ratchet within 24 (3-D) of the trigger path's first point → the members (`0x2e2da0`), 5, seq 3 (6 ticks) | wait | [`update`] |
//! | 2: next = (node + count + step) % count; `SpringTurn2` (0x262778: 0.01, 0.3, 0.1, +0x224) to it; its w ≠ 0: [`run_to`]; else 3, seq 2 (6 ticks) | to the collecting node | [`update`] |
//! | 3: anim speed 3; done → the members (`0x2e2da0`), every live one → state 3, 4, seq 6 (6 ticks), anim speed 1 | | [`update`] |
//! | 4: every live member pulled to joint 0 (`0x259cc8` = `0x2645a8`) by at most 4·dt (`0x269c20` = `0x2745f0`); within 2·dt: its state 4, slot freed, +0x22c + 1; none left → 5, seq 3 | collect | [`update`] |
//! | 5: the turn; every live member within 6 (xy): seq 6 (`ticks(20)`), `0x2afcf0(0, 0, member, m)` (= `0x2bcb90`), slot freed; no live member → seq 3 (`ticks(20)`); next is the last node: no second path → `DeleteMoby`, else 6; else [`run_to`] | run | [`update`] (`bolt::start_fly_to`) |
//! | 6: the second path's next node: the turn, [`run_to`]; Ratchet farther than 19 (3-D) → 7 | run away | [`update`] |
//! | 7: turn to Ratchet; within 17 or a Devastator missile locked on it (`0x269df8` = `0x274738`) → 6, the step reversed when Ratchet is nearer the next node than it | watch | [`update`] (`devastator::already_targeted`) |
//! | 8: `0x266b88` (= `0x271558`); landed or touched (& 3): `0x269580(1, 13, m, pos, −1)` (= `0x273f50`), `DeleteMoby`; else below 5 → `DeleteMoby` | the flight | [`update`] (`fx::death_explosion`) |
//! | `0x267a28` (= `0x2723f8`) the flash update | | [`update`] |
//!
//! [`run_to`]: d = node − pos set to 10·dt (`FastVecNormalize`), pos += d; within 0.5 (3-D) → the node is reached.
//! [`refill`] (`0x2e2da0`): the links' mobys of class 633 written into the member slots from the first.
//!
//! **Not the game's, noted [L]:** a link of −1 reads the moby slot before the table in the game (the port skips it); a
//! path of 0 points traps (`break 7`) in the game (the port does nothing). Native `f32`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, flash, fx, knock, turn};
use crate::moby_update::services::World;
use crate::ps2v::Pf;

/// The update in the level08 class table.
pub const UPDATE_FN: u32 = 0x2e_2df0;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 1] = [452];
/// Joint 0 (the collecting point) is read.
pub const JOINTS: [i16; 1] = [452];
/// The members' class (0x279; on no level).
pub const MEMBER: i16 = 633;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const ZERO: usize = 0x40;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const NODE: usize = 0xd0;
    pub const STEP: usize = 0xd4;
    pub const PATH: usize = 0xe0;
    pub const PATH_ID: usize = 0xe4;
    pub const NODE2: usize = 0x100;
    pub const STEP2: usize = 0x104;
    pub const PATH2: usize = 0x110;
    pub const PATH2_ID: usize = 0x114;
    pub const LINKS: usize = 0x130;
    pub const MEMBERS: usize = 0x158;
    pub const TRIGGER: usize = 0x220;
    pub const TURN_V: usize = 0x224;
    pub const TAKEN: usize = 0x22c;
    pub const SIZE: usize = 0x2a0;
    /// 10 links, 50 member slots.
    pub const N_LINKS: usize = 10;
    pub const N_MEMBERS: usize = 50;
}

/// The level08 `$gp` word and the update's literals (module doc).
pub mod k {
    pub const SPEED: f32 = 10.0;
    pub const WAKE: f32 = 24.0;
    pub const AWAY: f32 = 19.0;
    pub const NEAR: f32 = 17.0;
    pub const EAT: f32 = 6.0;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
/// `if (m+0x53 != seq) fun_00212f90(m, seq, 0, t)` with a raw tick count.
fn blend_raw(w: &mut World, id: MobyId, seq: u8, t: i32) { c::blend_to(w, id, seq, 0, t); }
fn spline(w: &World, o: i32) -> Option<usize> { usize::try_from(o).ok().filter(|&p| p < w.svc.splines.len()) }
fn point(w: &World, p: Option<usize>, i: i32) -> c::V {
    p.zip(usize::try_from(i).ok()).and_then(|(p, i)| w.svc.splines[p].get(i)).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4])
}
fn hero(w: &World) -> c::V { crate::moby_update::classes::units::hero_pos(w) }

/// The live member in slot `i` (state < 0x80).
fn member(w: &World, id: MobyId, i: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, pv::MEMBERS + 4 * i);
    usize::try_from(v - 1).ok().filter(|&m| v > 0 && m < w.table.mobys.len())
}
fn free(w: &mut World, id: MobyId, i: usize) { c::set_pi32(w, id, pv::MEMBERS + 4 * i, 0); }

/// `0x2e2da0(m)`: the links' mobys of class 633 into the member slots, from the first.
pub fn refill(w: &mut World, id: MobyId) {
    let mut slot = 0;
    for i in 0..pv::N_LINKS {
        let Some(m) = usize::try_from(c::pi32(w, id, pv::LINKS + 4 * i)).ok().filter(|&m| m < w.table.mobys.len()) else { continue };
        if w.m(m).o_class == MEMBER {
            c::set_pi32(w, id, pv::MEMBERS + 4 * slot, m as i32 + 1);
            slot += 1;
        }
    }
}

/// The next node index `(node + count + step) % count` of the path at `path_o` (None: no points).
fn next(w: &World, id: MobyId, path_o: usize, node_o: usize, step_o: usize) -> Option<(Option<usize>, i32, i32)> {
    let p = spline(w, c::pi32(w, id, path_o));
    let n = p.map_or(0, |p| w.svc.splines[p].len() as i32);
    if n == 0 { return None; }
    let step = c::pu8(w, id, step_o) as i8 as i32;
    Some((p, n, (c::pi32(w, id, node_o) + n + step).rem_euclid(n)))
}

/// `SpringTurn2(heading, 0.01, 0.3, 0.1, m, +0x224)` (0x262778).
fn turn_to(w: &mut World, id: MobyId, q: c::V) {
    let p = c::pos(w, id);
    turn::spring_turn2_pvar(w, id, c::atan(q[0] - p[0], q[1] - p[1]), 0.01, 0.3, 0.1, pv::TURN_V);
}

/// One run step toward `q` (module doc). True when within 0.5.
fn run_to(w: &mut World, id: MobyId, q: c::V) -> bool {
    let d = c::set_len3(c::sub(q, c::pos(w, id)), k::SPEED * c::DT);
    let p = c::add(c::pos(w, id), d);
    c::set_pos(w, id, p);
    c::dist3(p, q) < 0.5
}

/// To 5 with seq 3 (6 ticks).
fn to_run(w: &mut World, id: MobyId) {
    set_state(w, id, 5);
    blend_raw(w, id, 3, 6);
}

/// `0x2e2df0`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    // `0x26dae0(2.5, m, 1, +0x260)`: the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 1, id, 0x260);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 29.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x17;
        }
    }
    if w.get_hit(id, 0x21_0000, false).is_some() && state(w, id) != 8 {
        for i in 0..pv::N_MEMBERS {
            if let Some(m) = member(w, id, i) {
                if w.m(m).state == 3 { w.mm(m).state = 1; }
            }
        }
        let kr = pv::K;
        c::set_pf(w, id, kr + knock::k::GRAVITY, f32::from_bits(0x3c03_126f));
        c::set_pi32(w, id, kr + knock::k::FLAGS, 1);
        c::set_pf(w, id, kr + knock::k::UP, c::DT * 10.0);
        c::set_pf(w, id, kr + knock::k::DRAG, f32::from_bits(0x3a03_126f));
        c::set_pf(w, id, kr + knock::k::SPEED, c::DT * 8.0);
        c::set_pu8(w, id, kr + 0x3d, 0);
        let (p, h) = (c::pos(w, id), hero(w));
        let a = c::atan(p[0] - h[0], p[1] - h[1]);
        knock::start(w, id, kr, a, 4, 1, 0);
        c::set_pu8(w, id, pv::F + 7, 0x78);
        set_death_bits(w, id, 0, -1);
        set_state(w, id, 8);
    }
    w.mm(id).hit_slot = 0xff;
    match state(w, id) {
        0 => {
            let pid = c::pi32(w, id, pv::PATH_ID);
            c::set_pi32(w, id, pv::PATH, pid);
            let p0 = point(w, spline(w, pid), 0);
            c::set_pos(w, id, p0);
            c::set_pi32(w, id, pv::NODE, 1);
            let p2 = c::pi32(w, id, pv::PATH2_ID);
            if 0 <= p2 {
                c::set_pi32(w, id, pv::NODE2, 1);
                c::set_pi32(w, id, pv::PATH2, p2);
            }
            w.mm(id).mode |= mode::TARGETABLE;
            c::set_pu8(w, id, pv::D + 9, 0);
            crate::moby_update::classes::units::take_hero_light(w, id);
            c::set_pv4(w, id, pv::ZERO, [0.0; 4]);
            set_state(w, id, 1);
            blend_raw(w, id, 1, 0);
        }
        1 => {
            let q = point(w, spline(w, c::pi32(w, id, pv::TRIGGER)), 0);
            if c::dist3(hero(w), q) < k::WAKE {
                refill(w, id);
                to_run(w, id);
            }
        }
        2 => {
            let Some((p, _, nx)) = next(w, id, pv::PATH, pv::NODE, pv::STEP) else { return };
            let q = point(w, p, nx);
            turn_to(w, id, q);
            if q[3] != 0.0 {
                if run_to(w, id, q) { c::set_pi32(w, id, pv::NODE, nx); }
            } else {
                set_state(w, id, 3);
                blend_raw(w, id, 2, 6);
            }
        }
        3 => {
            w.mm(id).anim.speed = 3.0;
            if w.m(id).anim.flags & 2 != 0 {
                refill(w, id);
                for i in 0..pv::N_MEMBERS {
                    if let Some(m) = member(w, id, i).filter(|&m| w.m(m).state < 0x80) { w.mm(m).state = 3; }
                }
                set_state(w, id, 4);
                blend_raw(w, id, 6, 6);
                w.mm(id).anim.speed = 1.0;
            }
        }
        4 => {
            let mut n = 0;
            for i in 0..pv::N_MEMBERS {
                let Some(m) = member(w, id, i).filter(|&m| w.m(m).state < 0x80) else { continue };
                let jp = w.joint_point(id, 0);
                n += 1;
                let d = c::clamp_len3(c::sub(jp, c::pos(w, m)), c::DT * 4.0);
                let q = c::add(c::pos(w, m), d);
                c::set_pos(w, m, q);
                if c::len3(d) < c::DT + c::DT {
                    w.mm(m).state = 4;
                    free(w, id, i);
                    let t = c::pi32(w, id, pv::TAKEN) + 1;
                    c::set_pi32(w, id, pv::TAKEN, t);
                }
            }
            if n == 0 { to_run(w, id); }
        }
        5 => {
            let Some((p, n, nx)) = next(w, id, pv::PATH, pv::NODE, pv::STEP) else { return };
            let q = point(w, p, nx);
            turn_to(w, id, q);
            let mut any = false;
            for i in 0..pv::N_MEMBERS {
                let Some(m) = member(w, id, i).filter(|&m| w.m(m).state < 0x80) else { continue };
                any = true;
                if c::dist2(c::pos(w, id), c::pos(w, m)) < k::EAT {
                    let t = w.ticks(0x14);
                    c::blend_to(w, id, 6, 0, t);
                    crate::moby_update::classes::bolt::start_fly_to(w, m, Pf::ZERO, Pf::ZERO, Some(id));
                    free(w, id, i);
                }
            }
            if !any {
                let t = w.ticks(0x14);
                c::blend_to(w, id, 3, 0, t);
            }
            if n - 1 == nx {
                if c::pi32(w, id, pv::PATH2_ID) < 0 {
                    w.delete_moby(id);
                    return;
                }
                set_state(w, id, 6);
            } else if run_to(w, id, q) {
                c::set_pi32(w, id, pv::NODE, nx);
            }
        }
        6 => {
            let Some((p, _, nx)) = next(w, id, pv::PATH2, pv::NODE2, pv::STEP2) else { return };
            let q = point(w, p, nx);
            turn_to(w, id, q);
            if run_to(w, id, q) { c::set_pi32(w, id, pv::NODE2, nx); }
            if k::AWAY < c::dist3(hero(w), c::pos(w, id)) { set_state(w, id, 7); }
        }
        7 => {
            let h = hero(w);
            turn_to(w, id, h);
            let d = c::dist3(h, c::pos(w, id));
            if d < k::NEAR || crate::hero::devastator::already_targeted(w.table, id) {
                let Some((p, _, nx)) = next(w, id, pv::PATH2, pv::NODE2, pv::STEP2) else { return };
                set_state(w, id, 6);
                if c::dist3(h, point(w, p, nx)) < d {
                    let s = (c::pu8(w, id, pv::STEP2) as i8).wrapping_neg();
                    c::set_pu8(w, id, pv::STEP2, s as u8);
                }
            }
        }
        8 => {
            let r = knock::update(w, id, pv::K);
            if r & 3 == 0 {
                if 5.0 <= c::pos(w, id)[2] {
                    flash::update(w, id, pv::F);
                    return;
                }
            } else {
                let p = c::pos(w, id);
                fx::death_explosion(w, 1.0, 13.0, Some(id), p, -1);
            }
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    flash::update(w, id, pv::F);
}
