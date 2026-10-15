//! Eudora's path riders, class 617 (level 04, 4 created instances): level04 0x2d6f68 (census U162). Every tick the
//! moby stands on its path at the fraction `1 − t` read from another moby's pvar +0x00 (a lift or crank's progress),
//! and puts a third moby there too. Read from the level04 decomp of 0x2d6f68 (`FUN_001f2cc8` = L01 0x222170, the VU
//! `fmod`). Native `f32`.
//!
//! **Pvar block**: +0x00 s32 the source moby (its pvar +0x00 is `t`), +0x04 s32 the carried moby, +0x08 s32 the path
//! (`0x1b0630[i]` on 04: count, then the points from +0x10).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2d6f68 | x = (1 − t)·(count − 1); f = x − trunc(x) (0x1f2cc8 with 1); i = trunc(x); position = point i·(1 − f) + point (i + (f ≠ 0))·f (all four lanes; `vec_scale` / `vec_add`) | [`update`] |
//! | | the carried moby's position = this position | [`update`] |
//! | | no state, sound, particle, hit, flag (the game has no guard for a −1 index: the port does nothing [L]) | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::{pvar as p, World};

/// The update in the level04 class table.
pub const UPDATE_FN: u32 = 0x2d_6f68;
pub const REFERENCE_LEVEL: u32 = 4;
pub const CLASSES: [i16; 1] = [617];

/// Level04 0x2d6f68 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    let (src, dst, path) = (c::pi32(w, id, 0), c::pi32(w, id, 4), c::pi32(w, id, 8));
    let Some(pts) = usize::try_from(path).ok().and_then(|i| w.svc.splines.get(i)).cloned() else { return };
    let Some(t) = usize::try_from(src).ok().and_then(|s| w.table.mobys.get(s)).filter(|m| m.pvars.len() >= 4).map(|m| p::ff(&m.pvars, 0)) else { return };
    if pts.is_empty() { return; }
    let x = (1.0 - t) * (pts.len() as i32 - 1) as f32;
    let f = x - (x as i32) as f32;
    let i = x as i32;
    let j = i + (f != 0.0) as i32;
    let pt = |k: i32| pts.get(usize::try_from(k).unwrap_or(usize::MAX)).map(|q| q.map(f32::from_bits));
    let (Some(a), Some(b)) = (pt(i), pt(j)) else { return };
    let pos: [f32; 4] = std::array::from_fn(|k| a[k] * (1.0 - f) + b[k] * f);
    w.mm(id).position = pos;
    if let Some(m) = usize::try_from(dst).ok().filter(|&d| d < w.table.mobys.len()) { w.mm(m).position = pos; }
}
