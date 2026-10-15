//! The swinging lasers, class 123: level15 0x2a6e68, the same code on 17 (census U474; 46 created instances). A
//! laser emitter moves about its placed point by its kind (pvar +0x20): 0 circles at radius r in the vertical plane
//! across its yaw, 1 / 2 slide r·cos t along its heading (1) or up and down (2), 3 / 4 swing as a pendulum of
//! length r (±90°; 4 turned a quarter), always at the pvar speed along the path; its animation speed follows. Every
//! tick its beam runs 12 units along the moby's z row: a line with a hit template (damage 1, 0x10001) hits what it
//! crosses, a world line ends it, 40 short type-60 sparks run along it, and the draw callback puts one camera-facing
//! quad on it (0x2a7628). Read from the level15 decomp (0x2a6e68, 0x2a7628); the data (gp−0x56a8.., the ST table
//! 0x1e27f0) are the same bytes on 17. Native `f32`; the `rand` draws in the game's order.
//!
//! **Pvar block** (f32 unless noted): +0x00 the placed point (v4), +0x10 the beam's end (v4), +0x20 s32 the kind,
//! +0x24 r, +0x28 the path angle t (degrees in the data, radians after the init), +0x2c the plane's yaw, +0x30 the
//! speed.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2a6e68 state 0 | +0x00 = position, anim speed +0x58 = speed / 1.5, t ·= π/180; kind 0 → state 1, +0x2c = yaw; 1–2 → state 3; ≥ 3 → state 2, +0x2c = yaw | [`update`] |
//! | state 1 | t += speed/r·dt (`fast_add_rotations`, the game's `360/(2πr/speed)·π/180`); anim speed; offset = (0, 0, r) turned by t about (cos b, sin b, 0) (`0x274ac8`); position = placed + offset; rows = `EulerToMatrix(0, π − t, b + π/2)`, `NormaliseMatrixColumns` (0x271030), mode \| 0x100 | [`update`] (`blaster_shot::rotate`, `moby_anim::normalise_columns`) |
//! | state 2 | a(t) = `FastNormalizeAngle(cos t · π/2)` (+ π/2 for kind 4); a0 = a(t), t += rate, a1 = a(t); anim speed = r·(a1 − a0)/(1.5·dt); offset = (0, 0, −r) turned by a1; rows = Euler(0, −a1, b + π/2), normalised, mode \| 0x100 | [`update`] |
//! | state 3 | anim speed = (r cos t1 − r cos t0)/(1.5·dt); position = placed + `0x277b50(r cos t1, kind 2 ? 0 : yaw, kind 2 ? π/2 : 0)` | [`update`] (`targeting::polar`) |
//! | every state | to = position + rows · (0, 0, 12) (0x2215e0); template: attacker m, flags 0x10001, damage 1, w20 1, type bytes (1, 1), the class, dir = unit(to − position) with z 1 and the push marker; `CollLine_Fix(pos, to, 9, m, tmpl)`: the hit on the moby it crosses | [`beam`] (`services::line_hit_in`) |
//! | | `CollLine_Fix(pos, to, 2)`: miss → end = to, hit → end = the hit point (0x174460) | [`beam`] (`World::line`) |
//! | | ×40 (gp−0x56a8): s = rand() & 1 ? 1 : −1; life = trunc(`multiply_global_scale(randf(2.5, 5))`); v = (end − pos)·s; from = s < 0 ? end : pos; v ·= `randf(0, 0.9)`; from += v; speed = `randf(10·dt, \|v\|/(4·life))`; `PartType60Spawn(0.75, from, unit(v)·speed, 0x604040ff, life, randi(0xff), 0)` | [`beam`] (`World::part60`) |
//! | | `RegisterDrawCallback(0x2a7628, m)` (list 1) | [`beam`] (`Callback::UnitQuads`) |
//! | | the old position's motion `pos − pos₀` written next to the template: never read [L] | n/a |
//! | 0x2a7628 | start = pos + unit(end − pos)·0.5; e = start − end; a = unit((start − cam) × e)·0.1, b = unit((end − cam) × e)·0.1 (`FastVecCross(out, x, y)` = y × x); quad start ∓ a, end ∓ b; RGBA 0x80243278, FX 0xe, ALPHA 0x48 (additive), ST 0x1e27f0 | [`fx_quads`] + `rc-engine` fx_draw; the camera `0x1673c0` is the one the update saw [L] |
//! | | no sound, light, save flag, bolt | n/a |

use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::blaster_shot::rotate;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{add_rot, pf, pi32, pv4, set_pf, set_pv4, sub_rot, DT};
use crate::moby_update::services::{self as sv, HitTemplate, Services, World};
use crate::ps2v::Pf;

use super::{FxQuad, FxQuads};

/// The update in the level15 class table.
pub const UPDATE_FN: u32 = 0x2a_6e68;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [123];

/// The camera the beam's quad faces (`0x1673c0`), kept by the update (three words; `Services::units`).
pub const CAMERA_WORD: u32 = 0x16_73c0;

/// Pvar offsets.
pub mod pv {
    pub const HOME: usize = 0x00;
    pub const END: usize = 0x10;
    pub const KIND: usize = 0x20;
    pub const RADIUS: usize = 0x24;
    pub const ANGLE: usize = 0x28;
    pub const YAW: usize = 0x2c;
    pub const SPEED: usize = 0x30;
}

/// The beam's length (along the z row).
pub const REACH: f32 = 12.0;
/// gp−0x56a8..−0x56a0: sparks per tick, their size, their life (ticks, drawn in [½, 1]·).
pub const SPARKS: i32 = 40;
pub const SPARK_SIZE: f32 = 0.75;
pub const SPARK_LIFE: f32 = 5.0;
pub const SPARK_RGBA: u32 = 0x6040_40ff;
/// The quad: RGBA, FX texture, ST (0x1e27f0), half width.
pub const BEAM_RGBA: u32 = 0x8024_3278;
pub const FX: usize = 0xe;
pub const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const PI: f32 = f32::from_bits(0x4049_0fdb);

type V = [f32; 4];
fn sub(a: V, b: V) -> V { [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]] }
fn len(v: V) -> f32 { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() }
fn with_len(v: V, l: f32) -> V { let n = len(v); if n == 0.0 { [0.0, 0.0, 0.0, v[3]] } else { [v[0] * l / n, v[1] * l / n, v[2] * l / n, v[3]] } }

/// The path rate `360 / (2πr / speed) · π/180 · dt` as the game computes it.
fn rate(w: &World, id: MobyId) -> f32 {
    let (r, s) = (pf(w, id, pv::RADIUS), pf(w, id, pv::SPEED));
    (360.0 / ((r * f32::from_bits(0x40c9_0fdb)) / s)) * f32::from_bits(0x3c8e_fa35) * DT
}

/// `EulerToMatrix` into the moby's rows, `NormaliseMatrixColumns`, mode | 0x100.
fn set_rows(w: &mut World, id: MobyId, e: [f32; 3]) {
    let r = crate::follow_camera::script::euler_rows(e);
    let mut rows = [0, 1, 2].map(|k| [r[k][0].to_bits(), r[k][1].to_bits(), r[k][2].to_bits(), 0]);
    rc_formats::moby_anim::normalise_columns(&mut rows);
    let m = w.mm(id);
    for (row, r) in m.rows.iter_mut().zip(rows) { *row = r.map(f32::from_bits); }
    m.mode |= mode::KEEP_ROWS;
}

/// Level15 0x2a6e68 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x34 { return; }
    let pos0 = w.m(id).position;
    let home = pv4(w, id, pv::HOME);
    let kind = pi32(w, id, pv::KIND);
    let r = pf(w, id, pv::RADIUS);
    let b = pf(w, id, pv::YAW);
    match w.m(id).state {
        0 => {
            set_pv4(w, id, pv::HOME, pos0);
            let s = pf(w, id, pv::SPEED) / 1.5;
            w.mm(id).anim.speed = s;
            let a = pf(w, id, pv::ANGLE) * f32::from_bits(0x3c8e_fa35);
            set_pf(w, id, pv::ANGLE, a);
            let yaw = w.m(id).rotation[2];
            if kind == 0 { w.mm(id).state = 1; set_pf(w, id, pv::YAW, yaw); } else if kind < 3 { w.mm(id).state = 3; } else { w.mm(id).state = 2; set_pf(w, id, pv::YAW, yaw); }
        }
        1 => {
            let t = add_rot(pf(w, id, pv::ANGLE), rate(w, id));
            set_pf(w, id, pv::ANGLE, t);
            w.mm(id).anim.speed = pf(w, id, pv::SPEED) / 1.5;
            let o = rotate([0.0, 0.0, r], t, [b.cos(), b.sin(), 0.0]);
            w.mm(id).position = [home[0] + o[0], home[1] + o[1], home[2] + o[2], home[3]];
            set_rows(w, id, [0.0, add_rot(-t, PI), add_rot(b, HALF_PI)]);
        }
        2 => {
            let tilt = if kind == 4 { HALF_PI } else { 0.0 };
            let swing = |t: f32| add_rot(sv::fl(sv::normalize_angle(sv::pf(t.cos() * HALF_PI))), tilt);
            let a0 = swing(pf(w, id, pv::ANGLE));
            let t = add_rot(pf(w, id, pv::ANGLE), rate(w, id));
            set_pf(w, id, pv::ANGLE, t);
            let a1 = swing(t);
            w.mm(id).anim.speed = (r * sub_rot(a1, a0)) / (DT * 1.5);
            let o = rotate([0.0, 0.0, -r], a1, [b.cos(), b.sin(), 0.0]);
            w.mm(id).position = [home[0] + o[0], home[1] + o[1], home[2] + o[2], home[3]];
            set_rows(w, id, [0.0, add_rot(-a1, 0.0), add_rot(b, HALF_PI)]);
        }
        3 => {
            let t0 = pf(w, id, pv::ANGLE);
            let c0 = t0.cos();
            let t = add_rot(t0, rate(w, id));
            set_pf(w, id, pv::ANGLE, t);
            let c1 = t.cos() * r;
            w.mm(id).anim.speed = (c1 - c0 * r) / (DT * 1.5);
            let (yaw, pitch) = if kind == 2 { (0.0, HALF_PI) } else { (w.m(id).rotation[2], 0.0) };
            let o = crate::targeting::polar(c1, yaw, pitch);
            w.mm(id).position = [home[0] + o[0], home[1] + o[1], home[2] + o[2], home[3]];
        }
        _ => {}
    }
    beam(w, id);
}

/// The beam of every tick (module doc: the hit line, the end, the sparks, the draw callback).
pub fn beam(w: &mut World, id: MobyId) {
    let pos = w.m(id).position;
    let z = w.m(id).rows[2];
    let to = [pos[0] + z[0] * REACH, pos[1] + z[1] * REACH, pos[2] + z[2] * REACH, pos[3]];
    let d = with_len(sub(to, pos), 1.0);
    let tmpl = HitTemplate { dir: [sv::pf(d[0]), sv::pf(d[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x1_0001, b18: 1, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 };
    sv::line_hit_in(w.table, w.svc, w.classes, w.coll, sv::pv(pos), sv::pv(to), 9, Some(id), &tmpl);
    let end = match w.line(sv::pv(pos), sv::pv(to), 2, None) {
        None => to,
        Some(h) => [sv::fl(h.point[0]), sv::fl(h.point[1]), sv::fl(h.point[2]), sv::fl(h.point[3])],
    };
    set_pv4(w, id, pv::END, end);
    let scale = w.svc.timing.timer_scale.to_f32();
    for _ in 0..SPARKS {
        let s = if w.rng.rand() & 1 != 0 { 1.0 } else { -1.0 };
        let life = (w.rng.randf(SPARK_LIFE * 0.5, SPARK_LIFE) * scale) as i32;
        let mut v = sub(end, pos).map(|x| x * s);
        let l = len(v);
        let mut from = if s < 0.0 { end } else { pos };
        let k = w.rng.randf(0.0, f32::from_bits(0x3f66_6666));
        v = v.map(|x| x * k);
        for i in 0..4 { from[i] += v[i]; }
        let sp = w.rng.randf(DT * 10.0, l / ((life << 2) as f32));
        let v = with_len(v, sp);
        let rot = w.rng.randi(0xff) as u8;
        w.part60(SPARK_SIZE, from, v, SPARK_RGBA, (life & 0xff) as u16, rot, 0);
    }
    let c = w.camera_point();
    for (k, x) in c.iter().enumerate() { w.svc.units.set_word(CAMERA_WORD + 4 * k as u32, x.to_bits()); }
    if let Some(i) = super::row(REFERENCE_LEVEL, UPDATE_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
}

/// `FastVecCross(out, x, y)` = y × x.
fn cross_yx(x: [f32; 3], y: [f32; 3]) -> [f32; 3] { [y[1] * x[2] - y[2] * x[1], y[2] * x[0] - y[0] * x[2], y[0] * x[1] - y[1] * x[0]] }

/// Level15 0x2a7628 for the laser `id` (module doc): one quad along its beam.
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < 0x34 { return None; }
    let end = sv::pvar::v4f(&m.pvars, pv::END);
    let p = m.position;
    let d = with_len(sub(end, p), 0.5);
    let start = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
    let e3 = [end[0], end[1], end[2]];
    let cam: [f32; 3] = std::array::from_fn(|k| f32::from_bits(svc.units.word(CAMERA_WORD + 4 * k as u32)));
    let e: [f32; 3] = std::array::from_fn(|k| start[k] - e3[k]);
    let unit = |v: [f32; 3], l: f32| { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if n == 0.0 { [0.0; 3] } else { v.map(|x| x * l / n) } };
    let a = unit(cross_yx(std::array::from_fn(|k| start[k] - cam[k]), e), 0.1);
    let b = unit(cross_yx(std::array::from_fn(|k| e3[k] - cam[k]), e), 0.1);
    let corners = [
        std::array::from_fn(|k| start[k] - a[k]),
        std::array::from_fn(|k| start[k] + a[k]),
        std::array::from_fn(|k| e3[k] - b[k]),
        std::array::from_fn(|k| e3[k] + b[k]),
    ];
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads: vec![FxQuad { corners, st: ST, rgba: [BEAM_RGBA; 4] }] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    fn laser(kind: i32) -> Moby {
        let mut m = Moby { o_class: 123, pvars: vec![0; 0x34], ..Moby::default() };
        m.position = [10.0, 10.0, 10.0, 1.0];
        m.rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0; 4]];
        p::set_i32(&mut m.pvars, pv::KIND, kind);
        p::set_ff(&mut m.pvars, pv::RADIUS, 2.0);
        p::set_ff(&mut m.pvars, pv::SPEED, 4.0);
        m
    }

    #[test]
    fn paths_keep_their_radius_and_the_beam_reaches_twelve() {
        for kind in [0, 1, 2, 3, 4] {
            let mut t = MobyTable::new(vec![laser(kind)], 4);
            let hero = crate::hero::Hero::new();
            let mut rng = crate::rng::Rng::new();
            let classes = crate::moby_update::ClassTable::default();
            let mut svc = Services::new();
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1);
            update(&mut w, 0);
            let home = pv4(&w, 0, pv::HOME);
            assert_eq!(w.m(0).state, match kind { 0 => 1, 1 | 2 => 3, _ => 2 });
            for _ in 0..30 { update(&mut w, 0); }
            let o = sub(w.m(0).position, home);
            match kind {
                0 | 3 | 4 => assert!((len(o) - 2.0).abs() < 1e-3, "kind {kind}: {o:?}"),
                _ => assert!(len(o) <= 2.0 + 1e-3, "kind {kind}: {o:?}"),
            }
            if kind == 2 { assert!(o[0].abs() < 1e-5 && o[1].abs() < 1e-5, "up and down"); }
            // No mesh: the beam ends 12 along the z row.
            let end = pv4(&w, 0, pv::END);
            assert!((len(sub(end, w.m(0).position)) - REACH).abs() < 1e-3);
            assert_eq!(w.svc.draw_callbacks.list1.len(), 31);
            let q = fx_quads(w.table, w.svc, 0).unwrap();
            assert_eq!((q.quads.len(), q.fx, q.additive, q.quads[0].rgba[0]), (1, FX, true, BEAM_RGBA));
        }
    }
}
