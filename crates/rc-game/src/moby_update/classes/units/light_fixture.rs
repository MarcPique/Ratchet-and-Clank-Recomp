//! U207 / U472 (census 2026-09-29): class 1511, the breakable light fixtures of Rilgar and Quartu (level05 `0x31c8e0`:
//! 26 created, level14 `0x307ad8`: 22; the same code, its `$gp` tuning words at other offsets with the same values).
//! A lamp with a camera-facing glow that fades with the camera's distance; a hit near its head (or any hit of kind
//! 0x800000) breaks it: the class sound, the bare base 1514 left in its place, three pieces thrown off (`BreakFxB`,
//! G-CLS-026: the level's copy of the engine function), `BreakFxA`'s bolts, sparks and smoke, and the lamp is gone.
//! Read from the level05 decomp (the level14 copy compared line by line). The name is descriptive [L]. Native `f32`.
//!
//! **Pvars**: +0x60 the glow alpha (i32), +0x64..+0x66 the class glow colour (r, g, b), +0x67 the flicker flag,
//! +0x68 the spark colour (rgb, from +0x6c..+0x6e), +0x6f the full alpha, +0x70 the glow half size.
//!
//! | address | what it does | port |
//! |---|---|---|
//! | state 0 | → 1; moby glow +0x90 = `0x7f << 24 \| b66 << 16 \| b65 << 8 \| b64`; +0x68 = `b6e << 16 \| b6d << 8 \| b6c` | [`update`] |
//! | every tick | camera (0x1671c0 / L14 0x1674c0) farther than 48 (xy) → nothing | [`update`] (`World::camera`) |
//! | | +0x60 = full alpha within 32, fading to 0 at 48; `RegisterDrawCallback(0x31cf58)` | [`update`] (`Callback::UnitQuads`) |
//! | | `MobyGetHitMessage(m, 0x810000, 0)`; +0xa4 = 0xff; the head = pos + up·4 (gp 0x161fc8) | [`update`] (`World::get_hit`) |
//! | | breaks on a hit of kind 0x800000, a hit within 0.8 of the head, or an attacker of class 0x47 within 0.8 of it | [`update`] |
//! | break | `PlayClassSound(0, 0, m)` | [`update`] (`World::play_sound`) |
//! | | `0x31d160`: the base 1514 (0x5ea): update / draw distance, Euler, light word and ambient, position; drawn; `MobyBuildMatrix` (its update is `jr ra`) | [`base`] |
//! | | `BreakFxB(12·dt², m, 0x5ed, pos, rot, ticks(90), 0, 0, 0)`; its light word = the lamp's | [`update`] (`fx::break_piece_with`) |
//! | | `BreakFxA(m)`: `BoltBurst(m, 4, 7, …)` | [`update`] (`breakables::bolts`) |
//! | | two 0x5ee pieces with `v = row0·0.075·speed + (0, 0, 0.08·speed)` and `(−v.x, −v.y, v.z)`, the lamp's yaw turned by π in between (the lamp's own +0x48) | [`update`] |
//! | | 24 × (type 53 `(s/10, s, 0.003, head ± 0.15, ticks(60), +0x68 \| 0x7f000000, 0, ±1, v)`, type 53 `(0.07s, 0.7s, 0.003, …, 0x7f7f7f7f, 1, ∓1, v)`, type 23 smoke at the head: size `randf(60000, 200000)`, up `randf(0.01, 0.05)·speed`, 0x407f7f7f, life `ticks(rand_range(90, 120))`, phase 2, alpha `rand_range(64, 96)`, one in two blend 0x44), `s = randf(0.3, 4)`, `v = polar(randf(0.05, 0.1)·speed, rand_angle, randf(20°, 88°))` | [`sparks`] (`World::part53`, `particles::type23`) |
//! | | `DeleteMoby(m)` | [`update`] |
//! | 0x31cf58 (draw) | one quad of FX texture 11 (ALPHA 0x48) facing the camera at the head pulled 0.1 away from it, half size +0x70, alpha +0x60 | [`fx_quads`] |
//! | | +0x67 ≠ 0: `randi(8) == 0` halves +0x60 (a draw drawn in the render pass) | NOT ported: the port's draw pass has no game rand (the flicker; noted [L]) |
//! | | no light, save flag | n/a |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::breakables;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, add_rot, fx};
use crate::moby_update::services::{pf, pv, Services, World};
use super::{FxQuad, FxQuads};

pub const UPDATE_FN: u32 = 0x31_c8e0;
pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN_14: u32 = 0x30_7ad8;
pub const REFERENCE_LEVEL_14: u32 = 14;
pub const CLASSES: [i16; 1] = [1511];
pub const BASE_CLASS: i16 = 0x5ea;
pub const PIECE_A: i16 = 0x5ed;
pub const PIECE_B: i16 = 0x5ee;
/// The `$gp` tuning words (level05 0x161fc8.., level14 0x16229c..: the same values).
const HEAD: f32 = 4.0;
const PULL: f32 = -0.1;
const PIECE_OUT: f32 = 0.075;
const PIECE_UP: f32 = 0.08;
/// The effect texture and the quad corners (level05 0x215c70: (0, ∓1, ±1, 1)·size).
pub const FX: usize = 0xb;
const CORNERS: [[f32; 2]; 4] = [[-1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [1.0, -1.0]];
const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];

fn head(w: &World, id: MobyId) -> c::V {
    let m = w.m(id);
    [m.position[0] + m.rows[2][0] * HEAD, m.position[1] + m.rows[2][1] * HEAD, m.position[2] + m.rows[2][2] * HEAD, m.position[3]]
}

/// Level05 0x31c8e0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x74 { return; }
    if w.m(id).state == 0 {
        let m = w.mm(id);
        m.state = 1;
        let b = |k: usize| m.pvars[k] as u32;
        m.glow = b(0x66) << 16 | b(0x65) << 8 | 0x7f00_0000 | b(0x64);
        let s = b(0x6e) << 16 | b(0x6d) << 8 | b(0x6c);
        c::set_pi32(w, id, 0x68, s as i32);
    }
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let p = c::pos(w, id);
    let d = c::dist2(p, [cam[0], cam[1], cam[2], 0.0]);
    if 48.0 <= d { return; }
    let full = w.m(id).pvars[0x6f] as f32;
    let a = if 32.0 <= d { (full * (1.0 - (d - 32.0) * 0.0625)) as i32 } else { full as i32 };
    c::set_pi32(w, id, 0x60, a);
    let row = super::row(REFERENCE_LEVEL, UPDATE_FN);
    if let Some(i) = row {
        let f = glow_frame(head(w, id), cam);
        w.svc.draw_callbacks.register_with_matrix(Callback::UnitQuads(i), id, f);
    }
    let hit = w.get_hit(id, 0x81_0000, false);
    w.mm(id).hit_slot = 0xff;
    let Some(h) = hit else { return };
    let top = head(w, id);
    let near = |q: c::V| c::dist3(top, q) <= 0.8;
    let hp = crate::moby_update::services::fv(h.pos);
    let by = h.attacker.filter(|&k| w.m(k).o_class == 0x47).map(|k| c::pos(w, k));
    if !(h.flags & 0x80_0000 != 0 || near(hp) || by.is_some_and(near)) { return; }
    w.play_sound(0, 0, id);
    base(w, id);
    let (pos, light, amb) = { let m = w.m(id); (m.position, m.light, m.ambient) };
    let t = w.ticks(0x5a);
    let give_light = |w: &mut World, k: Option<MobyId>| if let Some(k) = k { let m = w.mm(k); m.light = light; m.ambient = amb; };
    let k = fx::break_piece_with(w, id, PIECE_A, pos, w.m(id).rotation, t, 0, [0.0; 4], [0.0; 4], [0.0; 4]);
    give_light(w, k);
    breakables::bolts(w, id);
    let r0 = w.m(id).rows[0];
    let mut v = [r0[0] * PIECE_OUT * c::SPEED, r0[1] * PIECE_OUT * c::SPEED, r0[2] * PIECE_OUT * c::SPEED + PIECE_UP * c::SPEED, 0.0];
    let t = w.ticks(0x5a);
    let k = fx::break_piece_with(w, id, PIECE_B, pos, w.m(id).rotation, t, 0, v, [0.0; 4], [0.0; 4]);
    give_light(w, k);
    v[0] = -v[0];
    v[1] = -v[1];
    let yaw = add_rot(w.m(id).rotation[2], std::f32::consts::PI);
    w.mm(id).rotation[2] = yaw;
    let t = w.ticks(0x5a);
    let k = fx::break_piece_with(w, id, PIECE_B, pos, w.m(id).rotation, t, 0, v, [0.0; 4], [0.0; 4]);
    give_light(w, k);
    sparks(w, id, top);
    w.delete_moby(id);
}

/// Level05 0x31d160: the bare base (class 1514) left where the lamp stood.
pub fn base(w: &mut World, id: MobyId) -> Option<MobyId> {
    let b = w.create_moby(BASE_CLASS)?;
    let (ud, dd, rot, light, amb, pos) = { let m = w.m(id); (m.update_dist, m.draw_dist, m.rotation, m.light, m.ambient, m.position) };
    let m = w.mm(b);
    m.update_dist = ud;
    m.visible = 1;
    m.draw_dist = dd;
    m.rotation[..3].copy_from_slice(&rot[..3]);
    m.light = light;
    m.ambient = amb;
    m.position = pos;
    w.build_matrix(b);
    Some(b)
}

/// The break's 24 bursts (module doc).
fn sparks(w: &mut World, id: MobyId, top: c::V) {
    let colour = c::pi32(w, id, 0x68) as u32 | 0x7f00_0000;
    for _ in 0..24 {
        let s = w.rng.randf(0.3, 4.0);
        let n = w.rng.randi(2);
        let sign = if n != 0 { n } else { -1 };
        let a = w.rng.rand_angle();
        let b = w.rng.randf(f32::from_bits(0x3eb2_b8c2), f32::from_bits(0x3fc4_9809));
        let sp = w.rng.randf(0.05, 0.1) * c::SPEED;
        let v = fx::polar(sp, a, b);
        let q = [top[0] + w.rng.randf_sym(0.0, 0.15), top[1] + w.rng.randf_sym(0.0, 0.15), top[2] + w.rng.randf_sym(0.0, 0.15), top[3]];
        let life = w.ticks(0x3c);
        let tiny = f32::from_bits(0x3b44_9ba6);
        w.part53(pf(s * 0.1), pf(s), pf(tiny), pv(q), life, colour, 0, sign as i8, pv(v));
        let life = w.ticks(0x3c);
        w.part53(pf(s * 0.07), pf(s * 0.7), pf(tiny), pv(q), life, 0x7f7f_7f7f, 1, -sign as i8, pv(v));
        let up = [0.0, 0.0, w.rng.randf(0.01, 0.05) * c::SPEED, 0.0];
        let size = w.rng.randf(60000.0, 200000.0);
        let x = w.rng.randi(2);
        let spin = if w.rng.randi(2) == 0 { x } else { -x };
        smoke(w, size, top, spin, up);
    }
}

/// `PartType23Spawn(0.3, 1, 1.02, size, p, spin, v, 0x407f7f7f)` and the caller's patch: life `ticks(rand_range(90,
/// 120))`, phase 2, alpha `rand_range(64, 96)`, the end alpha tick = the life's low byte, one in two blend 0x44.
fn smoke(w: &mut World, size: f32, p: c::V, spin: i32, v: c::V) {
    let hi = f32::from_bits(0x3f82_8f5c);
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let i = match w.particles.as_deref_mut() {
        Some(sys) => crate::particles::type23::spawn(sys, w.rng, 0.3, 1.0, hi, size, p, spin, v, 0x407f_7f7f),
        None => {
            for _ in 0..3 { w.rng.randf_sym(0.0, 0.3); }
            w.rng.randi(2);
            w.rng.randf(1.0, hi);
            Some(usize::MAX)
        }
    };
    let Some(i) = i else {
        w.svc.fx.part_failed += 1;
        return;
    };
    let n = w.rng.rand_range(0x5a, 0x78);
    let life = w.ticks(n);
    let alpha = w.rng.rand_range(0x40, 0x60) as u8;
    let blend = w.rng.randi(2) != 0;
    if let Some(r) = w.particles.as_deref_mut().and_then(|s| s.pool.recs.get_mut(i)) {
        use crate::particles::rec;
        rec::set_i16(r, 0xa, life as i16);
        rec::set_u32(r, 0x24, 2);
        r[0x2a] = alpha;
        r[0x2b] = r[0xa];
        if blend { r[3] = 0x44; }
    }
}

/// The callback's frame (0x31cf58): rows `d` (to the camera, `FastVecNormalize`), `side = unit(d × (0, 0, 1))`, `up =
/// side × d` (`FastVecCross`), and the centre (the head pulled 0.1 away from the camera). Built when the update
/// registers the callback, from the camera it saw (the game draws with the frame's camera) [L].
fn glow_frame(top: c::V, cam: [f32; 3]) -> [[f32; 4]; 4] {
    let unit = |v: [f32; 3]| { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if n == 0.0 { v } else { v.map(|x| x / n) } };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let d = unit([cam[0] - top[0], cam[1] - top[1], cam[2] - top[2]]);
    let side = unit(cross(d, [0.0, 0.0, 1.0]));
    let up = cross(side, d);
    let c = [top[0] + d[0] * PULL, top[1] + d[1] * PULL, top[2] + d[2] * PULL];
    [[d[0], d[1], d[2], 0.0], [side[0], side[1], side[2], 0.0], [up[0], up[1], up[2], 0.0], [c[0], c[1], c[2], 1.0]]
}

/// Level05 0x31cf58 for moby `id` (draw only): the glow quad facing the camera, corners `(0, y, z, 1)·size` through
/// the frame (0x221608: `y·side + z·up + centre`; `VecScale` leaves w at 1).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < 0x74 { return None; }
    let f = svc.draw_callbacks.matrices.get(&id)?;
    let size = crate::moby_update::services::pvar::ff(&m.pvars, 0x70);
    let alpha = crate::moby_update::services::pvar::i32(&m.pvars, 0x60) as u32;
    let colour = crate::moby_update::services::pvar::u32(&m.pvars, 0x68);
    let corners = CORNERS.map(|[y, z]| std::array::from_fn(|k| (f[1][k] * y + f[2][k] * z) * size + f[3][k]));
    let rgba = alpha << 24 | colour;
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads: vec![FxQuad { corners, st: ST, rgba: [rgba; 4] }] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::Moby;
    use crate::moby_update::services::pvar as p;

    fn lamp() -> Moby {
        let mut m = Moby { o_class: 1511, pvars: vec![0; 0x80], position: [10.0, 0.0, 0.0, 1.0], ..Moby::default() };
        for k in 0..3 { m.rows[k][k] = 1.0; }
        m.pvars[0x64..0x67].copy_from_slice(&[1, 2, 3]);
        m.pvars[0x6c..0x6f].copy_from_slice(&[4, 5, 6]);
        m.pvars[0x6f] = 0x60;
        p::set_ff(&mut m.pvars, 0x70, 0.5);
        m
    }

    fn run(t: &mut MobyTable, svc: &mut Services, cam: [f32; 3], f: impl FnOnce(&mut World)) {
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut w = World::new(t, &hero, &mut rng, &classes, svc, 1);
        w.camera = pv([cam[0], cam[1], cam[2], 1.0]);
        f(&mut w);
    }

    #[test]
    fn glow_fades_with_the_camera_distance() {
        let mut t = MobyTable::new(vec![lamp()], 8);
        let mut svc = Services::new();
        run(&mut t, &mut svc, [10.0, 40.0, 0.0], |w| update(w, 0));
        let m = &t.mobys[0];
        assert_eq!(m.state, 1);
        assert_eq!(m.glow, 0x7f03_0201);
        assert_eq!(p::u32(&m.pvars, 0x68), 0x06_0504);
        assert_eq!(p::i32(&m.pvars, 0x60), (96.0 * (1.0 - 8.0 * 0.0625)) as i32);
        run(&mut t, &mut svc, [10.0, 10.0, 0.0], |w| update(w, 0));
        assert_eq!(p::i32(&t.mobys[0].pvars, 0x60), 0x60);
        // Out of range: nothing more (the alpha stays).
        run(&mut t, &mut svc, [10.0, 60.0, 0.0], |w| update(w, 0));
        assert_eq!(p::i32(&t.mobys[0].pvars, 0x60), 0x60);
        // The quad (frame from the camera at (10, 10, 4)): centred on the head (4 up) pulled 0.1 away from it.
        run(&mut t, &mut svc, [10.0, 10.0, 4.0], |w| update(w, 0));
        let q = fx_quads(&t, &svc, 0).unwrap();
        assert_eq!((q.fx, q.additive, q.quads.len()), (FX, true, 1));
        let c = q.quads[0].corners;
        let mid: [f32; 3] = std::array::from_fn(|k| (c[0][k] + c[3][k]) * 0.5);
        assert!((mid[0] - 10.0).abs() < 1e-5 && (mid[1] + 0.1).abs() < 1e-5 && (mid[2] - 4.0).abs() < 1e-5, "{mid:?}");
        assert_eq!(q.quads[0].rgba[0], 0x6006_0504);
    }

    #[test]
    fn a_hit_at_the_head_breaks_it_into_a_base_three_pieces_and_bolts() {
        // Ratchet (at the origin) within 7: BreakFxA drops bolts.
        let mut m = lamp();
        m.position[0] = 3.0;
        let mut t = MobyTable::new(vec![m], 16);
        let mut svc = Services::new();
        let hit_at = |t: &mut MobyTable, svc: &mut Services, z: f32| {
            let tmpl = crate::moby_update::services::HitTemplate { flags: 0x1_0000, ..Default::default() };
            run(t, svc, [10.0, 10.0, 0.0], |w| w.deliver_hit(0, &tmpl));
            let slot = t.mobys[0].hit_slot as usize;
            svc.hits.records[slot].pos = pv([3.0, 0.0, z, 1.0]);
            run(t, svc, [10.0, 10.0, 0.0], |w| update(w, 0));
        };
        hit_at(&mut t, &mut svc, 0.0);
        assert!(!t.mobys[0].is_deleted(), "a hit 4 below the head");
        hit_at(&mut t, &mut svc, 4.5);
        assert!(t.mobys[0].is_deleted());
        let made: Vec<i16> = t.mobys.iter().skip(1).filter(|m| !m.is_deleted() && m.state != 0xff).map(|m| m.o_class).collect();
        assert_eq!(made.iter().filter(|&&c| c == BASE_CLASS).count(), 1, "{made:?}");
        assert_eq!(made.iter().filter(|&&c| c == PIECE_A).count(), 1);
        assert_eq!(made.iter().filter(|&&c| c == PIECE_B).count(), 2);
        assert!(made.iter().filter(|&&c| (13..=16).contains(&c)).count() >= 1, "BreakFxA's bolts: {made:?}");
        assert_eq!(svc.fx.part_spawns.get(&23).copied().unwrap_or(0), 24);
        // The two 0x5ee pieces fly out along ±row 0 (the given velocity, not a random one), both 0.08·speed up.
        let v: Vec<[f32; 4]> = t.mobys.iter().filter(|m| m.o_class == PIECE_B && !m.is_deleted()).map(|m| p::v4f(&m.pvars, 0)).collect();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0][..3], [0.075, 0.0, 0.08]);
        assert_eq!(v[1][..3], [-0.075, -0.0, 0.08]);
    }
}
