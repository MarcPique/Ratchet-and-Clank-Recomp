//! Kerwan's turntable, class 825 (level03 `0x2d3e58`, 1 placed: #923 at (290, 244), the only copy): a platform that
//! turns about z at 0.48 rad/s forever and carries its riders with the turn (its platform block at pvar +0x20, the
//! offset the level data's pvar +0x08 names). Read from the level03 disassembly (32 words).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2d3e58` | +0x30 = 0xff, +0x32 = 0xff; the old Euler (+0x40, +0x48) kept; +0x48 = `fast_add_rotations(+0x48, dt·0x3ef5be0b)`; `CarryRiders(pvar +0x20, 0, old, new)` (`0x24ee28` = `0x2755f8`) | [`update`] (`triggers::carry_riders`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{add_rot, DT};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const UPDATE_FN: u32 = 0x2d_3e58;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 1] = [825];
/// The platform block in the pvars.
const BLOCK: usize = 0x20;
/// The turn rate (rad/s).
const RATE: f32 = f32::from_bits(0x3ef5_be0b);

/// Level03 `0x2d3e58` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    if m.pvars.len() < BLOCK + 0x40 { m.pvars.resize(BLOCK + 0x40, 0); }
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    let old = m.rotation;
    m.rotation[2] = add_rot(m.rotation[2], DT * RATE);
    let new = m.rotation;
    triggers::carry_riders(&mut m.pvars, BLOCK, [0.0; 4], old, new);
}
