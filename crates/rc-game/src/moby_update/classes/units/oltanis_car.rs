//! **Oltanis's flying cars, class 1417** (level14 `0x306ee0`; census U520, 8 placed). Cars that fly a path (+0x78,
//! a loop with +0x8e set) at +0xc8 a second carrying a Swingshot target (the moby +0xa0, held at +0xa4 / +0xa8 /
//! +0xac along their rows) and sink while Ratchet hangs from it. Read from the level14 decomp; native `f32`.
//!
//! * 0 (the path set; else deleted): its chords (as a loop), at its start facing along it, update and draw distance
//!   0xff (the target too), its joint list 0 under a manipulator (+0xd0), the flash started; with a wake cuboid
//!   (+0xc4) hidden with its target until Ratchet enters it (1), else flying (2).
//! * 2: joint list 0 shrunk to nothing (scale 0.0001), the wobble (`0x277a80(0.15, 0.05, 0.025)`); along the path
//!   (`0x307620`): the end of an open path, or below z 25 → the explosion (`0x273f50(5, 13)`), hidden, the target
//!   hidden at (5, 5, 5), → 3. Ratchet pulling (0x25 / 0x26) toward its target or swinging (0x0e) on it: the sink
//!   speed += +0xb8 (at most 20·dt) and the sink grows by it; else the sink eases back 5 a tick to 0. The path point
//!   lowered by the sink (not below 10), springs (0.01 / 0.2) on its position, yaw, pitch and roll (50° × the turn
//!   over 1.45°). The target follows; Ratchet hanging from it (group 0xd pulling, 0xe swinging): the Swingshot
//!   camera looks along the car (`0x3135b8`) and the hold `ticks(30)`; else, the hold run out, the target's reach
//!   (+0x20) 40 with its spring (0.01, 0.2, 24·dt) for a swing target (803), else 0; while it runs 2.
//! * 3 (shot or crashed): `ticks(500)` later back at the path's start (its colour back, shown, the target with it;
//!   with a wake cuboid → 1, hidden).
//! * A 0x800000 hit (not in 3): the crash. Its hum (sound 0, flags 4) in state 2.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x306ee0` | 1417 | [`update`] |
//! | `0x307510` / `0x307620` | path setup, flight | [`setup`], [`fly`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, flash, fx, turn, DT};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::{manip, story};
use crate::ps2v::Pf;
use crate::spline::{advance, Cursor};

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x30_6ee0;
pub const CLASSES: [i16; 1] = [1417];

mod o {
    pub const F: usize = 0x60;
    pub const SEG: usize = 0x70;
    pub const T: usize = 0x74;
    pub const PATH: usize = 0x78;
    pub const VOICE: usize = 0x7c;
    pub const VEL: usize = 0x80;
    pub const TIMER: usize = 0x8c;
    pub const CLOSED: usize = 0x8e;
    pub const ROLL_V: usize = 0x90;
    pub const PITCH_V: usize = 0x94;
    pub const YAW_V: usize = 0x98;
    pub const TARGET: usize = 0xa0;
    pub const OFFSET: usize = 0xa4;
    pub const SINK: usize = 0xb0;
    pub const SINK_V: usize = 0xb4;
    pub const SINK_ACC: usize = 0xb8;
    pub const WOB: usize = 0xbc;
    pub const CUBOID: usize = 0xc4;
    pub const SPEED: usize = 0xc8;
    pub const HOLD: usize = 0xcc;
    pub const NODE: usize = 0xd0;
    pub const SIZE: usize = 0x110;
}
/// Level14 gp words (0x16226c..0x162298).
const K: f32 = 0.01;
const D: f32 = 0.2;
const RESPAWN: i32 = 500;
const FLOOR: f32 = 25.0;

fn link(w: &World, i: i32) -> Option<MobyId> { usize::try_from(i).ok().filter(|&m| m < w.table.mobys.len()) }
fn target(w: &World, id: MobyId) -> Option<MobyId> { link(w, c::pi32(w, id, o::TARGET)) }
fn points(w: &World, id: MobyId) -> Option<(usize, Vec<[f32; 4]>)> {
    let i = usize::try_from(c::pi32(w, id, o::PATH)).ok()?;
    let s = w.svc.splines.get(i).filter(|s| !s.is_empty())?;
    Some((i, s.iter().map(|q| q.map(f32::from_bits)).collect()))
}
fn set_shown(w: &mut World, m: MobyId, shown: bool, coll: bool) {
    let cl = super::class_collision(w, w.m(m).o_class);
    let mm = w.mm(m);
    if shown {
        mm.mode &= 0xffbe;
        if coll { mm.has_collision = cl; }
    } else {
        mm.mode |= 0x41;
        if coll { mm.has_collision = false; }
    }
}
fn release(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, o::VOICE);
    if v != -1 && w.sound_owner(v) == Some(id) { w.release_sound(v, id); }
    c::set_pi32(w, id, o::VOICE, -1);
}
/// The target at the car's offsets along its rows.
fn carry(w: &mut World, id: MobyId, t: MobyId) {
    let (pos, r) = (c::pos(w, id), w.m(id).rows);
    let at: [f32; 4] = std::array::from_fn(|k| if k == 3 { pos[3] } else { pos[k] + (0..3).map(|j| r[j][k] * c::pf(w, id, o::OFFSET + 4 * j)).sum::<f32>() });
    w.mm(t).position = at;
}
/// The crash (module doc).
fn crash(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    fx::death_explosion(w, 5.0, 13.0, Some(id), p, -1);
    w.mm(id).state = 3;
    set_shown(w, id, false, true);
    if let Some(t) = target(w, id) {
        let m = w.mm(t);
        m.position = [5.0, 5.0, 5.0, m.position[3]];
        m.mode |= 0x41;
    }
    release(w, id);
}

/// `0x307510`: the chords as a loop, the cursor and springs 0, at the path's start facing along it.
pub fn setup(w: &mut World, id: MobyId) {
    let Some((i, pts)) = points(w, id) else { return };
    let n = pts.len();
    for k in 0..n {
        w.svc.splines[i][k][3] = c::dist3(pts[k], pts[(k + 1) % n]).to_bits();
    }
    restart(w, id, &pts);
}
fn restart(w: &mut World, id: MobyId, pts: &[[f32; 4]]) {
    c::set_pi32(w, id, o::SEG, 0);
    c::set_pf(w, id, o::T, 0.0);
    let (p0, p1) = (pts[0], pts.get(1).copied().unwrap_or(pts[0]));
    let m = w.mm(id);
    m.position = p0;
    m.rotation[2] = c::atan(p1[0] - p0[0], p1[1] - p0[1]);
    for k in [o::ROLL_V, o::VEL + 8, o::VEL + 4, o::VEL, o::YAW_V, o::PITCH_V] { c::set_pf(w, id, k, 0.0); }
}

/// Ratchet pulling (0x25 / 0x26) toward `t` or swinging (group 0xe) on it.
fn hanging(w: &World, t: MobyId) -> bool {
    let h = w.hero;
    ((0x25..=0x26).contains(&h.state) && h.swing.pull == Some(t)) || (h.group == 0xe && h.swing.on == Some(t))
}

/// `0x307620`: along the path (module doc); false when it crashed.
pub fn fly(w: &mut World, id: MobyId) -> bool {
    let Some((_, pts)) = points(w, id) else { return false };
    let closed = c::pi16(w, id, o::CLOSED) != 0;
    let mut cur = Cursor { seg: c::pi32(w, id, o::SEG), t: c::pf(w, id, o::T) };
    let (q, ended) = advance(&pts, closed, c::pf(w, id, o::SPEED) * DT, &mut cur);
    c::set_pi32(w, id, o::SEG, cur.seg);
    c::set_pf(w, id, o::T, cur.t);
    if (!closed && ended) || w.m(id).position[2] <= FLOOR {
        crash(w, id);
        return false;
    }
    if target(w, id).is_some_and(|t| hanging(w, t)) {
        let v = (c::pf(w, id, o::SINK_V) + c::pf(w, id, o::SINK_ACC)).min(20.0 * DT);
        c::set_pf(w, id, o::SINK_V, v);
        c::set_pf(w, id, o::SINK, c::pf(w, id, o::SINK) + v);
    } else {
        c::set_pf(w, id, o::SINK_V, 0.0);
        let s = (c::pf(w, id, o::SINK) - 5.0 * DT).max(0.0);
        c::set_pf(w, id, o::SINK, s);
    }
    let q = [q[0], q[1], (q[2] - c::pf(w, id, o::SINK)).max(10.0)];
    for (a, &t) in q.iter().enumerate() {
        let (mut x, mut v) = (Pf::f(w.m(id).position[a]), Pf::f(c::pf(w, id, o::VEL + 4 * a)));
        crate::hero::physics::spring(Pf::f(t), Pf::f(K), Pf::f(D), Pf::ZERO, &mut x, &mut v);
        w.mm(id).position[a] = x.to_f32();
        c::set_pf(w, id, o::VEL + 4 * a, v.to_f32());
    }
    let pos = c::pos(w, id);
    let e = [q[0] - pos[0], q[1] - pos[1], q[2] - pos[2], 0.0];
    if e[0].abs() <= 0.01 || e[1].abs() <= 0.01 { return true; }
    let yaw = c::atan(e[0], e[1]);
    let turned = c::sub_rot(w.m(id).rotation[2], yaw);
    turn::spring_turn2_pvar(w, id, yaw, K, D, 0.0, o::YAW_V);
    let pitch = -c::atan(c::len2(e), e[2]);
    let roll = (turned / f32::from_bits(0x3ccf_5134)).clamp(-1.0, 1.0) * f32::from_bits(0x3f5f_66f3);
    for (axis, t, kk, vo) in [(1, pitch, K, o::PITCH_V), (0, roll, f32::from_bits(0x3b44_9ba6), o::ROLL_V)] {
        let (mut x, mut v) = (Pf::f(w.m(id).rotation[axis]), Pf::f(c::pf(w, id, vo)));
        crate::hero::physics::spring(Pf::f(t), Pf::f(kk), Pf::f(D), Pf::ZERO, &mut x, &mut v);
        w.mm(id).rotation[axis] = x.to_f32();
        c::set_pf(w, id, vo, v.to_f32());
    }
    true
}

/// Level14 `0x306ee0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let hit = w.get_hit(id, 0x80_0000, false).is_some();
    w.mm(id).hit_slot = 0xff;
    if hit && w.m(id).state != 3 { crash(w, id); }
    let st = w.m(id).state;
    match st {
        0 => {
            if points(w, id).is_none() {
                w.delete_moby(id);
                return;
            }
            setup(w, id);
            let m = w.mm(id);
            m.state = 2;
            m.update_dist = 0xff;
            m.draw_dist = 0xff;
            c::set_pi32(w, id, o::VOICE, -1);
            let t = w.ticks(RESPAWN);
            c::set_pi16(w, id, o::TIMER, t as i16);
            for k in [o::SINK, o::SINK_V, o::WOB, o::WOB + 4, o::HOLD] { c::set_pi32(w, id, k, 0); }
            if let Some(t) = target(w, id) {
                let (dd, ud) = (w.m(id).draw_dist, w.m(id).update_dist);
                let m = w.mm(t);
                m.draw_dist = dd;
                m.update_dist = ud;
            }
            w.mm(id).pvars[o::NODE..o::NODE + 0x40].fill(0);
            manip::attach(w, id, 0, id, o::NODE);
            flash::start(w, id, o::F);
            if c::pi32(w, id, o::CUBOID) >= 0 {
                set_shown(w, id, false, true);
                if let Some(t) = target(w, id) { set_shown(w, t, false, false); }
                w.mm(id).state = 1;
            }
        }
        1 => {
            let h = w.hero_point();
            if w.in_cuboid(h, c::pi32(w, id, o::CUBOID)) {
                set_shown(w, id, true, true);
                if let Some(t) = target(w, id) { set_shown(w, t, true, false); }
                w.mm(id).state = 2;
            }
        }
        2 => {
            {
                let pv = &mut w.mm(id).pvars;
                pv[o::NODE + 2] = 1;
                for k in 0..3 { p::set_ff(pv, o::NODE + manip::rec::SCALE + 4 * k, f32::from_bits(0x38d1_b717)); }
            }
            manip::sync(w, id, id, o::NODE);
            // 0x277a80(0.15, 0.05, 0.025, m, +0xbc, +0xc0): the wobble.
            let (a, b) = (c::pf(w, id, o::WOB), c::pf(w, id, o::WOB + 4));
            let m = w.mm(id);
            m.rotation[0] = 0.15 * a.sin() * b.sin();
            m.rotation[1] = 0.15 * a.sin() * b.cos();
            c::set_pf(w, id, o::WOB, c::add_rot(a, 0.05));
            c::set_pf(w, id, o::WOB + 4, c::add_rot(b, 0.025));
            if fly(w, id) {
                if let Some(t) = target(w, id) {
                    carry(w, id, t);
                    let h = w.hero;
                    if (h.group == 0xd && h.swing.pull == Some(t)) || (h.group == 0xe && h.swing.on == Some(t)) {
                        crate::cinematic::swing_follow(w, id);
                        let t = w.ticks(0x1e);
                        c::set_pi32(w, id, o::HOLD, t);
                    } else {
                        c::dec_timer_pvar_i32(w, id, o::HOLD);
                        story::pvars(w, t, 0x40);
                        if c::pi32(w, id, o::HOLD) == 0 {
                            c::set_pf(w, t, 0x20, 0.0);
                            if w.m(t).o_class == 0x323 {
                                c::set_pf(w, t, 0x20, 40.0);
                                c::set_pf(w, t, 0x2c, DT * 24.0);
                                c::set_pf(w, t, 0x24, 0.01);
                                c::set_pf(w, t, 0x28, 0.2);
                            }
                        } else {
                            c::set_pf(w, t, 0x20, 2.0);
                        }
                    }
                }
            }
        }
        3 if c::dec_timer_pvar_s16(w, id, o::TIMER) != 0 => {
            let amb = [c::pu8(w, id, o::F + 4), c::pu8(w, id, o::F + 5), c::pu8(w, id, o::F + 6), 0];
            w.mm(id).ambient = amb;
            set_shown(w, id, true, true);
            let t = w.ticks(RESPAWN);
            c::set_pi16(w, id, o::TIMER, t as i16);
            w.mm(id).state = 2;
            if let Some((_, pts)) = points(w, id) { restart(w, id, &pts); }
            if let Some(t) = target(w, id) {
                w.mm(t).mode &= 0xffbe;
                carry(w, id, t);
            }
            if c::pi32(w, id, o::CUBOID) >= 0 {
                w.mm(id).state = 1;
                set_shown(w, id, false, true);
            }
        }
        _ => {}
    }
    let st = w.m(id).state;
    if st != 0 && st != 3 && st != 1 && !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
        let v = w.play_sound(0, 4, id);
        c::set_pi32(w, id, o::VOICE, v);
    }
}
