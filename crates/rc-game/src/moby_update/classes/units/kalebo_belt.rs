//! Kalebo's reversing belts, class 471 (level 16, 37 created instances): level16 0x2c9878 (census U506; was U498 /
//! U499). A belt that does not move: it carries whoever rides it by a velocity along its x axis, reversing when
//! Ratchet steps into one of its two cuboids; its moby is hidden and its look is the draw callback 0x2c9cd0, two
//! layers of quad strips over the top face of its cuboid (+0x60), one scrolling with the belt's speed. The belts of a
//! group share one loop voice (`PlayClassSound(0, 0xd)`), handed to the member nearest the camera each tick
//! (0x2c9a50; the store `World::hand_over_sound`, G-AUD-010). Read from the level16 decomp of 0x2c9878, 0x2c9a50,
//! 0x2c9c38, 0x2c9cd0 and the overlay's data (gp−0x5274 .. −0x5250, 0x1d2ff0 .. 0x1d306c). Native `f32`.
//!
//! **Pvar block**: +0x08 the platform block's offset (0x20, the loader's), +0x20..+0x5f the platform block (+0x5c its
//! flags), +0x60 the drawn cuboid (−1: none), +0x64 / +0x68 the cuboids that turn it back / forward, +0x6c the speed
//! (units per tick, signed), +0x70 the scroll, +0x74 the voice slot.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2c9878 state 0 | block flags +0x5c \|= 4 (a moving floor), drawn last frame +0x31 = 0, mode \|= 1 (hidden); cuboid +0x60 = −1 → mode \|= 3 (no update; stays in state 0); else → state 2; slot +0x74 = −1 | [`update`] |
//! | state 1 | speed ≠ 4·dt (gp−0x5274): `Approach(4·dt, 12·dt², &speed)` (0x257160 = L01 0x270728), v = row 0 (+0xc0) at length speed (`FastVecNormalize`), 0x2c9c38(group, v) | [`update`], [`push_group`] |
//! | | Ratchet (0x13f3d0) in cuboid +0x64 (`PointInCuboid` 0x25b258 = L01 0x274820) → state 2; `RegisterDrawCallback(0x2c9cd0, self)` (0x1f7c48 = L01 0x21afe0) | [`update`] (`Callback::UnitQuads`, two rows: [`fx_quads`]) |
//! | state 2 | the same with −4·dt and cuboid +0x68 → state 1 | [`update`] |
//! | state ≠ 0 | 0x2c9a50 (the voice) | [`voice`] |
//! | 0x2c9c38 | each member of group `+0x21` (`0x1abcc0[g]`) of class 471: `CarryRiders(member +0x20, v, member rot +0x40, member rot)` (0x25c030 = L01 0x2755f8) | [`push_group`] (`triggers::carry_riders`) |
//! | 0x2c9a50 | no group list → nothing; the member of class 471 nearest the camera (0x1671c0; `vec_distance` 0x1f9b48, start 1024); none → [L] nothing (the game reads moby 0's address) | [`voice`] |
//! | | that distance < 64: slot < 0, or its owner (`0x13e5d8 + slot·0x70`) null or not class 471 → slot = `PlayClassSound(0, 0xd, nearest)` (`play_class_sound` 0x22da68), slot ≥ 0 → its position (+0x08) = **self's** position + self's row 2 at length 2.5 (the game's: self, not the nearest) | [`voice`] (`World::play_sound`, `World::hand_over_sound`) |
//! | | else the slot's position = the nearest's position + its row 2 at length 2.5, owner = the nearest | [`voice`] (`World::hand_over_sound`) |
//! | 0x2c9cd0 (draw) | scroll +0x70 += speed, wrapped into [0, 1] by ±1 (the game does it in the callback; the port in the update right after the registration: once per drawn frame [L]) | [`update`] |
//! | | L = \|cuboid +0x60 row 0\| (`FastVecLength`), n = trunc(2.001·L), step = 2/(2.001·L); colour A = `FastTweenColor(sin(2π·(frame % ticks(120))/ticks(120) − π)·½ + ½, 0x10e08020, 0x30e08020)` (gp−0x5258 / −0x5254; frame 0x15f5cc) | [`fx_quads`] |
//! | | quad A: FX 0x13 (gp−0x5270), colour A, ST (0.5, 0.5), (0.5, 1.5), (0.5, 0.5), (0.5, 1.5) (0x1d2ff0); quad B: FX 0x10 (gp−0x526c), colour 0x50808070 (gp−0x5250), ST (0 − scroll, 0), (0 − scroll, 1), (1 − scroll, 0), (1 − scroll, 1) (0x1d3010); ALPHA (0, 1, 0, 1) (gp−0x5268..−0x525c: not additive) | [`fx_quads`] |
//! | | corners (cuboid-local, 0x1d3030) (−1, 1, 1), (−1, −1, 1), (−1, 1, 1), (−1, −1, 1), corners 0 / 1 at x − step; n + 2 strips, each step further along x, through the cuboid's matrix (`FastDrawQuadReal(q, cuboid, 0)`); strip 0's corners 0 / 1 and the last strip's corners 2 / 3 get colour 0 on both quads (the fade at the ends) | [`fx_quads`] |
//! | | no particle, light, hit, save flag, other moby | n/a |

use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, turn::approach, DT, DT2};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{Services, World};
use crate::moby_update::triggers;
use rc_formats::volumes::ShapeKind;

use super::{FxQuad, FxQuads};

/// The update in the level16 class table.
pub const UPDATE_FN: u32 = 0x2c_9878;
/// The draw callback: the port's second row (quad B; draw only).
pub const DRAW_FN: u32 = 0x2c_9cd0;
pub const REFERENCE_LEVEL: u32 = 16;
pub const CLASSES: [i16; 1] = [471];

pub mod pv {
    pub const BLOCK: usize = 0x20;
    pub const FLAGS: usize = 0x5c;
    pub const CUBOID: usize = 0x60;
    pub const BACK: usize = 0x64;
    pub const FORWARD: usize = 0x68;
    pub const SPEED: usize = 0x6c;
    pub const SCROLL: usize = 0x70;
    pub const SLOT: usize = 0x74;
}

/// gp−0x5274: the belt speed (units per second).
pub const SPEED: f32 = 4.0;
/// The `Approach` rate factor (· dt²).
pub const ACCEL: f32 = 12.0;
/// The voice's range from the camera and its offset along row 2.
pub const VOICE_RANGE: f32 = 64.0;
pub const VOICE_UP: f32 = 2.5;
/// gp−0x5270 / −0x526c: the two layers' FX textures.
pub const FX_A: usize = 0x13;
pub const FX_B: usize = 0x10;
/// gp−0x5258 / −0x5254: colour A's tween ends; gp−0x5250: colour B.
pub const TWEEN_A: u32 = 0x10e0_8020;
pub const TWEEN_B: u32 = 0x30e0_8020;
pub const COLOUR_B: u32 = 0x5080_8070;
/// The frame counter `0x15f5cc`, kept for the draw callback (`Services::units`).
pub const COUNTER_WORD: u32 = 0x15_f5cc;

/// 6.28318 and 3.14159 as the callback's code loads them.
const TAU_LIT: f32 = f32::from_bits(0x40c9_0fd0);
const PI_LIT: f32 = f32::from_bits(0x4049_0fd0);
const ST_A: [[f32; 2]; 4] = [[0.5, 0.5], [0.5, 1.5], [0.5, 0.5], [0.5, 1.5]];
const ST_B: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
const CORNERS: [[f32; 3]; 4] = [[-1.0, 1.0, 1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, -1.0, 1.0]];

/// Level16 0x2c9878 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SLOT + 4 { return; }
    match w.m(id).state {
        0 => {
            let f = c::pi32(w, id, pv::FLAGS) | 4;
            c::set_pi32(w, id, pv::FLAGS, f);
            let cub = c::pi32(w, id, pv::CUBOID);
            let m = w.mm(id);
            m.visible = 0;
            m.mode |= mode::HIDDEN;
            if cub == -1 { m.mode |= mode::HIDDEN | mode::NO_UPDATE; } else { m.state = 2; }
            c::set_pi32(w, id, pv::SLOT, -1);
        }
        s @ (1 | 2) => {
            let (target, cub, next) = if s == 1 { (SPEED * DT, pv::BACK, 2) } else { (-(SPEED * DT), pv::FORWARD, 1) };
            let mut sp = c::pf(w, id, pv::SPEED);
            if sp != target {
                approach(target, DT2 * ACCEL, &mut sp);
                c::set_pf(w, id, pv::SPEED, sp);
                let v = c::set_len3(w.m(id).rows[0], sp);
                push_group(w, w.m(id).group, v);
            }
            if w.in_cuboid(w.hero_point(), c::pi32(w, id, cub)) { w.mm(id).state = next; }
            register(w, id);
        }
        _ => {}
    }
    if w.m(id).state != 0 { voice(w, id); }
}

/// The callback registration and its scroll (0x2c9cd0's first lines, here [L]).
fn register(w: &mut World, id: MobyId) {
    w.svc.units.set_word(COUNTER_WORD, w.counter as u32);
    for f in [UPDATE_FN, DRAW_FN] {
        if let Some(i) = super::row(REFERENCE_LEVEL, f) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
    }
    let mut s = c::pf(w, id, pv::SCROLL) + c::pf(w, id, pv::SPEED);
    if 1.0 < s { s -= 1.0; } else if s < 0.0 { s += 1.0; }
    c::set_pf(w, id, pv::SCROLL, s);
}

/// 0x2c9c38(group, v): the members of class 471 carry their riders by `v`.
fn push_group(w: &mut World, g: i8, v: [f32; 4]) {
    for m in group_ids(w, g) {
        let Some(mo) = w.table.mobys.get_mut(m) else { continue };
        if mo.o_class != CLASSES[0] || mo.pvars.len() < pv::BLOCK + 0x40 { continue; }
        let r = mo.rotation;
        triggers::carry_riders(&mut mo.pvars, pv::BLOCK, v, r, r);
    }
}

/// 0x2c9a50: the group's loop voice at the member nearest the camera.
fn voice(w: &mut World, id: MobyId) {
    let ids = group_ids(w, w.m(id).group);
    if ids.is_empty() { return; }
    let cam = w.camera_point();
    let cam = [cam[0], cam[1], cam[2], 0.0];
    let mut best = 1024.0;
    let mut near = None;
    for m in ids {
        let Some(mo) = w.table.mobys.get(m) else { continue };
        if mo.o_class != CLASSES[0] { continue; }
        let d = c::dist3(cam, mo.position);
        if d < best { best = d; near = Some(m); }
    }
    let Some(near) = near else { return };
    if VOICE_RANGE <= c::dist3(w.m(near).position, cam) { return; }
    let slot = c::pi32(w, id, pv::SLOT);
    let owner = w.sound_owner(slot).filter(|&o| w.table.mobys.get(o).is_some_and(|m| m.o_class == CLASSES[0]));
    let at = |w: &World, m: MobyId| {
        let mo = w.m(m);
        let p = c::add(mo.position, c::set_len3(mo.rows[2], VOICE_UP));
        [p[0], p[1], p[2]]
    };
    match owner {
        None => {
            let s = w.play_sound(0, 0xd, near);
            c::set_pi32(w, id, pv::SLOT, s);
            if s >= 0 {
                let p = at(w, id);
                w.hand_over_sound(s, near, p);
            }
        }
        Some(_) => {
            let p = at(w, near);
            w.hand_over_sound(slot, near, p);
        }
    }
}

/// The belt's quads (0x2c9cd0) for moby `id`: layer A (`UPDATE_FN`'s row) or B (`DRAW_FN`'s row).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId, layer_b: bool) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < pv::SLOT + 4 { return None; }
    let p = |o: usize| crate::moby_update::services::pvar::ff(&m.pvars, o);
    let cub = crate::moby_update::services::pvar::i32(&m.pvars, pv::CUBOID);
    let shape = svc.volumes.shape(ShapeKind::Cuboid, cub)?;
    let r0 = shape.matrix[0];
    let len = (r0[0] * r0[0] + r0[1] * r0[1] + r0[2] * r0[2]).sqrt();
    let n = (len * 2.001) as i32;
    let step = 2.0 / (len * 2.001);
    let period = svc.ticks(0x78).max(1);
    let frame = svc.units.word(COUNTER_WORD) as i32;
    // The game's own literals (0x40c90fd0 / 0x40490fd0), not τ / π.
    let t = ((frame % period) as f32 / period as f32) * TAU_LIT - PI_LIT;
    let colour = if layer_b { COLOUR_B } else { crate::hud::tween_color(t.sin() * 0.5 + 0.5, TWEEN_A, TWEEN_B) };
    let scroll = p(pv::SCROLL);
    let st = if layer_b { ST_B.map(|q| [q[0] - scroll, q[1]]) } else { ST_A };
    let mut local = CORNERS;
    local[0][0] -= step;
    local[1][0] -= step;
    let mut quads = Vec::with_capacity((n + 2).max(0) as usize);
    for k in 0..n + 2 {
        let mut rgba = [colour; 4];
        if k == 0 { rgba[0] = 0; rgba[1] = 0; }
        if k == n + 1 { rgba[2] = 0; rgba[3] = 0; if k == 1 { rgba[0] = 0; rgba[1] = 0; } }
        let corners = local.map(|l| shape.world(l));
        quads.push(FxQuad { corners, st, rgba });
        for v in local.iter_mut() { v[0] += step; }
    }
    Some(FxQuads { fx: if layer_b { FX_B } else { FX_A }, additive: false, subtract: false, quads })
}
