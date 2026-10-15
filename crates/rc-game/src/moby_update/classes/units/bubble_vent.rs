//! Quartu's bubble vents, class 1425 (level 15, 6 instances): level15 0x2eb928 (census unit U484). Each tick, one
//! time in two (`randi(3 − 1) == 0`, `$gp` 0x16219c = 3), a bubble rises from the vent to the water level in its
//! pvar +0x00: `PartType34Spawn(randf(0.06, 0.12)·210000, level, pos, (0, 0, randf(0.75, 1.5)·6·dt))` (`$gp`
//! 0x162198 = 6). Read from the level15 decomp. The name is descriptive [L]. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2eb928 | `randi(2)`; 0: `0x1f89d8` (zero vector), `randf(0.75, 1.5)`, `randf(0.06, 0.12)`, `PartType34Spawn` | [`update`] (`bomb_water::part34`, its six draws) |
//! | | no sound, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::bomb_water;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_b928;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [1425];
/// `$gp` 0x162198 and 0x16219c.
pub const RISE: f32 = 6.0;
pub const ODDS: i32 = 3;

/// Level15 0x2eb928 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 4 { return; }
    if w.rng.randi(ODDS - 1) != 0 { return; }
    let vz = w.rng.randf(0.75, 1.5) * RISE * DT;
    let size = w.rng.randf(0.06, 0.12) * 210000.0;
    let (level, pos) = (c::pf(w, id, 0), c::pos(w, id));
    bomb_water::part34(w, size, level, pos, [0.0, 0.0, vz]);
}
