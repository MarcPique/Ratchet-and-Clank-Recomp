//! **Oltanis's wind tunnels, class 610** (level14 `0x2eaf88`; census U498, 4 placed). A wind along the moby's yaw:
//! while Ratchet is in its outer cuboid (+0x500) it sets the weather's wind (20·dt along it, the gust timer 1) and
//! blows its motes; on a windy surface (0x140632) or a moby (0x13f65c), not wall-walking (0x13f658) and not in camera
//! mode 0xd, in its push cuboid (+0x00) it pushes him (his platform push 0x13f440 += the wind) — ramping up over
//! `ticks(+0x508)` — and once he leaves the push dies away over `ticks(30)` (at most `ticks(15)` off the windy
//! surface). Read from the level14 decomp; native `f32`.
//!
//! * **The motes** (`0x2eb388` / `0x2eb568`): 48 specks (size `randf(2, 6)`, speed `randf(15, 40)·dt`) started at the
//!   upwind face of the mote cuboid (+0x504; ±its row 1 / row 2 lengths across), living `ticks(4·|row 0| / speed)`,
//!   fading in and out over 30 ticks (alpha up to 50); with Ratchet (1 up) in the mote cuboid only the first 24 come
//!   back, and specks within 8 of the camera while it faces into the wind (over 135°) live at most `ticks(20)` more.
//! * **The draw** (`0x2eb810`): each live speck a camera-facing quad (FX 0x30, additive, 0x7f7f7f, its alpha).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2eaf88` | 610 | [`update`] |
//! | `0x2eb388` / `0x2eb568` | a speck, the specks | [`speck`], [`specks`] |
//! | `0x2eb810` | the draw | [`frame`], [`fx_quads`] |
//!
//! [L] The draw is built from the camera the frame part saw (+0x520).

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{pvar as p, Services, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2e_af88;
pub const DRAW_FN: u32 = 0x2e_b810;
pub const CLASSES: [i16; 1] = [610];

mod o {
    pub const PUSH: usize = 0x00;
    pub const SPEED: usize = 0x08;
    pub const DECAY: usize = 0x0c;
    pub const PUSHING: usize = 0x0e;
    pub const DECAY_INV: usize = 0x10;
    pub const LEN: usize = 0x14;
    pub const WIDE: usize = 0x18;
    pub const HIGH: usize = 0x1c;
    pub const PTS: usize = 0x20;
    pub const TIMER: usize = 0x320;
    pub const LIFE: usize = 0x380;
    pub const ALPHA: usize = 0x3e0;
    pub const RATE: usize = 0x440;
    pub const WAKE: usize = 0x500;
    pub const AREA: usize = 0x504;
    pub const RAMP_T: usize = 0x508;
    pub const RAMP: usize = 0x50a;
    pub const RAMP_INV: usize = 0x50c;
    pub const CAM: usize = 0x520;
    pub const SIZE: usize = 0x530;
}
/// Level14 gp words (0x161c04..0x161c3c).
const DECAY_TICKS: i32 = 30;
const WIND: f32 = 20.0;
const ALPHA_MAX: f32 = 50.0;
const FADE: i32 = 30;

fn cuboid(w: &World, i: i32) -> Option<rc_formats::volumes::Shape> { w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).copied() }
fn len(r: [f32; 4]) -> f32 { (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() }

/// `0x2eb388(m, i, dir)`: speck `i` restarted (module doc).
pub fn speck(w: &mut World, id: MobyId, i: usize, dir: [f32; 3]) {
    let Some(s) = cuboid(w, c::pi32(w, id, o::AREA)) else { return };
    // FastVecCross(out, dir, up) = up × dir.
    let side = [-dir[1], dir[0], 0.0];
    let ctr = s.centre();
    let l = c::pf(w, id, o::LEN);
    let a = w.rng.randf_sym(0.0, c::pf(w, id, o::WIDE));
    let b = w.rng.randf_sym(0.0, c::pf(w, id, o::HIGH));
    let p: [f32; 3] = std::array::from_fn(|k| ctr[k] - dir[k] * l + side[k] * a + [0.0, 0.0, 1.0][k] * b);
    let size = w.rng.randf(2.0, 6.0);
    c::set_pv4(w, id, o::PTS + 0x10 * i, [p[0], p[1], p[2], size]);
    let v = w.rng.randf(15.0, 40.0) * DT;
    c::set_pf(w, id, o::RATE + 4 * i, v);
    let t = w.ticks(((l + l) / v) as i32);
    c::set_pi16(w, id, o::TIMER + 2 * i, t as i16);
    c::set_pi16(w, id, o::LIFE + 2 * i, t as i16);
    c::set_pi16(w, id, o::ALPHA + 2 * i, 0);
}

/// `0x2eb568` (module doc).
pub fn specks(w: &mut World, id: MobyId) {
    let y = c::yaw(w, id);
    let dir = [y.cos(), y.sin(), 0.0];
    let h = super::hero_pos(w);
    let inside = w.in_cuboid([h[0], h[1], h[2] + 1.0], c::pi32(w, id, o::AREA));
    let cam = w.camera.map(|x| x.to_f32());
    for i in 0..0x30 {
        if c::dec_timer_pvar_s16(w, id, o::TIMER + 2 * i) == 0 {
            let (life, t) = (c::pi16(w, id, o::LIFE + 2 * i) as i32, c::pi16(w, id, o::TIMER + 2 * i) as i32);
            let f = if life - FADE < t { (life - t) as f32 / FADE as f32 } else { t as f32 / (life - FADE) as f32 };
            c::set_pi16(w, id, o::ALPHA + 2 * i, ((ALPHA_MAX - 0.0) * f + 0.0) as i16);
            let r = c::pf(w, id, o::RATE + 4 * i);
            let q = c::pv4(w, id, o::PTS + 0x10 * i);
            let q = [q[0] + dir[0] * r, q[1] + dir[1] * r, q[2] + dir[2] * r, q[3]];
            c::set_pv4(w, id, o::PTS + 0x10 * i, q);
            if inside && (q[0] - cam[0]).abs() < 8.0 && (q[1] - cam[1]).abs() < 8.0 && 2.356_194_5 < c::diff_rots(y, w.camera_yaw) {
                let cap = w.ticks(0x14);
                if cap < t { c::set_pi16(w, id, o::TIMER + 2 * i, cap as i16); }
            }
        } else if !inside || i < 0x18 {
            speck(w, id, i, dir);
        }
    }
    if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
        w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
        w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
    }
}

fn push(w: &mut World, id: MobyId, k: f32) {
    let y = c::yaw(w, id);
    let f = w.hero_fields_mut();
    f.platform[0] += y.cos() * k;
    f.platform[1] += y.sin() * k;
}

/// Level14 `0x2eaf88` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    if w.m(id).state == 0 {
        w.mm(id).state = 1;
        let Some(s) = cuboid(w, c::pi32(w, id, o::AREA)).filter(|_| c::pi32(w, id, o::PUSH) >= 0 && c::pi32(w, id, o::WAKE) >= 0) else {
            w.delete_moby(id);
            return;
        };
        c::set_pi16(w, id, o::PUSHING, 0);
        c::set_pf(w, id, o::LEN, len(s.matrix[0]) * 2.0);
        c::set_pf(w, id, o::WIDE, len(s.matrix[1]));
        c::set_pf(w, id, o::HIGH, len(s.matrix[2]));
        let t = w.ticks(c::pi16(w, id, o::RAMP_T) as i32);
        c::set_pi16(w, id, o::RAMP, t as i16);
        c::set_pf(w, id, o::RAMP_INV, 1.0 / t as f32);
        let y = c::yaw(w, id);
        let dir = [y.cos(), y.sin(), 0.0];
        for i in 0..0x30 { speck(w, id, i, dir); }
    }
    let h = w.hero_point();
    if w.in_cuboid(h, c::pi32(w, id, o::WAKE)) {
        let y = c::yaw(w, id);
        let u = &mut w.svc.units;
        u.set_word(super::weather::WIND_X, (y.cos() * WIND * DT).to_bits());
        u.set_word(super::weather::WIND_Y, (y.sin() * WIND * DT).to_bits());
        u.set_word(super::weather::WIND_X + 8, 0);
        u.set_word(super::weather::TIMER, 1.0f32.to_bits());
        specks(w, id);
        let hero = w.hero;
        if hero.f658 == 0 && hero.f15d4 != 0xd && (hero.f0632 != 0 || hero.f65c != 0) && w.in_cuboid(h, c::pi32(w, id, o::PUSH)) {
            c::dec_timer_pvar_s16(w, id, o::RAMP);
            c::set_pi16(w, id, o::PUSHING, 1);
            let k = (1.0 - c::pi16(w, id, o::RAMP) as f32 * c::pf(w, id, o::RAMP_INV)) * c::pf(w, id, o::SPEED) * DT;
            push(w, id, k);
            let t = w.ticks(DECAY_TICKS);
            c::set_pi16(w, id, o::DECAY, t as i16);
            c::set_pf(w, id, o::DECAY_INV, 1.0 / t as f32);
            return;
        }
    }
    if c::pi16(w, id, o::PUSHING) == 0 || c::pi16(w, id, o::DECAY) == 0 { return; }
    if c::dec_timer_pvar_s16(w, id, o::DECAY) != 0 {
        c::set_pi16(w, id, o::PUSHING, 0);
        let t = w.ticks(c::pi16(w, id, o::RAMP_T) as i32);
        c::set_pi16(w, id, o::RAMP, t as i16);
        c::set_pf(w, id, o::RAMP_INV, 1.0 / t as f32);
    }
    let hero = w.hero;
    if hero.f658 != 0 || hero.f0632 == 0 {
        let cap = w.ticks(0xf);
        if cap < c::pi16(w, id, o::DECAY) as i32 {
            c::set_pi16(w, id, o::DECAY, cap as i16);
            c::set_pf(w, id, o::DECAY_INV, 1.0 / cap as f32);
        }
    }
    let k = (c::pf(w, id, o::SPEED) * DT - 0.0) * c::pi16(w, id, o::DECAY) as f32 * c::pf(w, id, o::DECAY_INV) + 0.0;
    push(w, id, k);
}

/// The draw's frame part: the camera kept.
pub fn frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    let cam = crate::hero::physics::to_f32x3(w.camera);
    c::set_pv4(w, id, o::CAM, [cam[0], cam[1], cam[2], 1.0]);
}

/// Level14 `0x2eb810` (module doc).
pub fn fx_quads(table: &MobyTable, _svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= o::SIZE)?;
    let cam = [p::ff(&m.pvars, o::CAM), p::ff(&m.pvars, o::CAM + 4), p::ff(&m.pvars, o::CAM + 8)];
    let mut quads = Vec::new();
    for i in 0..0x30 {
        if p::i16(&m.pvars, o::TIMER + 2 * i) == 0 { continue; }
        let q = [p::ff(&m.pvars, o::PTS + 0x10 * i), p::ff(&m.pvars, o::PTS + 0x10 * i + 4), p::ff(&m.pvars, o::PTS + 0x10 * i + 8)];
        let s = p::ff(&m.pvars, o::PTS + 0x10 * i + 12);
        let rgba = 0x7f_7f7f | (p::i16(&m.pvars, o::ALPHA + 2 * i) as u32) << 24;
        let d = [q[0] - cam[0], q[1] - cam[1], q[2] - cam[2]];
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let d = if n == 0.0 { d } else { d.map(|x| x / n) };
        // FastVecCross(out, d, up) = up × d; then FastVecCross(out, side, d) = d × side.
        let side = { let v = [-d[1], d[0], 0.0]; let l = (v[0] * v[0] + v[1] * v[1]).sqrt(); if l == 0.0 { v } else { v.map(|x| x / l) } };
        let up = [d[1] * side[2] - d[2] * side[1], d[2] * side[0] - d[0] * side[2], d[0] * side[1] - d[1] * side[0]];
        let corners = [[1.0f32, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]].map(|[y, z]| std::array::from_fn(|k| (side[k] * y + up[k] * z) * s + q[k]));
        quads.push(FxQuad { corners, st: [[1.0, 1.0], [0.0, 1.0], [1.0, 0.0], [0.0, 0.0]], rgba: [rgba; 4] });
    }
    Some(FxQuads { fx: 0x30, additive: true, subtract: false, quads })
}
