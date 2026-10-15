//! **Oltanis's ride carts, class 685** (level14 `0x2ee8d8`; census U502, one placed). A cart that waits at an end of
//! its path A (+0xa0) or B (+0xb8) and runs along it while Ratchet rides it, keeping an 11-point wall around itself
//! (the path +0x114, Ratchet's wall spline 0x14162a) and its riders. Read from the level14 decomp; native `f32`.
//!
//! * 0: the paths checked (A not empty, the wall path of 11 points; else deleted); each path's chords and length (the
//!   last point's chord back to the first); at A's start with its start rotation (+0xd0), direction −1; the way on
//!   to B (+0x124) once its spawn is marked dead and the mission +0x110 is done; dim (0x80303030).
//! * 1 (waiting): its glow pulses while Ratchet is within 40 (x and y); his cuboids put it at A's start (+0x118), A's
//!   end (+0x11c) or B's end (+0x120); Ratchet standing on it within 1 (xy): sound 1 (at the stop volume), it
//!   starts (from A's end back to A's start; from the start along A, or B with the way on), 2.
//! * 2 (running, `0x2ef150`): along the path at 4·dt a tick (springs 0.003 / 0.2), its pitch turning in the second
//!   half and its roll and yaw over the run toward the end rotation (+0xe0 / +0xf0) (the reverse way: from it); its
//!   hum (sound 0, flags 4); brightening to 0x80808040; the wall (`0x2ef630`) and its riders; at the end → 3.
//! * 3: settling (springs onto the end, the hum's volume easing to a quarter); stopped (speed under 0.001): the hum
//!   off, sound 1 at a third, 4. 4: once Ratchet leaves it → 1.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ee8d8` | 685 | [`update`] |
//! | `0x2eef60` / `0x2ef150` / `0x2ef3e0` / `0x2ef630` / `0x2ef6f0` | setup, run, settle, wall, glow | [`update`] |
//!
//! [L] The volumes it keeps are read off voice slots 0 and 1 (0x13e5d0 / 0x13e640) when it first plays: taken as full
//! (0x400).

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::{story, triggers};
use crate::moby_update::services::World;
use crate::ps2v::Pf;
use crate::spline::{advance, Cursor};

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2e_e8d8;
pub const CLASSES: [i16; 1] = [685];

mod o {
    pub const RIDERS: usize = 0x60;
    pub const PATH_A: usize = 0xa0;
    pub const SEG: usize = 0xa4;
    pub const T: usize = 0xa8;
    pub const DIR: usize = 0xac;
    pub const RUN: usize = 0xb0;
    pub const LEN_A: usize = 0xb4;
    pub const PATH_B: usize = 0xb8;
    pub const LEN_B: usize = 0xbc;
    pub const VEL: usize = 0xc0;
    pub const ROT0: usize = 0xd0;
    pub const ROT_A: usize = 0xe0;
    pub const ROT_B: usize = 0xf0;
    pub const ROT_V: usize = 0x100;
    pub const MISSION: usize = 0x110;
    pub const WALL: usize = 0x114;
    pub const TO_START: usize = 0x118;
    pub const TO_END: usize = 0x11c;
    pub const TO_B: usize = 0x120;
    pub const WAY: usize = 0x124;
    pub const VOL: usize = 0x126;
    pub const VOL_V: usize = 0x128;
    pub const VOICE: usize = 0x12c;
    pub const VOL_HUM: usize = 0x130;
    pub const VOL_STOP: usize = 0x134;
    pub const GLOW: usize = 0x138;
    pub const SIZE: usize = 0x140;
}
const SPEED: f32 = 4.0;
const FULL: i32 = 0x400;
/// Level14 0x1e03b0: the wall's 11 points around the cart.
const WALL: [[f32; 2]; 11] = [
    [-1.886368, 1.962136], [1.886367, 1.962136], [2.464658, 1.005283], [2.667728, -0.004372], [2.464659, -1.014029], [1.886368, -1.962136],
    [-1.886369, -1.962136], [-2.46466, -1.014028], [-2.667728, -0.004372], [-2.464659, 1.005284], [-1.886368, 1.962136],
];

fn path(w: &World, i: i32) -> Option<(usize, Vec<[f32; 4]>)> {
    let k = usize::try_from(i).ok()?;
    let s = w.svc.splines.get(k)?;
    Some((k, s.iter().map(|q| q.map(f32::from_bits)).collect()))
}
fn on_path(w: &World, id: MobyId) -> Option<(usize, Vec<[f32; 4]>)> { path(w, c::pi32(w, id, if w.m(id).cmd == 0 { o::PATH_A } else { o::PATH_B })) }
fn riding(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) }
fn release(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, o::VOICE);
    if v != -1 && w.sound_owner(v) == Some(id) { w.release_sound(v, id); }
    c::set_pi32(w, id, o::VOICE, -1);
}
fn spring(w: &mut World, id: MobyId, target: f32, x: f32, v_off: usize) -> f32 {
    let (mut x, mut v) = (Pf::f(x), Pf::f(c::pf(w, id, v_off)));
    crate::hero::physics::spring(Pf::f(target), Pf::b(0x3b44_9ba6), Pf::b(0x3e4c_cccd), Pf::ZERO, &mut x, &mut v);
    c::set_pf(w, id, v_off, v.to_f32());
    x.to_f32()
}
/// The chords as a loop and the total length (`0x2eef60`).
fn chords(w: &mut World, i: i32) -> f32 {
    let Some((k, pts)) = path(w, i) else { return 0.0 };
    let n = pts.len();
    let mut total = 0.0;
    for j in 0..n.saturating_sub(1) {
        let d = c::dist3(pts[j], pts[j + 1]);
        w.svc.splines[k][j][3] = d.to_bits();
        total += d;
    }
    if 0 < n { w.svc.splines[k][n - 1][3] = c::dist3(pts[n - 1], pts[0]).to_bits(); }
    total
}

/// Level14 `0x2ee8d8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    if w.m(id).state != 0 && w.m(id).cmd == 2 && c::pf(w, id, o::DIR) < 0.0 {
        if let Some((_, b)) = path(w, c::pi32(w, id, o::PATH_B)) {
            c::set_pf(w, id, o::DIR, -c::pf(w, id, o::DIR));
            let k = b.len() as i32 - 2;
            c::set_pi32(w, id, o::SEG, k);
            c::set_pf(w, id, o::T, b.get(k.max(0) as usize).map_or(0.0, |q| q[3]));
            w.mm(id).cmd = 1;
        }
    }
    let (old, old_rot) = (c::pos(w, id), w.m(id).rotation);
    match w.m(id).state {
        0 => {
            w.mm(id).cmd = 0;
            w.mm(id).state = 1;
            let ok = path(w, c::pi32(w, id, o::PATH_A)).is_some_and(|(_, p)| !p.is_empty()) && path(w, c::pi32(w, id, o::WALL)).is_some_and(|(_, p)| p.len() == 0xb);
            if !ok {
                w.delete_moby(id);
                return;
            }
            let la = chords(w, c::pi32(w, id, o::PATH_A));
            c::set_pf(w, id, o::LEN_A, la);
            let lb = chords(w, c::pi32(w, id, o::PATH_B));
            c::set_pf(w, id, o::LEN_B, lb);
            c::set_pf(w, id, o::DIR, -1.0);
            c::set_pi32(w, id, o::SEG, 0);
            c::set_pf(w, id, o::T, 0.0);
            if let Some((_, a)) = path(w, c::pi32(w, id, o::PATH_A)) { w.mm(id).position = a[0]; }
            w.mm(id).rotation = c::pv4(w, id, o::ROT0);
            let sid = w.m(id).spawn_id;
            let collected = w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0);
            let dead = w.svc.save.death.contains(&(w.svc.level, sid));
            let way = !collected && dead && story::mission_done(w, c::pi32(w, id, o::MISSION));
            c::set_pi16(w, id, o::WAY, way as i16);
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.glow = 0x8030_3030;
            for k in [o::VOL_STOP, o::VOICE, o::VOL_HUM] { c::set_pi32(w, id, k, -1); }
            c::set_pf(w, id, o::GLOW, 0.0);
            return;
        }
        1 => {
            glow(w, id);
            let h = w.hero_point();
            if c::pi32(w, id, o::TO_START) >= 0 && w.in_cuboid(h, c::pi32(w, id, o::TO_START)) {
                w.mm(id).cmd = 0;
                c::set_pf(w, id, o::DIR, -1.0);
                c::set_pi32(w, id, o::SEG, 0);
                c::set_pf(w, id, o::T, 0.0);
            }
            for (cub, cmd, p) in [(o::TO_END, 0u8, o::PATH_A), (o::TO_B, 1, o::PATH_B)] {
                if c::pi32(w, id, cub) >= 0 && w.in_cuboid(h, c::pi32(w, id, cub)) {
                    w.mm(id).cmd = cmd;
                    if let Some((_, q)) = path(w, c::pi32(w, id, p)) {
                        let k = q.len() as i32 - 2;
                        c::set_pi32(w, id, o::SEG, k);
                        c::set_pf(w, id, o::T, q.get(k.max(0) as usize).map_or(0.0, |x| x[3]));
                    }
                    c::set_pf(w, id, o::DIR, 1.0);
                }
            }
            let hp = super::hero_pos(w);
            if riding(w, id) && w.hero.f65c == 0 && c::dist2(hp, c::pos(w, id)) < 1.0 {
                let v = w.play_sound(1, 0, id);
                if 0 <= v {
                    if c::pi32(w, id, o::VOL_STOP) < 0 { c::set_pi32(w, id, o::VOL_STOP, FULL); }
                    w.set_volume(v, c::pi32(w, id, o::VOL_STOP));
                }
                if c::pf(w, id, o::DIR) == -1.0 { w.mm(id).cmd = if c::pi16(w, id, o::WAY) == 0 { 0 } else { 2 }; }
                w.mm(id).state = 2;
                c::set_pf(w, id, o::RUN, 0.0);
                c::set_pf(w, id, o::DIR, -c::pf(w, id, o::DIR));
            }
        }
        2 => {
            if !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
                let v = w.play_sound(0, 4, id);
                c::set_pi32(w, id, o::VOICE, v);
                if v != 0 {
                    if c::pi32(w, id, o::VOL_HUM) < 0 { c::set_pi32(w, id, o::VOL_HUM, FULL); }
                    w.set_volume(v, c::pi32(w, id, o::VOL_HUM));
                }
            }
            let g = crate::hud::tween_color(0.1, w.m(id).glow, 0x8080_8040);
            w.mm(id).glow = g;
            if run(w, id) {
                w.mm(id).state = 3;
                c::set_pi16(w, id, o::VOL, c::pi32(w, id, o::VOL_HUM) as i16);
            }
            wall(w, id);
            carry(w, id, old, old_rot);
        }
        3 => {
            if 0.001 <= c::len3(c::pv4(w, id, o::VEL)) {
                if !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
                    let v = w.play_sound(0, 4, id);
                    c::set_pi32(w, id, o::VOICE, v);
                }
                let target = c::pi32(w, id, o::VOL_HUM) as f32 * 0.25;
                let vol = spring(w, id, target, c::pi16(w, id, o::VOL) as f32, o::VOL_V) as i16;
                c::set_pi16(w, id, o::VOL, vol);
                let voice = c::pi32(w, id, o::VOICE);
                if -1 < voice { w.set_volume(voice, vol as i32); }
                wall(w, id);
            } else {
                glow(w, id);
                release(w, id);
                let v = w.play_sound(1, 0, id);
                if -1 < v { w.set_volume(v, c::pi32(w, id, o::VOL_STOP) / 3); }
                w.mm(id).state = 4;
                if !riding(w, id) { w.mm(id).state = 1; }
            }
        }
        4 => {
            glow(w, id);
            if !riding(w, id) { w.mm(id).state = 1; }
        }
        _ => {}
    }
    let st = w.m(id).state;
    if st != 2 && st != 0 {
        settle(w, id);
        if riding(w, id) { carry(w, id, old, old_rot); }
    }
}

fn carry(w: &mut World, id: MobyId, old: [f32; 4], old_rot: [f32; 4]) {
    let pos = c::pos(w, id);
    let delta = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3]];
    let rot = w.m(id).rotation;
    triggers::carry_riders(&mut w.mm(id).pvars, o::RIDERS, delta, old_rot, rot);
}

/// `0x2ef6f0`: the glow pulses (`(4·sin − 3)·128`, 0x20..0x80) within 40 of Ratchet, else dims; eased by 0.1.
fn glow(w: &mut World, id: MobyId) {
    let (p, h) = (c::pos(w, id), super::hero_pos(w));
    let target = if (p[0] - h[0]).abs() <= 40.0 && (p[1] - h[1]).abs() <= 40.0 {
        let a = c::add_rot(c::pf(w, id, o::GLOW), DT * std::f32::consts::TAU);
        c::set_pf(w, id, o::GLOW, a);
        let g = (((a.sin() * 4.0 - 3.0) * 128.0) as i32).clamp(0x20, 0x80) as u32;
        g << 16 | g << 8 | 0x8000_0000 | g
    } else {
        0x8030_3030
    };
    let g = crate::hud::tween_color(0.1, w.m(id).glow, target);
    w.mm(id).glow = g;
}

/// `0x2ef150`: along the path (module doc); whether its end was reached.
fn run(w: &mut World, id: MobyId) -> bool {
    let Some((_, pts)) = on_path(w, id) else { return false };
    let (total, end) = if w.m(id).cmd == 0 { (c::pf(w, id, o::LEN_A), c::pv4(w, id, o::ROT_A)) } else { (c::pf(w, id, o::LEN_B), c::pv4(w, id, o::ROT_B)) };
    let step = SPEED * DT;
    let dir = c::pf(w, id, o::DIR);
    let mut cur = Cursor { seg: c::pi32(w, id, o::SEG), t: c::pf(w, id, o::T) };
    let (q, ended) = advance(&pts, false, step * dir, &mut cur);
    c::set_pi32(w, id, o::SEG, cur.seg);
    c::set_pf(w, id, o::T, cur.t);
    for (k, &t) in q.iter().enumerate() {
        let x = spring(w, id, t, w.m(id).position[k], o::VEL + 4 * k);
        w.mm(id).position[k] = x;
    }
    let ran = c::pf(w, id, o::RUN) + step;
    c::set_pf(w, id, o::RUN, ran);
    let mut f = (ran / total).min(1.0);
    if ended { f = 1.0; }
    if dir < 0.0 { f = 1.0 - f; }
    let g = if 0.0 < dir {
        if 0.5 < f { (f - 0.5) + (f - 0.5) } else { 0.0 }
    } else if f <= 0.5 {
        f + f
    } else {
        1.0
    }
    .min(1.0);
    let r0 = c::pv4(w, id, o::ROT0);
    let m = w.mm(id);
    m.rotation[0] = c::lerp_rot(r0[0], end[0], g);
    m.rotation[1] = c::lerp_rot(r0[1], end[1], f);
    m.rotation[2] = c::lerp_rot(r0[2], end[2], f);
    if ended {
        if 0.0 < dir {
            let k = pts.len() as i32 - 2;
            c::set_pi32(w, id, o::SEG, k);
            c::set_pf(w, id, o::T, pts.get(k.max(0) as usize).map_or(0.0, |x| x[3]));
        } else {
            c::set_pi32(w, id, o::SEG, 0);
            c::set_pf(w, id, o::T, 0.0);
        }
    }
    ended
}

/// `0x2ef3e0`: springs onto the end it is at (and, run forward, onto the end rotation).
fn settle(w: &mut World, id: MobyId) {
    let Some((_, pts)) = on_path(w, id) else { return };
    let end = if w.m(id).cmd == 0 { c::pv4(w, id, o::ROT_A) } else { c::pv4(w, id, o::ROT_B) };
    let fwd = 0.0 < c::pf(w, id, o::DIR);
    let q = if fwd { *pts.last().unwrap_or(&[0.0; 4]) } else { pts.first().copied().unwrap_or([0.0; 4]) };
    for (k, &t) in q.iter().take(3).enumerate() {
        let x = spring(w, id, t, w.m(id).position[k], o::VEL + 4 * k);
        w.mm(id).position[k] = x;
    }
    if fwd {
        for (k, &t) in end.iter().take(3).enumerate() {
            let x = spring(w, id, t, w.m(id).rotation[k], o::ROT_V + 4 * k);
            w.mm(id).rotation[k] = x;
        }
    }
}

/// `0x2ef630`: the 11 wall points turned by the cart round its position, Ratchet's wall spline.
fn wall(w: &mut World, id: MobyId) {
    let Some((k, _)) = path(w, c::pi32(w, id, o::WALL)) else { return };
    let (r, p) = (w.m(id).rows, c::pos(w, id));
    for (j, [x, y]) in WALL.into_iter().enumerate() {
        let q: [f32; 3] = std::array::from_fn(|i| r[0][i] * x + r[1][i] * y + p[i]);
        if let Some(pt) = w.svc.splines[k].get_mut(j) { *pt = [q[0].to_bits(), q[1].to_bits(), q[2].to_bits(), 1.0f32.to_bits()]; }
    }
    w.hero_fields_mut().wall_spline = Some(k as i16);
}
