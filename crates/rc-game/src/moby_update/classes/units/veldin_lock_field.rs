//! **The fields beside the Trespasser locks of Veldin's last level, class 1392** (level18 `0x2f1e68`, its draw callback
//! `0x2f1fd8`; census U593; two placed, #870 / #871, each naming its lock 615 (#259 / #260) in pvar +0x00). While its
//! lock is unsolved a field hums (class sound 0, looped) and shows two stacks of four pulsing bands, green fading to
//! orange as they widen, drawn fainter with the camera's distance; once the lock's state passes 2 the field goes
//! (no draw, its collision off, the hum stopped). Read from the level18 decomp and data (gp−0x4938..−0x4910,
//! 0x1da350 / 0x1da370). Native `f32`.
//!
//! **Pvars** (0x20): +0x00 the lock (moby index, −1 none), +0x04 the hum's voice slot, +0x08 the bands' phase, +0x0c
//! the field's strength (1, then toward 0 once solved). Port only: +0x10 the draw's alpha factor (the strength
//! limited by the camera distance, taken by the update; the game computes it in the draw callback).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | `0x2f1e68` state 0 | → 1, +0x0c = 1 | [`update`] |
//! | state 1 | `RegisterDrawCallback(0x2f1fd8)`; the hum not alive (`SoundIsAlive`) → `PlayClassSound(0, 4, m)` into +0x04 | [`update`] (`Callback::UnitQuads`) |
//! | | the lock (+0x00 ≠ −1) is a 615 (0x267) in state > 2: collision off (+0x94 = 0), → 2; the hum alive and still this moby's: `release_voice_slot`; +0x04 = −1 | [`update`] |
//! | state 2 | `Approach(0, 2·dt, &+0x0c)` (nothing draws it any more) | [`update`] |
//! | `0x2f1fd8` | +0x08 += 2·dt (gp−0x4920), −1 past 1 | [`update`] (at the registration) |
//! | | a = min(+0x0c, (48 − d)·0.25), not below 0 when the distance wins; d = 3-D distance to the camera (0x1677c0) | [`update`] (port pvar +0x10, the camera the update saw [L]) |
//! | | frame: the moby's rows (+0xc0, `0x1fa298`) and its position, w 1; twice: z += 2.035 (gp−0x4930), then four quads i = 0..3 (FX 0xe, TEX1 0xff9000000260, ALPHA A 0 B 2 C 0 D 1 FIX 0x80: additive): f = frac(+0x08 + i/4); colour `FastTweenColor(a, c & 0xffffff, c)` with c = `FastTweenColor(f, 0x3000ff00, 0x0000c0ff)` (gp−0x4938 / −0x4934); corners (0, ∓5, 0.24 ∓ f) (0x1da370: y −5, −5, 5, 5; z − f on the even corners, + f on the odd); ST (s + i/4, t) with (s, t) = (0, 0) (0, 1) (7, 0) (7, 1) (0x1da350); `FastDrawQuadReal(quad, frame, 0)` | [`fx_quads`] + `rc-engine` fx_draw |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{pi32, set_pi32, turn, DT};
use crate::moby_update::services::{pvar as p, Services, World};

use super::{FxQuad, FxQuads};

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2f_1e68;
pub const DRAW_FN: u32 = 0x2f_1fd8;
pub const CLASSES: [i16; 1] = [1392];

/// The Trespasser lock (0x267).
const LOCK: i16 = 0x267;
const LEN: usize = 0x20;
const LOCK_PV: usize = 0x00;
const VOICE: usize = 0x04;
const PHASE: usize = 0x08;
const STRENGTH: usize = 0x0c;
const ALPHA: usize = 0x10;
/// FX texture, scroll rate, the stacks' step, the band's growth and colours (module table).
const FX: usize = 0xe;
const RATE: f32 = 2.0;
const STEP: f32 = f32::from_bits(0x4002_3d71);
const GROW: f32 = 1.0;
const NEAR: u32 = 0x3000_ff00;
const FAR: u32 = 0x0000_c0ff;
/// 0x1da370 (y, z of the four corners; x 0) and 0x1da350 (ST).
const CORNERS: [[f32; 2]; 4] = [[-5.0, 0.24], [-5.0, 0.24], [5.0, 0.24], [5.0, 0.24]];
const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [7.0, 0.0], [7.0, 1.0]];

/// Level18 `0x2f1e68` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, LEN);
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            p::set_ff(&mut w.mm(id).pvars, STRENGTH, 1.0);
        }
        1 => {
            if let Some(i) = super::row(REFERENCE_LEVEL, DRAW_FN) {
                w.svc.draw_callbacks.register(Callback::UnitQuads(i), id);
                draw_state(w, id);
            }
            let slot = pi32(w, id, VOICE);
            if !w.sound_alive(slot, id) {
                let s = w.play_sound(0, 4, id);
                set_pi32(w, id, VOICE, s);
            }
            let lock = pi32(w, id, LOCK_PV);
            let solved = usize::try_from(lock).ok().and_then(|l| w.table.mobys.get(l)).is_some_and(|m| m.o_class == LOCK && 2 < m.state);
            if solved {
                w.mm(id).has_collision = false;
                w.mm(id).state = 2;
                let slot = pi32(w, id, VOICE);
                if w.sound_alive(slot, id) {
                    if slot != -1 && w.sound_owner(slot) == Some(id) { w.release_sound(slot, id); }
                    set_pi32(w, id, VOICE, -1);
                }
            }
        }
        2 => {
            let mut s = p::ff(&w.m(id).pvars, STRENGTH);
            turn::approach(0.0, 2.0 * DT, &mut s);
            p::set_ff(&mut w.mm(id).pvars, STRENGTH, s);
        }
        _ => {}
    }
}

/// The draw callback's own state (module table): the phase's scroll and the alpha factor.
fn draw_state(w: &mut World, id: MobyId) {
    let mut ph = p::ff(&w.m(id).pvars, PHASE) + RATE * DT;
    if 1.0 < ph { ph -= 1.0; }
    let pos = w.m(id).position;
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let d = crate::spline::dist3([pos[0], pos[1], pos[2]], cam);
    let mut a = p::ff(&w.m(id).pvars, STRENGTH);
    let near = (48.0 - d) * 0.25;
    if near <= a {
        a = near;
        if near < 0.0 { a = 0.0; }
    }
    let pv = &mut w.mm(id).pvars;
    p::set_ff(pv, PHASE, ph);
    p::set_ff(pv, ALPHA, a);
}

/// `0x2f1fd8`'s quads for field `id` (module table).
pub fn fx_quads(table: &MobyTable, _svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < LEN { return None; }
    let (ph, a) = (p::ff(&m.pvars, PHASE), p::ff(&m.pvars, ALPHA));
    let r = m.rows;
    let mut o = [m.position[0], m.position[1], m.position[2]];
    let mut quads = Vec::with_capacity(8);
    for _ in 0..2 {
        o[2] += STEP;
        for i in 0..4 {
            let s = i as f32 * 0.25;
            let mut f = ph + s;
            f -= (f as i32) as f32;
            let c = crate::hud::tween_color(f, NEAR, FAR);
            let rgba = crate::hud::tween_color(a, c & 0xff_ffff, c);
            let corners = std::array::from_fn(|k| {
                let [y, z] = CORNERS[k];
                let z = if k & 1 == 0 { z - f * GROW } else { z + f * GROW };
                std::array::from_fn(|j| y * r[1][j] + z * r[2][j] + o[j])
            });
            let st = std::array::from_fn(|k| [ST[k][0] + s, ST[k][1]]);
            quads.push(FxQuad { corners, st, rgba: [rgba; 4] });
        }
    }
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads })
}
