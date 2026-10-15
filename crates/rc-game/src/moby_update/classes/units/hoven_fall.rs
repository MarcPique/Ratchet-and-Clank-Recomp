//! **Hoven's falls, class 384** (level12 `0x2ec720`, its draw `0x2ecac0`; census U432; 3 placed). A curtain of eight
//! strips in the moby's frame (x 0.6, y −0.4..0.4 by 0.1, from an arch `cos(1.1·y)` down to −6; the arch flipped to
//! `2 − cos` while it pours) whose colour pulses between 0x40602020 and 0x40681818 with the sine of a phase turning
//! at 180°/s. Its height (pvar +0x08, 5 at the start) eases toward 3 while it is off and toward −1 once it pours
//! (6·dt a tick). A command 8 turns it on, 4 off (+0xbc 1 / 2), each with a ramp of 105 ticks during which drips (type
//! 2) fall from it with a probability rising (on) or falling (off) along the ramp. Nothing moves while the screen is
//! faded (0x15f3fc ≠ 0).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ec720` | the fall (module doc); a drip: three `rand_vec(0, 0.25)` A, B, C; velocities (A − B) and (B − C), each 2 higher, × 2/40 (z negated while on); at A + position, z −1 (on: +3); sizes `randf(0.05, 0.1)`; colours 0x40404040; times 1, `ticks(40)`, 1 | [`update`] |
//! | `0x2ecac0` | the strips: FX 0x10, ALPHA 0x44, ST 0.5 everywhere, the colour on every corner, the frame its rows at (x, y, z + height) | [`fx_quads`] (`Callback::UnitQuads`) |
//!
//! Read from the level12 decomp and disassembly (the corner stores); the corner table 0x1f5b20 and gp−0x5238..−0x51dc.
//! Native `f32`.

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx, turn};
use crate::moby_update::services::{pvar as p, Services, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x2e_c720;
pub const DRAW_FN: u32 = 0x2e_cac0;
pub const CLASSES: [i16; 1] = [384];

const DT: f32 = c::DT;
/// gp−0x5230: the ramp's length (raw ticks in the division, `ticks` for the timer).
const RAMP: i32 = 105;
/// gp−0x5214: the drips' life.
const DRIP_T: i32 = 40;

mod pv {
    pub const PHASE: usize = 0x00;
    pub const TIMER: usize = 0x04;
    pub const HEIGHT: usize = 0x08;
    pub const SIZE: usize = 0x0c;
}

/// Level12 `0x2ec720` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pv::SIZE);
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.cmd = 2;
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            c::set_pf(w, id, pv::HEIGHT, 5.0);
        }
        1 => {
            let cmd = w.m(id).cmd;
            let flip = if cmd & 8 != 0 { Some(1) } else if cmd & 4 != 0 { Some(2) } else { None };
            if let Some(v) = flip {
                w.mm(id).cmd = v;
                let t = w.ticks(RAMP);
                c::set_pi32(w, id, pv::TIMER, t);
            }
            let ph = c::add_rot(c::pf(w, id, pv::PHASE), 180.0 * 0.017_453_292 * DT);
            c::set_pf(w, id, pv::PHASE, ph);
        }
        _ => {}
    }
    if w.svc.cinematic.fade == 0.0 {
        if c::dec_timer_pvar_i32(w, id, pv::TIMER) == 0 {
            let on = w.m(id).cmd & 1 != 0;
            let t = c::pi32(w, id, pv::TIMER) as f32 / RAMP as f32;
            let frac = if on { 1.0 - t } else { t };
            if w.rng.randf(0.0, 1.0) < frac { drip(w, id, on); }
        }
        let cmd = w.m(id).cmd;
        let mut h = c::pf(w, id, pv::HEIGHT);
        if cmd & 1 == 0 || c::pi32(w, id, pv::TIMER) != 0 {
            if cmd & 2 != 0 { turn::approach(3.0, 6.0 * DT, &mut h); }
        } else {
            turn::approach(-1.0, 6.0 * DT, &mut h);
        }
        c::set_pf(w, id, pv::HEIGHT, h);
    }
    if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(row), id); }
}

/// A drip (module doc).
fn drip(w: &mut World, id: MobyId, on: bool) {
    let v3 = |w: &mut World| { let v = w.rng.rand_vec(0.0, 0.25); [v[0], v[1], v[2], 0.0] };
    let (a, b, cc) = (v3(w), v3(w), v3(w));
    let k = 2.0 / DRIP_T as f32;
    let mut v1 = c::sub(a, b);
    v1[2] += 1.0 - -1.0;
    let mut v2 = c::sub(b, cc);
    v2[2] += 3.0 - 1.0;
    let mut v1 = c::scale(v1, k);
    let mut v2 = c::scale(v2, k);
    let mut pos = c::add(a, c::pos(w, id));
    let mut z0 = -1.0;
    if on {
        v1[2] = -v1[2];
        v2[2] = -v2[2];
        z0 = 3.0;
    }
    pos[2] += z0;
    v1[3] = w.rng.randf(0.05, 0.1);
    v2[3] = w.rng.randf(0.05, 0.1);
    let t = w.ticks(DRIP_T);
    fx::part02(w, &crate::particles::type02::Spawn { pos, v1, v2, c1: 0x4040_4040, c2: 0x4040_4040, t: [1, t, 1], def: -1 });
}

/// Level12 `0x2ecac0`: the strips (module doc).
pub fn fx_quads(table: &MobyTable, _svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= pv::SIZE)?;
    let (r, pos) = (m.rows, m.position);
    let h = p::ff(&m.pvars, pv::HEIGHT);
    let base = [pos[0], pos[1], pos[2] + h];
    let to_world = |v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| r[0][i] * v[0] + r[1][i] * v[1] + r[2][i] * v[2] + base[i]) };
    let f = (p::ff(&m.pvars, pv::PHASE).sin() + 1.0) * 0.5;
    let col = crate::particles::tween_color(f.to_bits(), 0x4060_2020, 0x4068_1818);
    let on = m.cmd & 1 != 0;
    let arch = |y: f32| { let z = (y * 1.1).cos(); if on { -z + 2.0 } else { z } };
    let mut quads = Vec::new();
    let (mut a, mut b) = (-0.4f32, -0.3f32);
    loop {
        let corners = [[0.6, a, arch(a)], [0.6, b, arch(b)], [0.6, a, -6.0], [0.6, b, -6.0]].map(to_world);
        quads.push(FxQuad { corners, st: [[0.5; 2]; 4], rgba: [col; 4] });
        a += 0.1;
        if 0.36 <= a { break; }
        b = a + 0.1;
    }
    Some(FxQuads { fx: 0x10, additive: false, subtract: false, quads })
}
