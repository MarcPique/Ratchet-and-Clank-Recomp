//! **Rilgar's lift pads, class 877** (level05 `0x3156d0` with `0x315ee0` / `0x315f70`, 6 placed; census U204; the name
//! is descriptive [L]). A pad that rides between a bottom and a top height (pvar +0x64 / +0x60), turning from its placed
//! yaw to its cuboid's (+0x6c) as it rises, and carries Ratchet. Waiting, it pulses; stepping onto its middle (within
//! 0.5, xy) sends it with a click — with a spline (+0x68) the ride is a cutaway: the screen fades, the script camera
//! watches from behind the pad, the spline is laid as a ring of 3.5 round the pad (the hero's spline wall keeps him on)
//! and the pad eases over its whole height; on arrival it waits up to 2 s for him to step off (or ride back).
//! Otherwise Ratchet calls it from the other level by walking within 32 of it at that level's height. Read from the
//! level05 decomp and its words gp−0x4d40..−0x4d38.
//!
//! **Pvars** (0x90): +0x08 the platform block's offset (0x20), +0x60 / +0x64 the top / bottom z, +0x68 the wall spline
//! (−1 none), +0x6c the cuboid, +0x70 the height t (0 bottom, 1 top), +0x74 the hum's voice, +0x78 the fade, +0x7c the
//! timer, +0x80 the pulse phase, +0x84 the placed yaw, +0x88 the velocity of t.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | phase `random_angle_radians`, voice −1, +0x84 = yaw, → 3 | [`update`] |
//! | state 1 | the wall (0x14162a = +0x68); `Approach(1, 0.1333, &fade)`, 0x15f3fc = fade; fade 1 → `CameraScript(the view, 1, 0, 0)`, → +0xbc (2 up / 3 down), +0xbc = 1 | [`update`] ([`view`]) |
//! | states 2 (up) / 3 (down) | the hum (`SoundIsAlive`, else `PlayClassSound(0, 4)`); a cutaway (+0xbc ≠ 0): the wall, the fade out (`Approach(0, …)`), the camera's targets (`0x327278` / `0x3272d0`), the spring 6·dt²·k / 6·dt·k with k = 1 / (top − bottom); else dt² / dt; `0x285be8(target, a, a, vmax, &t, &v)`; z = (top − bottom)·t + bottom; yaw = placed + `sub_rot(cuboid Euler z, placed)`·t | [`update`] (`turn::spring`) |
//! | | \|t − target\| < 0.01: timer `ticks(120)`, the hum released (owner-guarded), voice −1, v clamped to ±0.00125; → 4 / 5 (after a cutaway 6 / 7) | [`update`] |
//! | states 4 (top) / 5 (bottom) | the pulse: phase += 2π·dt, glow = grey `clamp(trunc((4·sin − 3)·128), 0x20, 0x80)`; Ratchet on it, grounded, within 0.5 → `PlayClassSound(1, 0)`, a spline → the ring (`0x315f70`), glow 0x80208020, +0xbc = 3 / 2, → 1, fade 0; else his ground z (0x13f628) within 2 of the other end and he within 32 but beyond 5 → +0xbc = 0, → 3 / 2 | [`update`] ([`ring`]) |
//! | states 6 / 7 | the hum released; timer above `ticks(30)` or Ratchet beyond 0.5 → it counts down; Ratchet on it: once out, the pulse and, within 0.5 and grounded, the click, glow 0x80208020, → 3 / 2; not on it: `CameraScript2(1)`, → 4 / 5 | [`update`] |
//! | every tick | `CarryRiders(+0x20, position − old, old Euler, Euler)` | [`update`] (`triggers::carry_riders`) |
//! | `0x315ee0` | the view: position + row 0 at −3.5, z + 5; Euler (0, 45°, yaw) | [`view`] |
//! | `0x315f70` | the wall spline's n points: (3.5·cos u, 3.5·sin u, 0) + position, u += 2π/(n − 1) from 0 (w kept) | [`ring`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x31_56d0;
pub const CLASSES: [i16; 1] = [877];
/// gp−0x4d38 / −0x4d40 / −0x4d3c: the view's distance behind, height and pitch (degrees).
const VIEW_BACK: f32 = -3.5;
const VIEW_UP: f32 = 5.0;
const VIEW_PITCH: f32 = 45.0;
/// The fade's step (0x3e083127).
const FADE_STEP: f32 = f32::from_bits(0x3e08_3127);
const LIT: u32 = 0x8020_8020;
const RING: f32 = 3.5;

/// Pvar offsets (module doc).
pub mod pv {
    pub const BLOCK: usize = 0x20;
    pub const TOP: usize = 0x60;
    pub const BOTTOM: usize = 0x64;
    pub const WALL: usize = 0x68;
    pub const CUBOID: usize = 0x6c;
    pub const T: usize = 0x70;
    pub const VOICE: usize = 0x74;
    pub const FADE: usize = 0x78;
    pub const TIMER: usize = 0x7c;
    pub const PHASE: usize = 0x80;
    pub const HOME_YAW: usize = 0x84;
    pub const V: usize = 0x88;
    pub const SIZE: usize = 0x90;
}

/// `0x315ee0`: the script camera's view of the pad (module doc).
pub fn view(w: &World, id: MobyId) -> ([f32; 3], [f32; 3]) {
    let m = w.m(id);
    let mut p = c::add(c::set_len3(m.rows[0], VIEW_BACK), m.position);
    p[2] += VIEW_UP;
    ([p[0], p[1], p[2]], [0.0, VIEW_PITCH * 0.017_453_292, m.rotation[2]])
}

/// `0x315f70`: the wall spline laid as a ring round the pad (module doc).
#[allow(clippy::approx_constant)] // the code's own 6.28318.
pub fn ring(w: &mut World, id: MobyId) {
    let Some(i) = usize::try_from(c::pi32(w, id, pv::WALL)).ok().filter(|&i| i < w.svc.splines.len()) else { return };
    let pos = w.m(id).position;
    let n = w.svc.splines[i].len();
    let step = 6.28318 / (n as f32 - 1.0);
    let mut u = 0.0f32;
    for k in 0..n {
        let p = &mut w.svc.splines[i][k];
        p[0] = (u.cos() * RING + pos[0]).to_bits();
        p[1] = (u.sin() * RING + pos[1]).to_bits();
        p[2] = pos[2].to_bits();
        u = c::add_rot(u, step);
    }
}

fn wall(w: &mut World, id: MobyId) {
    let s = c::pi32(w, id, pv::WALL);
    w.hero_fields_mut().wall_spline = Some(s as i16);
}

fn fade(w: &mut World, id: MobyId, target: f32) -> f32 {
    let mut f = c::pf(w, id, pv::FADE);
    turn::approach(target, FADE_STEP, &mut f);
    c::set_pf(w, id, pv::FADE, f);
    crate::cinematic::set_fade(w, f);
    f
}

fn release(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, pv::VOICE);
    if v != -1 && w.sound_alive(v, id) { w.release_sound(v, id); }
}

fn pulse(w: &mut World, id: MobyId) {
    let ph = c::add_rot(c::pf(w, id, pv::PHASE), DT * std::f32::consts::TAU);
    c::set_pf(w, id, pv::PHASE, ph);
    let v = (((ph.sin() * 4.0 - 3.0) * 128.0) as i32).clamp(0x20, 0x80) as u32;
    w.mm(id).glow = v << 16 | v << 8 | 0x8000_0000 | v;
}

fn on_middle(w: &World, id: MobyId) -> bool {
    w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 && c::dist2(super::hero_pos(w), w.m(id).position) < 0.5
}

/// Level05 `0x3156d0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    let (old_pos, old_rot) = (w.m(id).position, w.m(id).rotation);
    let st = w.m(id).state;
    match st {
        0 => {
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            c::set_pi32(w, id, pv::VOICE, -1);
            let yaw = w.m(id).rotation[2];
            c::set_pf(w, id, pv::HOME_YAW, yaw);
            w.mm(id).state = 3;
        }
        1 => {
            wall(w, id);
            if fade(w, id, 1.0) == 1.0 {
                let (p, e) = view(w, id);
                crate::cinematic::camera_script(w, p, e, 1, 0, false);
                let m = w.mm(id);
                m.state = m.cmd;
                m.cmd = 1;
            }
        }
        2 | 3 => {
            if !w.sound_alive(c::pi32(w, id, pv::VOICE), id) {
                let s = w.play_sound(0, 4, id);
                c::set_pi32(w, id, pv::VOICE, s);
            }
            let (top, bottom) = (c::pf(w, id, pv::TOP), c::pf(w, id, pv::BOTTOM));
            let (mut a, mut vmax) = (DT2, DT);
            if w.m(id).cmd != 0 {
                wall(w, id);
                fade(w, id, 0.0);
                let (p, e) = view(w, id);
                crate::cinematic::camera_targets(w, Some(p), Some(e));
                let k = 1.0 / (top - bottom);
                a = DT2 * 6.0 * k;
                vmax = DT * 6.0 * k;
            }
            let target = if st == 2 { 1.0 } else { 0.0 };
            let (mut t, mut v) = (c::pf(w, id, pv::T), c::pf(w, id, pv::V));
            turn::spring(target, a, a, vmax, &mut t, &mut v);
            c::set_pf(w, id, pv::T, t);
            c::set_pf(w, id, pv::V, v);
            let home = c::pf(w, id, pv::HOME_YAW);
            let goal = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, pv::CUBOID)).map_or(home, |s| s.euler[2]);
            let m = w.mm(id);
            m.position[2] = (top - bottom) * t + bottom;
            m.rotation[2] = c::add_rot(c::sub_rot(goal, home) * t, home);
            if 0.01 <= (t - target).abs() {
                carry(w, id, old_pos, old_rot);
                return;
            }
            let t120 = w.ticks(120);
            c::set_pi32(w, id, pv::TIMER, t120);
            release(w, id);
            c::set_pi32(w, id, pv::VOICE, -1);
            let lim = f32::from_bits(0x3aa3_d70a);
            if lim < v {
                c::set_pf(w, id, pv::V, lim);
            } else if v < -lim {
                c::set_pf(w, id, pv::V, -lim);
            }
            let up = st == 2;
            w.mm(id).state = match (w.m(id).cmd != 0, up) { (false, true) => 4, (false, false) => 5, (true, true) => 6, (true, false) => 7 };
        }
        4 | 5 => {
            pulse(w, id);
            if on_middle(w, id) {
                w.play_sound(1, 0, id);
                if c::pi32(w, id, pv::WALL) != -1 { ring(w, id); }
                let m = w.mm(id);
                m.glow = LIT;
                m.cmd = if st == 5 { 2 } else { 3 };
                m.state = 1;
                c::set_pf(w, id, pv::FADE, 0.0);
            } else {
                let other = if st == 4 { c::pf(w, id, pv::BOTTOM) } else { c::pf(w, id, pv::TOP) };
                let d = c::dist2(w.m(id).position, super::hero_pos(w));
                let gz = w.hero.ground_z.to_f32();
                if (gz - other).abs() < 2.0 && d < 32.0 && 5.0 < d {
                    let m = w.mm(id);
                    m.cmd = 0;
                    m.state = if st == 4 { 3 } else { 2 };
                }
            }
        }
        6 | 7 => {
            let v = c::pi32(w, id, pv::VOICE);
            if w.sound_alive(v, id) {
                w.release_sound(v, id);
                c::set_pi32(w, id, pv::VOICE, -1);
            }
            let t30 = w.ticks(30);
            if t30 < c::pi32(w, id, pv::TIMER) || 0.5 < c::dist2(super::hero_pos(w), w.m(id).position) {
                c::dec_timer_pvar_i32(w, id, pv::TIMER);
            }
            if w.hero.ground_moby == Some(id) {
                if c::pi32(w, id, pv::TIMER) == 0 {
                    pulse(w, id);
                    if on_middle(w, id) {
                        w.play_sound(1, 0, id);
                        let m = w.mm(id);
                        m.glow = LIT;
                        m.state = if st == 7 { 2 } else { 3 };
                    }
                }
            } else {
                crate::cinematic::camera_script2(w, 1);
                w.mm(id).state = if st == 7 { 5 } else { 4 };
            }
        }
        _ => {}
    }
    carry(w, id, old_pos, old_rot);
}

fn carry(w: &mut World, id: MobyId, old_pos: c::V, old_rot: c::V) {
    let m = w.mm(id);
    let disp = c::sub(m.position, old_pos);
    let rot = m.rotation;
    triggers::carry_riders(&mut m.pvars, pv::BLOCK, disp, old_rot, rot);
}
