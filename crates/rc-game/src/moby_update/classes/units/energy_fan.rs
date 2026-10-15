//! **The energy fans, classes 1443 (Kalebo III) / 1890 (Veldin's last level)** (level18 `0x2fae48`, its draw callback
//! `0x2fb018` and helpers `0x2fb410` / `0x2fb470`; census U519; level16 `0x2e5708` is the same code with the same
//! data; one placed on 18, #971, three on 16). A humming disc of thirty blades whose glow runs round it, with four
//! rings turning through it at their own rates, standing in the way until every moby of its switch group (pvar +0x20)
//! is thrown (command byte +0xbc set): then it goes (no draw, no collision) and the group's member nearest to
//! Ratchet plays its class sound 0; it comes back if a switch flips back. Read from the level18 decomp and data
//! (gp−0x4678..−0x4638, 0x1efc10..0x1efcd0; level16's gp−0x4e00.. and 0x1d9780.. hold the same values). Native
//! `f32`.
//!
//! **Pvars** (0x28): +0x00 / +0x04 (1.0, unread), +0x08 the blades' angle, +0x0c the glow's phase, +0x10..+0x1c the
//! four rings' angles, +0x20 the switch group (−1 none), +0x24 the hum's voice slot. Port only: +0x28 the blades'
//! scale (scale / class scale, taken by the update: the draw reads the class header).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | `0x2fae48` state 0 | → 1, +0x30 = 0xff | [`update`] |
//! | state 1 | +0x0c += 1·dt (gp−0x4674), −1 past 1; the hum not alive (`SoundIsAlive`) → `PlayClassSound(0, 4, m)` into +0x24 | [`update`] |
//! | | +0x08 += 30°·dt (gp−0x4678); the rings +0x10..+0x1c += (−30, −16, 32, 15)°·dt (gp−0x4670..) (`fast_add_rotations`) | [`update`] |
//! | | drawn (+0x31) → `RegisterDrawCallback(0x2fb018)` | [`update`] (`Callback::UnitQuads`) |
//! | | +0x20 ≠ −1 and `0x2fb410(+0x20)` (every member's +0xbc ≠ 0; no list: false): collision off (+0x94 = 0), → 2, `0x2fb470(+0x20)`: the member nearest to Ratchet (3-D, from 50000; class 0x5a3 skipped) → `PlayClassSound(0, 0, it)` | [`update`], [`all_thrown`], [`nearest_sound`] |
//! | state 2 | `0x2fb410(+0x20)` false → 1, collision back (+0x94 = the class's, class +0x10) | [`update`] |
//! | `0x2fb018` | blades: FX 16 (gp−0x4640), ALPHA A 0 B 1 C 0 D 1 FIX 0x80; corners (0, ∓4, 0.9 / 0.4) (0x1efc50) times scale / class scale (+0x2c / class +0x24, `vec_scale`), ST (0.5, 0.5) (0.5, 1) (0.5, 0.5) (0.5, 1) (0x1efc10); f = +0x0c wrapped into [0, 1]; n = 30 (gp−0x4638) quads k: frame = Euler (+0x08 + k·2π/n, rot y, rot z) rows (`0x1fa030`) at the position; colour corners 0 / 1 = `FastTweenColor(f > 0.5 ? 2 − 2f : 2f, 0x60e08010, 0x20801030)`, corners 2 / 3 0x20801030; f += 3/n, −1 past 1 | [`fx_quads`] |
//! | | rings: FX 20 (gp−0x463c), corners (0, ∓1, ±1) (0x1efc90), ST (1, 0) (1, 1) (0, 0) (0, 1) (0x1efc30), colour 0x60e04010; one quad per ring at Euler (its angle, rot y, rot z) | [`fx_quads`] (`super::fx_quad_groups`: two textures) |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{add_rot, pi32, set_pi32, DT};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{self as sv, pvar as p, Services, World};

use super::{FxQuad, FxQuads};

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2f_ae48;
pub const DRAW_FN: u32 = 0x2f_b018;
pub const CLASSES: [i16; 1] = [1890];

const LEN: usize = 0x2c;
const ANGLE: usize = 0x08;
const GLOW: usize = 0x0c;
const RINGS: usize = 0x10;
const GROUP: usize = 0x20;
const VOICE: usize = 0x24;
const BLADE_SCALE: usize = 0x28;
/// The class the nearest-member search skips (level16's fan).
const SKIP: i16 = 0x5a3;
const SPIN: f32 = 30.0;
const GLOW_RATE: f32 = 1.0;
const RING_SPIN: [f32; 4] = [-30.0, -16.0, 32.0, 15.0];
const BLADES: i32 = 30;
const BLADE_FX: usize = 16;
const RING_FX: usize = 20;
const GLOW_A: u32 = 0x60e0_8010;
const GLOW_B: u32 = 0x2080_1030;
const RING_RGBA: u32 = 0x60e0_4010;
const BLADE: [[f32; 3]; 4] = [[0.0, -4.0, 0.9], [0.0, -4.0, 0.4], [0.0, 4.0, 0.9], [0.0, 4.0, 0.4]];
const BLADE_ST: [[f32; 2]; 4] = [[0.5, 0.5], [0.5, 1.0], [0.5, 0.5], [0.5, 1.0]];
const RING: [[f32; 3]; 4] = [[0.0, -1.0, 1.0], [0.0, -1.0, -1.0], [0.0, 1.0, 1.0], [0.0, 1.0, -1.0]];
const RING_ST: [[f32; 2]; 4] = [[1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [0.0, 1.0]];

/// Level18 `0x2fae48` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, LEN);
    let st = w.m(id).state;
    match st {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            {
                let pv = &mut w.mm(id).pvars;
                let mut g = p::ff(pv, GLOW) + GLOW_RATE * DT;
                if 1.0 < g { g -= 1.0; }
                p::set_ff(pv, GLOW, g);
            }
            let slot = pi32(w, id, VOICE);
            if !w.sound_alive(slot, id) {
                let s = w.play_sound(0, 4, id);
                set_pi32(w, id, VOICE, s);
            }
            {
                let pv = &mut w.mm(id).pvars;
                let a = add_rot(p::ff(pv, ANGLE), SPIN * 0.017453292 * DT);
                p::set_ff(pv, ANGLE, a);
                for (k, r) in RING_SPIN.iter().enumerate() {
                    let o = RINGS + 4 * k;
                    let a = add_rot(p::ff(pv, o), r * 0.017453292 * DT);
                    p::set_ff(pv, o, a);
                }
            }
            if w.m(id).visible != 0 {
                let cs = w.classes.info(w.m(id).o_class).map_or(1.0, |i| i.scale);
                let k = w.m(id).scale / cs;
                p::set_ff(&mut w.mm(id).pvars, BLADE_SCALE, k);
                if let Some(i) = super::row(REFERENCE_LEVEL, DRAW_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
            }
            let g = pi32(w, id, GROUP);
            if g != -1 && all_thrown(w, g) {
                let m = w.mm(id);
                m.has_collision = false;
                m.state = 2;
                nearest_sound(w, g);
            }
        }
        2 if !all_thrown(w, pi32(w, id, GROUP)) => {
            w.mm(id).state = 1;
            let has = w.classes.info(w.m(id).o_class).is_some_and(|i| i.has_collision);
            w.mm(id).has_collision = has;
        }
        _ => {}
    }
}

/// `0x2fb410(g)`: every member of group `g` has its command byte set (no list: false).
fn all_thrown(w: &World, g: i32) -> bool {
    let Ok(g) = i8::try_from(g) else { return false };
    if w.svc.groups.lists.get(g as u8 as usize).and_then(|l| l.as_ref()).is_none() { return false; }
    group_ids(w, g).into_iter().all(|k| w.table.mobys.get(k).is_some_and(|m| m.cmd != 0))
}

/// `0x2fb470(g)`: class sound 0 of the member of group `g` nearest to Ratchet.
fn nearest_sound(w: &mut World, g: i32) {
    let Ok(g) = i8::try_from(g) else { return };
    let hero = w.hero_point();
    let mut best = (50000.0f32, None);
    for k in group_ids(w, g) {
        let Some(m) = w.table.mobys.get(k) else { continue };
        if m.o_class == SKIP { continue; }
        let d = crate::spline::dist3(hero, [m.position[0], m.position[1], m.position[2]]);
        if d < best.0 { best = (d, Some(k)); }
    }
    if let Some(k) = best.1 { w.play_sound(0, 0, k); }
}

/// The frame of Euler `(x, rot y, rot z)` at `pos`: `l·rows + pos`.
fn frame(x: f32, rot: [f32; 4], pos: [f32; 4]) -> impl Fn([f32; 3]) -> [f32; 3] {
    let r = sv::euler_rows(sv::pv([x, rot[1], rot[2], 0.0])).map(sv::fv);
    move |l| std::array::from_fn(|j| l[0] * r[0][j] + l[1] * r[1][j] + l[2] * r[2][j] + pos[j])
}

/// `0x2fb018`'s quads for fan `id`: the blades, then the rings (module table).
pub fn fx_quad_groups(table: &MobyTable, _svc: &Services, id: MobyId) -> Vec<FxQuads> {
    let Some(m) = table.mobys.get(id).filter(|m| m.pvars.len() >= LEN) else { return Vec::new() };
    let pv = &m.pvars;
    let k = p::ff(pv, BLADE_SCALE);
    let mut f = p::ff(pv, GLOW);
    while f < 0.0 { f += 1.0; }
    while 1.0 < f { f -= 1.0; }
    let n = BLADES as f32;
    let mut blades = Vec::with_capacity(BLADES as usize);
    for i in 0..BLADES {
        let at = frame(add_rot(p::ff(pv, ANGLE), i as f32 * (6.2831855 / n)), m.rotation, m.position);
        let t = if 0.5 < f { -f + 1.0 + -f + 1.0 } else { f + f };
        let c = crate::hud::tween_color(t, GLOW_A, GLOW_B);
        f += 3.0 / n;
        if 1.0 < f { f -= 1.0; }
        let corners = std::array::from_fn(|q| at(BLADE[q].map(|x| x * k)));
        blades.push(FxQuad { corners, st: BLADE_ST, rgba: [c, c, GLOW_B, GLOW_B] });
    }
    let rings = (0..4).map(|r| {
        let at = frame(p::ff(pv, RINGS + 4 * r), m.rotation, m.position);
        FxQuad { corners: std::array::from_fn(|q| at(RING[q])), st: RING_ST, rgba: [RING_RGBA; 4] }
    }).collect();
    vec![FxQuads { fx: BLADE_FX, additive: false, subtract: false, quads: blades }, FxQuads { fx: RING_FX, additive: false, subtract: false, quads: rings }]
}

