//! Grass / plants, classes 724 / 725: `GrassUpdate` level01 0x2fa720 (the whole function).
//!
//! ```text
//! if VecDistance(hero.pos 0x13f3d0, m.pos) < 1.0            // 0x221360: 3-D, vsub; (x²+y²)+1·z²; vsqrt
//!    && dt + dt < FastVecLength(hero 0x13f450)               // the hero's displacement this tick
//!    && m+0x53 != 1:
//!     MobyAnimBlend(m, 1, 0, ticks(6))                        // fun_00212f90
//! if (m+0x70 & 2) && m+0x53 == 1:                             // sequence 1 wrapped this tick
//!     MobyAnimBlend(m, 0, 0, ticks(6))
//! ```
//! No state byte, no RNG. The sequence-end test reads the flags `MobyAnimAdvance` wrote before the update
//! (the blend clears bit 1, so a moby blended to 1 this tick never returns in the same tick).

use crate::hero::physics as ph;
use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pv, World, DT};
use crate::ps2v::Pf;

/// The grass update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2fa720;

/// Classes that run [`update`].
pub const CLASSES: [i16; 2] = [724, 725];

/// `GrassUpdate` (0x2fa720).
pub fn update(w: &mut World, id: MobyId) {
    let pos = pv(w.m(id).position);
    let d = ph::dist3(w.hero.pos, pos);
    if d < Pf::ONE && DT + DT < ph::len3(w.hero.disp) && w.m(id).anim.seq_b != 1 {
        let t = w.ticks(6);
        w.anim_blend(id, 1, 0, t);
    }
    let a = &w.m(id).anim;
    if a.flags & 2 != 0 && a.seq_b == 1 {
        let t = w.ticks(6);
        w.anim_blend(id, 0, 0, t);
    }
}
