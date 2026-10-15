//! **Blarg's glass**: the station's windows 1062 (level06 `0x2fd088`, draw callbacks `0x2fd460` / `0x2fd1a0` /
//! `0x2fd748`; 1 placed; census U236), the breakable window 1083 (`0x2ffcb0`, its break `0x300278`, shard maker
//! `0x300c60`, draw callbacks `0x300588` / `0x3003f0` / `0x300958` / `0x300720`; 2 placed; U239) and its shards
//! 1085..1088 (`0x300b90`, made only). The names are descriptive [L]. The glass is meshes of quads in the overlay
//! (positions ×0.01, ST, and for some a centre and normal per quad: drawn only when it faces the camera), FX 0x15 in
//! colour 0x20202020 blended (ALPHA A 0 B 1 C 0 D 1); the windows add a second pass, FX 0x2a in 0x60ffffff, and
//! scroll the first by half the camera's yaw / pitch. Read from the level06 decomp and data (the tables below) and the
//! words gp−0x4c84..−0x4c5c and −0x4bbc..−0x4b80. Native `f32`.
//!
//! **1062** (pvars: +0x00..+0x0c the inner cuboids, +0x40..+0x4c the outer ones): every tick the hall's glass
//! (`0x2fd460`, 58 quads, culled); the camera in an inner cuboid → the near panes (`0x2fd1a0`, 50 quads at their stored
//! positions, not culled); then Ratchet in group 0xf or the camera in an outer cuboid → the station's skin
//! (`0x2fd748`, 864 quads, culled).
//!
//! **1083** (pvars: +0x00 the variant, +0x04 whole, +0x08 the switch moby, +0x0c Ratchet's checkpoint cuboid, +0x10 /
//! +0x14 his spot / the camera's cuboid, +0x18 the ride, +0x1c its speed, +0x20 the moby whose leaving breaks it, +0x24
//! the timer):
//!
//! | state | what | port |
//! |---|---|---|
//! | every tick | scale = class scale · 0.7; the draw: whole → `0x300958` / `0x300720` (96 quads, culled), broken → `0x300588` / `0x3003f0` (64), by the variant | [`window_update`] |
//! | 0 | whole, → 1 | |
//! | 1 | the switch's command byte set, or no breaker moby (+0x20 −1): Ratchet within 32 (xy) → 2, timer `ticks(30)`; else it breaks, 4 | |
//! | 2 | the timer out → 3, the letterbox, `CameraScript(cuboid +0x14, 1, ticks(300))`, springs (0.001, 1, 1, 0.001, 1, 1), ride 0, cuboid +0x10 turned to face it, `HeroTeleport(+0x10, 0x72, 0)`; the last 10 ticks fade out | |
//! | 3 | the fade back; ride below 1: past 0.05 the checkpoint cuboid moves to Ratchet (0.5 lower, once); whole and the breaker moved off (0.1) → sound 0, it breaks; the ride spring (1, dt², dt², 1.5·dt); the camera lerped from the cuboid to 2 behind Ratchet and 0.85 up, pitch −10°, his yaw; at 1 → 5, `SetState(0, 0)`, `CameraScript2(2)`, the letterbox off | |
//! | 4 | Ratchet within 16 → the checkpoint cuboid to him, 5 | |
//! | `0x300278` | broken; the frame 0x43c made in its place (its light, mode, scale); hidden; 50 shards: `randf(1, 6)·dt` out from 2 around it, `randf(3.3, 7.3)` up, class `trunc(randf(1085, 1089))`, update 0xff, draw 0x7e, drawn, scale class · 0.1, spin `randf(±360°)·dt` each axis, life `scale(randf(60, 120))` in +0xbc | [`break_glass`] |
//! | `0x300b90` (shards) | scale class · 0.1; velocity z −= 15·dt²; position += velocity; rotation += spin, then each axis `fast_add_rotations` the spin again (the game's); +0xbc − 1 at 0 → deleted | [`shard_update`] |

use std::sync::Arc;

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::{Services, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 6;
pub const WINDOWS_FN: u32 = 0x2f_d088;
pub const WINDOWS_CLASSES: [i16; 1] = [1062];
pub const WINDOW_FN: u32 = 0x2f_fcb0;
pub const WINDOW_CLASSES: [i16; 1] = [1083];
pub const SHARD_FN: u32 = 0x30_0b90;
pub const SHARD_CLASSES: [i16; 4] = [1085, 1086, 1087, 1088];
pub const FRAME: i16 = 0x43c;
/// The draw callbacks, in [`TABLES`] order.
pub const DRAW_FNS: [u32; 7] = [0x2f_d460, 0x2f_d1a0, 0x2f_d748, 0x30_0588, 0x30_03f0, 0x30_0958, 0x30_0720];
/// Per draw: positions, quads, ST, quad count, scaled ×0.01, (centres, normals) for the cull.
type Table = (u32, u32, u32, usize, bool, Option<(u32, u32)>);
const TABLES: [Table; 7] = [
    (0x1e_ed30, 0x1e_f310, 0x1e_f6b0, 0x3a, true, Some((0x1f_0560, 0x1e_fbe0))),
    (0x1f_0900, 0x1f_1220, 0x1f_1540, 0x32, false, None),
    (0x1d_bff0, 0x1d_f6f0, 0x1e_2cf0, 0x360, true, Some((0x1e_b730, 0x1e_4a30))),
    (0x1f_5330, 0x1f_5930, 0x1f_5d30, 0x40, true, None),
    (0x1f_2eb0, 0x1f_34b0, 0x1f_38b0, 0x40, true, None),
    (0x1f_b5f0, 0x1f_bcf0, 0x1f_c2f0, 0x60, true, Some((0x1f_e1f0, 0x1f_d4f0))),
    (0x1f_77b0, 0x1f_7eb0, 0x1f_84b0, 0x60, true, Some((0x1f_aff0, 0x1f_a2f0))),
];
/// gp−0x4c70 (0x15) / 0x2a the textures, −0x4c6c / −0x4c68 the colours, −0x4c64 (0.01), −0x4c60 / −0x4c5c (0.5) the
/// windows' scroll by the camera's yaw / pitch.
const FX_GLASS: usize = 0x15;
const FX_SHINE: usize = 0x2a;
const GLASS_RGBA: u32 = 0x2020_2020;
const SHINE_RGBA: u32 = 0x60ff_ffff;
const SCROLL_K: f32 = 0.5;
/// The words the windows leave for their draws (the camera's pitch / yaw as the tick saw them).
const CAM_PITCH_KEY: u32 = 0x16_2390;
const CAM_YAW_KEY: u32 = 0x16_2394;
/// The camera's position for the draws' cull (0x167540 when the callbacks run; here as the updates saw it [L]).
const CAM_POS_KEY: u32 = 0x16_2398;

fn save_camera(w: &mut World) {
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    for (k, v) in cam.iter().take(3).enumerate() { w.svc.units.set_word(CAM_POS_KEY + 4 * k as u32, v.to_bits()); }
}
/// gp−0x4bbc (10) the fades, −0x4bb8 (0.7) the window's scale, −0x4bb4 (2.8) the shards' base height, −0x4bb0 (−10°).
const FADE: i32 = 10;
const WINDOW_SCALE: f32 = f32::from_bits(0x3f33_3333);
const SHARD_UP: f32 = f32::from_bits(0x4033_3333);
const CAM_PITCH: f32 = -10.0;
const SHARD_SCALE: f32 = 0.1;

/// One glass mesh: positions, ST, quads of (position, ST) index pairs, and the cull's centre / normal per quad.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub verts: Vec<[f32; 3]>,
    pub st: Vec<[f32; 2]>,
    pub quads: Vec<[(u16, u16); 4]>,
    pub cull: Option<Vec<([f32; 3], [f32; 3])>>,
}

/// Level 6's glass meshes ([`DRAW_FNS`] order; `Globals::blarg_glass`, set by the engine at the level load).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meshes {
    pub meshes: Vec<Mesh>,
}

impl Meshes {
    /// Reads the meshes from level 6's overlay (None on another level or when a table does not read).
    pub fn parse(ov: &rc_formats::water::Overlay, level: u32) -> Option<Arc<Meshes>> {
        if level != REFERENCE_LEVEL { return None; }
        let mut meshes = Vec::new();
        let v3 = |a: u32, k: f32| -> Option<[f32; 3]> { Some([ov.f32(a).ok()? * k, ov.f32(a + 4).ok()? * k, ov.f32(a + 8).ok()? * k]) };
        for &(va, qa, sa, n, scaled, cull) in &TABLES {
            let b = ov.read(qa, 16 * n).ok()?;
            let h = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
            let quads: Vec<[(u16, u16); 4]> = (0..n).map(|q| std::array::from_fn(|k| (h(16 * q + 4 * k), h(16 * q + 4 * k + 2)))).collect();
            let nv = quads.iter().flatten().map(|c| c.0 as u32 + 1).max()?;
            let ns = quads.iter().flatten().map(|c| c.1 as u32 + 1).max()?;
            let k = if scaled { 0.01 } else { 1.0 };
            let verts = (0..nv).map(|i| v3(va + 16 * i, k)).collect::<Option<Vec<_>>>()?;
            let st = (0..ns).map(|i| Some([ov.f32(sa + 8 * i).ok()?, ov.f32(sa + 8 * i + 4).ok()?])).collect::<Option<Vec<_>>>()?;
            let cull = match cull {
                Some((ca, na)) => Some((0..n as u32).map(|i| Some((v3(ca + 16 * i, 0.01)?, v3(na + 16 * i, 1.0)?))).collect::<Option<Vec<_>>>()?),
                None => None,
            };
            meshes.push(Mesh { verts, st, quads, cull });
        }
        Some(Arc::new(Meshes { meshes }))
    }
}

fn register(w: &mut World, id: MobyId, f: u32) {
    if let Some(r) = super::row(REFERENCE_LEVEL, f) { w.svc.draw_callbacks.register(Callback::UnitQuads(r), id); }
}

fn camera_in(w: &World, cub: i32) -> bool {
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    0 <= cub && w.in_cuboid([cam[0], cam[1], cam[2]], cub)
}

/// Level06 `0x2fd088` (module doc).
pub fn windows_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x50 { w.mm(id).pvars.resize(0x50, 0); }
    let e = w.hero.loop_in.cam_euler;
    save_camera(w);
    w.svc.units.set_word(CAM_PITCH_KEY, e[1].to_bits());
    w.svc.units.set_word(CAM_YAW_KEY, e[2].to_bits());
    register(w, id, DRAW_FNS[0]);
    if (0..4).any(|k| camera_in(w, c::pi32(w, id, 4 * k))) { register(w, id, DRAW_FNS[1]); }
    if w.hero.group == 0xf || (0..4).any(|k| camera_in(w, c::pi32(w, id, 0x40 + 4 * k))) { register(w, id, DRAW_FNS[2]); }
}

/// The quads of mesh `i`: culled (when it has a cull) by the camera, ST offset, one colour.
fn quads(svc: &Services, i: usize, st_off: [f32; 2], rgba: u32, cam: [f32; 3]) -> Vec<FxQuad> {
    let Some(mesh) = svc.units.blarg_glass.as_ref().and_then(|m| m.meshes.get(i)) else { return Vec::new() };
    mesh.quads.iter().enumerate().filter_map(|(qi, q)| {
        if let Some((ctr, n)) = mesh.cull.as_ref().and_then(|c| c.get(qi)) {
            let d = [ctr[0] - cam[0], ctr[1] - cam[1], ctr[2] - cam[2]];
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if 0.0 < (d[0] * n[0] + d[1] * n[1] + d[2] * n[2]) / l { return None; }
        }
        let corners = q.iter().map(|&(v, _)| mesh.verts.get(v as usize).copied()).collect::<Option<Vec<_>>>()?;
        let st = q.iter().map(|&(_, s)| mesh.st.get(s as usize).map(|s| [s[0] + st_off[0], s[1] + st_off[1]])).collect::<Option<Vec<_>>>()?;
        Some(FxQuad { corners: corners.try_into().ok()?, st: st.try_into().ok()?, rgba: [rgba; 4] })
    }).collect()
}

/// The glass draws (module doc; draw only): the windows' two passes, the window's one.
pub fn fx_quad_groups(_table: &MobyTable, svc: &Services, f: u32) -> Vec<FxQuads> {
    let Some(i) = DRAW_FNS.iter().position(|&d| d == f) else { return Vec::new() };
    let cam: [f32; 3] = std::array::from_fn(|k| f32::from_bits(svc.units.word(CAM_POS_KEY + 4 * k as u32)));
    if i < 3 {
        let (pitch, yaw) = (f32::from_bits(svc.units.word(CAM_PITCH_KEY)), f32::from_bits(svc.units.word(CAM_YAW_KEY)));
        let off = [SCROLL_K * yaw, SCROLL_K * pitch];
        vec![
            FxQuads { fx: FX_GLASS, additive: false, subtract: false, quads: quads(svc, i, off, GLASS_RGBA, cam) },
            FxQuads { fx: FX_SHINE, additive: false, subtract: false, quads: quads(svc, i, [0.0; 2], SHINE_RGBA, cam) },
        ]
    } else {
        vec![FxQuads { fx: FX_GLASS, additive: false, subtract: false, quads: quads(svc, i, [0.0; 2], GLASS_RGBA, cam) }]
    }
}

/// `0x300c60(m, p, v)`: one shard (module doc).
fn shard(w: &mut World, at: c::V, v: c::V) { shard_from(w, at, v, 1.0); }

/// [`shard`] with its scale then multiplied by `k` (the fire-wave bots' death: ×0.4, for the tick it is made).
pub fn shard_from(w: &mut World, at: c::V, v: c::V, k: f32) {
    let class = w.rng.randf(1085.0, 1089.0) as i32;
    let Some(s) = w.create_moby(class as i16) else { return };
    if w.m(s).pvars.len() < 0x1c { w.mm(s).pvars.resize(0x1c, 0); }
    let o = w.m(s).o_class;
    let sc = super::class_scale(w, o) * SHARD_SCALE * k;
    {
        let m = w.mm(s);
        m.update_dist = 0xff;
        m.draw_dist = 0x7e;
        m.visible = 1;
        m.scale = sc;
        m.position = at;
    }
    c::set_pv4(w, s, 0, v);
    for i in 0..3 {
        let r = w.rng.randf(-360.0, 360.0);
        c::set_pf(w, s, 0x10 + 4 * i, r * 0.017_453_292 * DT);
    }
    let f = w.rng.randf(60.0, 120.0);
    let t = w.svc.timing.scale(crate::ps2v::Pf::f(f)).to_f32() as i32;
    w.mm(s).cmd = t as u8;
    w.build_matrix(s);
}

/// `0x300278(m)` (module doc).
fn break_glass(w: &mut World, id: MobyId) {
    c::set_pi32(w, id, 0x04, 0);
    if let Some(f) = w.create_moby(FRAME) {
        let (vis, light, ambient, mode, pos, rot, sc) = { let m = w.m(id); (1u8, m.light, m.ambient, m.mode, m.position, m.rotation, m.scale) };
        let m = w.mm(f);
        m.visible = vis;
        m.draw_dist = 0x40;
        m.light = light;
        m.ambient = ambient;
        m.mode = mode;
        m.position = pos;
        m.rotation = rot;
        m.scale = sc;
        w.build_matrix(f);
    }
    w.mm(id).mode |= 1;
    for _ in 0..50 {
        let s = w.rng.randf(1.0, 6.0) * DT;
        let a = w.rng.rand_angle();
        let dir = [a.cos() * 2.0, a.sin() * 2.0, 0.0, 0.0];
        let mut at = c::add(c::pos(w, id), dir);
        at[2] += w.rng.randf(SHARD_UP + 0.5, SHARD_UP + 4.5);
        let v = c::set_len3(dir, s);
        shard(w, at, v);
    }
}

/// The checkpoint cuboid (+0x0c) moved to Ratchet, 0.5 lower, once.
fn move_checkpoint(w: &mut World, id: MobyId) {
    let i = c::pi32(w, id, 0x0c);
    if i < 0 { return; }
    let h = super::hero_pos(w);
    let v = Arc::make_mut(&mut w.svc.volumes);
    if let Some(s) = usize::try_from(i).ok().and_then(|i| v.cuboids.get_mut(i)) {
        s.matrix[3][0] = h[0];
        s.matrix[3][1] = h[1];
        s.matrix[3][2] = h[2] - 0.5;
    }
    c::set_pi32(w, id, 0x0c, -1);
}

/// Level06 `0x2ffcb0` (module doc).
pub fn window_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x28 { w.mm(id).pvars.resize(0x28, 0); }
    let o = w.m(id).o_class;
    w.mm(id).scale = super::class_scale(w, o) * WINDOW_SCALE;
    let state_now = w.m(id).state;
    match state_now {
        0 => {
            c::set_pi32(w, id, 0x04, 1);
            w.mm(id).state = 1;
        }
        1 => {
            let sw = c::pi32(w, id, 0x08);
            let fired = story::link(w, sw).is_some_and(|m| w.m(m).cmd != 0);
            if fired || c::pi32(w, id, 0x20) == -1 {
                if c::dist2(c::pos(w, id), super::hero_pos(w)) < 32.0 {
                    w.mm(id).state = 2;
                    let t = w.ticks(30);
                    c::set_pi32(w, id, 0x24, t);
                } else {
                    break_glass(w, id);
                    w.mm(id).state = 4;
                }
            }
        }
        2 => {
            if c::dec_timer_pvar_i32(w, id, 0x24) != 0 {
                w.mm(id).state = 3;
                crate::cinematic::letterbox(w, true);
                if let Some((cp, ce)) = story::cuboid(w, c::pi32(w, id, 0x14)) {
                    let t = w.ticks(300);
                    crate::cinematic::camera_script(w, cp, ce, 1, t, false);
                    crate::cinematic::script_springs(w, [f32::from_bits(0x3a83_126f), 1.0, 1.0, f32::from_bits(0x3a83_126f), 1.0, 1.0]);
                    crate::cinematic::camera_targets(w, Some(cp), Some(ce));
                }
                c::set_pf(w, id, 0x18, 0.0);
                let spot = c::pi32(w, id, 0x10);
                if let Some((sp, _)) = story::cuboid(w, spot) {
                    let me = c::pos(w, id);
                    let yaw = c::atan(me[0] - sp[0], me[1] - sp[1]);
                    let v = Arc::make_mut(&mut w.svc.volumes);
                    if let Some(s) = usize::try_from(spot).ok().and_then(|i| v.cuboids.get_mut(i)) { s.euler[2] = yaw; }
                }
                story::teleport_to(w, spot, 0x72, false);
            }
            let n = c::pi32(w, id, 0x24);
            if n < FADE { crate::cinematic::set_fade(w, 1.0 - n as f32 / FADE as f32); }
        }
        3 => {
            let n = c::pi32(w, id, 0x24);
            if n < FADE {
                c::set_pi32(w, id, 0x24, n + 1);
                crate::cinematic::set_fade(w, 1.0 - (n + 1) as f32 / FADE as f32);
            }
            let t = c::pf(w, id, 0x18);
            if t < 0.99999 {
                if 0.05 <= t { move_checkpoint(w, id); }
                if c::pi32(w, id, 0x04) == 1 {
                    let mover = story::link(w, c::pi32(w, id, 0x20));
                    if mover.is_some_and(|m| 0.1 < c::dist2(c::pos(w, id), w.m(m).position)) {
                        w.play_sound(0, 0, id);
                        break_glass(w, id);
                    }
                }
                let (mut t, mut v) = (t, c::pf(w, id, 0x1c));
                turn::spring(1.0, DT2, DT2, DT * 1.5, &mut t, &mut v);
                c::set_pf(w, id, 0x18, t);
                c::set_pf(w, id, 0x1c, v);
                if let Some((cp, ce)) = story::cuboid(w, c::pi32(w, id, 0x14)) {
                    let h = super::hero_pos(w);
                    let f = w.hero.rows[0].map(|x| f32::from_bits(x.0));
                    let back = c::set_len3([f[0], f[1], f[2], 0.0], -2.0);
                    let to = [h[0] + back[0], h[1] + back[1], h[2] + back[2] + 0.85];
                    let yaw = f32::from_bits(w.hero.rot[2].0);
                    let te = [0.0, CAM_PITCH * 0.017_453_292, yaw];
                    let p = std::array::from_fn(|i| cp[i] + (to[i] - cp[i]) * t);
                    let e = std::array::from_fn(|i| ce[i] + (te[i] - ce[i]) * t);
                    crate::cinematic::camera_targets(w, Some(p), Some(e));
                }
            } else {
                w.mm(id).state = 5;
                crate::cinematic::hero_state(w, 0, false);
                crate::cinematic::camera_script2(w, 2);
                crate::cinematic::letterbox(w, false);
            }
        }
        4 if c::dist2(c::pos(w, id), super::hero_pos(w)) < 16.0 => {
            move_checkpoint(w, id);
            w.mm(id).state = 5;
        }
        _ => {}
    }
    save_camera(w);
    let whole = c::pi32(w, id, 0x04) != 0;
    let variant = c::pi32(w, id, 0x00) != 0;
    let f = match (whole, variant) {
        (false, false) => DRAW_FNS[3],
        (false, true) => DRAW_FNS[4],
        (true, false) => DRAW_FNS[5],
        (true, true) => DRAW_FNS[6],
    };
    register(w, id, f);
}

/// Level06 `0x300b90` (module doc).
pub fn shard_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1c { w.mm(id).pvars.resize(0x1c, 0); }
    let o = w.m(id).o_class;
    w.mm(id).scale = super::class_scale(w, o) * SHARD_SCALE;
    let mut v = c::pv4(w, id, 0);
    v[2] -= DT2 * 15.0;
    c::set_pv4(w, id, 0, v);
    let spin = c::pv4(w, id, 0x10);
    let m = w.mm(id);
    for k in 0..3 {
        m.position[k] += v[k];
        let r = m.rotation[k] + spin[k];
        m.rotation[k] = c::add_rot(r, spin[k]);
    }
    let n = m.cmd.wrapping_sub(1);
    m.cmd = n;
    if n == 0 { w.delete_moby(id); }
}

