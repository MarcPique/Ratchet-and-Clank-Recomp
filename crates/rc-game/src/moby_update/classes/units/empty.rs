//! The empty update (census U99): 15 update functions on 10 levels (classes 87, 283, 346, 419, 690, 765, 789, 1104,
//! 1140, 1278, 1280, 1672; 67 created instances) are `jr ra; nop`. The class then has an update, so
//! `InitMobyInstance` does not set mode 2: the scheduler advances its animation and rebuilds its matrix every tick
//! it is active, which is all the game does for it. `LevelPorts` gives this row to every class-table entry whose code
//! starts with `jr ra` with a `nop` in the delay slot ([`is_empty`]): such a function returns at once whatever
//! follows it, and a 2-word function is too short for the code-identity match.
//!
//! | address | what | port |
//! |---|---|---|
//! | level02 0x2dd4d0 (and 14 copies) | `jr ra; nop` | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

/// The reference copy in the level02 class table (690; 789 runs 0x2e2220).
pub const UPDATE_FN: u32 = 0x2d_d4d0;
pub const REFERENCE_LEVEL: u32 = 2;
pub const CLASSES: [i16; 2] = [690, 789];

/// `jr ra; nop`.
pub fn update(_: &mut World, _: MobyId) {}

/// An update whose first instruction is `jr ra` with a `nop` in the delay slot.
pub fn is_empty(code: &[u32]) -> bool { code.starts_with(&[0x03e0_0008, 0]) }
