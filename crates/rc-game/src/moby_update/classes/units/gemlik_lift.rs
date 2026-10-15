//! **Gemlik's lifts, classes 21 and 244** (level13 `0x2b39e0`; census U449; 1 + 3 placed). A pad between its top
//! (+0x60) and bottom (+0x64) heights that starts by sinking to the bottom. Resting (4 at the top, 5 at the bottom)
//! its glow pulses (once a second, `(4·sin − 3)·128` clamped to 0x20..0x80 grey); once Ratchet has stepped off it,
//! his standing on it within 0.5 (xy) takes him for a ride: the glow 0x80208020, Ratchet held (`SetState(0x72)`),
//! the script camera from where the camera is (`CameraScript(camera, its Euler, 1, 0, 0)`) eased over to a point
//! beside the pad (2.5 out at a right angle and 1.5 up; 2 and 1.2 for class 21) looking at him, then the pad moves
//! (a spring at 8·dt², up to 40·dt), its hum playing; arrived: Ratchet free, the follow camera back
//! (`CameraScript2(1)`). With Ratchet away (5..32 xy) and his ground near the other end (within 2) it goes there by
//! itself. Riders ride the platform block (`CarryRiders` at +0x20).
//!
//! **Pvars**: +0x20 the platform block, +0x60 / +0x64 top / bottom, +0x68 the vertical speed, +0x6c the hum, +0x70
//! the camera blend (+0x74 its speed), +0x78 the glow phase, +0x7c stepped off.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2b39e0` | the lift (module doc) | [`update`] |
//! | `0x2b4060` | the camera point beside it and its Euler (0, 0, toward Ratchet's body point) | [`camera_point`] |
//! | `0x30a6d8` | the tipping lift | [`tip_update`] |
//! | `0x30ad38` | its camera point (module doc) | [`tip_camera`] |
//!
//! **The tipping lift, class 1270** (`0x30a6d8`, census U477, one placed): the same ride but for its pitch, which
//! follows its height (`(1 − (z − bottom) / (top − bottom))·π` + +0x7c degrees), its start resting at the top (state
//! 4), the camera point 2.25 out and 1.5 up in its own (pitched) frame with the pitch in the camera's Euler
//! (`0x30ad38`), arriving: `CameraScript2(2)` and Ratchet freed only `ticks(70)` later (+0x84), and boarding whenever
//! he stands on it within 0.5 (no stepping off first); it rests again as soon as he is off it.
//!
//! Read from the level13 decomp and (1270, which the export lacks) disassembly. The camera (0x1670c0 / 0x1670d0) is
//! the moby loop's view of it. Native `f32`.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn};
use crate::moby_update::services::World;
use crate::moby_update::triggers;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x2b_39e0;
pub const CLASSES: [i16; 2] = [21, 244];
pub const TIP_FN: u32 = 0x30_a6d8;
pub const TIP_CLASSES: [i16; 1] = [1270];

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;

mod pv_ {
    pub const BLOCK: usize = 0x20;
    pub const TOP: usize = 0x60;
    pub const BOTTOM: usize = 0x64;
    pub const VZ: usize = 0x68;
    pub const HUM: usize = 0x6c;
    pub const BLEND: usize = 0x70;
    pub const BLEND_V: usize = 0x74;
    pub const PHASE: usize = 0x78;
    pub const OFF: usize = 0x7c;
    pub const SIZE: usize = 0x80;
}
use pv_ as o;

/// `0x2b4060(m, &pos, &euler)` (module doc).
fn camera_point(w: &World, id: MobyId) -> ([f32; 3], [f32; 3]) {
    let m = w.m(id);
    let odd = |b: u8| 2 <= b && b & 1 != 0;
    let a = if !odd(m.cmd) && !odd(m.state) { c::add_rot(m.rotation[2], FRAC_PI_2) } else { c::sub_rot(m.rotation[2], FRAC_PI_2) };
    let (d, h) = if m.o_class == 0x15 { (2.0, 1.2) } else { (2.5, 1.5) };
    let p = [a.cos() * d + m.position[0], a.sin() * d + m.position[1], h + m.position[2]];
    let b = w.hero.body_point.map(|x| f32::from_bits(x.0));
    ([p[0], p[1], p[2]], [0.0, 0.0, c::atan(b[0] - p[0], b[1] - p[1])])
}

/// Ratchet standing on it (ground moby 0x13f64c, no air ticks 0x13f65e).
fn standing(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 }

/// Level13 `0x2b39e0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    let (pos0, rot0) = (c::pos(w, id), w.m(id).rotation);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let cam_e = w.hero.loop_in.cam_euler;
    match w.m(id).state {
        0 => {
            let a = w.rng.rand_angle();
            c::set_pf(w, id, o::PHASE, a);
            c::set_pi32(w, id, o::HUM, -1);
            w.mm(id).state = 3;
        }
        1 => {
            let old = c::pf(w, id, o::BLEND);
            let (mut x, mut v) = (old, c::pf(w, id, o::BLEND_V));
            turn::spring(1.0, DT2 * 4.0, DT2 * 4.0, DT * 4.0, &mut x, &mut v);
            c::set_pf(w, id, o::BLEND, x);
            c::set_pf(w, id, o::BLEND_V, v);
            if 1.0 <= x {
                let m = w.mm(id);
                m.state = m.cmd;
                m.cmd = 1;
            } else {
                let f = (1.0 - x) / (1.0 - old);
                let (mut p, mut e) = camera_point(w, id);
                for k in 0..3 { p[k] += (cam[k] - p[k]) * f; }
                e[1] = c::add_rot(c::sub_rot(cam_e[1], e[1]) * f, e[1]);
                e[2] = c::add_rot(c::sub_rot(cam_e[2], e[2]) * f, e[2]);
                crate::cinematic::camera_targets(w, Some(p), Some(e));
            }
        }
        s @ (2 | 3) => {
            let hum = c::pi32(w, id, o::HUM);
            if !w.sound_alive(hum, id) {
                let v = w.play_sound(0, 4, id);
                c::set_pi32(w, id, o::HUM, v);
            }
            if w.m(id).cmd != 0 {
                let (p, e) = camera_point(w, id);
                crate::cinematic::camera_targets(w, Some(p), Some(e));
            }
            let t = c::pf(w, id, if s == 2 { o::TOP } else { o::BOTTOM });
            let (mut z, mut v) = (c::pos(w, id)[2], c::pf(w, id, o::VZ));
            turn::spring(t, DT2 * 8.0, DT2 * 8.0, DT * 40.0, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, o::VZ, v);
            if (z - t).abs() == 0.0 {
                w.mm(id).state = if s == 2 { 6 } else { 7 };
                if w.m(id).cmd != 0 {
                    crate::cinematic::hero_state(w, 0, false);
                    crate::cinematic::camera_script2(w, 1);
                }
            }
        }
        s @ (4 | 5) => {
            let ph = c::add_rot(c::pf(w, id, o::PHASE), DT * TAU);
            c::set_pf(w, id, o::PHASE, ph);
            let g = (((ph.sin() * 4.0 - 3.0) * 128.0) as i32).clamp(0x20, 0x80) as u32;
            w.mm(id).glow = g << 16 | g << 8 | 0x8000_0000 | g;
            let stand = standing(w, id);
            let d = c::dist2(super::hero_pos(w), c::pos(w, id));
            if !stand || 1.1 < d { c::set_pu8(w, id, o::OFF, 1); }
            if c::pu8(w, id, o::OFF) != 0 && stand && d < 0.5 {
                let m = w.mm(id);
                m.glow = 0x8020_8020;
                m.cmd = if s == 5 { 2 } else { 3 };
                m.state = 1;
                crate::cinematic::hero_state(w, 0x72, true);
                crate::cinematic::camera_script(w, [cam[0], cam[1], cam[2]], cam_e, 1, 0, false);
                c::set_pf(w, id, o::BLEND, 0.0);
            } else {
                let d = c::dist2(c::pos(w, id), super::hero_pos(w));
                let other = c::pf(w, id, if s == 4 { o::BOTTOM } else { o::TOP });
                let dz = (w.hero.ground_z.to_f32() - other).abs();
                if dz < 2.0 && d < 32.0 && 5.0 < d {
                    let m = w.mm(id);
                    m.cmd = 0;
                    m.state = if s == 4 { 3 } else { 2 };
                    c::set_pf(w, id, o::VZ, if s == 4 { -(DT * 5.0) } else { DT * 5.0 });
                }
            }
        }
        s @ (6 | 7) => {
            let hum = c::pi32(w, id, o::HUM);
            if w.sound_alive(hum, id) {
                w.release_sound(hum, id);
                c::set_pi32(w, id, o::HUM, -1);
            }
            let stand = standing(w, id);
            let near = c::dist2(super::hero_pos(w), c::pos(w, id)) <= 0.5;
            if !(stand && near) {
                c::set_pu8(w, id, o::OFF, 0);
                w.mm(id).state = if s == 7 { 5 } else { 4 };
            }
        }
        _ => {}
    }
    let m = w.mm(id);
    let (d, r) = (c::sub(m.position, pos0), m.rotation);
    triggers::carry_riders(&mut m.pvars, o::BLOCK, d, rot0, r);
}

/// Level13 `0x30ad38(m, &pos, &euler)` (module doc).
fn tip_camera(w: &World, id: MobyId) -> ([f32; 3], [f32; 3]) {
    let m = w.m(id);
    let odd = |b: u8| 2 <= b && b & 1 != 0;
    let (a, ex) = if odd(m.cmd) || odd(m.state) { (-FRAC_PI_2, m.rotation[1]) } else { (FRAC_PI_2, -m.rotation[1]) };
    let l = [a.cos() * 2.25, a.sin() * 2.25, 1.5];
    let r = m.rows;
    let p: [f32; 3] = std::array::from_fn(|i| r[0][i] * l[0] + r[1][i] * l[1] + r[2][i] * l[2] + m.position[i]);
    let b = w.hero.body_point.map(|x| f32::from_bits(x.0));
    (p, [ex, 0.0, c::atan(b[0] - p[0], b[1] - p[1])])
}

/// Level13 `0x30a6d8`: the tipping lift (module doc).
pub fn tip_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    let (pos0, rot0) = (c::pos(w, id), w.m(id).rotation);
    let (top, bottom) = (c::pf(w, id, o::TOP), c::pf(w, id, o::BOTTOM));
    let frac = (c::pos(w, id)[2] - bottom) / (top - bottom);
    w.mm(id).rotation[1] = c::add_rot((1.0 - frac) * PI, c::pf(w, id, 0x7c) * 0.017_453_292);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let cam_e = w.hero.loop_in.cam_euler;
    match w.m(id).state {
        0 => {
            let a = w.rng.rand_angle();
            c::set_pf(w, id, o::PHASE, a);
            c::set_pi32(w, id, o::HUM, -1);
            c::set_pi32(w, id, 0x84, 0);
            w.mm(id).state = 4;
        }
        1 => {
            let old = c::pf(w, id, o::BLEND);
            let (mut x, mut v) = (old, c::pf(w, id, o::BLEND_V));
            turn::spring(1.0, DT2 * 4.0, DT2 * 4.0, DT * 4.0, &mut x, &mut v);
            c::set_pf(w, id, o::BLEND, x);
            c::set_pf(w, id, o::BLEND_V, v);
            if x < 1.0 {
                let f = (1.0 - x) / (1.0 - old);
                let (mut p, mut e) = tip_camera(w, id);
                for k in 0..3 { p[k] += (cam[k] - p[k]) * f; }
                e[1] = c::add_rot(c::sub_rot(cam_e[1], e[1]) * f, e[1]);
                e[2] = c::add_rot(c::sub_rot(cam_e[2], e[2]) * f, e[2]);
                crate::cinematic::camera_targets(w, Some(p), Some(e));
            } else {
                let m = w.mm(id);
                m.state = m.cmd;
                m.cmd = 1;
            }
        }
        s @ (2 | 3) => {
            let hum = c::pi32(w, id, o::HUM);
            if !w.sound_alive(hum, id) {
                let v = w.play_sound(0, 4, id);
                c::set_pi32(w, id, o::HUM, v);
            }
            if w.m(id).cmd != 0 {
                let (p, e) = tip_camera(w, id);
                crate::cinematic::camera_targets(w, Some(p), Some(e));
            }
            let t = if s == 2 { top } else { bottom };
            let (mut z, mut v) = (c::pos(w, id)[2], c::pf(w, id, o::VZ));
            turn::spring(t, DT2 * 8.0, DT2 * 8.0, DT * 40.0, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, o::VZ, v);
            if (z - t).abs() == 0.0 {
                let hold = w.ticks(0x46);
                c::set_pi32(w, id, 0x84, hold);
                w.mm(id).state = if s == 2 { 6 } else { 7 };
                if w.m(id).cmd != 0 { crate::cinematic::camera_script2(w, 2); }
            }
        }
        s @ (6 | 7) => {
            if c::pi32(w, id, 0x84) != 0 && c::dec_timer_pvar_i32(w, id, 0x84) != 0 { crate::cinematic::hero_state(w, 0, false); }
            let hum = c::pi32(w, id, o::HUM);
            if w.sound_alive(hum, id) {
                w.release_sound(hum, id);
                c::set_pi32(w, id, o::HUM, -1);
            }
            if w.hero.ground_moby != Some(id) { w.mm(id).state = if s == 7 { 5 } else { 4 }; }
        }
        s @ (4 | 5) => {
            let ph = c::add_rot(c::pf(w, id, o::PHASE), DT * TAU);
            c::set_pf(w, id, o::PHASE, ph);
            let g = (((ph.sin() * 4.0 - 3.0) * 128.0) as i32).clamp(0x20, 0x80) as u32;
            w.mm(id).glow = g << 16 | g << 8 | 0x8000_0000 | g;
            if standing(w, id) && c::dist2(super::hero_pos(w), c::pos(w, id)) < 0.5 {
                let m = w.mm(id);
                m.glow = 0x8020_8020;
                m.cmd = if s == 5 { 2 } else { 3 };
                m.state = 1;
                crate::cinematic::hero_state(w, 0x72, true);
                crate::cinematic::camera_script(w, [cam[0], cam[1], cam[2]], cam_e, 1, 0, false);
                c::set_pf(w, id, o::BLEND, 0.0);
            } else {
                let d = c::dist2(c::pos(w, id), super::hero_pos(w));
                let other = if s == 4 { bottom } else { top };
                if (w.hero.ground_z.to_f32() - other).abs() < 2.0 && d < 32.0 && 5.0 < d {
                    let m = w.mm(id);
                    m.cmd = 0;
                    m.state = if s == 4 { 3 } else { 2 };
                    c::set_pf(w, id, o::VZ, if s == 4 { -(DT * 5.0) } else { DT * 5.0 });
                }
            }
        }
        _ => {}
    }
    let m = w.mm(id);
    let (d, r) = (c::sub(m.position, pos0), m.rotation);
    triggers::carry_riders(&mut m.pvars, o::BLOCK, d, rot0, r);
}
