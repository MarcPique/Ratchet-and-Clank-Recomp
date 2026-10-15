//! **Batalia's ferries** (class 424, level08 `0x2dbdb0`, census U295, 2 placed): a hovering barge that docks at the
//! end of one of three paths (pvar +0x80, +0x84, +0x88) nearest Ratchet. Standing on its deck (clear of its rail) the
//! prompt shows where it goes; △ rides it along the path, Ratchet held on the deck (the jump lockout, the edge brake,
//! the deck spline +0x8c as his wall), to the far end. It bobs and sways, its propeller (joint list 0's manipulator)
//! spins faster while it moves. A ferry without paths (+0x80 = −1) only floats where it was placed. Read from the
//! level08 decomp and data (the deck outline 0x1db280).
//!
//! **Pvars** (0x150): +0x20 the riders' block, +0x60 / +0x70 the dock pose (position, Euler), +0x80 / +0x84 / +0x88
//! the paths, +0x8c the deck spline (8 points, rewritten every tick), +0x94 / +0x98 two missions, +0xa0 the
//! manipulator (+0xb0 its rotation), +0xe0 (s16) −1, +0xe4 / +0xe8 the ride's progress (0..1) and its speed, +0xec /
//! +0xf0 / +0xf4 the bob and sway phases, +0x100 the dock (0..3), +0x104 the ride's goal (0 or 1), +0x108 / +0x10c
//! the turn at the end and its speed, +0x110 the propeller's Euler, +0x120 the ride's path, +0x124 shown.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | game mode 2 or +0x124 = 0 → hidden (+0x31 = 0, mode \| 1), else shown; the manipulator attached once (joint list 0); +0x110.x += π/2·dt, the record's rotation from it (`0x26ee30`) | [`update`] (`manip::attach`, `manip::set_quat`) |
//! | 0 | +0x110 = 0; no paths → the placement is the dock, → 4, shown; else +0xe0 = −1, +0x100 = −1, the dock, → 2, +0x30 = 0xff | [`update`] |
//! | 2 | Ratchet on it (0x13f64c), the help box idle (0x179c10 = 0, 0x179c34 = −1), his feet clear of the deck's rail (`0x26c090(0.5)` moves them less than 0.001): the prompt (`try_set_help_message(10, …)`: 0x1f4d going back, 0x1f50 on path +0x80, 0x1f4f on +0x88, else 0x1f4e); △ pressed, on the ground, the prompt his (owner 10) → 3; not on it → the dock again | [`update`] (`interact::try_prompt`) |
//! | 3 | the hero fields: jump lockout 5, edge brake 5, wall spline +0x8c (0x13f542 / 0x13f544 / 0x14162a); the ride (`0x2dc648`); arrived → the dock, → 2 | [`ride`] |
//! | tail | the pose (`0x2dc4a0`: the dock pose, z + 0.25·sin(+0xec += 2π/3·dt), Euler y + 5°·sin(+0xf0 += 1.693·dt), x + 5°·sin(+0xf4 += 0.925·dt), x and y within ±10°); the deck spline (`0x2dc0c8`: 8 points, the outline through the rows plus the position, w kept nonzero); `CarryRiders(+0x20, position − old, old Euler, Euler)` | [`update`] |
//! | `0x2dc180` | the dock: d0 = Ratchet to the last point of +0x88, d1 to the first of +0x80, d2 to the first of +0x84 (3-D); 1 when d1 < d0, 3 when d2 is the least, else 0 (2 is never chosen); unchanged → nothing; else +0xe4 = 0, goal 1, shown, +0x124 = 1, collision on; 1: at +0x80's first point, facing its second, ride +0x80; 3: the same on +0x84; 0: at +0x88's last point facing back, progress 1, goal 0, ride: mission +0x98 done → +0x84; else mission +0x94 done → +0x80; else (Ratchet beyond 32 or the first 4 ticks) hidden, +0x124 = 0, collision off | [`dock`] |
//! | `0x2dc648` | progress springs to the goal (`0x270830`: 0.1·dt², 0.1·dt², 0.08·dt); propeller x += speed·50; the pose on the ride's path at progress·(n − 1) (`0x277d40`, open), Euler y = 0; within 0.2 of the goal the end turn springs to π (2π/3·dt², 2π·dt), else 0; goal 0 → yaw + π; yaw + the end turn; true at the goal | [`ride`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2, V};
use crate::moby_update::manip;
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 8;
pub const UPDATE_FN: u32 = 0x2d_bdb0;
pub const CLASSES: [i16; 1] = [424];

mod pv {
    pub const LEN: usize = 0x150;
    pub const RIDERS: usize = 0x20;
    pub const DOCK_POS: usize = 0x60;
    pub const DOCK_ROT: usize = 0x70;
    pub const PATH_A: usize = 0x80;
    pub const PATH_B: usize = 0x84;
    pub const PATH_C: usize = 0x88;
    pub const DECK: usize = 0x8c;
    pub const MISSION_A: usize = 0x94;
    pub const MISSION_B: usize = 0x98;
    pub const MANIP: usize = 0xa0;
    pub const E0: usize = 0xe0;
    pub const PROG: usize = 0xe4;
    pub const PROG_V: usize = 0xe8;
    pub const BOB: usize = 0xec;
    pub const SWAY_Y: usize = 0xf0;
    pub const SWAY_X: usize = 0xf4;
    pub const DOCK: usize = 0x100;
    pub const GOAL: usize = 0x104;
    pub const END_TURN: usize = 0x108;
    pub const END_TURN_V: usize = 0x10c;
    pub const PROP: usize = 0x110;
    pub const RIDE: usize = 0x120;
    pub const SHOWN: usize = 0x124;
}

/// The deck outline 0x1db280 (in the barge's frame).
const DECK: [[f32; 4]; 8] = [
    [-0.2, -1.25, -0.45, 1.0],
    [2.6, -1.25, -0.45, 1.0],
    [3.3, -1.05, -0.45, 1.0],
    [3.75, -0.6, -0.45, 1.0],
    [3.75, 0.6, -0.45, 1.0],
    [3.3, 1.05, -0.45, 1.0],
    [2.6, 1.25, -0.45, 1.0],
    [-0.2, 1.25, -0.45, 1.0],
];
const TEN_DEG: f32 = 0.174_532_92;

fn path(w: &World, i: i32) -> Option<Vec<[f32; 4]>> {
    usize::try_from(i).ok().and_then(|k| w.svc.splines.get(k)).map(|v| v.iter().map(|q| q.map(f32::from_bits)).collect())
}
fn path_of(w: &World, id: MobyId, o: usize) -> Option<Vec<[f32; 4]>> { path(w, c::pi32(w, id, o)) }

/// Level08 `0x2dbdb0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pv::LEN);
    let (start, rot0) = (c::pos(w, id), w.m(id).rotation);
    if w.svc.game_mode == 2 || c::pi32(w, id, pv::SHOWN) == 0 {
        let m = w.mm(id);
        m.visible = 0;
        m.mode |= 1;
    } else {
        let m = w.mm(id);
        m.visible = 1;
        m.mode &= !1;
    }
    if !manip::attached(w, id, pv::MANIP) { manip::attach(w, id, 0, id, pv::MANIP); }
    let prop = c::add_rot(c::pf(w, id, pv::PROP), DT * std::f32::consts::FRAC_PI_2);
    c::set_pf(w, id, pv::PROP, prop);
    let e = c::pv4(w, id, pv::PROP);
    manip::set_quat(w, id, id, pv::MANIP, crate::hero::idle::euler_quat([e[0], e[1], e[2]]));
    match w.m(id).state {
        0 => {
            c::set_pv4(w, id, pv::PROP, [0.0; 4]);
            if c::pi32(w, id, pv::PATH_A) == -1 {
                let (pos, rot) = (c::pos(w, id), w.m(id).rotation);
                c::set_pv4(w, id, pv::DOCK_POS, pos);
                c::set_pv4(w, id, pv::DOCK_ROT, rot);
                w.mm(id).state = 4;
                c::set_pi32(w, id, pv::SHOWN, 1);
            } else {
                c::set_pi16(w, id, pv::E0, -1);
                c::set_pi32(w, id, pv::DOCK, -1);
                dock(w, id);
                let m = w.mm(id);
                m.state = 2;
                m.update_dist = 0xff;
            }
        }
        2 => {
            if w.hero.ground_moby == Some(id) {
                if w.svc.help.idle() {
                    let h = super::hero_pos(w);
                    let deck = c::pi32(w, id, pv::DECK);
                    let clear = usize::try_from(deck).ok().is_none_or(|d| crate::moby_update::creature::region::push_out(w, 0.5, d, h).is_none_or(|q| c::dist3(h, q) < 0.001));
                    if clear {
                        let lease = prompt(w, id);
                        if w.hero.loop_in.pad.pressed & crate::pad::button::TRIANGLE != 0 && w.hero.air_ticks == 0 && lease != 0 {
                            w.mm(id).state = 3;
                        }
                    }
                }
            } else {
                dock(w, id);
            }
        }
        3 => {
            let deck = c::pi32(w, id, pv::DECK) as i16;
            let f = w.hero_fields_mut();
            f.jump_lockout = 5;
            f.edge_brake = 5;
            f.wall_spline = Some(deck);
            if ride(w, id) {
                dock(w, id);
                w.mm(id).state = 2;
            }
        }
        _ => {}
    }
    pose(w, id);
    if c::pi32(w, id, pv::DECK) != -1 { deck(w, id); }
    let delta = c::sub(c::pos(w, id), start);
    let rot = w.m(id).rotation;
    crate::moby_update::triggers::carry_riders(&mut w.mm(id).pvars, pv::RIDERS, delta, rot0, rot);
}

/// `0x2dc7f0`: the prompt by where the ride goes; the lease (non-zero: the prompt is this barge's, owner 10).
fn prompt(w: &mut World, id: MobyId) -> i32 {
    let ride = c::pi32(w, id, pv::RIDE);
    let msg = if 0.9 < c::pf(w, id, pv::GOAL) {
        0x1f4d
    } else if ride == c::pi32(w, id, pv::PATH_A) {
        0x1f50
    } else if ride == c::pi32(w, id, pv::PATH_C) {
        0x1f4f
    } else {
        0x1f4e
    };
    w.svc.interact.try_prompt(10, msg)
}

/// `0x2dc180` (module doc).
fn dock(w: &mut World, id: MobyId) {
    let (Some(pa), Some(pb), Some(pc)) = (path_of(w, id, pv::PATH_A), path_of(w, id, pv::PATH_B), path_of(w, id, pv::PATH_C)) else { return };
    if pa.len() < 2 || pb.len() < 2 || pc.len() < 2 { return; }
    let h = super::hero_pos(w);
    let d0 = c::dist3(h, *pc.last().unwrap());
    let d1 = c::dist3(h, pa[0]);
    let d2 = c::dist3(h, pb[0]);
    let mut sel = (d1 < d0) as i32;
    if d2 < d0.min(d1) { sel = 3; }
    if c::pi32(w, id, pv::DOCK) == sel { return; }
    c::set_pi32(w, id, pv::DOCK, sel);
    c::set_pf(w, id, pv::PROG, 0.0);
    c::set_pf(w, id, pv::GOAL, 1.0);
    show(w, id, true);
    let face = |a: [f32; 4], b: [f32; 4]| c::atan(b[0] - a[0], b[1] - a[1]);
    let (at, yaw, ride) = match sel {
        1 => (pa[0], face(pa[0], pa[1]), c::pi32(w, id, pv::PATH_A)),
        3 => (pb[0], face(pb[0], pb[1]), c::pi32(w, id, pv::PATH_B)),
        _ => {
            let n = pc.len();
            let at = pc[n - 1];
            c::set_pv4(w, id, pv::DOCK_POS, at);
            c::set_pf(w, id, pv::DOCK_ROT + 8, face(at, pc[n - 2]));
            c::set_pf(w, id, pv::PROG, 1.0);
            c::set_pf(w, id, pv::GOAL, 0.0);
            let lvl = |o: usize| story::mission_done(w, c::pi32(w, id, o));
            if lvl(pv::MISSION_B) {
                c::set_pi32(w, id, pv::RIDE, c::pi32(w, id, pv::PATH_B));
            } else if lvl(pv::MISSION_A) {
                c::set_pi32(w, id, pv::RIDE, c::pi32(w, id, pv::PATH_A));
            } else if 32.0 < c::dist2(c::pos(w, id), h) || w.counter <= 4 {
                show(w, id, false);
            }
            return;
        }
    };
    c::set_pv4(w, id, pv::DOCK_POS, at);
    c::set_pf(w, id, pv::DOCK_ROT + 8, yaw);
    c::set_pi32(w, id, pv::RIDE, ride);
}

fn show(w: &mut World, id: MobyId, on: bool) {
    let has = on && super::class_collision(w, w.m(id).o_class);
    let m = w.mm(id);
    m.visible = on as u8;
    if on { m.mode &= !1; } else { m.mode |= 1; }
    m.has_collision = has;
    c::set_pi32(w, id, pv::SHOWN, on as i32);
}

/// `0x2dc648` (module doc); true at the goal.
fn ride(w: &mut World, id: MobyId) -> bool {
    let goal = c::pf(w, id, pv::GOAL);
    let (mut x, mut v) = (c::pf(w, id, pv::PROG), c::pf(w, id, pv::PROG_V));
    turn::spring(goal, DT2 * 0.1, DT2 * 0.1, DT * 0.08, &mut x, &mut v);
    c::set_pf(w, id, pv::PROG, x);
    c::set_pf(w, id, pv::PROG_V, v);
    let prop = c::add_rot(c::pf(w, id, pv::PROP), v * 50.0);
    c::set_pf(w, id, pv::PROP, prop);
    let raw = usize::try_from(c::pi32(w, id, pv::RIDE)).ok().and_then(|k| w.svc.splines.get(k)).filter(|p| p.len() >= 2).cloned();
    if let Some(pts) = raw {
        let (pos, rot) = crate::path::pose(&pts, false, x * (pts.len() as f32 - 1.0), true);
        c::set_pv4(w, id, pv::DOCK_POS, pos);
        c::set_pv4(w, id, pv::DOCK_ROT, rot);
    }
    c::set_pf(w, id, pv::DOCK_ROT + 4, 0.0);
    if (x - goal).abs() < 0.2 {
        let (mut a, mut av) = (c::pf(w, id, pv::END_TURN), c::pf(w, id, pv::END_TURN_V));
        let k = DT2 * 2.094_395_2;
        turn::spring(f32::from_bits(0x4049_0fd0), k, k, DT * std::f32::consts::TAU, &mut a, &mut av);
        c::set_pf(w, id, pv::END_TURN, a);
        c::set_pf(w, id, pv::END_TURN_V, av);
    } else {
        c::set_pf(w, id, pv::END_TURN, 0.0);
    }
    let mut yaw = c::pf(w, id, pv::DOCK_ROT + 8);
    if goal == 0.0 { yaw = c::add_rot(yaw, f32::from_bits(0x4049_0fd0)); }
    yaw = c::add_rot(yaw, c::pf(w, id, pv::END_TURN));
    c::set_pf(w, id, pv::DOCK_ROT + 8, yaw);
    x == goal
}

/// `0x2dc4a0`: the pose, the bob and the sway (module doc).
fn pose(w: &mut World, id: MobyId) {
    let mut pos = c::pv4(w, id, pv::DOCK_POS);
    let mut rot = c::pv4(w, id, pv::DOCK_ROT);
    let bob = c::add_rot(c::pf(w, id, pv::BOB), DT * 2.094_395_2);
    c::set_pf(w, id, pv::BOB, bob);
    pos[2] += bob.sin() * 0.25;
    let sy = c::add_rot(c::pf(w, id, pv::SWAY_Y), DT * 1.692_969_3);
    c::set_pf(w, id, pv::SWAY_Y, sy);
    rot[1] = c::add_rot(rot[1], sy.sin() * 0.087_266_46);
    let sx = c::add_rot(c::pf(w, id, pv::SWAY_X), DT * 0.925_024_5);
    c::set_pf(w, id, pv::SWAY_X, sx);
    rot[0] = c::add_rot(rot[0], sx.sin() * 0.087_266_46);
    rot[0] = rot[0].clamp(-TEN_DEG, TEN_DEG);
    rot[1] = rot[1].clamp(-TEN_DEG, TEN_DEG);
    c::set_pos(w, id, pos);
    w.mm(id).rotation = rot;
}

/// `0x2dc0c8`: the deck spline through the barge's rows (module doc).
fn deck(w: &mut World, id: MobyId) {
    let Some(d) = usize::try_from(c::pi32(w, id, pv::DECK)).ok() else { return };
    if w.svc.splines.get(d).is_none_or(|s| s.len() != 8) { return; }
    let r = w.m(id).rows;
    let pos = c::pos(w, id);
    let pts: Vec<[u32; 4]> = DECK
        .iter()
        .map(|q| {
            let v: V = std::array::from_fn(|k| r[0][k] * q[0] + r[1][k] * q[1] + r[2][k] * q[2] + if k == 3 { q[3] } else { 0.0 });
            c::add(v, pos).map(f32::to_bits)
        })
        .collect();
    w.svc.splines[d] = pts;
}
