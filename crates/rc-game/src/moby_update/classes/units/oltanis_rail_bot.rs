//! **The rail bots, class 211** (level14 `0x2d5f40`; census U493, 3 placed). Robots that ride a grind rail (+0x60)
//! alongside Ratchet and throw arcs ahead of him (the arc slots, [`super::oltanis_arcs`]). Read from the level14
//! decomp and disassembly; native `f32`.
//!
//! * 0: the rail, the three-point marks path (+0x88) and the end cuboid (+0x1cc) set → the rail cursor and span
//!   (`0x2d6358`: mark B's and mark A's nearest segments on the rail, the chords between them), hidden without
//!   collision, the lights −1, the manipulators on lists 2, 3 and 4 → 1. Else deleted.
//! * 1: waits for the rail camera (camera class 20): its delay `ticks(+0x80)` → 2.
//! * 2: the delay out → shown, its collision, the hard cut 2 at half speed, `ticks(60)` → 3 at the start of its
//!   rise path (+0x84).
//! * 3 / 4: past key time 3 it slides from the rise path's start to the rail's (`0x2d64d0`, over `ticks(60)`);
//!   the animation wrapped → 4 (sequence 0 in 10, speed 1); the slide done → 5 (`ticks(120)`).
//! * 5: rides the rail at 12 a tick (`0x2d7198`).
//! * 6 (set by the race start `0x2d6570`, the intro camera's (class 20), with Ratchet put on the rail): the chase (`0x2d67e8`): it
//!   keeps 9 behind Ratchet's nearest point on the rail, closing to 3 ahead as he nears mark A, springing onto it
//!   (0.01 / 0.2) and facing along the rail; its command byte runs the attack: 5 waits +0x1c8 → 1; 1 the rest pose
//!   (the head's frame in joint 5's) and `ticks(140)` → 2; 2 the head turns to the point 1..14 ahead on the rail and
//!   the glow builds (a big spark 1000..120000 following +0xd0, a light 0.1..5 (1, 1, 2)) for `ticks(30)` → 3 (an arc
//!   slot, lights 3, sparks following); 3 the arc from joint 0 to that point (a line test, flags 9, that hits for 1);
//!   at the end → 4, the glow fades, → 0. Its head (list 2) turns to Ratchet within ±70°. In the end cuboid: the
//!   piece 325 thrown forward (0.4 a tick), a 403 riding the rail at 13, the explosion (sound 1), gone.
//! * Every tick after (states 2..6): the eye glow (joint list 1, 0.5 toward the camera), in 3..5 its light (radius
//!   6, (0.5, 0.5, 1), 1 above), its sparks (type 69), joint list 6's point and the draw (`0x2d71f0`: the eye 0.4
//!   white and the list-6 glow 0.2 red).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2d5f40` | 211 | [`update`] |
//! | `0x2d6358` / `0x2d64d0` / `0x2d7198` | rail setup, the slide, the ride | [`setup`], [`slide`], [`update`] |
//! | `0x2d67e8` | the chase | [`chase`] |
//! | `0x2d7490` / `0x2d71f0` | the tail and its draw | [`tail`], [`fx_quad_groups`] |
//! | `0x2d6570` | the race start (the bots' part; Ratchet's is the camera's: `crate::follow_camera::race`) | [`race_start`] |
//! | `0x315d48` (camera class 21) | the attack calls: the commands 1 / 5 and the waits +0x1c8 | [`bot_call`] |
//!
//! The draw is built from the camera the frame part saw (+0x210).

use super::oltanis_arcs::{self as arcs, billboard};
use super::oltanis_zapper::{make_325, make_403};
use super::FxQuads;
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx, DT, SPEED};
use crate::moby_update::services::{self as sv, pvar as p, HitTemplate, Services, World};
use crate::moby_update::{manip, story};
use crate::particles::rec;
use crate::point_lights::PointLight;
use crate::ps2v::Pf;
use crate::spline::{self, Cursor};

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2d_5f40;
pub const DRAW_FN: u32 = 0x2d_71f0;
pub const CLASSES: [i16; 1] = [211];

mod o {
    pub const RAIL: usize = 0x60;
    pub const SEG: usize = 0x64;
    pub const T: usize = 0x68;
    pub const TIMER: usize = 0x6c;
    pub const INV: usize = 0x70;
    pub const BEHIND: usize = 0x74;
    pub const SPAN: usize = 0x78;
    pub const DELAY: usize = 0x80;
    pub const RISE: usize = 0x84;
    pub const MARKS: usize = 0x88;
    pub const MARK_A: usize = 0x8c;
    pub const MARK_B: usize = 0x8e;
    pub const VEL: usize = 0x90;
    pub const J0: usize = 0xa0;
    pub const ARC_START: usize = 0xd0;
    pub const EYE: usize = 0xe0;
    pub const LIGHT: usize = 0xf0;
    pub const LIGHT2: usize = 0xf4;
    pub const N2: usize = 0x100;
    pub const N3: usize = 0x140;
    pub const N4: usize = 0x180;
    pub const ATK_T: usize = 0x1c0;
    pub const ATK_N: usize = 0x1c4;
    pub const REST_T: usize = 0x1c8;
    pub const CUBOID: usize = 0x1cc;
    pub const REST_Q: usize = 0x1d0;
    pub const AIM: usize = 0x1e0;
    pub const P6: usize = 0x200;
    pub const CAM: usize = 0x210;
    pub const SIZE: usize = 0x220;
}

/// Level14 gp words (0x161a2c..0x161a60).
const RISE_TICKS: i32 = 60;
const RIDE_SPEED: f32 = 12.0;
const RIDE_TICKS: i32 = 120;
const BEHIND0: f32 = 9.0;
const ATTACK_TICKS: i32 = 140;
const AHEAD: f32 = 14.0;
const LIMIT: f32 = 1.221_730_5;

fn rail(w: &World, id: MobyId) -> Option<Vec<[f32; 4]>> {
    let i = usize::try_from(c::pi32(w, id, o::RAIL)).ok()?;
    let g = w.svc.volumes.grind_paths.get(i)?;
    (!g.points.is_empty()).then(|| g.points.clone())
}
fn path_pts(w: &World, i: i32) -> Option<Vec<[f32; 4]>> {
    let s = w.svc.splines.get(usize::try_from(i).ok()?)?;
    Some(s.iter().map(|q| q.map(f32::from_bits)).collect())
}
fn nearest(pts: &[[f32; 4]], p: c::V) -> Cursor { spline::nearest(pts, false, 20.0, 5.0, 0.0, [p[0], p[1], p[2]]).map(|(_, k)| k).unwrap_or_default() }
fn chords(pts: &[[f32; 4]], from: i32, to: i32) -> f32 { (from.max(0)..to.max(0)).filter_map(|i| pts.get(i as usize)).map(|q| q[3]).sum() }

/// The race cameras' stores of the last camera update (`crate::follow_camera::race::RaceOut`), in order: Ratchet's
/// `HeroTeleport`, the race start, an attack call, Ratchet's head turn.
pub fn camera_stores(w: &mut World) {
    for out in std::mem::take(&mut w.svc.camera_race) {
        if let Some(t) = out.teleport { crate::cinematic::hero_teleport(w, t.pos, [0.0, 0.0, t.yaw], t.state, false); }
        if let Some(st) = out.start { race_start(w, st); }
        if let Some(cl) = out.call { bot_call(w, cl); }
        if let Some(hd) = out.head { w.hero_fields_mut().head_look = Some(hd); }
    }
}

/// `0x2d6570` (the intro camera's end): Ratchet on his rail at the start (the camera found the place), then the race
/// moby's group's first three members: state 6, facing from the race moby to Ratchet, 9 behind his cursor along their
/// rails, the chase spring's velocity cleared, +0xf8 = 2k, +0xfc = 2k + 1 (no reader), their light freed.
pub fn race_start(w: &mut World, st: crate::follow_camera::race::RaceStart) {
    if st.moby >= w.table.mobys.len() { return; }
    let (cur, hp) = match st.hero {
        Some((cur, place)) => {
            crate::cinematic::hero_teleport(w, place.pos, [0.0, 0.0, place.yaw], place.state, false);
            w.hero_fields_mut().rail_cursor = Some((cur.seg, cur.t));
            (cur, place.pos)
        }
        None => { let p = super::hero_pos(w); (w.hero.boots.cur, [p[0], p[1], p[2]]) }
    };
    let origin = c::pos(w, st.moby);
    let ids = crate::moby_update::scheduler::group_ids(w, w.m(st.moby).group);
    for (k, &id) in ids.iter().take(3).enumerate() {
        w.mm(id).state = 6;
        c::set_yaw(w, id, c::atan(hp[0] - origin[0], hp[1] - origin[1]));
        let mut at = cur;
        if let Some(r) = rail(w, id) {
            let (q, _) = spline::advance(&r, false, -BEHIND0, &mut at);
            let m = w.mm(id);
            m.position = [q[0], q[1], q[2], m.position[3]];
        }
        c::set_pi32(w, id, o::SEG, at.seg);
        c::set_pf(w, id, o::T, at.t);
        w.mm(id).pvars[o::VEL..o::VEL + 0x10].fill(0);
        c::set_pi32(w, id, 0xf8, 2 * k as i32);
        c::set_pi32(w, id, 0xfc, 2 * k as i32 + 1);
        let l = c::pi32(w, id, o::LIGHT);
        if l != -1 {
            if let Ok(i) = usize::try_from(l) { w.svc.point_lights.free(i); }
            c::set_pi32(w, id, o::LIGHT, -1);
        }
    }
}

/// An attack call of the race camera (`0x315d48`): the bot on Ratchet's rail attacks now (command 1); from the second
/// call another waits (command 5, +0x1c8 = `ticks(80)`), from the third the one behind it 45 and the last 110 ticks.
/// With B on his rail a coin (`0x260050(2)`) picks C or A to wait first.
pub fn bot_call(w: &mut World, cl: crate::follow_camera::race::BotCall) {
    let n = cl.calls;
    let first = if n == 3 { 0x2d } else { 0x50 };
    let cmd = |w: &mut World, k: usize, v: u8, rest: Option<i32>| {
        let Some(id) = cl.bots[k].filter(|&id| id < w.table.mobys.len()) else { return };
        w.mm(id).cmd = v;
        if let Some(t) = rest {
            let t = w.ticks(t);
            c::set_pi32(w, id, o::REST_T, t);
        }
    };
    let (on, a, b) = match cl.on {
        0 => (0, 1, 2),
        2 => (2, 1, 0),
        _ if n > 0 && w.rng.randi(2) == 0 => (1, 2, 0),
        _ => (1, 0, 2),
    };
    cmd(w, on, 1, None);
    if n <= 0 { return; }
    if n != 2 {
        cmd(w, a, 5, Some(first));
    } else {
        cmd(w, a, 5, Some(0x2d));
        cmd(w, b, 5, Some(0x6e));
    }
}

/// `0x2d6358`: the cursor 0, the marks' segments on the rail, the span between them.
pub fn setup(w: &mut World, id: MobyId) {
    c::set_pf(w, id, o::BEHIND, BEHIND0);
    c::set_pf(w, id, o::T, 0.0);
    c::set_pi32(w, id, o::SEG, 0);
    let (Some(r), Some(marks)) = (rail(w, id), path_pts(w, c::pi32(w, id, o::MARKS))) else { return };
    let a = nearest(&r, marks.get(2).copied().unwrap_or([0.0; 4])).seg;
    c::set_pi16(w, id, o::MARK_A, a as i16);
    let b = nearest(&r, marks.get(1).copied().unwrap_or([0.0; 4])).seg;
    c::set_pi16(w, id, o::MARK_B, b as i16);
    c::set_pf(w, id, o::SPAN, if b < a { chords(&r, b, a) } else { 0.0 });
}

/// `0x2d64d0`: the slide from the rise path's start to the rail's; whether its timer ran out.
pub fn slide(w: &mut World, id: MobyId) -> bool {
    let out = c::dec_timer_pvar_i32(w, id, o::TIMER) != 0;
    let f = c::pi32(w, id, o::TIMER) as f32 * c::pf(w, id, o::INV);
    let a = rail(w, id).and_then(|r| r.first().copied()).unwrap_or([0.0; 4]);
    let b = path_pts(w, c::pi32(w, id, o::RISE)).and_then(|r| r.first().copied()).unwrap_or([0.0; 4]);
    w.mm(id).position = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f);
    out
}

fn show(w: &mut World, id: MobyId) {
    let coll = super::class_collision(w, w.m(id).o_class);
    let m = w.mm(id);
    m.mode = m.mode & 0xffbe | 0x1000;
    m.has_collision = coll;
}

/// Level14 `0x2d5f40` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    match w.m(id).state {
        0 => {
            let ok = rail(w, id).is_some()
                && path_pts(w, c::pi32(w, id, o::MARKS)).is_some_and(|p| p.len() == 3)
                && c::pi32(w, id, o::CUBOID) != -1;
            if !ok {
                w.delete_moby(id);
                return;
            }
            setup(w, id);
            let m = w.mm(id);
            m.state = 1;
            m.has_collision = false;
            m.mode = m.mode & 0xefff | 0x41;
            c::set_pi32(w, id, o::LIGHT, -1);
            c::set_pi32(w, id, o::LIGHT2, -1);
            for (ofs, list) in [(o::N2, 2), (o::N3, 3), (o::N4, 4)] {
                w.mm(id).pvars[ofs..ofs + 0x40].fill(0);
                manip::attach(w, id, list, id, ofs);
            }
            return;
        }
        1 => {
            if w.camera_class != 0x14 { return; }
            let t = w.ticks(c::pi32(w, id, o::DELAY));
            c::set_pi32(w, id, o::TIMER, t);
            w.mm(id).state = 2;
            return;
        }
        2 => {
            if c::dec_timer_pvar_i32(w, id, o::TIMER) == 0 { return; }
            show(w, id);
            w.mm(id).state = 3;
            c::hard_cut(w, id, 2, 0);
            w.mm(id).anim.speed = 0.5;
            let t = w.ticks(RISE_TICKS);
            c::set_pi32(w, id, o::TIMER, t);
            c::set_pf(w, id, o::INV, 1.0 / t as f32);
            if let Some(p0) = path_pts(w, c::pi32(w, id, o::RISE)).and_then(|p| p.first().copied()) { w.mm(id).position = p0; }
            rise(w, id);
        }
        3 => rise(w, id),
        4 => {
            if slide(w, id) {
                w.mm(id).state = 5;
                let t = w.ticks(RIDE_TICKS);
                c::set_pi32(w, id, o::TIMER, t);
            }
        }
        5 => {
            // 0x2d7198.
            if let Some(r) = rail(w, id) {
                let mut cur = Cursor { seg: c::pi32(w, id, o::SEG), t: c::pf(w, id, o::T) };
                let (q, _) = spline::advance(&r, false, RIDE_SPEED * DT, &mut cur);
                c::set_pi32(w, id, o::SEG, cur.seg);
                c::set_pf(w, id, o::T, cur.t);
                let m = w.mm(id);
                m.position = [q[0], q[1], q[2], m.position[3]];
            }
        }
        6 => {
            chase(w, id);
            let p = c::pos(w, id);
            if w.in_cuboid([p[0], p[1], p[2]], c::pi32(w, id, o::CUBOID)) {
                let jp = w.joint_point(id, 2);
                if let Some(piece) = make_325(w, id, jp) {
                    let y = c::yaw(w, id);
                    c::set_pf(w, piece, 0, y.cos() * SPEED * 0.4);
                    c::set_pf(w, piece, 4, y.sin() * SPEED * 0.4);
                }
                make_403(w, id, 13.0, 0);
                let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 5, sound: 1, shake: true };
                fx::beam_explosion(w, &b, Some(id), p);
                w.delete_moby(id);
                return;
            }
        }
        _ => return,
    }
    tail(w, id);
}

/// States 2 (its last tick) and 3 (module doc).
fn rise(w: &mut World, id: MobyId) {
    if 3.0 <= c::ground::key_time(w, id) { slide(w, id); }
    let a = &w.m(id).anim;
    if a.flags & 2 != 0 && a.seq_a == a.seq_b {
        w.mm(id).state = 4;
        w.anim_blend(id, 0, 0, 10);
        w.mm(id).anim.speed = 1.0;
    }
}

fn rows3(m: [[f32; 4]; 4]) -> [[f32; 3]; 3] { [0, 1, 2].map(|k| { let r = m[k]; let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt(); if n == 0.0 { [0.0; 3] } else { [r[0] / n, r[1] / n, r[2] / n] } }) }
/// `fun_001fa378(out, Jᵀ, F)` then `fun_00214260`: the frame `f`'s rows in joint 5's (normalised) axes as a quaternion.
fn in_joint(w: &World, id: MobyId, f: [[f32; 3]; 3]) -> [f32; 4] {
    let jr = rows3(w.joint_matrix(id, 5));
    let l: [[f32; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|k| f[i][0] * jr[k][0] + f[i][1] * jr[k][1] + f[i][2] * jr[k][2]));
    crate::hero::metal_detector::quat_of(l)
}

/// `FastVecCross(out, a, b)` = b × a.
fn fast_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [b[1] * a[2] - b[2] * a[1], b[2] * a[0] - b[0] * a[2], b[0] * a[1] - b[1] * a[0]] }
fn unit(v: [f32; 3]) -> [f32; 3] { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if n == 0.0 { [0.0; 3] } else { v.map(|x| x / n) } }

/// `0x2d67e8` (module doc).
pub fn chase(w: &mut World, id: MobyId) {
    let Some(r) = rail(w, id) else { return };
    let (b, seg) = (c::pi16(w, id, o::MARK_B) as i32, c::pi32(w, id, o::SEG));
    if b <= seg {
        let d = chords(&r, b, seg) + c::pf(w, id, o::T);
        let f = (d / c::pf(w, id, o::SPAN)).min(1.0);
        c::set_pf(w, id, o::BEHIND, BEHIND0 + (-3.0 - BEHIND0) * f);
    }
    let mut cur = nearest(&r, super::hero_pos(w));
    let (q, _) = spline::advance(&r, false, -c::pf(w, id, o::BEHIND), &mut cur);
    c::set_pi32(w, id, o::SEG, cur.seg);
    c::set_pf(w, id, o::T, cur.t);
    for (a, &t) in q.iter().enumerate() {
        let (mut x, mut v) = (Pf::f(w.m(id).position[a]), Pf::f(c::pf(w, id, o::VEL + 4 * a)));
        crate::hero::physics::spring(Pf::f(t), Pf::f(0.01), Pf::f(0.2), Pf::ZERO, &mut x, &mut v);
        w.mm(id).position[a] = x.to_f32();
        c::set_pf(w, id, o::VEL + 4 * a, v.to_f32());
    }
    let (p0, p5) = (r[(cur.seg.max(0) as usize).min(r.len() - 1)], r[((cur.seg.max(0) + 5) as usize).min(r.len() - 1)]);
    c::set_yaw(w, id, c::atan(p5[0] - p0[0], p5[1] - p0[1]));
    if w.m(id).cmd == 5 && c::dec_timer_pvar_i32(w, id, o::REST_T) != 0 { w.mm(id).cmd = 1; }
    if w.m(id).cmd == 1 {
        let t = w.ticks(ATTACK_TICKS);
        c::set_pi32(w, id, o::ATK_T, t);
        c::set_pi32(w, id, o::ATK_N, 0);
        w.mm(id).cmd = 2;
        let j4 = w.joint_matrix(id, 4);
        let q = in_joint(w, id, [[j4[0][0], j4[0][1], j4[0][2]], [j4[1][0], j4[1][1], j4[1][2]], [j4[2][0], j4[2][1], j4[2][2]]]);
        c::set_pv4(w, id, o::REST_Q, q);
    }
    let mut toward = c::yaw(w, id);
    let cmd = w.m(id).cmd;
    if (2..=4).contains(&cmd) {
        let j0 = w.joint_matrix(id, 0);
        for (k, row) in j0.iter().enumerate() { c::set_pv4(w, id, o::J0 + 0x10 * k, *row); }
        let k = c::pi32(w, id, o::ATK_T) as f32 / w.ticks(ATTACK_TICKS) as f32;
        let mut ahead = Cursor { seg: c::pi32(w, id, o::SEG), t: c::pf(w, id, o::T) };
        let (aim, _) = spline::advance(&r, false, AHEAD + (1.0 - AHEAD) * k, &mut ahead);
        let aim = [aim[0], aim[1], aim[2], 1.0];
        c::set_pv4(w, id, o::AIM, aim);
        let d = c::sub(aim, c::pos(w, id));
        toward = c::atan(d[0], d[1]);
        let mut turn = c::sub_rot(toward, c::yaw(w, id)).clamp(-LIMIT, LIMIT);
        let n = c::pi32(w, id, o::ATK_N) + 1;
        c::set_pi32(w, id, o::ATK_N, n);
        if w.ticks(0x1e) < n {
            if w.m(id).cmd == 4 {
                w.mm(id).cmd = 0;
            } else {
                arcs::register(w, id, 3, 1);
                w.mm(id).cmd = 3;
            }
            let t = w.ticks(0x1e);
            c::set_pi32(w, id, o::ATK_N, t);
        }
        let mut fr = 1.0;
        let cmd = w.m(id).cmd;
        if cmd == 2 || cmd == 4 {
            fr = c::pi32(w, id, o::ATK_N) as f32 / w.ticks(0x1e) as f32;
            if cmd == 4 { fr = 1.0 - fr; }
            turn = (turn - 0.0) * fr + 0.0;
        }
        manip::set_axis(w, id, id, o::N3, turn, 2);
        // The head frame along d (row 1), level sideways (row 2), the third (row 0); in joint 5's axes.
        let r1 = unit([d[0], d[1], d[2]]);
        let r2 = unit(fast_cross(r1, [0.0, 0.0, 1.0]));
        let r0 = fast_cross(r2, r1);
        let q = in_joint(w, id, [r0, r1, r2]);
        let rest = c::pv4(w, id, o::REST_Q);
        let q = if cmd == 2 || cmd == 4 { sv::fv(sv::quat_nlerp(Pf::f(fr), sv::pv(rest), sv::pv(q))) } else { q };
        let lift = 512.0 / w.m(id).scale;
        {
            let pv = &mut w.mm(id).pvars;
            p::set_v4f(pv, o::N4 + manip::rec::QUAT, q);
            pv[o::N4 + manip::rec::MODE] = 1;
            p::set_ff(pv, o::N4 + manip::rec::WEIGHT, 1.0);
            p::set_v4f(pv, o::N4 + manip::rec::SCALE, [1.0; 4]);
            p::set_v4f(pv, o::N4 + manip::rec::TRANS, [0.0, lift, 0.0, 1.0]);
        }
        manip::sync(w, id, id, o::N4);
        if c::dec_timer_pvar_i32(w, id, o::ATK_T) != 0 {
            w.mm(id).cmd = 4;
            c::set_pi32(w, id, o::ATK_N, 0);
        }
        let cmd = w.m(id).cmd;
        let start = c::pv4(w, id, o::ARC_START);
        if cmd == 3 {
            let frame = [0, 1, 2, 3].map(|k| c::pv4(w, id, o::J0 + 0x10 * k));
            if !arcs::aim(w, id, frame, aim) { arcs::register(w, id, 3, 1); }
            let y = c::yaw(w, id);
            let tmpl = HitTemplate { dir: sv::pv([y.cos(), y.sin(), 1.0, f32::from_bits(0x45af_df66)]), attacker: Some(id), flags: 0x1_0001, b18: 0, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 };
            sv::line_hit_in(w.table, w.svc, w.classes, w.coll, sv::pv(start), sv::pv(aim), 9, Some(id), &tmpl);
            head(w, id, toward);
            return;
        }
        if cmd == 2 || cmd == 4 {
            let row0 = c::pv4(w, id, o::J0);
            let at = c::add(c::scale(row0, 0.3), start);
            if let Some(i) = fx::part69(w, at, [0.0; 4], 0x7f, Some(id)) {
                let t = w.ticks(4);
                if let Some(rr) = fx::rec_mut(w, i) {
                    rec::set_ff(rr, 0xc, (120_000.0 - 1000.0) * fr + 1000.0);
                    rec::set_i16(rr, 0x36, 1);
                    rec::set_i16(rr, 0x34, (fr * 111.0 + 16.0) as i16);
                    rec::set_i16(rr, 10, t as i16);
                    rec::set_u32(rr, 0x38, 0x7f7f7f);
                    rec::set_ff(rr, 0x30, 1.0 / t as f32);
                }
            }
            let radius = (5.0 - 0.1) * fr + 0.1;
            let l = PointLight { color: [1.0, 1.0, 2.0], intensity: 0.0, pos: [start[0], start[1], start[2] - 0.5], radius };
            light(w, id, l);
            head(w, id, toward);
            return;
        }
    }
    let s = c::pi32(w, id, o::LIGHT);
    if s != -1 {
        if let Ok(i) = usize::try_from(s) { w.svc.point_lights.free(i); }
        c::set_pi32(w, id, o::LIGHT, -1);
    }
    head(w, id, toward);
}

/// The light +0xf0: taken once, then moved.
fn light(w: &mut World, id: MobyId, l: PointLight) {
    let s = c::pi32(w, id, o::LIGHT);
    if s == -1 {
        let load = f32::from_bits(w.svc.frame_load[1].0);
        let got = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i32);
        c::set_pi32(w, id, o::LIGHT, got);
    } else if let Ok(i) = usize::try_from(s) {
        w.svc.point_lights.set(i, l);
    }
}

/// The head (list 2) toward Ratchet within ±70° of `toward`.
fn head(w: &mut World, id: MobyId, toward: f32) {
    let (h, p) = (super::hero_pos(w), c::pos(w, id));
    let f = c::sub_rot(c::atan(h[0] - p[0], h[1] - p[1]), toward).clamp(-LIMIT, LIMIT);
    manip::set_axis(w, id, id, o::N2, f, 2);
}

/// `0x2d7490` (module doc).
pub fn tail(w: &mut World, id: MobyId) {
    let cam = w.camera.map(|x| x.to_f32());
    let p1 = w.joint_point(id, 1);
    c::set_pv4(w, id, o::EYE, c::add(p1, c::set_len3(c::sub(p1, cam), -0.5)));
    if (3..=5).contains(&w.m(id).state) {
        let l = PointLight { color: [0.5, 0.5, 1.0], intensity: 0.0, pos: [p1[0], p1[1], p1[2] + 1.0], radius: 6.0 };
        light(w, id, l);
    }
    let v = fx::rand_vec_ab(w, f32::from_bits(0x3ba3_d70a), f32::from_bits(0x3cf5_c28f));
    if let Some(i) = fx::part69(w, p1, v, 0x7f, Some(id)) {
        let s = w.rng.randf(f32::from_bits(0x45bb_8000), 32000.0);
        if let Some(r) = fx::rec_mut(w, i) { rec::set_ff(r, 0xc, s); }
    }
    for k in 0..3 {
        let Some(i) = fx::part69(w, p1, [0.0; 4], 0x7f, Some(id)) else { continue };
        let s = if k == 2 && w.rng.randi(8) == 0 { f32::from_bits(0x482f_c800) } else { w.rng.randf(f32::from_bits(0x479c_4000), f32::from_bits(0x47ea_6000)) };
        let t = w.ticks(2);
        if let Some(r) = fx::rec_mut(w, i) {
            rec::set_ff(r, 0xc, s);
            rec::set_i16(r, 10, t as i16);
            rec::set_i16(r, 0x36, 3);
            rec::set_u32(r, 0x38, 0x7f7f7f);
            rec::set_ff(r, 0x30, 1.0 / t as f32);
        }
    }
    let p6 = w.joint_point(id, 6);
    c::set_pv4(w, id, o::P6, p6);
    if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
        w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
        w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
    }
}

/// The draw's frame part: the camera kept.
pub fn frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    let cam = crate::hero::physics::to_f32x3(w.camera);
    c::set_pv4(w, id, o::CAM, [cam[0], cam[1], cam[2], 1.0]);
}

/// Level14 `0x2d71f0` (module doc).
pub fn fx_quad_groups(table: &MobyTable, _svc: &Services, id: MobyId) -> Vec<FxQuads> {
    let Some(m) = table.mobys.get(id).filter(|m| m.pvars.len() >= o::SIZE) else { return Vec::new() };
    let v = |off: usize| -> [f32; 3] { [p::ff(&m.pvars, off), p::ff(&m.pvars, off + 4), p::ff(&m.pvars, off + 8)] };
    let cam = v(o::CAM);
    let quads = vec![billboard(cam, v(o::EYE), 0.4, [0.0; 3], 0x80ff_ffff), billboard(cam, v(o::P6), 0.2, [0.0; 3], 0x2020_20ff)];
    vec![FxQuads { fx: 0xb, additive: true, subtract: false, quads }]
}
