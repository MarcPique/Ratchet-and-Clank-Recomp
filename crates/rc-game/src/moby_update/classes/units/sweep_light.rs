//! U180: the sweeping searchlights, class 823 (Rilgar 05, Blarg 07; level05 0x30c0a8, its draw callback 0x30c220;
//! level07 0x304738 is the same code). The lamp swings its yaw about its placed heading and tilts its head (joint list
//! 0) with a manipulator (`crate::moby_update::manip`), and its callback draws the light beam from the head. Read from
//! the level05 disassembly and data (0x1d67b0.., gp−0x4f0c..−0x4ef4). Native `f32`.
//!
//! Pvar block: +0x00 the manipulator record, +0x40 the placed yaw, +0x44 / +0x48 the yaw / tilt phases, +0x4c / +0x50
//! their rates.
//!
//! | address | what it does | port |
//! |---|---|---|
//! | 0x30c0d4..0x30c14c | state 0: P+0x40 = yaw; P+0x48, P+0x44 = `random_angle_radians`; P+0x50 = `randf_sym(0.4712·dt, 0.5236·dt)`, P+0x4c = `randf_sym(0.2094·dt, 0.2618·dt)` (in that order); ambient 0x80 0x80 0x80 (`0x2650d0`); `AttachManipulator(m, 0, P)`; state 1 | [`update`] ([`manip::attach`]) |
//! | 0x30c160 | state 1: `RegisterDrawCallback(0x30c220, m)` (list 1) | [`update`] (`Callback::UnitQuads`, the frame in [`DrawCallbacks::matrices`](crate::moby_update::classes::draw_callbacks::DrawCallbacks)) |
//! | 0x30c168..0x30c19c | P+0x44 += P+0x4c; yaw = P+0x40 + sin(P+0x44)·0.5236 (`fast_add_rotations`) | [`update`] |
//! | 0x30c1a0..0x30c1d8 | P+0x48 += P+0x50; `FUN_00221e38(sin(P+0x48)·0.5236 + 0.5236, P+0x10, 1)` (the head's tilt about y) | [`update`] ([`manip::set_axis`]) |
//! | other states | nothing | — |
//! | 0x30c220 | the beam: at the head (joint point of list 0, `0x279a30`), the frame of Euler (0, tilt, yaw) with x = z × to-camera, y = x × z; ten segments i = 0..9 of two quads (FX 0x13, ALPHA 0x48: additive), vertices (±0.8, 0, 0 / 10) and (±0.4, 0, 0 / 12) with x ± t·8 / t·6, z + i·10 / i·12, t = (i + k&1)·0.1, colour `FastTweenColor(t, 0x40807070, 0x00804040)`, ST (0.5, 1) (0.5, 1) (0.5, 0) (0.5, 0) | [`fx_quads`] + `rc-engine` fx_draw; the frame is built at registration from the camera the update saw [L] |
//!
//! Side effects: the moby's yaw and ambient, its joint list, the draw callback (no sound, particles or other mobys).

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{add_rot, pf, set_pf};
use crate::moby_update::manip;
use crate::moby_update::services::{Services, World};

use super::{FxQuad, FxQuads};

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x30_c0a8;
pub const CLASSES: [i16; 1] = [823];
/// The beam's FX texture (gp−0x4ef4 = 0x13).
pub const FX: usize = 0x13;
/// `FastTweenColor` ends (gp−0x4efc, gp−0x4ef8).
pub const COLOR_NEAR: u32 = 0x4080_7070;
pub const COLOR_FAR: u32 = 0x0080_4040;
const REC: usize = 0x00;
const HOME: usize = 0x40;
const YAW_PHASE: usize = 0x44;
const TILT_PHASE: usize = 0x48;
const YAW_RATE: usize = 0x4c;
const TILT_RATE: usize = 0x50;
const SIZE: usize = 0x54;
/// 0x3f060a92: π/6.
const SWING: f32 = f32::from_bits(0x3f06_0a92);

/// Level05 0x30c0a8.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { w.mm(id).pvars.resize(SIZE, 0); }
    let dt = crate::moby_update::services::fl(crate::moby_update::services::DT);
    match w.m(id).state {
        0 => {
            let yaw = w.m(id).rotation[2];
            set_pf(w, id, HOME, yaw);
            let a = w.rng.rand_angle();
            set_pf(w, id, TILT_PHASE, a);
            let a = w.rng.rand_angle();
            set_pf(w, id, YAW_PHASE, a);
            let r = w.rng.randf_sym(dt * f32::from_bits(0x3ef1_4639), dt * SWING);
            set_pf(w, id, TILT_RATE, r);
            let r = w.rng.randf_sym(dt * f32::from_bits(0x3e56_7750), dt * f32::from_bits(0x3e86_0a92));
            set_pf(w, id, YAW_RATE, r);
            w.mm(id).ambient[..3].copy_from_slice(&[0x80; 3]);
            manip::attach(w, id, 0, id, REC);
            w.mm(id).state = 1;
        }
        1 => {
            let yp = add_rot(pf(w, id, YAW_PHASE), pf(w, id, YAW_RATE));
            set_pf(w, id, YAW_PHASE, yp);
            let yaw = add_rot(pf(w, id, HOME), yp.sin() * SWING);
            w.mm(id).rotation[2] = yaw;
            let tp = add_rot(pf(w, id, TILT_PHASE), pf(w, id, TILT_RATE));
            set_pf(w, id, TILT_PHASE, tp);
            manip::set_axis(w, id, id, REC, tp.sin() * SWING + SWING, 1);
            let m = beam_frame(w, id);
            let i = super::PORTS.iter().position(|u| u.level == REFERENCE_LEVEL && u.func == UPDATE_FN).unwrap_or(0) as u16;
            w.svc.draw_callbacks.register_with_matrix(Callback::UnitQuads(i), id, m);
        }
        _ => {}
    }
}

/// The callback's frame (module doc): rows x, y, z of Euler (0, tilt, yaw) turned about z to face the camera, and the
/// head's joint point.
fn beam_frame(w: &World, id: MobyId) -> [[f32; 4]; 4] {
    let j = w.joint_point(id, 0);
    let e = [0.0, pf(w, id, TILT_PHASE).sin() * SWING + SWING, w.m(id).rotation[2]];
    let r = rc_formats::moby_light::rotation_rows(e).map(|r| r.map(f32::from_bits));
    let z = [r[2][0], r[2][1], r[2][2]];
    let cam = w.camera.map(|x| x.to_f32());
    let c = norm([cam[0] - j[0], cam[1] - j[1], cam[2] - j[2]]);
    let x = cross(z, c);
    let y = cross(x, z);
    [[x[0], x[1], x[2], 0.0], [y[0], y[1], y[2], 0.0], [z[0], z[1], z[2], 0.0], [j[0], j[1], j[2], 1.0]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { v } else { v.map(|x| x / l) }
}

/// `FastTweenColor(t, a, b)`: each byte `a + (b − a)·t`.
fn tween(t: f32, a: u32, b: u32) -> u32 {
    (0..4).fold(0, |acc, k| {
        let (x, y) = (((a >> (8 * k)) & 0xff) as f32, ((b >> (8 * k)) & 0xff) as f32);
        acc | ((((x + (y - x) * t) as i32).clamp(0, 255) as u32) << (8 * k))
    })
}

/// The beam's quads (0x30c220) for moby `id`, from the frame its update registered.
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    table.mobys.get(id)?;
    let m = svc.draw_callbacks.matrices.get(&id)?;
    let world = |v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| v[0] * m[0][k] + v[1] * m[1][k] + v[2] * m[2][k] + m[3][k]) };
    const ST: [[f32; 2]; 4] = [[0.5, 1.0], [0.5, 1.0], [0.5, 0.0], [0.5, 0.0]];
    let mut quads = Vec::with_capacity(20);
    for i in 0..10 {
        for (half, width, len) in [(0.8f32, 8.0f32, 10.0f32), (0.4, 6.0, 12.0)] {
            let mut corners = [[0.0; 3]; 4];
            let mut rgba = [0; 4];
            for k in 0..4 {
                let t = (i + (k & 1)) as f32 * 0.1;
                let sign = if k > 1 { -1.0 } else { 1.0 };
                let base = [half * sign, 0.0, if k & 1 == 1 { len } else { 0.0 }];
                corners[k] = world([base[0] + t * width * sign, 0.0, base[2] + i as f32 * len]);
                rgba[k] = tween(t, COLOR_NEAR, COLOR_FAR);
            }
            quads.push(FxQuad { corners, st: ST, rgba });
        }
    }
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::Moby;
    use crate::moby_update::services::pvar as p;

    #[test]
    fn sweeps_tilts_and_registers_its_beam() {
        let m = Moby { o_class: 823, pvars: vec![0; SIZE], rotation: [0.0, 0.0, 1.0, 0.0], ..Moby::default() };
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = Services::new();
        svc.joint_targets.insert(823, vec![3]);
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        let mut r = *w.rng;
        update(&mut w, 0);
        let dt = 1.0 / 60.0f32;
        let (tp, yp) = (r.rand_angle(), r.rand_angle());
        let (tr, yr) = (r.randf_sym(dt * f32::from_bits(0x3ef1_4639), dt * SWING), r.randf_sym(dt * f32::from_bits(0x3e56_7750), dt * f32::from_bits(0x3e86_0a92)));
        let pv = &w.m(0).pvars;
        assert_eq!([p::ff(pv, HOME), p::ff(pv, TILT_PHASE), p::ff(pv, YAW_PHASE), p::ff(pv, TILT_RATE), p::ff(pv, YAW_RATE)], [1.0, tp, yp, tr, yr]);
        assert_eq!((w.m(0).state, &w.m(0).ambient[..3]), (1, &[0x80u8; 3][..]));
        assert_eq!(w.m(0).joint_mods.len(), 1);
        update(&mut w, 0);
        let yp1 = add_rot(yp, yr);
        assert_eq!(w.m(0).rotation[2], add_rot(1.0, yp1.sin() * SWING));
        let tilt = add_rot(tp, tr).sin() * SWING + SWING;
        assert_eq!(w.m(0).joint_mods[0].quat, crate::hero::idle::axis_quat(tilt, 1));
        assert!(w.svc.draw_callbacks.matrices.contains_key(&0));
        // The beam: 20 quads, FX 0x13 additive; the first segment starts at the head (joint point: the origin here).
        let q = fx_quads(w.table, w.svc, 0).unwrap();
        assert_eq!((q.quads.len(), q.fx, q.additive), (20, FX, true));
        assert_eq!(q.quads[0].rgba, [COLOR_NEAR, tween(0.1, COLOR_NEAR, COLOR_FAR), COLOR_NEAR, tween(0.1, COLOR_NEAR, COLOR_FAR)]);
        let c0 = q.quads[0].corners;
        assert!((c0[0][0].powi(2) + c0[0][1].powi(2) + c0[0][2].powi(2)).sqrt() - 0.8 < 1e-5);
        // The last segment's far end: t = 1 (alpha 0), 100 + 10 along the beam axis.
        assert_eq!(q.quads[18].rgba[1] >> 24, 0);
    }
}
