//! **Batalia's small classes** (level08; read from the decomp):
//!
//! * **468 / 469, the crank-turned bridges** (`0x2ea4c0`, census U305; 5 placed each, in pairs): pvar +0x00 the bolt
//!   crank (class 280, `bolt_crank`) whose progress (its pvar +0x00, 0..1) turns them, +0x04 / +0x08 the end and start
//!   angles (degrees), +0x0c a story flag (0x13d3b8 + it), +0x10.. the placement, +0x20 the hum's slot. Euler y =
//!   start + (end − start)·progress; position = the placement + the Euler rows' third row ·30. 468 also hums (class sound 0,
//!   faded in and out over `ticks(60)`) while it turns and, at progress 1, sets its flag, plays sound 1 and stops.
//! * **1553, the scene's thrusters** (`0x3084d8`, U318): in game mode 2, scenes 1 / 4 → actor 3's, 2 / 5 → actor 2's
//!   thrusters (`0x278450`); scene 1 only past its tick `ticks(2254)`.
//! * **1629, the falling streaks** (`0x3085f0`, U319; level14 `0x3088b0`): with the camera within pvar +0x04 (xy),
//!   +0x14 streaks (type 0, kind +0x00) at an angle `randf(+0x28, +0x2c)` (degrees, as `(a − 180)/180·π`) and a
//!   distance `randf(+0x0c, +0x08)` from it, those beyond +0x10 of the camera only: velocity (`randf(±+0x24·dt)` +
//!   the weather's wind, `−randf(+0x18, +0x1c)·dt`), the floor +0x20 below it.
//! * **1641, the steam puffs** (`0x308c90`, U320): +0x14 puffs (type 77, size +0x0c, life +0x10) at (−1,
//!   `randf(±+0x04)`, 0) in its frame, velocity (0, `randf(+0x18, +0x1c)·dt`, `randf(+0x20, +0x24)·dt`).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ea4c0` | 0: → 1, the placement kept; 1: the turn, the hum (`SoundIsAlive` / `SoundSetVolume`; the volume the game reads back from the slot (0x13e5d0 + 0x70·slot) is kept in pvar +0x24 [L]), the move, 468's end | [`bridge_update`] |
//! | `0x3084d8` | 0: → 1, +0x30 = 0xff; 1: the scenes | [`scene_fx_update`] (`cutscene_fx::infobot_thrusters`) |
//! | `0x3085f0` | the streaks | [`streaks_update`] (`particles::type00`) |
//! | `0x308c90` | the puffs | [`steam_update`] (`particles::type77`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::cutscene_fx::infobot_thrusters;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;
use crate::particles::{type00, type77};

pub const REFERENCE_LEVEL: u32 = 8;
pub const BRIDGE_FN: u32 = 0x2e_a4c0;
pub const BRIDGE_CLASSES: [i16; 2] = [468, 469];
pub const SCENE_FX_FN: u32 = 0x30_84d8;
pub const SCENE_FX_CLASSES: [i16; 1] = [1553];
pub const STREAKS_FN: u32 = 0x30_85f0;
pub const STREAKS_CLASSES: [i16; 1] = [1629];
pub const OLTANIS_LEVEL: u32 = 14;
pub const OLTANIS_STREAKS_FN: u32 = 0x30_88b0;
pub const STEAM_FN: u32 = 0x30_8c90;
pub const STEAM_CLASSES: [i16; 1] = [1641];

const HUMMER: i16 = 0x1d4;
const SLOT: usize = 0x20;
const VOLUME: usize = 0x24;
const DEG: f32 = 0.017_453_292;

fn f(w: &World, id: MobyId, o: usize) -> f32 { p::ff(&w.m(id).pvars, o) }
fn i(w: &World, id: MobyId, o: usize) -> i32 { p::i32(&w.m(id).pvars, o) }

/// Level08 `0x2ea4c0` (module doc).
pub fn bridge_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x30 { w.mm(id).pvars.resize(0x30, 0); }
    let crank = usize::try_from(i(w, id, 0)).ok().and_then(|k| w.table.mobys.get(k)).filter(|m| m.pvars.len() >= 4);
    let prog = crank.map_or(0.0, |m| p::ff(&m.pvars, 0));
    let hums = w.m(id).o_class == HUMMER;
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            let pos = c::pos(w, id);
            c::set_pv4(w, id, 0x10, pos);
        }
        1 => {
            let (end, start) = (f(w, id, 4) * DEG, f(w, id, 8) * DEG);
            let target = c::add_rot(c::sub_rot(end, start) * prog, start);
            let slot = i(w, id, SLOT);
            let step = 0x400 / w.ticks(0x3c).max(1);
            if c::diff_rots(w.m(id).rotation[1], target) == 0.0 {
                if hums && w.sound_alive(slot, id) {
                    let v = i(w, id, VOLUME);
                    if v < 0x100 {
                        w.release_sound(slot, id);
                        p::set_i32(&mut w.mm(id).pvars, SLOT, -1);
                    } else {
                        set_volume(w, id, slot, v - step);
                    }
                }
            } else {
                w.mm(id).rotation[1] = target;
                if hums {
                    if !w.sound_alive(slot, id) {
                        let s = w.play_sound(0, 4, id);
                        p::set_i32(&mut w.mm(id).pvars, SLOT, s);
                        set_volume(w, id, s, 1);
                    } else {
                        let v = i(w, id, VOLUME);
                        if v < 0x400 { set_volume(w, id, slot, v + step); }
                    }
                }
            }
            let rot = w.m(id).rotation;
            let r = crate::moby_update::services::euler_rows(crate::moby_update::services::pv(rot));
            let up = r[2].map(|x| x.to_f32());
            let base = c::pv4(w, id, 0x10);
            c::set_pos(w, id, [base[0] + up[0] * 30.0, base[1] + up[1] * 30.0, base[2] + up[2] * 30.0, base[3] + up[3] * 30.0]);
            if prog == 1.0 && hums {
                let flag = story::flag_index(0x13_d3b8) + i(w, id, 0xc).max(0) as usize;
                story::set_flag(w, flag, 1);
                w.mm(id).state = 2;
                w.play_sound(1, 0, id);
                let slot = i(w, id, SLOT);
                if w.sound_alive(slot, id) { w.release_sound(slot, id); }
            }
        }
        _ => {}
    }
}

fn set_volume(w: &mut World, id: MobyId, slot: i32, v: i32) {
    w.set_volume(slot, v);
    p::set_i32(&mut w.mm(id).pvars, VOLUME, v);
}

/// Level08 `0x3084d8` (module doc).
pub fn scene_fx_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if w.svc.game_mode != 2 { return; }
            let Some(scene) = w.svc.cinematic.scene.clone() else { return };
            let actor = match scene.id {
                1 | 4 => 3,
                2 | 5 => 2,
                _ => return,
            };
            if scene.id == 1 && scene.tick <= w.ticks(0x8ce) { return; }
            if let Some(a) = scene.actors.get(actor) { infobot_thrusters(w, a); }
        }
        _ => {}
    }
}

/// Level08 `0x3085f0` (module doc).
pub fn streaks_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x30 { return; }
    let pos = c::pos(w, id);
    let cam = w.camera.map(|x| x.to_f32());
    let (dx, dy) = (pos[0] - cam[0], pos[1] - cam[1]);
    let reach = f(w, id, 4);
    if reach * reach <= dx * dx + dy * dy { return; }
    let spread = f(w, id, 0x24) * DT;
    let a0 = (f(w, id, 0x28) - 180.0) / 180.0 * std::f32::consts::PI;
    let a1 = (f(w, id, 0x2c) - 180.0) / 180.0 * std::f32::consts::PI;
    let (wx, wy) = (f32::from_bits(w.svc.units.word(super::weather::WIND_X)), f32::from_bits(w.svc.units.word(super::weather::WIND_Y)));
    let kind = i(w, id, 0);
    for _ in 0..i(w, id, 0x14).max(0) {
        let a = w.rng.randf(a0, a1);
        let r = w.rng.randf(f(w, id, 0xc), f(w, id, 8));
        let q = [pos[0] + a.cos() * r, pos[1] + a.sin() * r, pos[2], pos[3]];
        let d = ((q[0] - cam[0]).powi(2) + (q[1] - cam[1]).powi(2)).sqrt();
        if d <= f(w, id, 0x10) { continue; }
        let vx = w.rng.randf(-spread, spread) + wx;
        let vy = w.rng.randf(-spread, spread) + wy;
        let s = w.rng.randf(f(w, id, 0x18), f(w, id, 0x1c));
        let v = [vx, vy, -(s * DT), 0.0];
        let floor = pos[2] - f(w, id, 0x20);
        *w.svc.fx.part_spawns.entry(0).or_default() += 1;
        let Some(sys) = w.particles.as_deref_mut() else { continue };
        if type00::spawn(sys, w.rng, floor, q, kind, v).is_none() { w.svc.fx.part_failed += 1; }
    }
}

/// Level08 `0x308c90` (module doc).
pub fn steam_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x28 { return; }
    let rot = w.m(id).rotation;
    let rows = crate::moby_update::services::euler_rows(crate::moby_update::services::pv(rot)).map(|r| r.map(|x| x.to_f32()));
    let pos = c::pos(w, id);
    for _ in 0..i(w, id, 0x14).max(0) {
        let r = f(w, id, 4);
        let y = w.rng.randf(-r, r);
        let q: [f32; 4] = std::array::from_fn(|k| if k == 3 { pos[3] + 1.0 } else { -rows[0][k] + y * rows[1][k] + pos[k] });
        let vy = w.rng.randf(f(w, id, 0x18) * DT, f(w, id, 0x1c) * DT);
        let vz = w.rng.randf(f(w, id, 0x20) * DT, f(w, id, 0x24) * DT);
        *w.svc.fx.part_spawns.entry(77).or_default() += 1;
        let (size, life) = (f(w, id, 0xc), i(w, id, 0x10));
        let Some(sys) = w.particles.as_deref_mut() else { continue };
        if type77::spawn(sys, size, q, [0.0, vy, vz], life).is_none() { w.svc.fx.part_failed += 1; }
    }
}
