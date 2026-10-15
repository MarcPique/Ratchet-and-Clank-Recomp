//! Deleted markers, classes 915 / 916 / 917: level03 0x2dc310, the same code on 10 (census U139; 19 created
//! instances). The update measures the distance and the heading from the moby to Ratchet (`VecDistance` 0x221360,
//! `FastArcTan` 0x2217c0, `FastDiffRots` 0x222100), discards the results and deletes the moby: every instance is gone
//! after its first update. Read from the level03 decomp (0x2dc310).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2dc310 | distance / heading to Ratchet (no side effect, no `rand`) | n/a (results unused) |
//! | | `DeleteMoby` (0x2636c0) | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

/// The update in the level03 class table.
pub const UPDATE_FN: u32 = 0x2d_c310;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 3] = [915, 916, 917];

/// Level03 0x2dc310 (module doc).
pub fn update(w: &mut World, id: MobyId) { w.delete_moby(id); }
