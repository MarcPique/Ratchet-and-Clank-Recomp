//! U557: class 644, the cutaway camera of Veldin's last arena (level18 `0x2df608` with its set-up `0x2dfaa0` and the
//! two queries `0x2dfb90` / `0x2dfba0`; 1 placed, #307). The name is descriptive [L]. A director the boss 1422 drives:
//! the boss hands it a camera move (two points, two Euler angles, Ratchet's place and yaw, the glide's length and a
//! delay) and watches its progress; the director fades the screen to black, holds Ratchet (state 0x72) at the place
//! with the letterbox on, cuts the script camera to the first view, glides it to the second on a spring, fades back
//! and hands Ratchet and the camera back. With the help flag it also shows help message 18000 and waits for it.
//!
//! **Pvars** (0xe0): +0x00 / +0x10 the camera's start / end point, +0x20 / +0x30 its start / end Euler, +0x40 Ratchet's
//! place (w = his yaw), +0x60 the glide t, +0x64 its spring velocity, +0x68 the spring's acceleration, +0x6c the fade,
//! +0x70 the spring's top speed, +0x74 the delay before the glide, +0x78 the help flag.
//!
//! ## Coverage
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | 0 | update distance 0xff, → 1 | [`update`] |
//! | 1, 6.. | nothing (waits for [`start`]) | [`update`] |
//! | 2 | 0x17ef08 = 1 (the level's copy of 0x17e988: `HudDraw` skips the HUD this frame); `Approach(1, 4·dt, &+0x6c)`; 0x15f3fc = +0x6c when it is above it; below 1: done for the tick | [`update`] (`svc.visibomb.hud_off_at`, `cinematic::set_fade`) |
//! | | at 1: 0x15f404 = 1 (letterbox), `HeroTeleport(+0x40, (0, 0, +0x4c), 0x72, 1)` (`0x21e0e8`), `CameraScript(+0x00, +0x20, 1, 0, 0)` (`0x3036d8`, a cut); +0x78 = 1 → `Help_Request(18000, 0x85)` (`0x20c6f0`), 4; else 3 | [`update`] (`cinematic::letterbox`, `hero_teleport`, `camera_script`, `help.request`) |
//! | 3 | 0x17ef08 = 1; `FastDecTimer(+0x74)` not running → `0x25dcb0(1, +0x68, +0x68, +0x70, &t, &+0x64)`; camera point `lerp(+0x00, +0x10, t)` (`0x1f9a40`), Euler (0, `lerp_rot(+0x24, +0x34, t)`, `lerp_rot(+0x28, +0x38, t)`) (`0x303610` / `0x303668`); `Approach(0, 4·dt, &+0x6c)`, 0x15f3fc = it | [`update`] (`turn::spring`, `cinematic::camera_targets`) |
//! | | t < 1: Ratchet not in 0x72 → `HeroTeleport(+0x40, yaw, 0x72, 1)`; t ≥ 1: letterbox off, `HeroTeleport(+0x40, yaw, 0, 0)`, `CameraScript2(2)` (`0x303850`), 5 | [`update`] |
//! | 4 | the same glide (no HUD word, no fade-out first); t < 1 and (t ≤ 0.2 or the help box busy: 0x179e10 ≠ 0 / 0x179e34 ≠ −1): Ratchet in 0x72 → `Approach(0, 4·dt, &+0x6c)`, 0x15f3fc = it; else the hold teleport | [`update`] (`help.idle`) |
//! | | else `Approach(1, 4·dt, &+0x6c)`, 0x15f3fc = it; at 1: letterbox off, t = 1, `HeroTeleport(+0x40, yaw, 0, 0)`, `CameraScript2(3)`, 5 | [`update`] |
//! | 5 | +0x6c > 0 → `Approach(0, 4·dt, &+0x6c)`, 0x15f3fc = it; else 1 | [`update`] |
//! | `0x2dfaa0(m, from, to, e_from, e_to, place, n, delay, help)` | t = v = 0; the five vectors; state 2; +0x68 = 0.333·dt², +0x70 = 1 / `ticks(n)`, +0x78 = help, +0x74 = `ticks(delay)`; 0x13f510 = `ticks(n)` (Ratchet's hit invulnerability) | [`start`] (`HeroFields::invulnerable`) |
//! | `0x2dfb90(m)` | +0x60 (t) | [`progress`] |
//! | `0x2dfba0(m)` | 1 in state 2, 2 in state 3, else 0 | [`phase`] |
//! | | no sound, particle, light, hit, save write | n/a |

use crate::cinematic;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;

/// The update in the level18 class table.
pub const UPDATE_FN: u32 = 0x2d_f608;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [644];
/// `Help_Request(18000, 0x85)` of the help flag.
pub const HELP_MSG: i32 = 18000;
pub const HELP_REC: i32 = 0x85;
/// Ratchet's hold state.
pub const HELD: i32 = 0x72;

pub mod pv {
    pub const FROM: usize = 0x00;
    pub const TO: usize = 0x10;
    pub const E_FROM: usize = 0x20;
    pub const E_TO: usize = 0x30;
    pub const PLACE: usize = 0x40;
    pub const T: usize = 0x60;
    pub const VEL: usize = 0x64;
    pub const ACCEL: usize = 0x68;
    pub const FADE: usize = 0x6c;
    pub const VMAX: usize = 0x70;
    pub const DELAY: usize = 0x74;
    pub const HELP: usize = 0x78;
    pub const SIZE: usize = 0x7c;
}

fn ok(w: &World, id: MobyId) -> bool { w.m(id).pvars.len() >= pv::SIZE }

/// `0x2dfaa0(m, from, to, e_from, e_to, place, n, delay, help)` (module doc).
#[allow(clippy::too_many_arguments)]
pub fn start(w: &mut World, id: MobyId, from: c::V, to: c::V, e_from: c::V, e_to: c::V, place: c::V, n: i32, delay: i32, help: i32) {
    if !ok(w, id) { return; }
    c::set_pf(w, id, pv::T, 0.0);
    c::set_pf(w, id, pv::VEL, 0.0);
    c::set_pv4(w, id, pv::FROM, from);
    c::set_pv4(w, id, pv::TO, to);
    c::set_pv4(w, id, pv::E_FROM, e_from);
    c::set_pv4(w, id, pv::E_TO, e_to);
    c::set_pv4(w, id, pv::PLACE, place);
    w.mm(id).state = 2;
    c::set_pf(w, id, pv::ACCEL, DT2 * 0.333);
    let t = w.ticks(n);
    c::set_pf(w, id, pv::VMAX, 1.0 / t as f32);
    c::set_pi32(w, id, pv::HELP, help);
    let d = w.ticks(delay);
    c::set_pi32(w, id, pv::DELAY, d);
    w.hero_fields_mut().invulnerable = Some(t);
}

/// `0x2dfb90(m)`: the glide's t.
pub fn progress(w: &World, id: MobyId) -> f32 { if ok(w, id) { c::pf(w, id, pv::T) } else { 0.0 } }

/// `0x2dfba0(m)`: 1 while it fades in (state 2), 2 while it glides (state 3), else 0.
pub fn phase(w: &World, id: MobyId) -> i32 {
    match w.m(id).state {
        2 => 1,
        3 => 2,
        _ => 0,
    }
}

fn place(w: &World, id: MobyId) -> ([f32; 3], [f32; 3]) {
    let p = c::pv4(w, id, pv::PLACE);
    ([p[0], p[1], p[2]], [0.0, 0.0, p[3]])
}

fn approach_fade(w: &mut World, id: MobyId, target: f32) -> f32 {
    let mut f = c::pf(w, id, pv::FADE);
    turn::approach(target, 4.0 * DT, &mut f);
    c::set_pf(w, id, pv::FADE, f);
    f
}

/// The delay, then the spring and the camera's targets (states 3 and 4).
fn glide(w: &mut World, id: MobyId) -> f32 {
    if c::dec_timer_pvar_i32(w, id, pv::DELAY) != 0 {
        let (a, vmax) = (c::pf(w, id, pv::ACCEL), c::pf(w, id, pv::VMAX));
        let (mut t, mut v) = (c::pf(w, id, pv::T), c::pf(w, id, pv::VEL));
        turn::spring(1.0, a, a, vmax, &mut t, &mut v);
        c::set_pf(w, id, pv::T, t);
        c::set_pf(w, id, pv::VEL, v);
    }
    let t = c::pf(w, id, pv::T);
    let (a, b) = (c::pv4(w, id, pv::FROM), c::pv4(w, id, pv::TO));
    let p: [f32; 4] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
    let (ea, eb) = (c::pv4(w, id, pv::E_FROM), c::pv4(w, id, pv::E_TO));
    let e = [0.0, c::lerp_rot(ea[1], eb[1], t), c::lerp_rot(ea[2], eb[2], t)];
    cinematic::camera_targets(w, Some([p[0], p[1], p[2]]), Some(e));
    t
}

fn hold(w: &mut World, id: MobyId) {
    let (p, e) = place(w, id);
    cinematic::hero_teleport(w, p, e, HELD, true);
}

fn release(w: &mut World, id: MobyId, kind: u8) {
    cinematic::letterbox(w, false);
    let (p, e) = place(w, id);
    cinematic::hero_teleport(w, p, e, 0, false);
    cinematic::camera_script2(w, kind);
    w.mm(id).state = 5;
}

/// Level18 0x2df608 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if !ok(w, id) { return; }
    match w.m(id).state {
        0 => {
            w.mm(id).update_dist = 0xff;
            w.mm(id).state = 1;
        }
        2 => {
            w.svc.visibomb.hud_off_at = Some(w.counter);
            let f = approach_fade(w, id, 1.0);
            if w.svc.cinematic.fade < f { cinematic::set_fade(w, f); }
            if f < 1.0 { return; }
            cinematic::letterbox(w, true);
            hold(w, id);
            let (p, e) = (c::pv4(w, id, pv::FROM), c::pv4(w, id, pv::E_FROM));
            cinematic::camera_script(w, [p[0], p[1], p[2]], [e[0], e[1], e[2]], 1, 0, false);
            if c::pi32(w, id, pv::HELP) == 1 {
                w.svc.help.request(HELP_MSG, HELP_REC);
                w.mm(id).state = 4;
                return;
            }
            w.mm(id).state = 3;
        }
        3 => {
            w.svc.visibomb.hud_off_at = Some(w.counter);
            let t = glide(w, id);
            let f = approach_fade(w, id, 0.0);
            cinematic::set_fade(w, f);
            if t < 1.0 {
                if w.hero.state != HELD { hold(w, id); }
                return;
            }
            release(w, id, 2);
        }
        4 => {
            let t = glide(w, id);
            if t < 1.0 && (t <= 0.2 || !w.svc.help.idle()) {
                if w.hero.state == HELD {
                    let f = approach_fade(w, id, 0.0);
                    cinematic::set_fade(w, f);
                } else {
                    hold(w, id);
                }
                return;
            }
            let f = approach_fade(w, id, 1.0);
            cinematic::set_fade(w, f);
            if f < 1.0 { return; }
            c::set_pf(w, id, pv::T, 1.0);
            release(w, id, 3);
        }
        5 => {
            if 0.0 < c::pf(w, id, pv::FADE) {
                let f = approach_fade(w, id, 0.0);
                cinematic::set_fade(w, f);
                return;
            }
            w.mm(id).state = 1;
        }
        _ => {}
    }
}
