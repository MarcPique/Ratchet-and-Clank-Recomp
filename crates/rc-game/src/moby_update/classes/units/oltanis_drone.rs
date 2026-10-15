//! **Oltanis's grenade drones, class 31** (level14 `0x2b46e8`; census U492, 4 placed), with the pieces they leave
//! (**81**, `0x2bac78`) and their grenades (**1193**, `0x300e00`). Read from the level14 decomp and disassembly (the
//! state table 0x1fbf80, which the decompiler could not follow); native `f32`.
//!
//! A hovering drone (bobbing, wobbling, its jets going) that waits hidden at the start of path A (+0xd0) until it has
//! a target (`0x274df8` within 255 in its area +0xd4), flies along A (or B, +0x150, on its other side), then holds
//! over its end facing along path C (+0xdc), lobbing grenades at the points of path D (+0xd8) one after another; too
//! close to its target (10) it slips along C to C's other end and turns; no target: back along A to hide.
//!
//! * **Every tick**: a wrapped sequence other than 0 → 0 (blend 10 for 2, else a cut); the hits (`0x2b5290`, its scale
//!   from the class, mask 0x330000; dead → 9, red, its death bits; stunned → sequence 2 at speed 1.75, red; the
//!   flash), the target (`0x2b5530`), in 4..6 / 8 the springs onto its point +0x180 and toward its yaw +0x17c
//!   (`0x2b5850`), the hover (`0x2b5428`: in 2 / 3 / 7 the wobble springs to 0.15, else to 0.1 with the bob rate
//!   springing to 0.035 and the bob 0.25), the jets (`0x2b5dd8`), within 40 of the camera the shadow probe.
//! * 0: the paths checked (else deleted); at A's start facing along it; the chords of A, B and C; hidden; the
//!   manipulator on joint list 0 (+0x110) → 1. 1: a target → 2 (shown, `0x2b5d18` at the path's start). 2: along the
//!   path at 0.15 a tick (springs 0.005 / 0.2, turn 0.004 / 0.2); at its end → 4 holding its last point, facing along
//!   C. 3: no target: back along the path (0.0075 / 0.2, 0.006 / 0.2) → at its start 1, hidden. 4: the head eases
//!   (+0x104 → 0 over `ticks(180)`); within 10 of the target → 7; no target → 3 (from the path's end); the timer out
//!   (not in sequence 2) → 5. 5: the head turns to the next point of D (eased over `ticks(30)`), then 6 with the cut 1
//!   (in sequence 2: back to 4). 6: at key 3 the grenade 1193 from joint list 1 toward the point (over `ticks(45)`,
//!   gravity 25·dt², life `ticks(120)`; its z speed goes to +0x48, the grenade starts flat), the muzzle smoke and
//!   sparks (`0x2b5968`) and a flash light; next point → 5, the last → 4 (`ticks(180)`). 7: along C to its other end
//!   → 4, the side flipped. 9: two pieces 81 (from joint lists 2 and 3, the second turned back), the explosion, gone.
//! * **81** (`0x2bac78`): thrown off (0.075..0.2 away from the drone, turned up to 45°, up 0.05..0.32, tumbling),
//!   falling (0.003, ×0.97) for `ticks(200)`, smoking (type 21, three type 23) with a light (radius 3, (1, 0.8, 0.2))
//!   on one side of it; touching something or within 10 (x and y) of Ratchet it explodes (damage 4 within 2, sound 1
//!   of its owner's class), gone.
//! * **1193** (`0x300e00`): flies (gravity), trailing smoke for `ticks(12)`; a moby other than its owner within 0.65,
//!   or its life out → the explosion (damage 1 within 2), gone. On the ground it bounces (sound 1; the first bounce
//!   6·dt flat, then a third), after four it rolls, slowing (×0.96, at least 0.2·dt), its glow fading to (67, 230,
//!   132), spinning with its speed.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2b46e8` / table 0x1fbf80 | 31 | [`update`] |
//! | `0x2b5290` / `0x2b5530` / `0x2b5850` / `0x2b5428` / `0x2b5dd8` | hits, target, steer, hover, jets | [`update`] |
//! | `0x2b5730` / `0x2b55b0` / `0x2b5d18` / `0x2b5c30` / `0x2b5c98` | path follow, chords, path start / end, hide | [`follow`] |
//! | `0x3014e0` / `0x2b5968` | the grenade, the muzzle | [`throw`] |
//! | `0x2bb310` / `0x2bac78` | 81 | [`make_piece`], [`piece_update`] |
//! | `0x300e00` | 1193 | [`grenade_update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, target, turn, DT, DT2, SPEED};
use crate::moby_update::services::{self as sv, World};
use crate::moby_update::{manip, story};
use crate::particles::rec;
use crate::ps2v::Pf;
use crate::spline::{advance, Cursor};

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2b_46e8;
pub const CLASSES: [i16; 1] = [31];
pub const PIECE_FN: u32 = 0x2b_ac78;
pub const PIECE_CLASSES: [i16; 1] = [81];
pub const GRENADE_FN: u32 = 0x30_0e00;
pub const GRENADE_CLASSES: [i16; 1] = [1193];

mod o {
    pub const D: usize = 0x20;
    pub const VZ: usize = 0x48;
    pub const F: usize = 0x60;
    pub const T: usize = 0x70;
    pub const TM: usize = 0xb0;
    pub const KIND: usize = 0xb4;
    pub const BOB: usize = 0xc0;
    pub const WOB: usize = 0xc8;
    pub const PATH_A: usize = 0xd0;
    pub const AREA: usize = 0xd4;
    pub const PATH_D: usize = 0xd8;
    pub const PATH_C: usize = 0xdc;
    pub const YAW_V: usize = 0xe4;
    pub const SEG: usize = 0xe8;
    pub const DIST: usize = 0xec;
    pub const VEL: usize = 0xf0;
    pub const THROW: usize = 0x100;
    pub const TIMER: usize = 0x102;
    pub const AIM: usize = 0x104;
    pub const VOICE: usize = 0x108;
    pub const SIDE: usize = 0x10c;
    pub const NODE: usize = 0x110;
    pub const PATH_B: usize = 0x150;
    pub const TURN: usize = 0x154;
    pub const WOB_AMP: usize = 0x160;
    pub const WOB_AMP_V: usize = 0x164;
    pub const BOB_RATE: usize = 0x168;
    pub const BOB_RATE_V: usize = 0x16c;
    pub const MOVE: usize = 0x170;
    pub const YAW_T: usize = 0x17c;
    pub const POINT: usize = 0x180;
    pub const SIZE: usize = 0x190;
}
/// Level14 gp words (0x161530..0x161594).
const HOVER: f32 = 0.1;
const OUT_SPEED: f32 = 0.15;
const IDLE_TICKS: i32 = 180;
const AIM_TICKS: i32 = 30;
const THROW_TICKS: i32 = 45;
const THROW_G: f32 = 25.0;
const GRENADE_LIFE: i32 = 120;
const NEAR: f32 = 10.0;
/// The springs out along a path (k, d, max for the position; k, d, max for the turn) and back.
const OUT: [f32; 6] = [0.005, 0.2, 0.0, 0.004, 0.2, 0.0];
const BACK: [f32; 6] = [0.0075, 0.2, 0.0, 0.006, 0.2, 0.0];

fn path(w: &World, i: i32) -> Option<Vec<[f32; 4]>> {
    let s = w.svc.splines.get(usize::try_from(i).ok()?)?;
    (!s.is_empty()).then(|| s.iter().map(|q| q.map(f32::from_bits)).collect())
}
fn side_path(w: &World, id: MobyId) -> i32 { if c::pi32(w, id, o::SIDE) != 0 { c::pi32(w, id, o::PATH_B) } else { c::pi32(w, id, o::PATH_A) } }
fn link(w: &World, v: i32) -> Option<MobyId> { usize::try_from(v - 1).ok().filter(|&m| v > 0 && m < w.table.mobys.len()) }
fn chords(w: &mut World, i: i32) {
    let Some(pts) = path(w, i) else { return };
    for k in 0..pts.len().saturating_sub(1) { w.svc.splines[i as usize][k][3] = c::dist3(pts[k], pts[k + 1]).to_bits(); }
}
fn release(w: &mut World, id: MobyId, off: usize) {
    let v = c::pi32(w, id, off);
    if v != -1 && w.sound_owner(v) == Some(id) { w.release_sound(v, id); }
    c::set_pi32(w, id, off, -1);
}
/// `0x2b5c98`: hidden, no collision, the hum released.
fn hide(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    m.has_collision = false;
    m.mode |= 0x41;
    release(w, id, o::VOICE);
}
#[allow(clippy::too_many_arguments)]
fn spring_axis(w: &mut World, id: MobyId, a: usize, t: f32, k: f32, d: f32, max: f32, v_off: usize) {
    let (mut x, mut v) = (Pf::f(w.m(id).position[a]), Pf::f(c::pf(w, id, v_off)));
    crate::hero::physics::spring(Pf::f(t), Pf::f(k), Pf::f(d), Pf::f(max), &mut x, &mut v);
    w.mm(id).position[a] = x.to_f32();
    c::set_pf(w, id, v_off, v.to_f32());
}
fn spring_pv(w: &mut World, id: MobyId, t: f32, k: f32, d: f32, x_off: usize, v_off: usize) {
    let (mut x, mut v) = (Pf::f(c::pf(w, id, x_off)), Pf::f(c::pf(w, id, v_off)));
    crate::hero::physics::spring(Pf::f(t), Pf::f(k), Pf::f(d), Pf::ZERO, &mut x, &mut v);
    c::set_pf(w, id, x_off, x.to_f32());
    c::set_pf(w, id, v_off, v.to_f32());
}

/// `0x2b5730(speed, springs, m, path)`: along the path, springs onto the point, turned along the velocity; whether an
/// end was reached.
pub fn follow(w: &mut World, id: MobyId, speed: f32, s: [f32; 6], i: i32) -> bool {
    let Some(pts) = path(w, i) else { return false };
    let mut cur = Cursor { seg: c::pi32(w, id, o::SEG), t: c::pf(w, id, o::DIST) };
    let (q, ended) = advance(&pts, false, speed, &mut cur);
    c::set_pi32(w, id, o::SEG, cur.seg);
    c::set_pf(w, id, o::DIST, cur.t);
    for (a, &t) in q.iter().enumerate() { spring_axis(w, id, a, t, s[0], s[1], s[2], o::VEL + 4 * a); }
    let yaw = c::atan(c::pf(w, id, o::VEL), c::pf(w, id, o::VEL + 4));
    turn::spring_turn2_pvar(w, id, yaw, s[3], s[4], s[5], o::YAW_V);
    ended
}

/// The end points of path C for the side (first, then the other).
fn c_ends(w: &World, id: MobyId) -> Option<([f32; 4], [f32; 4])> {
    let p = path(w, c::pi32(w, id, o::PATH_C))?;
    let (a, b) = (p[0], *p.last()?);
    Some(if c::pi32(w, id, o::SIDE) == 0 { (a, b) } else { (b, a) })
}
fn set_springs(w: &mut World, id: MobyId, s: [f32; 6]) {
    for (k, v) in [s[0], s[1], s[2]].into_iter().enumerate() { c::set_pf(w, id, o::MOVE + 4 * k, v); }
    for (k, v) in [s[3], s[4], s[5]].into_iter().enumerate() { c::set_pf(w, id, o::TURN + 4 * k, v); }
}

/// Level14 `0x2b46e8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let a = w.m(id).anim;
    if a.flags & 2 != 0 && a.seq_a == a.seq_b && a.seq_a != 0 {
        if a.seq_a == 2 { w.anim_blend(id, 0, 0, 10); } else { c::hard_cut(w, id, 0, 0); }
    }
    hits(w, id);
    if !w.table.mobys.get(id).is_some_and(|m| m.state < 0xfd) { return; }
    let st = w.m(id).state;
    if st != 0 && st != 9 {
        let area = usize::try_from(c::pi32(w, id, o::AREA)).ok();
        let t = target::acquire_in(w, id, 255.0, area);
        c::set_pv4(w, id, o::T, t.pos);
        c::set_pv4(w, id, o::T + 0x10, t.rot);
        c::set_pv4(w, id, o::T + 0x20, t.aim);
        c::set_pv4(w, id, o::T + 0x30, t.body);
        c::set_pi32(w, id, o::TM, t.moby.or(w.hero_moby).map_or(0, |m| m as i32 + 1));
        c::set_pi32(w, id, o::KIND, t.kind as i32);
    }
    // 0x2b5850.
    if (4..=6).contains(&st) || st == 8 {
        let pt = c::pv4(w, id, o::POINT);
        let (k, d, max) = (c::pf(w, id, o::MOVE), c::pf(w, id, o::MOVE + 4), c::pf(w, id, o::MOVE + 8));
        for (a, &t) in pt.iter().take(3).enumerate() { spring_axis(w, id, a, t, k, d, max, o::VEL + 4 * a); }
        let (yt, tk, td, tm) = (c::pf(w, id, o::YAW_T), c::pf(w, id, o::TURN), c::pf(w, id, o::TURN + 4), c::pf(w, id, o::TURN + 8));
        turn::spring_turn2_pvar(w, id, yt, tk, td, tm, o::YAW_V);
    }
    // 0x2b5428.
    if st != 0 && st != 9 && st != 1 {
        let amp = if (2..=3).contains(&st) || st == 7 {
            HOVER * 1.5
        } else {
            spring_pv(w, id, f32::from_bits(0x3d0f_5c29), f32::from_bits(0x3b03_126f), 0.2, o::BOB_RATE, o::BOB_RATE_V);
            let rate = c::pf(w, id, o::BOB_RATE);
            super::bob(w, id, 0.25, rate, o::BOB, o::BOB + 4);
            HOVER
        };
        spring_pv(w, id, amp, f32::from_bits(0x3b44_9ba6), 0.2, o::WOB_AMP, o::WOB_AMP_V);
        let k = c::pf(w, id, o::WOB_AMP);
        super::wobble(w, id, k, 0.05, 0.025, o::WOB, o::WOB + 4);
    }
    if 1 < st && st != 9 { jets(w, id); }
    if w.m(id).visible != 0 {
        let d = c::dist3(c::pos(w, id), w.camera.map(|x| x.to_f32()));
        if d < 40.0 && st != 0 && st != 1 && st != 9 { crate::shadows::probe_down(w, id); }
    }
    match st {
        0 => start(w, id),
        1 => {
            if c::pi32(w, id, o::KIND) != 2 {
                w.mm(id).state = 2;
                let coll = super::class_collision(w, w.m(id).o_class);
                let m = w.mm(id);
                m.mode &= 0xffbe;
                m.has_collision = coll;
                to_start(w, id);
            }
        }
        2 => {
            let p = side_path(w, id);
            if c::pi32(w, id, o::KIND) == 2 {
                w.mm(id).state = 3;
            } else if follow(w, id, OUT_SPEED * SPEED, OUT, p) {
                if let Some(last) = path(w, p).and_then(|q| q.last().copied()) { c::set_pv4(w, id, o::POINT, last); }
                if let Some((a, b)) = c_ends(w, id) { c::set_pf(w, id, o::YAW_T, c::atan(b[0] - a[0], b[1] - a[1])); }
                set_springs(w, id, OUT);
                w.mm(id).state = 4;
                c::set_pf(w, id, o::AIM, 0.0);
                c::set_pi16(w, id, o::TIMER, 0);
            }
        }
        3 => {
            let p = side_path(w, id);
            if c::pi32(w, id, o::KIND) != 2 {
                w.mm(id).state = 2;
            } else if follow(w, id, -(OUT_SPEED * SPEED), BACK, p) {
                w.mm(id).state = 1;
                hide(w, id);
            }
        }
        4 => hold(w, id),
        5 => aim(w, id),
        6 => throw(w, id),
        7 => {
            let speed = if c::pi32(w, id, o::SIDE) == 1 { -(OUT_SPEED * SPEED) } else { OUT_SPEED * SPEED };
            if follow(w, id, speed, BACK, c::pi32(w, id, o::PATH_C)) {
                w.mm(id).state = 4;
                let s = if c::pi32(w, id, o::SIDE) != 0 { 0 } else { 1 };
                c::set_pi32(w, id, o::SIDE, s);
                set_springs(w, id, BACK);
                if let Some((a, b)) = c_ends(w, id) {
                    c::set_pf(w, id, o::YAW_T, c::atan(b[0] - a[0], b[1] - a[1]));
                    c::set_pv4(w, id, o::POINT, a);
                }
            }
        }
        9 => {
            let (j2, j3) = (w.joint_point(id, 2), w.joint_point(id, 3));
            let y = c::yaw(w, id);
            make_piece(w, y, id, j2);
            make_piece(w, c::add_rot(y, std::f32::consts::PI), id, j3);
            let p = c::pos(w, id);
            let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 6, debris: 1, sound: 1, shake: true };
            fx::beam_explosion(w, &b, Some(id), p);
            release(w, id, o::VOICE);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    let st = w.m(id).state;
    if 1 < st && st != 9 && !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
        let v = w.play_sound(0, 4, id);
        c::set_pi32(w, id, o::VOICE, v);
    }
}

/// State 0 (module doc).
fn start(w: &mut World, id: MobyId) {
    let ok = [o::PATH_A, o::AREA, o::PATH_D, o::PATH_B].iter().all(|&k| path(w, c::pi32(w, id, k)).is_some());
    if !ok {
        w.delete_moby(id);
        return;
    }
    let a = path(w, c::pi32(w, id, o::PATH_A)).unwrap_or_default();
    w.mm(id).position = a[0];
    let p1 = a.get(1).copied().unwrap_or(a[0]);
    let pos = c::pos(w, id);
    c::set_yaw(w, id, c::atan(p1[0] - pos[0], p1[1] - pos[1]));
    c::set_pf(w, id, o::WOB_AMP, HOVER);
    for k in [o::WOB_AMP_V, o::BOB_RATE, o::BOB_RATE_V] { c::set_pf(w, id, k, 0.0); }
    for k in [o::PATH_A, o::PATH_B, o::PATH_C] { chords(w, c::pi32(w, id, k)); }
    c::set_pi32(w, id, o::SIDE, 0);
    c::set_pi32(w, id, o::VOICE, -1);
    let m = w.mm(id);
    m.update_dist = 0x80;
    m.state = 1;
    hide(w, id);
    w.mm(id).pvars[o::NODE..o::NODE + 0x40].fill(0);
    manip::attach(w, id, 0, id, o::NODE);
    w.mm(id).b7f = 0x20;
}

/// `0x2b5d18`: at the side's path start facing along it, the springs and hover reset.
fn to_start(w: &mut World, id: MobyId) {
    c::set_pi32(w, id, o::SEG, 0);
    c::set_pf(w, id, o::WOB_AMP, HOVER * 1.5);
    for k in [o::DIST, o::WOB_AMP_V, o::BOB_RATE, o::BOB_RATE_V, o::BOB, o::BOB + 4, o::YAW_V] { c::set_pi32(w, id, k, 0); }
    if let Some(p) = path(w, side_path(w, id)) {
        w.mm(id).position = p[0];
        let p1 = p.get(1).copied().unwrap_or(p[0]);
        c::set_yaw(w, id, c::atan(p1[0] - p[0][0], p1[1] - p[0][1]));
    }
    c::set_pv4(w, id, o::VEL, [0.0; 4]);
}

/// State 4 (module doc).
fn hold(w: &mut World, id: MobyId) {
    let t = w.ticks(IDLE_TICKS) as f32;
    let a = crate::follow_camera::script::cos_interp(0.0, c::pf(w, id, o::AIM), c::pi16(w, id, o::TIMER) as f32 / t);
    manip::set_axis(w, id, id, o::NODE, a, 2);
    let tgt = link(w, c::pi32(w, id, o::TM)).map_or([0.0; 4], |m| c::pos(w, m));
    if c::dist3(c::pos(w, id), tgt) < NEAR {
        if c::pi32(w, id, o::SIDE) == 0 {
            c::set_pf(w, id, o::DIST, 0.0);
            c::set_pi32(w, id, o::SEG, 0);
        } else if let Some(p) = path(w, c::pi32(w, id, o::PATH_C)) {
            let k = p.len() as i32 - 2;
            c::set_pi32(w, id, o::SEG, k);
            c::set_pf(w, id, o::DIST, p.get(k.max(0) as usize).map_or(0.0, |q| q[3]));
        }
        w.mm(id).state = 7;
        c::set_pv4(w, id, o::VEL, [0.0; 4]);
        c::set_pf(w, id, o::YAW_V, 0.0);
        return;
    }
    if c::pi32(w, id, o::KIND) == 2 {
        w.mm(id).state = 3;
        // 0x2b5c30: the cursor at the path's end.
        if let Some(p) = path(w, side_path(w, id)) {
            let k = p.len() as i32 - 2;
            c::set_pi32(w, id, o::SEG, k);
            c::set_pf(w, id, o::YAW_V, 0.0);
            c::set_pf(w, id, o::DIST, p.get(k.max(0) as usize).map_or(0.0, |q| q[3]));
        }
        c::set_pv4(w, id, o::VEL, [0.0; 4]);
        return;
    }
    if c::dec_timer_pvar_s16(w, id, o::TIMER) == 0 { return; }
    if w.m(id).anim.seq_b == 2 { return; }
    w.mm(id).state = 5;
    let t = w.ticks(AIM_TICKS);
    c::set_pi16(w, id, o::TIMER, t as i16);
    c::set_pf(w, id, o::AIM, 0.0);
    c::set_pi16(w, id, o::THROW, 0);
}

/// The next point of path D.
fn throw_point(w: &World, id: MobyId) -> [f32; 4] {
    let i = c::pi16(w, id, o::THROW).max(0) as usize;
    path(w, c::pi32(w, id, o::PATH_D)).and_then(|p| p.get(i).copied()).unwrap_or([0.0; 4])
}

/// State 5 (module doc).
fn aim(w: &mut World, id: MobyId) {
    let p = throw_point(w, id);
    let pos = c::pos(w, id);
    let rel = c::sub_rot(c::atan(p[0] - pos[0], p[1] - pos[1]), c::yaw(w, id));
    let t = w.ticks(AIM_TICKS) as f32;
    let a = crate::follow_camera::script::cos_interp(rel, c::pf(w, id, o::AIM), c::pi16(w, id, o::TIMER) as f32 / t);
    manip::set_axis(w, id, id, o::NODE, a, 2);
    if w.m(id).anim.seq_b == 2 {
        w.mm(id).state = 4;
        let t = w.ticks(IDLE_TICKS);
        c::set_pi16(w, id, o::TIMER, t as i16);
        return;
    }
    if c::dec_timer_pvar_s16(w, id, o::TIMER) == 0 { return; }
    c::set_pf(w, id, o::AIM, a);
    w.mm(id).state = 6;
    c::hard_cut(w, id, 1, 0);
}

/// State 6 (module doc).
pub fn throw(w: &mut World, id: MobyId) {
    if !c::ground::passed_frame(w, id, 3.0) { return; }
    let t = w.ticks(THROW_TICKS) as f32;
    let aim = c::pf(w, id, o::AIM);
    manip::set_axis(w, id, id, o::NODE, aim, 2);
    let start = w.joint_point(id, 1);
    let p = throw_point(w, id);
    let mut v = c::sub(p, start);
    v[2] = 0.0;
    let v = c::scale(v, 1.0 / t);
    let g = THROW_G * DT2;
    let vz = -((start[2] - p[2]) + -g * (t * t) * 0.5) / t;
    c::set_pf(w, id, o::VZ, vz);
    let life = w.ticks(GRENADE_LIFE);
    make_grenade(w, g, id, start, v, life);
    muzzle(w, id, aim, start);
    let n = c::pi16(w, id, o::THROW) + 1;
    c::set_pi16(w, id, o::THROW, n);
    let count = path(w, c::pi32(w, id, o::PATH_D)).map_or(0, |q| q.len()) as i16;
    if n < count {
        let t = w.ticks(AIM_TICKS);
        c::set_pi16(w, id, o::TIMER, t as i16);
        w.mm(id).state = 5;
    } else {
        w.mm(id).state = 4;
        let t = w.ticks(IDLE_TICKS);
        c::set_pi16(w, id, o::TIMER, t as i16);
    }
}

/// `0x3014e0(gravity, owner, pos, vel, life)`: the grenade 1193.
fn make_grenade(w: &mut World, g: f32, owner: MobyId, at: c::V, vel: c::V, life: i32) -> Option<MobyId> {
    let m = w.create_moby(0x4a9)?;
    story::pvars(w, m, 0x30);
    let t = w.ticks(0xc);
    {
        let mm = w.mm(m);
        mm.update_dist = 0xff;
        mm.draw_dist = 0xff;
        mm.visible = 1;
        mm.state = 0;
        mm.position = at;
        mm.rotation[0] = 0.0;
        mm.rotation[1] = 0.0;
        mm.rotation[2] = c::atan(vel[0], vel[1]);
        mm.scale *= f32::from_bits(0x3fa6_6666);
    }
    c::set_pi32(w, m, 0x24, owner as i32 + 1);
    c::set_pv4(w, m, 0x10, [vel[0], vel[1], vel[2], g]);
    c::set_pi32(w, m, 0x20, life);
    c::set_pi16(w, m, 0x28, t as i16);
    c::set_pi16(w, m, 0x2a, 0);
    w.build_matrix(m);
    Some(m)
}

/// `0x2b5968(aim, m, at)`: the muzzle smoke, sparks and flash 0.8 out along the aim.
fn muzzle(w: &mut World, id: MobyId, aim: f32, at: c::V) {
    let a = c::add_rot(aim, c::yaw(w, id));
    let dir = [a.cos() * 0.8, a.sin() * 0.8, 0.0, 0.0];
    let p = c::add(at, dir);
    for _ in 0..6 {
        let mut k = w.rng.randi(2);
        if w.rng.randi(2) != 0 { k = -k; }
        let size = w.rng.randf(f32::from_bits(0x471c_4000), f32::from_bits(0x4812_7c00));
        let g = w.rng.rand_range(10, 0x7f) as u32;
        let Some(i) = fx::part23_rec(w, [f32::from_bits(0x3e4c_cccd), 1.0, f32::from_bits(0x3f81_47ae), size], p, k, [0.0; 4], g | g << 16 | g << 8 | 0x7f00_0000) else { continue };
        let t = w.ticks(0x78);
        if let Some(r) = fx::rec_mut(w, i) {
            rec::set_i16(r, 10, t as i16);
            rec::set_u32(r, 0x24, 2);
            r[0x2a] = 0x7f;
            r[0x2b] = r[10];
        }
    }
    let axis = [dir[0], dir[1], dir[2]];
    let mut s = c::scale(dir, 0.0625);
    s[2] = w.rng.randf(f32::from_bits(0x3ccc_cccd), f32::from_bits(0x3d75_c28f));
    for i in 0..0x10 {
        let j = w.rng.randf_sym(0.0, 0.5);
        let v = rotate([s[0], s[1], s[2]], i as f32 * 0.6283 + j, axis);
        let q = [0; 3].map(|_| w.rng.randf_sym(0.0, f32::from_bits(0x3e4c_cccd)));
        fx::part19(w, [p[0] + q[0], p[1] + q[1], p[2] + q[2], p[3]], [v[0], v[1], v[2], 0.0]);
    }
    fx::light_spawn(w, &FLASH_LIGHT, p);
}

/// Level14 0x1d88d0: the muzzle's flash light (radius 2, 20 ticks).
const FLASH_LIGHT: fx::LightTemplate = fx::LightTemplate {
    offset: [0.0; 4],
    bytes: [0x00, 0x00, 0xff, 0x00, 0x19, 0x19, 0x00, 0xff, 0x00, 0x19, 0x19, 0x00, 0x64, 0x00, 0x19, 0x19],
    radius: [2.0, 2.0, 2.0],
    grow: 0.01,
    shrink: 0.01,
    life: 0x14,
    flags: 0x0008_0000,
    draw: 0xff,
    holds: [0, 0, 0],
    periods: [0, 0, 0],
    period_r: 0,
    hold_r: 0,
    delay: 0,
};

/// `FUN_00274ac8`: `v` turned by `a` about `axis` (Rodrigues; a near-zero angle keeps `v`).
fn rotate(v: [f32; 3], a: f32, axis: [f32; 3]) -> [f32; 3] {
    if a.abs() < 1e-5 { return v; }
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if n == 0.0 { return v; }
    let k = axis.map(|x| x / n);
    let (s, cs) = a.sin_cos();
    let d = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    let x = [k[1] * v[2] - k[2] * v[1], k[2] * v[0] - k[0] * v[2], k[0] * v[1] - k[1] * v[0]];
    std::array::from_fn(|i| v[i] * cs + x[i] * s + k[i] * d * (1.0 - cs))
}

/// `0x2b5290` (module doc).
fn hits(w: &mut World, id: MobyId) {
    let st = w.m(id).state;
    if st == 0 || st == 9 { return; }
    let sc = w.class_scale(w.m(id).o_class).to_f32();
    w.mm(id).scale = sc;
    let mut hit = w.get_hit(id, 0x33_0000, false);
    if let Some(h) = hit {
        let own = w.m(id).o_class;
        if h.attacker.is_some_and(|a| w.table.mobys.get(a).is_some_and(|m| m.o_class == own || m.o_class == 0x370)) { hit = None; }
    }
    let r = damage::resolve(w, id, hit, o::D, 0, 4);
    if r.out5 != 1 && w.m(id).state != 9 {
        let health = c::pf(w, id, o::D) - r.damage;
        c::set_pf(w, id, o::D, health);
        let reaction = if health <= 0.0 { 1 } else { r.reaction };
        match reaction {
            1 | 2 => {
                w.mm(id).state = 9;
                w.mm(id).mode &= 0xefff;
                c::set_pu8(w, id, o::F + 7, 0xfa);
                set_death_bits(w, id, 0, -1);
            }
            3..=8 => {
                w.anim_blend(id, 2, 0, 5);
                w.mm(id).anim.speed = 1.75;
                c::set_pu8(w, id, o::F + 7, 0xfa);
            }
            9 | 10 => c::set_pu8(w, id, o::F + 7, 0xfa),
            _ => {}
        }
        flash::start(w, id, o::F);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, o::F);
}

/// `0x2b5dd8`: two flames 1.25 back, three cores 1 back (type 23).
fn jets(w: &mut World, id: MobyId) {
    let r0 = w.m(id).rows[0];
    let r0 = [r0[0], r0[1], r0[2], 0.0];
    let a = c::add(c::pos(w, id), c::scale(r0, -1.25));
    for _ in 0..2 {
        let k = w.rng.randi(0x10);
        let s = if w.rng.randi(2) == 0 { k } else { -k };
        let Some(i) = fx::part23_rec(w, [f32::from_bits(0x3e4c_cccd), 1.0, 0.75, 160_000.0], a, s, [0.0; 4], 0x7f30_30ff) else { continue };
        let t = w.ticks(6);
        if let Some(r) = fx::rec_mut(w, i) {
            rec::set_i16(r, 10, t as i16);
            rec::set_u32(r, 0x24, 2);
            r[0x2a] = 0x7f;
            r[0x2b] = r[10];
        }
    }
    let b = c::add(c::pos(w, id), c::scale(r0, -1.0));
    let (mut spin, mut life, mut size) = (0x10, w.ticks(2) as i16, 100_000.0f32);
    for _ in 0..3 {
        if let Some(i) = fx::part23_rec(w, [f32::from_bits(0x3d4c_cccd), 1.0, 1.0, size], b, spin, [0.0; 4], 0x7fff_ffff) {
            let rot = w.rng.randi(0xff) as u8;
            if let Some(r) = fx::rec_mut(w, i) {
                rec::set_i16(r, 10, life);
                r[8] = rot;
                rec::set_u32(r, 0x24, 2);
                r[0x2a] = 0x7f;
                r[0x2b] = r[10];
            }
        }
        spin = -spin;
        life <<= 1;
        size -= 20000.0;
    }
}

/// `0x2bb310(yaw, owner, at)`: the piece 81 (module doc).
pub fn make_piece(w: &mut World, yaw: f32, owner: MobyId, at: c::V) -> Option<MobyId> {
    let m = w.create_moby(0x51)?;
    story::pvars(w, m, 0x28);
    {
        let mm = w.mm(m);
        mm.position = at;
        mm.rotation[2] = yaw;
        mm.visible = 1;
        mm.state = 0;
        mm.draw_dist = 0xff;
        mm.update_dist = 0xff;
    }
    let op = c::pos(w, owner);
    let len = w.rng.randf(SPEED * 0.075, SPEED * 0.2);
    let d = c::set_len3([at[0] - op[0], at[1] - op[1], 0.0, 0.0], len);
    let ang = w.rng.randf_sym(0.0, f32::from_bits(0x3f49_0fdb));
    let v = rotate([d[0], d[1], d[2]], ang, [0.0, 0.0, -1.0]);
    let up = w.rng.randf(SPEED * 0.05, SPEED * 0.32);
    c::set_pv4(w, m, 0, [v[0], v[1], up, 0.0]);
    for o in [0x14, 0x18, 0x1c] {
        let r = w.rng.randf_sym(0.0, f32::from_bits(0x3d0e_fa35));
        c::set_pf(w, m, o, r);
    }
    let t = w.ticks(200);
    c::set_pi32(w, m, 0x20, owner as i32 + 1);
    c::set_pi16(w, m, 0x12, -1);
    c::set_pi16(w, m, 0x10, t as i16);
    w.build_matrix(m);
    Some(m)
}

/// `0x2bb4d8`: the light freed, gone.
fn piece_gone(w: &mut World, id: MobyId) {
    let l = c::pi16(w, id, 0x12);
    if l != -1 {
        w.svc.point_lights.free(l as u16 as usize);
        c::set_pi16(w, id, 0x12, -1);
    }
    w.delete_moby(id);
}

/// Level14 `0x2bac78`: 81 (module doc).
pub fn piece_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x28);
    if c::dec_timer_pvar_s16(w, id, 0x10) != 0 {
        piece_gone(w, id);
        return;
    }
    let mut vel = c::pv4(w, id, 0);
    let p = c::add(c::pos(w, id), vel);
    w.mm(id).position = [p[0], p[1], p[2], w.m(id).position[3]];
    let f = (0.97 - 1.0) * SPEED + 1.0;
    for v in vel.iter_mut().take(2) {
        if 0.05 * SPEED < v.abs() { *v *= f; }
    }
    if 0.0 < vel[2] { vel[2] *= f; }
    vel[2] -= 0.003;
    c::set_pv4(w, id, 0, vel);
    let pos = c::pos(w, id);
    if !pos[..3].iter().all(|&x| (2.0..=1021.0).contains(&x)) {
        piece_gone(w, id);
        return;
    }
    let h = super::hero_pos(w);
    let far = 10.0 <= (pos[0] - h[0]).abs() || 10.0 <= (pos[1] - h[1]).abs();
    if w.coll_sphere(sv::pv(pos), Pf::f(0.75), 0, None).is_none() && far {
        smoke(w, id);
        return;
    }
    let owner_class = link(w, c::pi32(w, id, 0x20)).map_or(-1, |m| w.m(m).o_class);
    w.play_sound_as(1, 0, id, owner_class);
    let b = fx::Beam { damage_r: 2.0, damage: 4.0, flash: 1.0, flash2: 0.5, flash_dist: 9.0, scale: 1.0, light: 5.0, streaks: 5, sparks: 2, puffs: 4, debris: 2, sound: -1, shake: false };
    fx::beam_explosion(w, &b, Some(id), pos);
    piece_gone(w, id);
}

/// 81's tumble, smoke and light (module doc).
fn smoke(w: &mut World, id: MobyId) {
    for k in 0..3 {
        let r = c::add_rot(w.m(id).rotation[k], c::pf(w, id, 0x14 + 4 * k));
        w.mm(id).rotation[k] = r;
    }
    let pos = c::pos(w, id);
    let rows = w.m(id).rows.map(|r| [r[0], r[1], r[2], 0.0]);
    let j0 = w.joint_point(id, 0);
    let a = w.rng.randf_sym(f32::from_bits(0x3c23_d70a), 0.5);
    let mut s = c::add(j0, c::scale(rows[0], a));
    let v = c::set_len3(c::sub(j0, pos), f32::from_bits(0x3d4c_cccd));
    let life = { let (lo, hi) = (w.ticks(10), w.ticks(0x14)); w.rng.rand_range(lo, hi) };
    fx::part21(w, f32::from_bits(0x471c_4000), s, v, 0x4f00_7fff, 0x1fff_ffff, life, 1);
    for k in 0..3 {
        if k != 2 {
            // The row-0 point is overwritten by the row-1 one (both draws made).
            w.rng.randf_sym(f32::from_bits(0x3c23_d70a), 0.5);
            let r = w.rng.randf_sym(f32::from_bits(0x3c23_d70a), 0.5);
            s = c::add(pos, c::scale(rows[1], r));
        }
        let mut spin = w.rng.randi(6);
        if w.rng.randi(2) != 0 { spin = -spin; }
        let size = w.rng.randf(f32::from_bits(0x469c_4000), 100_000.0);
        let g = w.rng.rand_range(0x10, 0xff) as u32;
        let Some(i) = fx::part23_rec(w, [f32::from_bits(0x3ccc_cccd), 1.0, 1.0, size], s, spin, [0.0; 4], g | g << 16 | g << 8 | 0x6000_0000) else { continue };
        let t = w.ticks(0x3c);
        if let Some(r) = fx::rec_mut(w, i) {
            r[3] = 0x44;
            rec::set_i16(r, 10, t as i16);
            r[0x2a] = 0x60;
            rec::set_u32(r, 0x24, 2);
            r[0x2b] = r[10];
        }
    }
    let d = c::scale(c::sub(j0, pos), 2.5);
    let side = if w.rng.randi(2) == 0 { rows[2] } else { c::scale(rows[2], -1.0) };
    let at = c::add(c::add(pos, d), side);
    let l = crate::point_lights::PointLight { color: [1.0, f32::from_bits(0x3f4c_cccd), f32::from_bits(0x3e4c_cccd)], intensity: 0.0, pos: [at[0], at[1], at[2]], radius: 3.0 };
    let slot = c::pi16(w, id, 0x12);
    if slot == -1 {
        let load = f32::from_bits(w.svc.frame_load[1].0);
        let got = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i16);
        c::set_pi16(w, id, 0x12, got);
    } else {
        w.svc.point_lights.set(slot as u16 as usize, l);
    }
}

/// Level14 `0x300e00`: 1193 (module doc).
pub fn grenade_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x30);
    let owner = link(w, c::pi32(w, id, 0x24));
    let pos = c::pos(w, id);
    if let Some(h) = w.coll_sphere(sv::pv(pos), Pf::f(f32::from_bits(0x3f26_6666)), 0, Some(id)) {
        if h.moby.is_some_and(|m| Some(m) != owner) { w.mm(id).state = 3; }
    }
    if c::dec_timer_pvar_i32(w, id, 0x20) != 0 { w.mm(id).state = 3; }
    match w.m(id).state {
        2 => {
            let f = c::pi32(w, id, 0x20) as f32 * c::pf(w, id, 0x2c);
            let (r, g, b) = (((67.0 - 255.0) * f + 255.0) as i32, ((230.0 - 0.0) * f + 0.0) as i32, ((132.0 - 0.0) * f + 0.0) as i32);
            w.mm(id).glow = (b as u32) << 16 | 0xff00_0000 | (g as u32) << 8 | r as u32;
            let s = (c::pf(w, id, 0x1c) * ((0.96 - 1.0) * SPEED + 1.0)).max(0.2 * DT);
            c::set_pf(w, id, 0x1c, s);
            let v = c::set_len3(c::pv4(w, id, 0x10), s);
            c::set_pv4(w, id, 0x10, [v[0], v[1], v[2], s]);
            let p = c::add(c::pos(w, id), [v[0], v[1], v[2], 0.0]);
            w.mm(id).position = p;
        }
        3 => {
            let p = c::pos(w, id);
            let b = fx::Beam { damage_r: 2.0, damage: 1.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: 0, shake: true };
            fx::beam_explosion(w, &b, Some(id), p);
            w.delete_moby(id);
            return;
        }
        0 | 1 => fly(w, id, owner),
        _ => {}
    }
    let st = w.m(id).state;
    let f = match st {
        0 | 1 => c::len2(c::pv4(w, id, 0x10)),
        2 => c::pf(w, id, 0x1c),
        _ => return,
    };
    let (lo, hi) = (DT * 0.2, DT * 6.0);
    let f = f.max(lo).min(hi);
    let r = c::add_rot(w.m(id).rotation[1], (((f - lo) / (hi - lo)) * 7.0 + 1.0) * 0.017_453_292);
    w.mm(id).rotation[1] = r;
}

/// 1193 flying and bouncing (module doc).
fn fly(w: &mut World, id: MobyId, owner: Option<MobyId>) {
    let pos = c::pos(w, id);
    let mut vel = c::pv4(w, id, 0x10);
    let cand = [pos[0] + vel[0], pos[1] + vel[1], pos[2] + vel[2], pos[3]];
    vel[2] -= vel[3];
    c::set_pf(w, id, 0x18, vel[2]);
    if w.m(id).state == 1 && vel[2] < 0.0 { w.mm(id).state = 0; }
    c::dec_timer_pvar_s16(w, id, 0x28);
    let smoke = c::pi16(w, id, 0x28);
    if smoke != 0 {
        let full = w.ticks(0xc) as f32;
        for i in 0..3 {
            let t = i as f32 * 0.3333;
            let p: c::V = std::array::from_fn(|k| pos[k] + (cand[k] - pos[k]) * t);
            let mut spin = w.rng.randi(6);
            if w.rng.randi(2) != 0 { spin = -spin; }
            let size = (smoke as f32 / full) * 45000.0 + 5000.0;
            let Some(r) = fx::part23_rec(w, [f32::from_bits(0x3ccc_cccd), 1.0, 1.0, size], p, spin, [0.0; 4], 0x8080_8080) else { continue };
            let tt = w.ticks(0x78);
            if let Some(rr) = fx::rec_mut(w, r) {
                rec::set_i16(rr, 10, tt as i16);
                rr[0x2a] = 0x7f;
                rec::set_u32(rr, 0x24, 2);
                rr[0x2b] = rr[10];
            }
        }
    }
    let next = match w.coll_line(sv::pv(pos), sv::pv(cand), 0, Some(id)) {
        Some(h) if h.moby.is_none() || h.moby != owner => [h.point[0], h.point[1], h.point[2], cand[3]],
        _ => cand,
    };
    w.mm(id).position = next;
    let r = f32::from_bits(0x3e54_fdf4);
    let Some(h) = w.coll_sphere(sv::pv(next), Pf::f(r), 0, Some(id)) else { return };
    if w.m(id).state != 0 && 0.0 <= next[2] - h.point[2] { return; }
    if h.moby.is_some() && h.moby == owner { return; }
    let n = c::set_len3([h.normal[0], h.normal[1], h.normal[2], 0.0], r);
    w.mm(id).position = [h.point[0] + n[0], h.point[1] + n[1], h.point[2] + n[2], next[3]];
    let v = c::pv4(w, id, 0x10);
    let rv = crate::hero::guns::reflect([v[0], v[1], v[2]], h.normal);
    let mut v = [rv[0], rv[1], rv[2], v[3]];
    w.play_sound(1, 0, id);
    if w.m(id).state == 0 {
        w.mm(id).state = 1;
        let b = c::pi16(w, id, 0x2a) + 1;
        c::set_pi16(w, id, 0x2a, b);
        let k = f32::from_bits(0x3eaa_a64c);
        let flat = if b == 1 {
            Some(6.0 * DT)
        } else {
            let t = c::scale(v, k);
            if c::len2(t) < 0.2 * DT { Some(0.2 * DT) } else {
                v = [t[0], t[1], t[2], v[3]];
                None
            }
        };
        if let Some(l) = flat {
            let s = c::set_len2(v, l);
            v = [s[0], s[1], s[2] * k, v[3]];
        }
    }
    c::set_pv4(w, id, 0x10, v);
    if 4 <= c::pi16(w, id, 0x2a) {
        w.mm(id).state = 2;
        let l = c::len2(v);
        let s = (l * f32::from_bits(0x3eaa_a64c)).max(0.2 * DT);
        let n = c::set_len3([v[0], v[1], 0.0, 0.0], s);
        c::set_pv4(w, id, 0x10, [n[0], n[1], n[2], s]);
        let life = c::pi32(w, id, 0x20) as f32;
        c::set_pf(w, id, 0x2c, 1.0 / life);
    }
}
