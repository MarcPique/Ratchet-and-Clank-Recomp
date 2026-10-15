//! **The burning wreck, class 1510** (level01 update `0x30ba18`, its spawner `0x30be70` and draw callback `0x30bfa8`):
//! what is left of a Blarg flyer 660 or a gunship 688 shot down over Novalis. It flies off along the ship's last
//! heading at 0.8..1.2 a tick, tumbling, slowed by the air (×0.995 a tick sideways and while rising) and pulled down
//! by gravity 10, trailing a glow and two grey smoke puffs a tick and flickering a small white glow (half the ticks).
//! It explodes where it touches anything (a 2.0 sphere, its ship ignored; it bounces off the face it hit first) or
//! after 200 ticks, and vanishes without a blast once it leaves the level's box (2..1021 on every axis). Read from the
//! level01 decomp and disassembly (the sphere's radius f12 = 2.0 from the box test; gp−0x4ba8 10.0, gp−0x4ba4 0.995,
//! gp−0x4ba0 0, gp−0x4b9c / −0x4b94 the glow's offsets −0.258 / 0.448; the quad 0x20b9d0). Native `f32`.
//!
//! **Pvars** (0x28): +0x00 the velocity, +0x10 / +0x14 / +0x18 the spin (x, y, z), +0x20 the timer, +0x24 the ship.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | `0x30be70(scale, ship, dir)` | `CreateMoby(0x5e6)`: +0x24 the ship; its position and Euler; state 0, drawn, update / draw distance 0xff; scale ·= `scale`; velocity = dir at `randf(0.8, 1.2)`; spin `randf_sym(0, 0.5°)`, `randf_sym(3°, 8°)`, `randf_sym(0, 1°)`; timer `ticks(200)`; its matrix | [`spawn`] |
//! | `0x30ba18` | `randi(2)`: `RegisterDrawCallback2(0x30bfa8)`; position += velocity; \|v.x\|, \|v.y\| > 0: ×0.995 (`(0.995 − 1)·0x15ed60 + 1`), v.z > 0: ×0.995; v.z −= 10·dt²; Euler += spin | [`update`] (`Callback::UnitQuads`) |
//! | | outside 2..1021 on any axis → deleted; else `0x212960(2.0, position, 0, the ship)` and the timer (`FastDecTimer`): running and nothing hit → `PartType21Spawn(randf(0.5, 1.5)·210000, position, 0, 0x4f007fff, 0x1fffffff, rand_range(ticks(10), ticks(20)), 1)` and two `PartType23Spawn(0.1, 1, 1, randf(40000, 100000), position, ±randi(6), 0, 0x60 grey g = rand_range(0x30, 0xff))` (ALPHA 0x44, life `ticks(60)`, the fade from 0x60) | [`update`] |
//! | | else: a hit with a face kind (+0x1c) → velocity reflected off its normal (`0x221570`), a hit without → velocity 0; `SpawnBeamExplosion(0, 0, 4, 2, 100000, 3, 15, m, v, position, 20, 3, 4, sound 0, shake)`; deleted | [`update`] |
//! | `0x30bfa8` | at position + row 0·−0.258 + row 2·0.448: a camera-facing quad (c = unit(camera − p), b = −unit(gravity × c), a = c × b) of corners (0, ±1, ±1)·0.4, colour 0x202020ff, ST (0, 0) (0, 1) (1, 0) (1, 1), FX 0xb, ALPHA 0x48 | [`fx_quads`] (the camera the update saw [L]) |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::classes::units::{FxQuad, FxQuads};
use crate::moby_update::creature::{self as c, fx, DT2, SPEED};
use crate::moby_update::services::{pvar as p, Services, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 1;
pub const UPDATE_FN: u32 = 0x30_ba18;
pub const DRAW_FN: u32 = 0x30_bfa8;
pub const CLASS: i16 = 0x5e6;
pub const CLASSES: [i16; 1] = [CLASS];

const LEN: usize = 0x28;
const VEL: usize = 0x00;
const SPIN: usize = 0x10;
const TIMER: usize = 0x20;
const SHIP: usize = 0x24;
/// Port only: the draw's quad corners (world), taken by the update.
const QUAD: usize = 0x28;
const DRAG: f32 = 0.995;
const GRAVITY: f32 = 10.0;
const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
const CORNERS: [[f32; 2]; 4] = [[-1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [1.0, -1.0]];

/// `0x30be70(scale, ship, dir)` (module table).
pub fn spawn(w: &mut World, scale: f32, ship: MobyId, dir: c::V) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    let (pos, rot) = (w.m(ship).position, w.m(ship).rotation);
    {
        let m = w.mm(id);
        if m.pvars.len() < LEN + 0x30 { m.pvars.resize(LEN + 0x30, 0); }
        p::set_i32(&mut m.pvars, SHIP, ship as i32 + 1);
        m.position = pos;
        m.state = 0;
        m.rotation = rot;
        m.visible = 1;
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.scale *= scale;
    }
    let s = w.rng.randf(0.8, 1.2);
    let v = c::set_len3(dir, s);
    c::set_pv4(w, id, VEL, v);
    let a = w.rng.randf_sym(0.0, f32::from_bits(0x3c0e_fa35));
    c::set_pf(w, id, SPIN, a);
    let a = w.rng.randf_sym(f32::from_bits(0x3d56_7750), f32::from_bits(0x3e0e_fa35));
    c::set_pf(w, id, SPIN + 4, a);
    let a = w.rng.randf_sym(0.0, f32::from_bits(0x3c8e_fa35));
    c::set_pf(w, id, SPIN + 8, a);
    let t = w.ticks(200);
    c::set_pi32(w, id, TIMER, t);
    w.build_matrix(id);
    Some(id)
}

/// Level01 `0x30ba18` (module table).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, LEN + 0x30);
    if w.rng.randi(2) != 0 {
        if let Some(i) = crate::moby_update::classes::units::row(REFERENCE_LEVEL, DRAW_FN) {
            w.svc.draw_callbacks.register(Callback::UnitQuads(i), id);
            glow(w, id);
        }
    }
    let mut v = c::pv4(w, id, VEL);
    let pos = c::add(c::pos(w, id), v);
    w.mm(id).position = pos;
    let k = (DRAG - 1.0) * SPEED + 1.0;
    if 0.0 * SPEED < v[0].abs() { v[0] *= k; }
    if 0.0 * SPEED < v[1].abs() { v[1] *= k; }
    if 0.0 < v[2] { v[2] *= k; }
    v[2] -= GRAVITY * DT2;
    c::set_pv4(w, id, VEL, v);
    {
        let spin = [0, 1, 2].map(|k| c::pf(w, id, SPIN + 4 * k));
        let m = w.mm(id);
        for (r, sp) in m.rotation.iter_mut().zip(spin) { *r = c::add_rot(*r, sp); }
    }
    let inside = (0..3).all(|k| (2.0..=1021.0).contains(&pos[k]));
    if !inside {
        w.delete_moby(id);
        return;
    }
    let ship = usize::try_from(c::pi32(w, id, SHIP) - 1).ok();
    let hit = w.coll_sphere(crate::moby_update::services::pv(pos), Pf::f(2.0), 0, ship);
    let running = c::dec_timer_pvar_i32(w, id, TIMER) == 0;
    if running && hit.is_none() {
        smoke(w, pos);
        return;
    }
    if let Some(h) = &hit {
        v = if h.kind != 0 { crate::moby_update::classes::decoy::reflect(v, [h.normal[0], h.normal[1], h.normal[2], 0.0]) } else { [0.0; 4] };
        c::set_pv4(w, id, VEL, v);
    }
    let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 100000.0, scale: 3.0, light: 15.0, streaks: 20, sparks: 3, puffs: 4, debris: 0, sound: 0, shake: true };
    fx::beam_explosion(w, &b, Some(id), pos);
    w.delete_moby(id);
}

/// The trail of a running wreck (module table).
fn smoke(w: &mut World, pos: c::V) {
    let s = w.rng.randf(0.5, 1.5);
    let (a, b) = (w.ticks(10), w.ticks(0x14));
    let life = w.rng.rand_range(a, b);
    fx::part21(w, s * 210000.0, pos, [0.0; 4], 0x4f00_7fff, 0x1fff_ffff, life, 1);
    for _ in 0..2 {
        let r = w.rng.randi(6);
        let spin = if w.rng.randi(2) != 0 { -r } else { r };
        let size = w.rng.randf(40000.0, 100000.0);
        let g = w.rng.rand_range(0x30, 0xff) as u32;
        let rgba = 0x6000_0000 | g << 16 | g << 8 | g;
        *w.svc.fx.part_spawns.entry(23).or_default() += 1;
        let Some(sys) = w.particles.as_deref_mut() else { continue };
        let Some(i) = crate::particles::type23::spawn(sys, w.rng, 0.1, 1.0, 1.0, size, pos, spin, [0.0; 4], rgba) else {
            w.svc.fx.part_failed += 1;
            continue;
        };
        let life = w.ticks(0x3c);
        let r = &mut w.particles.as_deref_mut().unwrap().pool.recs[i];
        use crate::particles::rec;
        r[3] = 0x44;
        rec::set_i16(r, 0xa, life as i16);
        rec::set_u32(r, 0x24, 2);
        r[0x2a] = 0x60;
        r[0x2b] = r[0xa];
    }
}

/// The draw's quad (module table), kept in the port's pvars past the game's block for [`fx_quads`].
fn glow(w: &mut World, id: MobyId) {
    let m = w.m(id);
    let (r, pos) = (m.rows, m.position);
    let pt: [f32; 3] = std::array::from_fn(|j| pos[j] + r[0][j] * f32::from_bits(0xbe84_1893) + r[2][j] * f32::from_bits(0x3ee5_6042));
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let g = crate::hero::physics::to_f32x3(w.hero.gravity_dir);
    let unit = |v: [f32; 3], l: f32| { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if n == 0.0 { [0.0; 3] } else { v.map(|x| x * l / n) } };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let cv = unit([cam[0] - pt[0], cam[1] - pt[1], cam[2] - pt[2]], 1.0);
    let bv = unit(cross(g, cv), -1.0);
    let av = cross(cv, bv);
    let pv = &mut w.mm(id).pvars;
    for (k, [y, z]) in CORNERS.iter().enumerate() {
        let q: [f32; 3] = std::array::from_fn(|j| y * 0.4 * bv[j] + z * 0.4 * av[j] + pt[j]);
        for (j, x) in q.iter().enumerate() { p::set_ff(pv, QUAD + 12 * k + 4 * j, *x); }
    }
}

/// `0x30bfa8`'s quad for wreck `id`.
pub fn fx_quads(table: &MobyTable, _svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= QUAD + 48)?;
    let corners = std::array::from_fn(|k| std::array::from_fn(|j| p::ff(&m.pvars, QUAD + 12 * k + 4 * j)));
    Some(FxQuads { fx: 0xb, additive: true, subtract: false, quads: vec![FxQuad { corners, st: ST, rgba: [0x2020_20ff; 4] }] })
}
