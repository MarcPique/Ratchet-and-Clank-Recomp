//! Activation zones, class 258: level05 0x2f5200 (with its appliers 0x2f5190 / 0x2f4f60), the same code on levels
//! 5, 7, 9, 10, 12, 13, 14, 15, 17 (50 placed instances; no level01 copy, so the reference is level 05). A hidden
//! controller that switches parts of the level on and off as Ratchet (or the camera) enters and leaves a zone:
//! when the zone test's answer changes (and once at its first update) it sets, for every moby of up to 12 moby
//! groups and 11 single mobys, whether the moby updates, whether it is drawn and whether it collides. Read from the
//! level05 decomp (level05 0x2f5200, 0x2f5190, 0x2f4f60); the helpers are
//! the engine's `vec_distance` (3-D), `PointInPathPolygon` and `PointInCuboid` (clusters.tsv: level01 0x221360
//! family, 0x26e6c0, 0x274820). Native `f32`.
//!
//! **Pvar block** (P, s32 words): P[0] a path (−1: none), P[1] f32 a sphere radius around the controller (≤ 0:
//! none), P[2..7] six cuboids (−1: none), P[8..0x13] twelve moby groups (−1: none; the loader's group lists
//! `0x1abcc0`), P[0x14..0x1e] eleven moby indices (−1: none). Bytes +0x7c / +0x7d / +0x7f: the update / draw /
//! collision rule (s8: > 0 on inside, < 0 inverted, 0 untouched); +0x7e ≠ 0: the probe point is the camera
//! (`0x1671c0` on level05) instead of Ratchet's feet 0x13f3d0.
//!
//! **Update** (every tick, update distance 0xff): inside = `vec_distance(moby, probe) < P[1]` (when P[1] > 0), else
//! the path, else any of the cuboids. `moby+0xbc` holds the last answer; when the answer differs from it, or on the
//! first update (state 0): +0xbc = inside, state = inside + 1, then the rule is applied to each group's mobys
//! (in list order, the list's end bit included) and to each single moby.
//!
//! **The rule** (`0x2f4f60(moby, P, inside)`) with u / d / c = the three bytes, "on" = inside:
//! * update: u > 0 → the moby updates when on (mode bit 2 cleared on, set off); u < 0 the reverse. Leaving the
//!   zone does not touch classes 902, 942 and 1263; entering touches every class.
//! * draw: d > 0 → shown when on (+0x31 = 1, mode bit 1 cleared) and hidden when off (+0x31 = 0, bit 1 set);
//!   d < 0 the reverse. Classes 902, 942, 1263 and 1393 are never touched.
//! * collision: c > 0 → the class's collision (+0x94 = class +0x10) when on (only for a moby with a class), none
//!   when off; c < 0 the reverse (off: the class's collision, unchecked in the game).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::services::{pvar as p, World};

/// The update address in the level05 class table (the reference overlay: [`REFERENCE_LEVEL`]).
pub const UPDATE_FN: u32 = 0x2f5200;
/// The overlay [`UPDATE_FN`] belongs to.
pub const REFERENCE_LEVEL: u32 = 5;
/// Classes that run [`update`].
pub const CLASSES: [i16; 1] = [258];

/// Classes the update rule leaves alone when the zone is left (`0x386`, `0x3ae`, `0x4ef`).
pub const KEEP_UPDATE_ON_EXIT: [i16; 3] = [902, 942, 1263];
/// Classes the draw rule never touches (those three and `0x571`).
pub const KEEP_DRAW: [i16; 4] = [902, 942, 1263, 1393];

/// Pvar offsets.
pub mod pv {
    pub const PATH: usize = 0x00;
    pub const RADIUS: usize = 0x04;
    pub const CUBOIDS: usize = 0x08;
    pub const GROUPS: usize = 0x20;
    pub const MOBYS: usize = 0x50;
    pub const UPDATE_RULE: usize = 0x7c;
    pub const DRAW_RULE: usize = 0x7d;
    pub const USE_CAMERA: usize = 0x7e;
    pub const COLLISION_RULE: usize = 0x7f;
    pub const LEN: usize = 0x80;
}

/// The zone test (module doc).
pub fn inside(w: &World, id: MobyId) -> bool {
    let pv = &w.m(id).pvars;
    let q = if p::u8(pv, pv::USE_CAMERA) != 0 { w.camera_point() } else { w.hero_point() };
    let r = p::ff(pv, pv::RADIUS);
    if 0.0 < r {
        let m = w.m(id).position;
        let d = ((m[0] - q[0]).powi(2) + (m[1] - q[1]).powi(2) + (m[2] - q[2]).powi(2)).sqrt();
        if d < r { return true; }
    }
    let path = p::i32(pv, pv::PATH);
    if path != -1 && w.in_path(q, path) { return true; }
    (0..6).map(|k| p::i32(pv, pv::CUBOIDS + 4 * k)).any(|c| c != -1 && w.in_cuboid(q, c))
}

/// Level05 0x2f5200 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::LEN { return; }
    w.mm(id).update_dist = 0xff;
    let on = inside(w, id);
    let (cmd, state) = (w.m(id).cmd, w.m(id).state);
    if on == (cmd != 0) && state != 0 { return; }
    {
        let m = w.mm(id);
        m.cmd = on as u8;
        m.state = m.cmd + 1;
    }
    let pv = w.m(id).pvars.clone();
    let rules = [p::u8(&pv, pv::UPDATE_RULE) as i8, p::u8(&pv, pv::DRAW_RULE) as i8, p::u8(&pv, pv::COLLISION_RULE) as i8];
    for k in 0..12 {
        let g = p::i32(&pv, pv::GROUPS + 4 * k);
        if g == -1 { continue; }
        let list: Vec<u16> = usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g)).and_then(|l| l.clone()).unwrap_or_default();
        for m in list { apply(w, m as MobyId, rules, on); }
    }
    for k in 0..11 {
        let m = p::i32(&pv, pv::MOBYS + 4 * k);
        if m != -1 {
            if let Ok(m) = usize::try_from(m) { apply(w, m, rules, on); }
        }
    }
}

/// `0x2f4f60(moby, P, inside)`: the three rules on one moby (module doc).
pub fn apply(w: &mut World, id: MobyId, [u, d, c]: [i8; 3], on: bool) {
    let Some(m) = w.table.mobys.get(id) else { return };
    let class = m.o_class;
    let coll = w.classes.info(class).map(|i| i.has_collision && !i.no_header).unwrap_or(false);
    let m = &mut w.table.mobys[id];
    // Update.
    if on || !KEEP_UPDATE_ON_EXIT.contains(&class) {
        let run = if u > 0 { Some(on) } else if u < 0 { Some(!on) } else { None };
        match run {
            Some(true) => m.mode &= !mode::NO_UPDATE,
            Some(false) => m.mode |= mode::NO_UPDATE,
            None => {}
        }
    }
    // Draw.
    if !KEEP_DRAW.contains(&class) {
        let show = if d > 0 { Some(on) } else if d < 0 { Some(!on) } else { None };
        match show {
            Some(true) => {
                m.visible = 1;
                m.mode &= !mode::HIDDEN;
            }
            Some(false) => {
                m.visible = 0;
                m.mode |= mode::HIDDEN;
            }
            None => {}
        }
    }
    // Collision.
    let collide = if c > 0 { Some(on) } else if c < 0 { Some(!on) } else { None };
    match collide {
        // Entering sets it only for a moby with a class; leaving (c < 0) sets it unchecked.
        Some(true) if on && !m.has_class => {}
        Some(true) => m.has_collision = coll,
        Some(false) => m.has_collision = false,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    fn table(classes: &[i16]) -> MobyTable {
        let mobys = classes.iter().map(|&c| Moby { o_class: c, has_class: true, state: 1, ..Moby::default() }).collect();
        MobyTable::new(mobys, 4)
    }

    #[test]
    fn rules_follow_the_zone() {
        let mut t = table(&[100, 902, 1393]);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        // Update on inside, shown inside, collision off inside.
        for id in 0..3 { apply(&mut w, id, [1, 1, -1], false); }
        assert_eq!(w.m(0).mode & (mode::NO_UPDATE | mode::HIDDEN), mode::NO_UPDATE | mode::HIDDEN);
        // 902 keeps its update and draw on exit, 1393 keeps its draw.
        assert_eq!(w.m(1).mode & (mode::NO_UPDATE | mode::HIDDEN), 0);
        assert_eq!(w.m(2).mode & mode::HIDDEN, 0);
        assert_eq!(w.m(2).mode & mode::NO_UPDATE, mode::NO_UPDATE);
        for id in 0..3 { apply(&mut w, id, [1, 1, -1], true); }
        assert_eq!(w.m(0).mode & (mode::NO_UPDATE | mode::HIDDEN), 0);
        assert_eq!((w.m(0).visible, w.m(0).has_collision), (1, false));
        assert_eq!(w.m(1).mode & mode::NO_UPDATE, 0);
    }
}
