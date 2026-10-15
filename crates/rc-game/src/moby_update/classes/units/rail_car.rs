//! Kalebo's rail cars, class 257: level16 0x2c3d38 (census U495; 21 created instances). A car parked at its path's
//! first point; when Ratchet grinds (state group 0xf) inside its cuboid it runs along the path with its class's
//! loop sound 0 (and sound 1 once, when Ratchet comes within 7 in the plane), is hidden at the end, and is put
//! back at the start once its start point is out of view while Ratchet is not grinding. Read from the level16
//! decomp and disassembly of 0x2c3d38; native `f32`.
//!
//! **Pvar block**: +0x00 s32 the cuboid, +0x04 s32 the path (`0x1b0930[i]`), +0x08 f32 the speed (units/s),
//! +0x0c f32 the step per tick, +0x10 f32 t, +0x14 s16 the loop-sound slot, +0x16 s16 "sound 1 played".
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | cuboid −1 or path −1 → `DeleteMoby`; step = speed · dt / \|p₀ − p₁\|; state 1; update distance 100; +0xbc = 0 | [`update`] |
//! | state 1 | `0x1413dc` = 0xf and `PointInCuboid(0x13f420 body point, cuboid)` (0x25b258) → state 2, +0x16 = 0 | [`update`] (`triggers::point_in_cuboid`) |
//! | state 2 | t += step; t ≥ count − 1 → update distance 0xff, state 3; else `0x25e338(t, path, 0, &pos, &rot, 0)` ([`crate::path::pose`]) | [`update`] |
//! | | +0xbc ≠ 0 → collision off (+0x94 = 0) | [`update`] |
//! | | `SoundIsAlive(m, +0x14)` else +0x14 = `PlayClassSound(0, 4, m)`; +0x16 = 0 and Ratchet (0x13f3d0) within 7 in xy (`VecDistance2`) → +0x16 = 1, `PlayClassSound(1, 0, m)` | [`update`] (`World::sound_alive`, `play_sound`) |
//! | state 3 | not drawn (+0x31 = 0) → collision off, mode \|= 1 (hidden) | [`update`] |
//! | | `0x1413dc` ≠ 0xf and `FastBSphereCheck(64, (p₀, 3))` = −1 → +0xbc = 0, t = 0, the pose at 0, +0x31 = 1, mode &= ~1, collision on (class +0x10), +0x16 = 0, state 1 | [`update`] (`BSphereView::culled`; no view: out of view) |
//! | | no particle, light, hit or save flag | — |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{dist2, dist3, pf, pi32, set_pf, DT};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::triggers::point_in_cuboid;
use crate::path::pose;

pub const UPDATE_FN: u32 = 0x2c_3d38;
pub const REFERENCE_LEVEL: u32 = 16;
pub const CLASSES: [i16; 1] = [257];

/// Ratchet's grind state group (`0x1413dc`).
const GRIND_GROUP: i32 = 0xf;

pub mod pv {
    pub const CUBOID: usize = 0x00;
    pub const PATH: usize = 0x04;
    pub const SPEED: usize = 0x08;
    pub const STEP: usize = 0x0c;
    pub const T: usize = 0x10;
    pub const SOUND: usize = 0x14;
    pub const NEAR: usize = 0x16;
}

fn path(w: &World, id: MobyId) -> Vec<[u32; 4]> {
    usize::try_from(pi32(w, id, pv::PATH)).ok().and_then(|i| w.svc.splines.get(i)).cloned().unwrap_or_default()
}

/// Level16 0x2c3d38 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    match w.m(id).state {
        0 => {
            let pts = path(w, id);
            if pi32(w, id, pv::CUBOID) == -1 || pi32(w, id, pv::PATH) == -1 || pts.len() < 2 {
                w.delete_moby(id);
                return;
            }
            let (a, b) = (pts[0].map(f32::from_bits), pts[1].map(f32::from_bits));
            let step = (pf(w, id, pv::SPEED) * DT) / dist3(a, b);
            set_pf(w, id, pv::STEP, step);
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 100;
            m.cmd = 0;
        }
        1 => {
            let body = w.hero_body_point();
            if w.hero.group == GRIND_GROUP && point_in_cuboid(&w.svc.volumes, body, pi32(w, id, pv::CUBOID)) {
                let m = w.mm(id);
                m.state = 2;
                p::set_i16(&mut m.pvars, pv::NEAR, 0);
            }
        }
        2 => {
            let pts = path(w, id);
            let t = pf(w, id, pv::T) + pf(w, id, pv::STEP);
            set_pf(w, id, pv::T, t);
            if (pts.len() as i32 - 1) as f32 <= t {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.state = 3;
            } else {
                let (pos, rot) = pose(&pts, false, t, true);
                let m = w.mm(id);
                m.position = pos;
                m.rotation = rot;
            }
            if w.m(id).cmd != 0 { w.mm(id).has_collision = false; }
            let slot = p::i16(&w.m(id).pvars, pv::SOUND) as i32;
            if !w.sound_alive(slot, id) {
                let h = w.play_sound(0, 4, id);
                p::set_i16(&mut w.mm(id).pvars, pv::SOUND, h as i16);
            }
            let hp = super::hero_pos(w);
            if p::i16(&w.m(id).pvars, pv::NEAR) == 0 && dist2(w.m(id).position, hp) < 7.0 {
                p::set_i16(&mut w.mm(id).pvars, pv::NEAR, 1);
                w.play_sound(1, 0, id);
            }
        }
        3 => {
            if w.m(id).visible == 0 {
                let m = w.mm(id);
                m.has_collision = false;
                m.mode |= 1;
            }
            if w.hero.group == GRIND_GROUP { return; }
            let pts = path(w, id);
            let Some(p0) = pts.first().map(|q| q.map(f32::from_bits)) else { return };
            let out = w.view.map(|v| v.culled(64.0, [p0[0], p0[1], p0[2], 3.0])).unwrap_or(true);
            if !out { return; }
            set_pf(w, id, pv::T, 0.0);
            let (pos, rot) = pose(&pts, false, 0.0, true);
            let coll = super::class_collision(w, w.m(id).o_class);
            let m = w.mm(id);
            m.cmd = 0;
            m.position = pos;
            m.rotation = rot;
            m.visible = 1;
            m.mode &= !1;
            m.has_collision = coll;
            p::set_i16(&mut m.pvars, pv::NEAR, 0);
            m.state = 1;
        }
        _ => {}
    }
}
