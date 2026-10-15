//! **The cutscene effects of Veldin's last level, class 1563** (level18 `0x2f88e8`, its draw callbacks `0x2f9780`,
//! `0x2f9a98`, `0x2f9c20`, `0x2f9eb8` and the trail puffs `0x2fa250`; census U599; one placed, #907). While a scene
//! plays (game mode 2) it adds the finale's effects to the scene actors: the glowing seat lights of the boss's copy
//! (scenes 0..3, 1), its exhausts as the ships fly off (scene 4: jet glows on actors 3..7, then each blows up, then a
//! big blast over the arena), a glow and a growing beam out of the turned-over piece 1435 and a white flash filling
//! the view (scenes 4 and 7), a ring of grey puffs before the camera (scene 3) and a jet trail (scene 5). The
//! Morph-o-Ray beam fires from actor 4 in scene 1 (the item's beam code). Read from the level18 decomp and data
//! (gp−0x4770..−0x46d4, 0x1f2c20, 0x1dfb90, 0x1dfbd0; the seat glow's words 0x162450..). Native `f32`.
//!
//! Scene ticks below are `ticks(n)` of the scene tick 0x16d294 (scene id 0x16d290; actors 0x16d3d8[k]).
//!
//! ## Coverage (`0x2f88e8`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, +0x30 = 0xff; the windows (flash 355 / 365, glow 60 / 200, beam 190 / 365) into gp−0x4768.. | [`update`] ([`Fx::windows`]) |
//! | state 1, game mode 2 | (else nothing) | [`update`] |
//! | scene 4, tick 1 | 0x162528 = the first moby of class 0x59b (1435), kept when there is none | [`update`] (the first in table order [L]) |
//! | | 500..860: actors 3..7 (non-null): joint list 0 point; `0x266140(s, 0.4·s, p, p, 0)` with s = 1.2e6 / 5e5 / 8e5 / 4e5 / 2e5 (0x1f2c20); actor 3 skipped in 645..736; actor 5 in 645..730: s = 300000 | [`update`] (`fx::jet_puffs`) |
//! | | ticks 706 / 710 / 720 / 730: actor 7 / 4 / 6 / 5 (non-null): `SpawnBeamExplosion(0, 0, 8, 4, 9, 1, 30, actor, (0, 0, 1), its joint list 0 point, 50, 25, 25, −1, 0)` | [`update`] (`fx::beam_explosion`) |
//! | | tick 748: `SpawnBeamExplosion(0, 0, 12, 6, 9, 1, 60, m, (0, 0, 1), (634.55, 454.16, 103.0), 300, 200, 200, −1, 0)` | [`update`] |
//! | | glow window: alpha 0x162524 = trunc((t − ticks(a)) / ticks(b − a)·255), `RegisterDrawCallback(0x2f9c20, the piece)` | [`update`] |
//! | | beam window: length 0x16252c = (t − ticks(a)) / ticks(b − a)·200 + 0.25, `(0x2f9eb8, the piece)` | [`update`] |
//! | | flash window (to 380, gp−0x4760): `(0x2f9780, actor 0)`; before its end b: size 0x162520 = 0.1 + 1.9·(t − ticks(a)) / ticks(b − a); else 10 | [`update`] |
//! | scene 7 | tick 1: the windows become flash 80 / 90, glow 10 / 100, beam 70 / 100 (gp−0x475c..), the piece found again; the three windows as in scene 4 with the fractions of the raw ticks | [`update`] |
//! | scene 1 | actor 3: `(0x2f9a98, actor 3)` | [`update`] (`Callback::UnitGlow`) |
//! | scene 5, 138..264, actor 2 | after 140: joint lists 0 / 2; twelve steps i: `0x2fa250(15000, 6000, q, q, 0)` at q = lerp(i/12, now, then) for each; then = now (also at 138..140) | [`update`], [`trail_puffs`] |
//! | scene 1, 446..502, actor 4 | the Morph-o-Ray's beam fired by the driver as if it were the item: +0x10 (target) = 0; joint lists 3 / 10 of the actor: the muzzle +0x00 = j10, row 1 (+0xd0) = unit(j10 − j3), yaw +0x48 = atan(x, y), pitch +0x4c = atan(\|xy\|, z); range gp−0x54b4 = 24 (never set back: the hero's item would see it too [L], not reproduced); the beam `0x2bf908` (L01 `0x2d2d08`: the lay, the length by the line (flags 0x14, Ratchet ignored) up to 24, the shape, the sparkles and sparks, the pulses, `RegisterDrawCallback2(0x2c04b8)`); then 0x1617f4 = 0x207f7f, 0x1617f8 = 0x7f7f20 and 0x16174c put back (level 01's light timer / sound slot of the item at −0x40: no reader in the scene [L]) | [`morph_beam`] (`hero::morph_ray::beam_step` on the driver's own `Beam`; the draw's quads `Beam::quads` built at the step, the camera the update saw [L]) |
//! | scenes 0, 2, and 3 before 586 | actor 2: `(0x2f9a98, actor 2)` | [`update`] |
//! | scene 3, tick 586 | p = camera (0x1677c0) + (10, 0, 0.6) in the camera rows (0x1679d0); 32 puffs k: v = the left row turned by k·π/16 about the forward row (`0x261f48`), length 0.0025·0x15ed60; `PartType23Spawn(0, 1, 1.01, 20000, p, ±randi(16), v, 0x7f7f7f7f)`, life `ticks(60)`, the fade patch | [`update`] (`fx::puff23`) |
//! | `0x2fa250(g, c, a, b, v)` | two glow puffs at a (`randi(16)`, sign `randi(2)`; 0, 1 → 0.9, g, 0x7f204080, life `ticks(15)`), three cores at b (0, 1 → 0.97, c, spin ±16, 0x7fffffff, `ticks(4)`, a rotation byte) | [`trail_puffs`] |
//! | `0x2f9780` | tick = b + 1 of the flash window: offset 0x162550 = (1, 0, 0.15 (scene 7: −0.15), 1); p = the offset in the camera rows + the camera; a camera-facing frame (c = unit(p − camera), b = unit(up × c), a = c × b); three quads of 0x1dfb90 ((0, ∓1, ±1)) scaled 1 / 0.75 / 0.5 × size, colours 0x7f7f7f7f / 0xffffffff / 0xffffffff, FX 0xb, ALPHA 0x48 | [`update`] (at the registration, the camera the update saw [L]) |
//! | `0x2f9c20` | tick = a + 1 of the glow window: offset 0x162560 = (0, 0, −45, 1); p = the piece's rows · offset + its position; the camera-facing frame; quads ×4 (0x00ffffff) and ×7 (0x001050ff), alpha 0x162524 | [`update`] |
//! | `0x2f9eb8` | p as above; d = the piece's rows · (0, 0, −1); frame (d, f × d, f = unit(d × (p − camera)), p); 0x1dfbd0 ((∓1, 0, ±1)) with x × length and z × 2 / 2.5 / 3; colours 0xffffffff / 0x7f1050ff / 0x7f1050ff | [`update`] |
//! | `0x2f9a98` | the 60-tick pulse of 0x80404080 / 0x20004080 (0x162458.., as the boss's seat); at joint lists 1 and 2 (`moby_attach_to_joint`): point − 0.125·row 2; glow quads 0.25, pull −0.15 | [`update`] (`Callback::UnitGlow`, [`glow_quads`]) |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx, SPEED};
use crate::moby_update::services::{Services, World};
use crate::scene_player::SceneActorState;

use super::{FxQuad, FxQuads, GlowQuad};

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2f_88e8;
pub const FLASH_FN: u32 = 0x2f_9780;
pub const SEAT_FN: u32 = 0x2f_9a98;
pub const GLOW_FN: u32 = 0x2f_9c20;
pub const BEAM_FN: u32 = 0x2f_9eb8;
/// The Morph-o-Ray beam's draw (level18's copy of level01 `0x2d38b8`).
pub const MORPH_FN: u32 = 0x2c_04b8;
/// gp−0x54b4 as scene 1 sets it.
const MORPH_RANGE: f32 = 24.0;
pub const CLASSES: [i16; 1] = [1563];

/// The piece the glow and the beam come out of (0x59b).
const PIECE: i16 = 0x59b;
/// The windows (flash a, b; glow a, b; beam a, b) at the start and from scene 7's first tick; the flash's last tick.
const WINDOWS: [i32; 6] = [355, 365, 60, 200, 190, 365];
const WINDOWS_7: [i32; 6] = [80, 90, 10, 100, 70, 100];
const FLASH_END: i32 = 380;
/// Scene 4's jet sizes for actors 3..7 (0x1f2c20).
const JETS: [f32; 5] = [1.2e6, 5e5, 8e5, 4e5, 2e5];
/// Scene 4's explosions: (tick, actor).
const BLASTS: [(i32, usize); 4] = [(706, 7), (710, 4), (720, 6), (730, 5)];
const BLAST_AT: [f32; 4] = [f32::from_bits(0x441e_a396), f32::from_bits(0x43e3_14dd), f32::from_bits(0x42cd_ff7d), 1.0];
const QUAD_ST: [[f32; 2]; 4] = [[1.0, 1.0], [0.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
const FLASH_QUAD: [[f32; 2]; 4] = [[-1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [1.0, -1.0]];
const SEAT_RGBA: [u32; 2] = [0x8040_4080, 0x2000_4080];

/// The driver's level words and its draws for this tick (`Globals::veldin_finale`).
#[derive(Clone, Debug, Default)]
pub struct Fx {
    /// gp−0x4768.. (flash a, b; glow a, b; beam a, b).
    pub windows: [i32; 6],
    /// 0x162520 the flash's size, 0x162524 the glow's alpha, 0x16252c the beam's length.
    pub flash: f32,
    pub alpha: i32,
    pub beam: f32,
    /// 0x162528 the piece.
    pub piece: Option<MobyId>,
    /// 0x162530 / 0x162540: the trail's last points.
    pub then: [c::V; 2],
    /// 0x162550 the flash's offset, 0x162560 the glow's.
    pub flash_off: [f32; 3],
    pub glow_off: [f32; 3],
    /// The draws registered this tick (port: built by the update).
    pub draws: Vec<(u32, Vec<FxQuads>)>,
    pub glows: Vec<GlowQuad>,
    /// The Morph-o-Ray beam's globals as scene 1 drives them (the game shares them with the item).
    pub morph: crate::hero::morph_ray::Beam,
}

/// Level18 `0x2f88e8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            w.svc.units.veldin_finale.windows = WINDOWS;
            return;
        }
        1 => {}
        _ => return,
    }
    w.svc.units.veldin_finale.draws.clear();
    w.svc.units.veldin_finale.glows.clear();
    if w.svc.game_mode != 2 { return; }
    let Some(scene) = w.svc.cinematic.scene.clone() else { return };
    let (sid, t) = (scene.id as i32, scene.tick);
    let actor = |k: usize| scene.actors.get(k);
    if sid == 4 {
        if t == 1 { find_piece(w); }
        if w.ticks(500) <= t && t <= w.ticks(0x35c) {
            for k in 3..8 {
                let Some(a) = actor(k) else { continue };
                let p = a.joint_point(0);
                let mut s = JETS[k - 3];
                if k == 3 && !(t < w.ticks(0x285) || w.ticks(0x2e0) < t) { continue; }
                if k == 5 && w.ticks(0x285) <= t && t <= w.ticks(0x2da) { s = 300000.0; }
                fx::jet_puffs(w, s, s * 0.4, p, p, [0.0; 4]);
            }
        }
        for (at, k) in BLASTS {
            if t != w.ticks(at) { continue; }
            let Some(a) = actor(k) else { continue };
            let p = a.joint_point(0);
            let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 8.0, flash2: 4.0, flash_dist: 9.0, scale: 1.0, light: 30.0, streaks: 50, sparks: 25, puffs: 25, debris: 0, sound: -1, shake: false };
            fx::beam_explosion(w, &b, a.moby, p);
        }
        if t == w.ticks(0x2ec) {
            let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 12.0, flash2: 6.0, flash_dist: 9.0, scale: 1.0, light: 60.0, streaks: 300, sparks: 200, puffs: 200, debris: 0, sound: -1, shake: false };
            fx::beam_explosion(w, &b, Some(id), BLAST_AT);
        }
        windows(w, id, &scene, t, true);
    }
    if sid == 7 {
        if t == 1 {
            w.svc.units.veldin_finale.windows = WINDOWS_7;
            find_piece(w);
        }
        windows(w, id, &scene, t, false);
    }
    if sid == 1 {
        if let Some(a) = actor(3) { seat(w, id, a); }
    }
    if sid == 1 && w.ticks(0x1be) <= t && t < w.ticks(0x1f6) {
        if let Some(a) = actor(4) { morph_beam(w, id, a); }
    }
    if sid == 5 && w.ticks(0x8a) <= t && t <= w.ticks(0x108) {
        if let Some(a) = actor(2) {
            let now = [a.joint_point(0), a.joint_point(2)];
            if w.ticks(0x8c) < t {
                let then = w.svc.units.veldin_finale.then;
                for i in 0..12 {
                    let f = i as f32 / 12.0;
                    for k in 0..2 {
                        let q = lerp(f, now[k], then[k]);
                        trail_puffs(w, 15000.0, 15000.0 * 0.4, q, q, [0.0; 4]);
                    }
                }
            }
            w.svc.units.veldin_finale.then = now;
        }
    }
    let seat2 = sid == 2 || sid == 0 || (sid == 3 && t < w.ticks(0x24a));
    if seat2 {
        if let Some(a) = actor(2) { seat(w, id, a); }
    }
    if sid == 3 && t == w.ticks(0x24a) { burst(w); }
}

/// The first 1435 of the table into 0x162528 (kept when there is none).
fn find_piece(w: &mut World) {
    if let Some(i) = w.table.mobys.iter().position(|m| m.o_class == PIECE && m.state < 0x80) { w.svc.units.veldin_finale.piece = Some(i); }
}

/// The three windows of scenes 4 / 7 (module table); `scaled`: the fractions over `ticks()` (scene 4) or raw (7).
fn windows(w: &mut World, id: MobyId, scene: &crate::scene_player::SceneState, t: i32, scaled: bool) {
    let [fa, fb, ga, gb, ba, bb] = w.svc.units.veldin_finale.windows;
    let frac = |w: &World, a: i32, b: i32| -> f32 {
        if scaled { (t - w.ticks(a)) as f32 / w.ticks(b - a) as f32 } else { (t - a) as f32 / (b - a) as f32 }
    };
    if w.ticks(ga) <= t && t < w.ticks(gb) {
        w.svc.units.veldin_finale.alpha = (frac(w, ga, gb) * 255.0) as i32;
        if t == w.ticks(ga + 1) { w.svc.units.veldin_finale.glow_off = [0.0, 0.0, -45.0]; }
        piece_glow(w, id);
    }
    if w.ticks(ba) <= t && t < w.ticks(bb) {
        w.svc.units.veldin_finale.beam = frac(w, ba, bb) * 200.0 + 0.25;
        piece_beam(w, id);
    }
    if w.ticks(fa) <= t && t < w.ticks(FLASH_END) {
        if t < w.ticks(fb) {
            w.svc.units.veldin_finale.flash = 0.1 + (2.0 - 0.1) * frac(w, fa, fb);
        } else {
            w.svc.units.veldin_finale.flash = 10.0;
        }
        if t == w.ticks(fa + 1) { w.svc.units.veldin_finale.flash_off = [1.0, 0.0, if scene.id == 7 { -0.15 } else { 0.15 }]; }
        flash(w, id, scene.actors.first().and_then(|a| a.moby));
    }
}

/// `a + (b − a)·t` (`fun_001f9a40`).
fn lerp(t: f32, a: c::V, b: c::V) -> c::V { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t) }

/// The camera-facing frame at `p` (rows c, b, a: `0x2f9780` / `0x2f9c20`).
fn facing(w: &World, p: [f32; 3]) -> [[f32; 3]; 3] {
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let c0 = unit([p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]]);
    let b0 = unit(cross([0.0, 0.0, 1.0], c0));
    [c0, b0, cross(c0, b0)]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { [0.0; 3] } else { v.map(|x| x / l) }
}

/// One quad of the table `tab` scaled (`kx` on x, `ky` on y, `kz` on z) in the frame `r` at `p`.
fn quad(tab: [[f32; 3]; 4], k: [f32; 3], r: [[f32; 3]; 3], p: [f32; 3], rgba: u32) -> FxQuad {
    let corners = std::array::from_fn(|q| {
        let v = [tab[q][0] * k[0], tab[q][1] * k[1], tab[q][2] * k[2]];
        std::array::from_fn(|j| v[0] * r[0][j] + v[1] * r[1][j] + v[2] * r[2][j] + p[j])
    });
    FxQuad { corners, st: QUAD_ST, rgba: [rgba; 4] }
}

fn flat(t: [[f32; 2]; 4], x: bool) -> [[f32; 3]; 4] { t.map(|[a, b]| if x { [a, 0.0, b] } else { [0.0, a, b] }) }

/// Registers draw `f` for moby `m` (the driver itself when the game's moby is missing) with its quads.
fn register(w: &mut World, id: MobyId, m: Option<MobyId>, f: u32, quads: Vec<FxQuads>) {
    let Some(i) = super::row(REFERENCE_LEVEL, f) else { return };
    w.svc.draw_callbacks.register(Callback::UnitQuads(i), m.unwrap_or(id));
    w.svc.units.veldin_finale.draws.push((f, quads));
}

/// The piece's point (its rows · the glow offset + its position).
fn piece_point(w: &World) -> Option<([f32; 3], [[f32; 4]; 4])> {
    let m = w.table.mobys.get(w.svc.units.veldin_finale.piece?)?;
    let o = w.svc.units.veldin_finale.glow_off;
    let r = m.rows;
    Some((std::array::from_fn(|j| o[0] * r[0][j] + o[1] * r[1][j] + o[2] * r[2][j] + m.position[j]), r))
}

/// `0x2f9c20` (module table).
fn piece_glow(w: &mut World, id: MobyId) {
    let Some((p, _)) = piece_point(w) else { return };
    let f = facing(w, p);
    let a = (w.svc.units.veldin_finale.alpha as u32) << 24;
    let t = flat(FLASH_QUAD, false);
    let quads = vec![quad(t, [1.0, 4.0, 4.0], f, p, 0x00ff_ffff | a), quad(t, [1.0, 7.0, 7.0], f, p, 0x0010_50ff | a)];
    let piece = w.svc.units.veldin_finale.piece;
    register(w, id, piece, GLOW_FN, vec![FxQuads { fx: 0xb, additive: true, subtract: false, quads }]);
}

/// `0x2f9eb8` (module table).
fn piece_beam(w: &mut World, id: MobyId) {
    let Some((p, r)) = piece_point(w) else { return };
    let d: [f32; 3] = std::array::from_fn(|j| -r[2][j]);
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let fr = unit(cross(d, [p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]]));
    let frame = [d, cross(fr, d), fr];
    let len = w.svc.units.veldin_finale.beam;
    let t = flat(FLASH_QUAD, true);
    let quads = [(2.0, 0xffff_ffff), (2.5, 0x7f10_50ff), (3.0, 0x7f10_50ff)].map(|(k, rgba)| quad(t, [len, 1.0, k], frame, p, rgba)).to_vec();
    let piece = w.svc.units.veldin_finale.piece;
    register(w, id, piece, BEAM_FN, vec![FxQuads { fx: 0xb, additive: true, subtract: false, quads }]);
}

/// `0x2f9780` (module table).
fn flash(w: &mut World, id: MobyId, actor0: Option<MobyId>) {
    let [fwd, left, up] = w.camera_rows;
    let o = w.svc.units.veldin_finale.flash_off;
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let p: [f32; 3] = std::array::from_fn(|j| o[0] * fwd[j] + o[1] * left[j] + o[2] * up[j] + cam[j]);
    let f = facing(w, p);
    let s = w.svc.units.veldin_finale.flash;
    let t = flat(FLASH_QUAD, false);
    let quads = [(1.0, 0x7f7f_7f7f), (0.75, 0xffff_ffff), (0.5, 0xffff_ffff)].map(|(k, rgba)| quad(t, [s * k; 3], f, p, rgba)).to_vec();
    register(w, id, actor0, FLASH_FN, vec![FxQuads { fx: 0xb, additive: true, subtract: false, quads }]);
}

/// `0x2f9a98` for actor `a` (module table).
fn seat(w: &mut World, id: MobyId, a: &SceneActorState) {
    let n = w.ticks(60).max(1) as u64;
    let f = (w.counter % n) as f32 / n as f32;
    let s = (f * 6.18318 - f32::from_bits(0x4049_0fd0)).sin();
    let rgba = crate::hud::tween_color(s * 0.5 + 0.5, SEAT_RGBA[0], SEAT_RGBA[1]);
    for k in [1, 2] {
        let m = a.joint_matrix(k);
        let point = [m[3][0] - 0.125 * m[2][0], m[3][1] - 0.125 * m[2][1], m[3][2] - 0.125 * m[2][2]];
        w.svc.units.veldin_finale.glows.push(GlowQuad { size: 0.25, pull: -0.15, point, rgba });
    }
    if let Some(i) = super::row(REFERENCE_LEVEL, SEAT_FN) { w.svc.draw_callbacks.register(Callback::UnitGlow(i), a.moby.unwrap_or(id)); }
}

/// Scene 3's ring of puffs before the camera (module table).
fn burst(w: &mut World) {
    let [fwd, left, up] = w.camera_rows;
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let p: c::V = [cam[0] + 10.0 * fwd[0] + 0.6 * up[0], cam[1] + 10.0 * fwd[1] + 0.6 * up[1], cam[2] + 10.0 * fwd[2] + 0.6 * up[2], 1.0];
    for k in 0..32 {
        let v = crate::moby_update::classes::blaster_shot::rotate(left, k as f32 * 0.19634955, fwd);
        let v = unit(v).map(|x| x * 0.0025 * SPEED);
        let r = w.rng.randi(0x10);
        let spin = if w.rng.randi(2) == 0 { r } else { -r };
        let life = w.ticks(0x3c);
        fx::puff23(w, [0.0, 1.0, f32::from_bits(0x3f81_47ae), 20000.0], p, spin, [v[0], v[1], v[2], 0.0], 0x7f7f_7f7f, life, false, 0x7f);
    }
}

/// `0x2fa250(glow, core, a, b, vel)`: [`fx::jet_puffs`] without jitter, with shorter lives (15 / 4 ticks).
fn trail_puffs(w: &mut World, glow: f32, core: f32, a: c::V, b: c::V, vel: c::V) {
    for _ in 0..2 {
        let r = w.rng.randi(0x10);
        let spin = if w.rng.randi(2) == 0 { r } else { -r };
        let life = w.ticks(0xf);
        fx::puff23(w, [0.0, 1.0, f32::from_bits(0x3f66_6666), glow], a, spin, vel, 0x7f20_4080, life, false, 0x7f);
    }
    let mut spin = 0x10;
    for _ in 0..3 {
        let life = w.ticks(4);
        fx::puff23(w, [0.0, 1.0, f32::from_bits(0x3f78_51ec), core], b, spin, vel, 0x7fff_ffff, life, true, 0x7f);
        spin = -spin;
    }
}

/// Scene 1's Morph-o-Ray beam from actor `a` (module table).
fn morph_beam(w: &mut World, id: MobyId, a: &SceneActorState) {
    use crate::moby_update::services::{fv, pv};
    let (j3, j10) = (a.joint_point(3), a.joint_point(10));
    let dir = unit([j10[0] - j3[0], j10[1] - j3[1], j10[2] - j3[2]]);
    {
        let m = w.mm(id);
        m.rows[1] = [dir[0], dir[1], dir[2], m.rows[1][3]];
    }
    let muzzle = [j10[0], j10[1], j10[2]];
    let yaw = c::atan(dir[0], dir[1]);
    let pitch = c::atan((dir[0] * dir[0] + dir[1] * dir[1]).sqrt(), dir[2]);
    let mut beam = std::mem::take(&mut w.svc.units.veldin_finale.morph);
    beam.range = MORPH_RANGE;
    beam.lay(muzzle, dir);
    let reach = crate::targeting::polar(MORPH_RANGE, yaw, pitch);
    let end = [muzzle[0] + reach[0], muzzle[1] + reach[1], muzzle[2] + reach[2], 1.0];
    let len = match w.line(pv([muzzle[0], muzzle[1], muzzle[2], 1.0]), pv(end), 0x14, w.hero_moby) {
        Some(h) => { let q = fv(h.point); ((q[0] - muzzle[0]).powi(2) + (q[1] - muzzle[1]).powi(2)).sqrt() }
        None => MORPH_RANGE,
    };
    let gravity = crate::hero::physics::to_f32x3(w.hero.gravity_dir);
    let mut parts = Vec::new();
    crate::hero::morph_ray::beam_step(&mut beam, muzzle, dir, yaw, pitch, None, len, gravity, w.rng, &mut parts);
    beam.drawn = Some(w.counter);
    if let Some(sys) = w.particles.as_deref_mut() {
        for s in &parts { crate::hero::fx::create_one(sys, s, 0); }
    }
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let mut groups: Vec<FxQuads> = Vec::new();
    for (fx, corners, st, rgba) in beam.quads(cam) {
        let q = FxQuad { corners, st, rgba };
        match groups.last_mut() {
            Some(g) if g.fx == fx => g.quads.push(q),
            _ => groups.push(FxQuads { fx, additive: true, subtract: false, quads: vec![q] }),
        }
    }
    w.svc.units.veldin_finale.morph = beam;
    register(w, id, Some(id), MORPH_FN, groups);
}

/// The quads of draw `f` this tick.
pub fn fx_quad_groups(svc: &Services, f: u32) -> Vec<FxQuads> {
    svc.units.veldin_finale.draws.iter().filter(|d| d.0 == f).flat_map(|d| d.1.iter().cloned()).collect()
}

/// `0x2f9a98`'s glows this tick.
pub fn glow_quads(_table: &MobyTable, svc: &Services) -> Vec<GlowQuad> { svc.units.veldin_finale.glows.clone() }
