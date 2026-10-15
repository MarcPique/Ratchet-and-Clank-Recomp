//! **The Fleet's lift pads, class 1380** (level17 `0x2ee2b0`; census U611, 4 placed). Pads that carry Ratchet between
//! a top (+0x60) and a bottom (+0x64) height, turning over on the way (rot y from 0 at the top to π at the bottom,
//! unless +0x7c), so that his Magneboots walk him from one face to the other; on the pad with the boots holding him,
//! the pad's outline (11 points, level17 0x1d9f70, through its frame into the spline +0x80) keeps him on it
//! (0x14162a). Read from the level17 decomp; native `f32`.
//!
//! * **0**: a pulse phase at random; no 11-point spline +0x80 → gone; → 6.
//! * **6 / 7** (resting at the top / the bottom): a grey pulse (`(4·sin − 3)·128` within 0x20..0x80, one cycle a
//!   second); with the boots on (or +0x7c clear) and Ratchet standing on it within 0.5: sound 1, → 2 (down from the
//!   top... the far end) / 3; Ratchet waiting near the other end (within half the span of it, 2..32 away): it comes
//!   to fetch him (→ 3 / 2, +0xbc 0).
//! * **2 / 3** (carrying) and **4 / 5** (going empty): glow to 0x80808040; Ratchet off it → 4 / 5; the hum (sound 0);
//!   the height sprung (0.3·dt², 3·dt) to the top (2 / 4) or the bottom; there → 8 / 9 (carrying) or 6 / 7, the hum
//!   released, `SetState(0)` after an intro (+0xbc). 2 / 3 keep the outline spline under Ratchet (`0x2eea70`).
//! * **8 / 9**: the hum released; Ratchet off it → 6 / 7.
//! * **1** (entered by other code, +0xbc the state to go back to): the script camera eased from 2.5 out and 1.5 up
//!   of the pad, facing Ratchet, toward the current camera (+0x70 sprung to 1 over 4·dt) (`0x2ee9c0`).
//! * **Every tick**: its riders carried (`CarryRiders`, block +0x20).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ee2b0` | 1380 | [`update`] |
//! | `0x2ee9c0` / `0x2eea70` | the intro camera's place, the outline spline | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::flyer::wrap_frac;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::{story, triggers, World};
use std::f32::consts::{PI, TAU};

pub const REFERENCE_LEVEL: u32 = 17;
pub const UPDATE_FN: u32 = 0x2e_e2b0;
pub const CLASSES: [i16; 1] = [1380];

mod o {
    pub const RIDERS: usize = 0x20;
    pub const TOP: usize = 0x60;
    pub const BOTTOM: usize = 0x64;
    pub const ZV: usize = 0x68;
    pub const VOICE: usize = 0x6c;
    pub const INTRO: usize = 0x70;
    pub const INTRO_V: usize = 0x74;
    pub const PULSE: usize = 0x78;
    pub const NO_FLIP: usize = 0x7c;
    pub const OUTLINE: usize = 0x80;
    pub const SIZE: usize = 0x84;
}

/// Level17 0x1d9f70: the pad's outline in its frame.
const OUTLINE: [[f32; 2]; 11] = [
    [-1.886368, 1.962136], [1.886367, 1.962136], [2.464658, 1.005283], [2.667728, -0.004372], [2.464659, -1.014029],
    [1.886368, -1.962136], [-1.886369, -1.962136], [-2.46466, -1.014028], [-2.667728, -0.004372], [-2.464659, 1.005284],
    [-1.886368, 1.962136],
];

fn on_it(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) }
fn release(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, o::VOICE);
    if w.sound_alive(v, id) {
        w.release_sound(v, id);
        c::set_pi32(w, id, o::VOICE, -1);
    }
}

/// Level17 `0x2ee2b0` (module doc).
#[allow(clippy::too_many_lines)]
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let old = c::pos(w, id);
    let old_rot = w.m(id).rotation;
    if c::pi32(w, id, o::NO_FLIP) == 0 {
        let (top, bot) = (c::pf(w, id, o::TOP), c::pf(w, id, o::BOTTOM));
        let f = (1.0 - (w.m(id).position[2] - bot) / (top - bot)).clamp(0.0, 1.0);
        w.mm(id).rotation[1] = wrap_frac(f * PI);
    }
    match w.m(id).state {
        0 => {
            let a = w.rng.rand_angle();
            c::set_pf(w, id, o::PULSE, a);
            c::set_pi32(w, id, o::VOICE, -1);
            let pi = c::pi32(w, id, o::OUTLINE);
            let ok = usize::try_from(pi).ok().and_then(|i| w.svc.splines.get(i)).is_some_and(|s| s.len() == 11);
            if !ok {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 6;
        }
        1 => {
            let old_t = c::pf(w, id, o::INTRO);
            let (mut x, mut v) = (old_t, c::pf(w, id, o::INTRO_V));
            turn::spring(1.0, DT2 * 4.0, DT2 * 4.0, DT * 4.0, &mut x, &mut v);
            c::set_pf(w, id, o::INTRO, x);
            c::set_pf(w, id, o::INTRO_V, v);
            if x < 1.0 {
                let f = (1.0 - x) / (1.0 - old_t);
                let p = c::pos(w, id);
                let cam = w.camera_point();
                let mut at = c::set_len2([cam[0] - p[0], cam[1] - p[1], 0.0, 0.0], 2.5);
                at[2] = 1.5;
                let at = c::add(at, p);
                let b = w.hero.body_point.map(|q| q.to_f32());
                let mut e = [0.0, 0.0, c::atan(b[0] - at[0], b[1] - at[1])];
                let pos: [f32; 3] = std::array::from_fn(|k| at[k] + (cam[k] - at[k]) * f);
                let (cp, cy) = (w.hero.loop_in.cam_euler[1], w.camera_yaw);
                e[1] = c::add_rot(c::sub_rot(cp, e[1]) * f, e[1]);
                e[2] = c::add_rot(c::sub_rot(cy, e[2]) * f, e[2]);
                crate::cinematic::camera_targets(w, Some(pos), Some(e));
            } else {
                let m = w.mm(id);
                m.state = m.cmd;
                m.cmd = 1;
            }
        }
        s @ 2..=5 => {
            if s <= 3 && w.hero.f658 != 0 && on_it(w, id) {
                let pi = c::pi32(w, id, o::OUTLINE) as usize;
                let (r, p) = (w.m(id).rows, w.m(id).position);
                if let Some(sp) = w.svc.splines.get_mut(pi) {
                    for (k, q) in OUTLINE.iter().enumerate().take(sp.len()) {
                        let v: [f32; 3] = std::array::from_fn(|j| r[0][j] * q[0] + r[1][j] * q[1] + p[j]);
                        sp[k] = [v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), 1.0f32.to_bits()];
                    }
                }
                w.hero_fields_mut().wall_spline = Some(pi as i16);
            }
            let g = crate::hud::tween_color(0.1, w.m(id).glow, 0x8080_8040);
            w.mm(id).glow = g;
            if s == 2 && !on_it(w, id) { w.mm(id).state = 4; }
            if w.m(id).state == 3 && !on_it(w, id) { w.mm(id).state = 5; }
            if !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
                let v = w.play_sound(0, 4, id);
                c::set_pi32(w, id, o::VOICE, v);
            }
            let st = w.m(id).state;
            let goal = if st == 2 || st == 4 { c::pf(w, id, o::TOP) } else { c::pf(w, id, o::BOTTOM) };
            let (mut z, mut v) = (w.m(id).position[2], c::pf(w, id, o::ZV));
            turn::spring(goal, DT2 * 0.3, DT2 * 0.3, DT * 3.0, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, o::ZV, v);
            if z - goal == 0.0 {
                w.mm(id).state = match st { 4 => 6, 5 => 7, 2 => 8, _ => 9 };
                if w.m(id).cmd != 0 { crate::cinematic::hero_state(w, 0, false); }
                release(w, id);
            }
        }
        s @ (6 | 7) => {
            let a = c::add_rot(c::pf(w, id, o::PULSE), DT * TAU);
            c::set_pf(w, id, o::PULSE, a);
            let l = (((a.sin() * 4.0 - 3.0) * 128.0) as i32).clamp(0x20, 0x80) as u32;
            w.mm(id).glow = l << 16 | l << 8 | 0x8000_0000 | l;
            if w.hero.f658 != 0 || c::pi32(w, id, o::NO_FLIP) == 0 {
                if on_it(w, id) && w.hero.air_ticks == 0 && c::dist2(super::hero_pos(w), c::pos(w, id)) < 0.5 {
                    w.play_sound(1, 0, id);
                    w.mm(id).state = if s == 7 { 2 } else { 3 };
                } else {
                    let d = c::dist2(c::pos(w, id), super::hero_pos(w));
                    let (top, bot) = (c::pf(w, id, o::TOP), c::pf(w, id, o::BOTTOM));
                    let gz = w.hero.ground_z.to_f32();
                    let (far_end, next) = if s == 6 { (bot, 3) } else { (top, 2) };
                    let near = (gz - far_end).abs() < (top - bot).abs() * 0.5;
                    if near && d < 32.0 && 2.0 < d {
                        w.mm(id).cmd = 0;
                        w.mm(id).state = next;
                        c::set_pf(w, id, o::ZV, 0.0);
                    }
                }
            }
        }
        s @ (8 | 9) => {
            release(w, id);
            if !on_it(w, id) { w.mm(id).state = if s == 9 { 7 } else { 6 }; }
        }
        _ => {}
    }
    let p = c::pos(w, id);
    let delta = [p[0] - old[0], p[1] - old[1], p[2] - old[2], p[3]];
    let rot = w.m(id).rotation;
    triggers::carry_riders(&mut w.mm(id).pvars, o::RIDERS, delta, old_rot, rot);
}
