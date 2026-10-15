//! Kalebo's glowing beacons, class 470 (level 16, 8 created instances): level16 0x2c9480 (census U505), with its glow
//! init 0x2c9568 and glow 0x2c9650. The gold bolts' item glow with Kalebo's own data (fourth consumer of
//! `gold_bolt::glow_init_with` / `item_glow_with`): four soft type-59 sprites turning at −1, −2.25, 1.25 and 2.5 a
//! tick, sizes 3.5 / 4.2 / 4.2 / 3.5, pulsing 0x4040ffff ↔ 0x1040ffff, centred 0.1 above the moby; the moby spins at
//! 240°/s and is set 0.9 above the ground when it was placed within 1 of it. Read from the level16 decomp of the three
//! functions. Native `f32`; the `rand` draws in the game's order.
//!
//! **Pvar block**: the glow block at +0x00 (angles, spins, timers, sizes: `gold_bolt::glow`).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | 0x2c9568: angles `randi(255)` ×4 (0x253228 = L01 0x26c930), spins (−1, −2.25, 1.25, 2.5), timers `ticks(63 + 64k)`, sizes (3.5, 4.2, 4.2, 3.5) | [`update`] (`gold_bolt::glow_init_with`) |
//! | | scale +0x2c ·= 0.9; g = `GroundHeight(0.5, position, 0)` (0x254fd0 = L01 0x26e618); \|g − z\| < 1 → z = g + 0.9; → 1 | [`update`] |
//! | state 1 | yaw += 4π/3·dt (`fast_add_rotations`); 0x2c9650 | [`update`] |
//! | 0x2c9650 | centre = position + (0, 0, 0.1); from 0.3 behind it (away from the camera 0x1671c0) 0.1 apart: per sprite the angle += spin (wrapped into (0, 255)), `FastDecTimer` out → `ticks(255)`, colour `FastTweenColor(\|0.5 − (ticks(255) − timer)/ticks(255)\|, 0x4040ffff, 0x1040ffff)`, `PartType59Spawn(size, at, colour, trunc(angle), 0x35, 1, 2, 0)` (0x26e3a8 = L01 0x288410) | [`update`] (`gold_bolt::item_glow_with`) |
//! | | no sound, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::gold_bolt::{glow_init_with, item_glow_with, GLOW_A, GLOW_B};
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{self as sv, World};

/// The update in the level16 class table.
pub const UPDATE_FN: u32 = 0x2c_9480;
pub const REFERENCE_LEVEL: u32 = 16;
pub const CLASSES: [i16; 1] = [470];
pub const SPINS: [f32; 4] = [-1.0, -2.25, 1.25, 2.5];
pub const SIZES: [f32; 4] = [3.5, 4.2, 4.2, 3.5];

/// Level16 0x2c9480 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    match w.m(id).state {
        0 => {
            glow_init_with(w, id, 0, SPINS, SIZES);
            w.mm(id).scale *= 0.9;
            let p = w.m(id).position;
            let g = sv::fl(w.ground_height(sv::pf(0.5), sv::pv(p), 0));
            if (g - p[2]).abs() < 1.0 { w.mm(id).position[2] = g + 0.9; }
            w.mm(id).state = 1;
        }
        1 => {
            let y = c::add_rot(w.m(id).rotation[2], DT * 4.188_790_3);
            w.mm(id).rotation[2] = y;
            let p = w.m(id).position;
            item_glow_with(w, id, 0, [p[0], p[1], p[2] + 0.1], 1.0, true, (GLOW_A, GLOW_B));
        }
        _ => {}
    }
}
