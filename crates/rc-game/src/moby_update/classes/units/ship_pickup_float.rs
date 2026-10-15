//! **The floating ship pickups, classes 224 (missiles) and 228 (health)** (level13 `0x2e1cc8`, census U433, eight of
//! each placed on Gemlik; the same code on level 17 (`0x2c53d8`), where the fleet's fighters 1843 drop them through
//! the spawner `0x2c5360`, [`super::ship_fighter`]). One unit through [`LevelPorts`](super::LevelPorts) code identity. A
//! pickup can wait for a moby to reach a state (its pvars), then for a delay, before it shows; shown, it spins and bobs
//! and is drawn toward the flown ship ([`crate::vehicle`], classes 1242 / 69 / 1379) within 40; the ship touching it
//! (or within 4) collects it: five missiles or ten health; it shrinks away and, when it respawns, waits again. Read
//! from the level13 decomp. Native `f32`.
//!
//! **Pvar block** (0x1c; the instance's words): +0x00 the moby waited for (index, −1 none), +0x04 its state (−1: any
//! but gone), +0x08 the respawn delay (−1: none), +0x0c (u8) shown from the start, +0x0d (u8) respawns, +0x10 the delay
//! timer, +0x14 / +0x18 the bob's offset and phase.
//!
//! ## Coverage (`0x2e1cc8`)
//! | address | what | port |
//! |---|---|---|
//! | 0 | shown from the start → 4 at the class scale; else no collision, not drawn, hidden, scale = class scale / 10, → 1 | [`update`] |
//! | 1 | a moby to wait for → 2; else → 3 | [`update`] |
//! | 2 | the moby alive, a state given and not reached → wait; else → 3 | [`update`] |
//! | 3 | no delay, or `FastDecTimer(+0x10)` out → 4: drawn, the class collision, shown, scale = class scale / 10 | [`update`] |
//! | 4 | rot.z += 45°·dt; the position clamped to [20, 1003]; scale += (class scale − scale)·0.1; joint 0's point: `coll_sphere(2, it, 1, m)` touching a ship → collected; Ratchet in 0x32 with a live ship: d = \|ship − position\|; d < 4 → collected; d < 40 and drawn → position += (ship − position) at 0.8 / √d | [`update`] |
//! | 4 | collected: no collision; 224: missiles + 5 (u8) up to the most, `PlayClassSound(4, 0, ship)`; 228: health + 10 up to the most, `PlayClassSound(5, 0, ship)`; → 5 | [`update`] |
//! | 5 | rot.z += 2·(class scale − scale); scale += (class scale / 100 − scale)·0.3; below class scale / 20 → 6 | [`update`] |
//! | 6 | no respawn → `DeleteMoby`; else no collision, not drawn, hidden, scale = class scale / 10, +0x10 = the delay, → 1 | [`update`] |
//! | tail | the bob (`0x277a00(0.7, 70°·dt, m, +0x18, +0x14)`); the position clamped to [20, 1003] | [`update`] (`units::bob`) |
//! | level17 0x2c5360 | `CreateMoby(class)`: distances 0xff, drawn, state 0, +0xbc = 0, position; `MobyBuildMatrix`; Ratchet's light (`0x272078`) | [`spawn`] (a created pickup's pvars are zero: it waits for moby 0 to be in state 0) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, add, add_rot, dist3, set_len3, DT};
use crate::moby_update::services::{pf as to_pf, pv, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x2e_1cc8;
pub const MISSILES: i16 = 0xe0;
pub const HEALTH: i16 = 0xe4;
pub const CLASSES: [i16; 2] = [MISSILES, HEALTH];

pub mod pvo {
    pub const WAIT_MOBY: usize = 0x00;
    pub const WAIT_STATE: usize = 0x04;
    pub const DELAY: usize = 0x08;
    pub const SHOWN: usize = 0x0c;
    pub const RESPAWN: usize = 0x0d;
    pub const TIMER: usize = 0x10;
    pub const BOB: usize = 0x14;
    pub const PHASE: usize = 0x18;
    pub const LEN: usize = 0x1c;
}

fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn alive(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s != 0xfe && s != 0xfd }
fn class_scale(w: &World, id: MobyId) -> f32 { super::class_scale(w, w.m(id).o_class) }

/// Level17 `0x2c5360(pos, class)` (module doc).
pub fn spawn(w: &mut World, pos: [f32; 4], class: i16) -> Option<MobyId> {
    let id = w.create_moby(class)?;
    story::pvars(w, id, pvo::LEN);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.cmd = 0;
        m.position = pos;
    }
    w.build_matrix(id);
    story::copy_hero_light(w, id);
    Some(id)
}

fn hide(w: &mut World, id: MobyId) {
    let s = class_scale(w, id) * 0.1;
    let m = w.mm(id);
    m.has_collision = false;
    m.visible = 0;
    m.mode |= 1;
    m.scale = s;
}

fn clamp(w: &mut World, id: MobyId) {
    let p = w.m(id).position;
    let cl = |x: f32| x.clamp(20.0, 1003.0);
    w.mm(id).position = [cl(p[0]), cl(p[1]), cl(p[2]), p[3]];
}

/// Level13 `0x2e1cc8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    match w.m(id).state {
        0 => {
            if c::pu8(w, id, pvo::SHOWN) != 0 {
                let s = class_scale(w, id);
                let m = w.mm(id);
                m.state = 4;
                m.scale = s;
            } else {
                hide(w, id);
                w.mm(id).state = 1;
            }
        }
        1 => w.mm(id).state = if pi(w, id, pvo::WAIT_MOBY) != -1 { 2 } else { 3 },
        2 => {
            let m = usize::try_from(pi(w, id, pvo::WAIT_MOBY)).ok().filter(|&m| m < w.table.mobys.len());
            let want = pi(w, id, pvo::WAIT_STATE);
            let waiting = m.is_some_and(|m| alive(w, m) && want != -1 && w.m(m).state as i32 != want);
            if !waiting { w.mm(id).state = 3; }
        }
        3 => {
            if pi(w, id, pvo::DELAY) == -1 || c::dec_timer_pvar_i32(w, id, pvo::TIMER) != 0 { w.mm(id).state = 4; }
            if w.m(id).state == 4 {
                let coll = super::class_collision(w, w.m(id).o_class);
                let s = class_scale(w, id) * 0.1;
                let m = w.mm(id);
                m.visible = 1;
                m.has_collision = coll;
                m.mode &= !1;
                m.scale = s;
            }
        }
        4 => {
            let rz = add_rot(w.m(id).rotation[2], DT * std::f32::consts::FRAC_PI_4);
            w.mm(id).rotation[2] = rz;
            clamp(w, id);
            let joint = w.joint_point(id, 0);
            let cs = class_scale(w, id);
            {
                let m = w.mm(id);
                m.scale += (cs - m.scale) * 0.1;
            }
            let touched = w.coll_sphere(pv(joint), to_pf(2.0), 1, Some(id)).and_then(|h| h.moby).is_some_and(|m| super::ship_pickup::SHIPS.contains(&w.m(m).o_class));
            let ship = w.svc.vehicle.moby.filter(|&s| w.hero.state == 0x32 && alive(w, s) && super::ship_pickup::SHIPS.contains(&w.m(s).o_class));
            let d = ship.map_or(100_000.0, |s| dist3(w.m(s).position, w.m(id).position));
            if !touched && 4.0 <= d {
                if let Some(s) = ship.filter(|_| d < 40.0 && w.m(id).visible != 0) {
                    let k = 0.8 / d.sqrt();
                    let v = set_len3(c::sub(w.m(s).position, w.m(id).position), k);
                    let p = add(w.m(id).position, v);
                    w.mm(id).position = p;
                }
            } else {
                w.mm(id).has_collision = false;
                let live = w.svc.vehicle.moby.filter(|&s| alive(w, s));
                match w.m(id).o_class {
                    MISSILES => {
                        let v = &mut w.svc.vehicle;
                        v.missiles = v.missiles.wrapping_add(5);
                        if let Some(s) = live { w.play_sound(4, 0, s); }
                        let v = &mut w.svc.vehicle;
                        if v.missiles_max < v.missiles { v.missiles = v.missiles_max; }
                    }
                    HEALTH => {
                        w.svc.vehicle.health += 10.0;
                        if let Some(s) = live { w.play_sound(5, 0, s); }
                        let v = &mut w.svc.vehicle;
                        if v.health_max < v.health { v.health = v.health_max; }
                    }
                    _ => {}
                }
                w.mm(id).state = 5;
            }
        }
        5 => {
            let cs = class_scale(w, id);
            let d = cs - w.m(id).scale;
            let rz = add_rot(w.m(id).rotation[2], d + d);
            let m = w.mm(id);
            m.rotation[2] = rz;
            m.scale += (cs * 0.01 - m.scale) * 0.3;
            if m.scale < cs * 0.05 { m.state = 6; }
        }
        6 => {
            if c::pu8(w, id, pvo::RESPAWN) == 0 {
                w.delete_moby(id);
                return;
            }
            hide(w, id);
            let t = pi(w, id, pvo::DELAY);
            c::set_pi32(w, id, pvo::TIMER, t);
            w.mm(id).state = 1;
        }
        _ => {}
    }
    super::bob(w, id, f32::from_bits(0x3f33_3333), DT * 1.221_730_5, pvo::PHASE, pvo::BOB);
    clamp(w, id);
}
