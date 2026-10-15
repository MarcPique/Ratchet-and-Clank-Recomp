//! Hoverboard-course sparkles, class 1139: level05 0x31abe0 (census U203; 12 created instances on Rilgar and
//! Kalebo, the level16 copy 0x2e0f78 is the same code). An invisible marker that, while Ratchet rides the
//! hoverboard (movement group 0x16) and its sphere is in view, starts three type-67 orbiting sparks around itself
//! every tick. Read from the level05 decomp (0x31abe0). Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | update distance +0x30 = 0x80, → 1, mode \|= 0x41 (hidden, no animation) | [`update`] |
//! | state 1 | `FastBSphereCheck(128, (pos.xyz, 8))` ≠ −1 and Ratchet's movement group (0x1413dc) = 0x16 → 3 × `PartType67Spawn(rand_angle, rand_angle, m, 0x15f580 (zero), ticks(40), mode 1, red 0x7f, no dir)` (0x289850; the two angle draws, then the spawner's `randi(255)`) | [`update`] (`particles::type67`; no view: out of view) |
//! | | no sound, hit, light, save flag or other moby | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::services::World;
use crate::particles::type67;

/// The update in the level05 class table.
pub const UPDATE_FN: u32 = 0x31_abe0;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 1] = [1139];

/// Ratchet's movement group on the hoverboard (0x1413dc).
pub const BOARD_GROUP: i32 = 0x16;
/// The view test's draw distance and sphere radius.
const VIEW_DIST: f32 = 128.0;
const VIEW_R: f32 = 8.0;

/// Level05 0x31abe0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.update_dist = 0x80;
            m.state = 1;
            m.mode |= mode::HIDDEN | mode::NO_ANIM;
        }
        1 => {
            let p = w.m(id).position;
            let seen = w.view.is_some_and(|v| !v.culled(VIEW_DIST, [p[0], p[1], p[2], VIEW_R]));
            if !seen || w.hero.group != BOARD_GROUP { return; }
            for _ in 0..3 {
                let a = w.rng.rand_angle();
                let b = w.rng.rand_angle();
                let life = w.ticks(40);
                let s = type67::Spawn { a, b, moby: Some(id), pos: [0.0; 4], life, mode: 1, red: 0x7f, dir: None };
                *w.svc.fx.part_spawns.entry(type67::TYPE).or_default() += 1;
                match w.particles.as_deref_mut() {
                    Some(ps) => {
                        if type67::spawn(ps, w.rng, s).is_none() { w.svc.fx.part_failed += 1; }
                    }
                    None => { w.rng.randi(0xff); }
                }
            }
        }
        _ => {}
    }
}
