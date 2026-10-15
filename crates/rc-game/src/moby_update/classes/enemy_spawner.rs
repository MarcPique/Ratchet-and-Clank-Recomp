//! Class 815, the enemy spawner (`EnemySpawnerUpdate` 0x3021b8 with its re-creation `0x302328`; level01 only: no other
//! overlay has either function, `rc-trace overlay-diff`). Spec `docs/plan/creatures.md` "Spawner 815".
//!
//! Pvars (0x40): +0x00 moby group (−1 none), +0x08 spawns so far. The spawner never creates a moby: its "spawn" takes
//! the first live member of its group (the group's parked amoeboids, waiting hidden in state 0xc) and throws it out
//! of the spawner, away from Ratchet. **On Novalis all six spawners have group −1** (verified from the pvars): they
//! only make the 0 → 1 step; the re-creation runs for a spawner placed with a group (a modded level, or a level copy
//! the census finds later: plug and play through [`update`] and the group services of `scheduler`).
//!
//! | address | what | here |
//! |---|---|---|
//! | 0x3021b8 | state 0 → 1 | [`update`] |
//! | 0x3021b8 | state 1: group ≥ 0, `MobyGroupCount(g, 0xc)` (0x26e008) = 0 (every live member waits in 0xc) and spawns < `gp−0x4d70` (0x161e90) = 5 → spawns + 1, `0x302328` | [`update`] (`scheduler::group_count`) |
//! | 0x302328 | `0x26e150(&m, g, 0, 0)`: the group's first live member (`scheduler::group_first`, `GroupWalk::Alive`) | [`recreate`] |
//! | 0x302328 | +0x30 update distance 0x80, +0x32 draw distance 0x80, +0x38 light word and ambient (u64) = the spawner's, +0x31 = 1, +0x94 = class +0x10 (collision), mode \|= 0x1020, +0x2c scale = class +0x24 (the class scale alone), position and Euler (with w) = the spawner's | [`recreate`] |
//! | 0x302328 | member pvar +0x250 size = 1.0, +0x20 health = 3.0, +0x170 = `(int)819.2` = 819 (the walker record's radius ·1024), +0x224 range = 30.0; `fun_0020def8` (matrix) | [`recreate`] (`World::build_matrix`) |
//! | 0x302328 | no live member: the game writes through a null pointer (low memory, harmless on the EE) and returns 0 | n/a (nothing written; the count still rises) |
//! | 0x3021b8 | member ≠ 0: knockback record K = pvar +0x60: K+0x24 flags = 8, K+0x20 radius = `(int)(size·0.8·1024)`, K+0x50 apex key = 7.0, K+0x28 centre height = size·0.8, K+0x54 landing key = 13.0, K+0x18 / +0x1c speeds = 0 | [`update`] |
//! | 0x3021b8 | `0x271418(add_rot(FastArcTan(hero.x − x, hero.y − y), π), m, K, 5, 8, 0)` (the flight away from Ratchet, sequence 5 over 8 ticks), member state 9 | [`update`] (`creature::knock::start`) |
//!
//! The member then runs its own update (the amoeboid's state 9: the knockback flight and landing, `classes::amoeboid`).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, knock};
use crate::moby_update::scheduler::{group_count, group_first, GroupWalk};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x3021b8;
pub const CLASSES: [i16; 1] = [815];
/// `gp−0x4d70` (0x161e90): the spawn limit.
pub const MAX_SPAWNS: i32 = 5;

/// The member's knockback record (amoeboid pvar +0x60).
const K: usize = 0x60;

/// `EnemySpawnerUpdate` 0x3021b8.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    match w.m(id).state {
        0 => w.mm(id).state = 1,
        1 => {
            let g = c::pi32(w, id, 0);
            if g < 0 || group_count(w, g, 0xc) != 0 || MAX_SPAWNS <= c::pi32(w, id, 8) { return; }
            let n = c::pi32(w, id, 8) + 1;
            c::set_pi32(w, id, 8, n);
            let Some(m) = recreate(w, id, g) else { return };
            if w.m(m).pvars.len() < 0x254 { return; }
            let size = c::pf(w, m, 0x250);
            c::set_pi32(w, m, K + knock::k::FLAGS, 8);
            c::set_pi32(w, m, K + knock::k::RADIUS, (size * 0.8 * 1024.0) as i32);
            c::set_pf(w, m, K + knock::k::KEY_APEX, 7.0);
            c::set_pf(w, m, K + knock::k::ZOFF, size * 0.8);
            c::set_pf(w, m, K + knock::k::KEY_LAND, 13.0);
            c::set_pf(w, m, K + knock::k::SPEED, 0.0);
            c::set_pf(w, m, K + knock::k::UP, 0.0);
            let hero = crate::moby_update::classes::units::hero_pos(w);
            let p = w.m(id).position;
            let angle = c::add_rot(c::atan(hero[0] - p[0], hero[1] - p[1]), std::f32::consts::PI);
            knock::start(w, m, K, angle, 5, 8, 0);
            w.mm(m).state = 9;
        }
        _ => {}
    }
}

/// `0x302328(spawner)`: the first live member of group `g` reset at the spawner (module table); None when the group
/// has no live member.
fn recreate(w: &mut World, id: MobyId, g: i32) -> Option<MobyId> {
    let m = group_first(w, g, GroupWalk::Alive)?;
    let (light, ambient, pos, rot) = { let s = w.m(id); (s.light, s.ambient, s.position, s.rotation) };
    let oc = w.m(m).o_class;
    let (coll, scale) = (w.classes.info(oc).is_some_and(|i| i.has_collision), crate::moby_update::services::fl(w.class_scale(oc)));
    let mm = w.mm(m);
    mm.update_dist = 0x80;
    mm.draw_dist = 0x80;
    mm.light = light;
    mm.ambient = ambient;
    mm.visible = 1;
    mm.has_collision = coll;
    mm.mode |= mode::TARGETABLE | 0x20;
    mm.scale = scale;
    mm.position = pos;
    mm.rotation = rot;
    if w.m(m).pvars.len() >= 0x254 {
        c::set_pf(w, m, 0x250, 1.0);
        c::set_pf(w, m, 0x20, 3.0);
        c::set_pi32(w, m, 0x170, 819.2f32 as i32);
        c::set_pf(w, m, 0x224, 30.0);
    }
    w.build_matrix(m);
    Some(m)
}
