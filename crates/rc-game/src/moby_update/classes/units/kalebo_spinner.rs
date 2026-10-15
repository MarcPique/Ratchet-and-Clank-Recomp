//! **Kalebo's spinning sign, class 1891** (level16 `0x2e88d0`, 2 placed at (158, 264), the second turned π; census
//! U569; the name is descriptive [L]): at its first update it grows fourfold and moves to z 160, then turns −15° a
//! second for good. Read from the level16 decomp.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, update distance 0xff, draw distance (+0x32) 0x200, scale ·4, z = 160 | [`update`] |
//! | state 1 | yaw = `fast_add_rotations(yaw, dt·−15°)` (0.2617994) | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{add_rot, DT};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_88d0;
pub const CLASSES: [i16; 1] = [1891];
pub const HEIGHT: f32 = 160.0;
const RATE: f32 = -0.261_799_4;

/// Level16 `0x2e88d0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    match m.state {
        0 => {
            m.state = 1;
            m.update_dist = 0xff;
            m.draw_dist = 0x200;
            m.scale *= 4.0;
            m.position[2] = HEIGHT;
        }
        1 => m.rotation[2] = add_rot(m.rotation[2], DT * RATE),
        _ => {}
    }
}
