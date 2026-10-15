//! The Pyrocitor's pilot flame, class 179 (level01 update `0x2d1068`; created by the Pyrocitor's update through
//! `0x2d0fc8`, `crate::hero::pyrocitor`): a small flickering flame at the nozzle, drawn with alpha 0x50.
//!
//! **Update**: gone with its owner (the owner's state byte 0, or 0xf0.. deleted) → `DeleteMoby`; the owner's hidden bit
//! (mode bit 0: the first-person view hides the item) copied; hidden and not animated (mode |= 0x41) while the slot has
//! been ready for less than 25 ticks; the class's texture scroll steps by −0xc0 (wrapping in 0..0x1000:
//! `FUN_00263d90(class, 0, d)`, kept at +0x04; drawn by `rc-engine` as the record's ST offset); scale = class scale ×
//! 0.5 (+0x0c) × `randf(0.5, 1)` (one draw a tick); its spin +0x44 += 0.5 rad (`FUN_002731d0`: the angle wrapped).
//! The owner writes its rows and position every tick (the item update). Standard `f32`.
//!
//! Two owners: the Pyrocitor's hand item and the Visibomb's missile 172 (`super::visibomb`, its glow `0x2cb808`), both
//! creating it through `0x2d0fc8` ([`init`]). The owner's fields the update reads are written into this moby's pvars by
//! the owner ([`crate::hero::pyrocitor::glow_pv`]; the hand item is not a moby of the table).

use crate::hero::pyrocitor::glow_pv as pv;
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

/// The update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2d1068;
pub const CLASS: i16 = crate::hero::pyrocitor::GLOW_CLASS;
pub const CLASSES: [i16; 1] = [CLASS];

fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let x = (a + PI) / TAU;
    (x - x.floor()) * TAU - PI
}

/// `0x2d0fc8(owner)`'s set-up of a new flame after `CreateMoby(0xb3)` (the caller creates it: the Pyrocitor through
/// the hero's hit sink, the Visibomb's missile in the moby loop): distances 0xff, visible, mode 0x204 (plus the port's
/// KEEP_ROWS: the owner writes the rows every tick), alpha 0x50, state 0, rotation 0, the owner's rows; the position
/// where the caller wants it (None: as created, until the owner places it).
pub fn init(m: &mut crate::moby_runtime::Moby, rows: [[f32; 3]; 3], pos: Option<[f32; 3]>) {
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.visible = 1;
    m.mode = 0x204 | 0x100;
    m.alpha = 0x50;
    m.state = 0;
    m.rotation = [0.0; 4];
    for (row, src) in m.rows.iter_mut().zip(rows) { *row = [src[0], src[1], src[2], 0.0]; }
    if let Some(p) = pos { m.position = [p[0], p[1], p[2], 0.0]; }
    if m.pvars.len() < 0x10 { m.pvars.resize(0x10, 0); }
}

/// `0x2d1068`.
pub fn update(w: &mut World, id: MobyId) {
    let class_scale = w.classes.info(CLASS).map_or(1.0, |c| c.scale);
    let m = w.mm(id);
    if m.pvars.len() < 0x10 { m.pvars.resize(0x10, 0); }
    let owner = m.pvars[pv::OWNER_STATE];
    if owner == 0 || owner & 0xf0 == 0xf0 {
        w.delete_moby(id);
        return;
    }
    let hidden = m.pvars[pv::OWNER_HIDDEN] as u16 & 1;
    if m.mode & 1 != hidden { m.mode = (m.mode & !1) | hidden; }
    let ready = i16::from_le_bytes([m.pvars[pv::READY], m.pvars[pv::READY + 1]]) as i32;
    if ready < crate::hero::physics::ticks(25) { m.mode |= 0x41; }
    let mut s = i32::from_le_bytes(m.pvars[pv::SCROLL..pv::SCROLL + 4].try_into().unwrap()) - 0xc0;
    if 0x1000 < s { s -= 0x1000; } else if s < 0 { s += 0x1000; }
    m.pvars[pv::SCROLL..pv::SCROLL + 4].copy_from_slice(&s.to_le_bytes());
    let k = f32::from_le_bytes(m.pvars[pv::SCALE..pv::SCALE + 4].try_into().unwrap());
    let r = w.rng.randf(0.5, 1.0);
    let m = w.mm(id);
    m.scale = class_scale * k * r;
    m.rotation[1] = wrap(m.rotation[1] + wrap(k));
}

#[cfg(test)]
mod tests {
    #[test]
    fn wrap_keeps_small_angles() {
        assert!((super::wrap(0.5) - 0.5).abs() < 1e-6);
        assert!((super::wrap(3.5) - (3.5 - std::f32::consts::TAU)).abs() < 1e-5);
    }
}
