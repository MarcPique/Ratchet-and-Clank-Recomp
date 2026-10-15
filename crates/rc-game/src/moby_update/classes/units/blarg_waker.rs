//! Blarg's creature wakers, class 1066 (level 06, 6 placed; census U225): level06 0x2fda30. A moby that waits for
//! Ratchet to enter its cuboid, plays its opening animation, then wakes the dormant members of its moby group one
//! every 20 ticks: each member parked in state 0xf (class 827's dormant state) is put at the waker's position and
//! thrown out of it in a random direction (`0x2e9d10`, the 827 family's wake; the knockback flight `0x271418`). When
//! no member is left asleep it stops for good. Read from the level06 decomp and disassembly. Native `f32`.
//!
//! **Not a spawner system**: the waker creates nothing; the wake is a function of the creature family it wakes (the
//! census cluster 99346c2b3975, level 06 only), ported here with its one caller ([`wake`]). The group is walked
//! through the engine's group list (`scheduler::group_ids`), the throw is the shared knockback start
//! (`creature::knock::start`). The woken 827 then runs its own update (827 is not ported: G-CLS-001, U211).
//!
//! **Pvar block**: +0x00 s32 the moby group (−1 none), +0x04 s32 the cuboid (−1 none), +0x08 s32 the wake timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | cuboid < 0 → nothing; Ratchet's position (0x13f3d0) in the cuboid (`0x26e770` = L01 0x274820) → state 1, and sequence B ≠ 1 → `MobyAnimBlend(m, 1, 0, 5)` (0x266540; 5 ticks, not `ticks(5)`) | [`update`] (`triggers::point_in_cuboid`, `World::anim_blend`) |
//! | state 1 | animation flags +0x70 bit 2 (the sequence wrapped) clear → nothing; else state = group ≥ 0 ? 2 : 3, and sequence B ≠ 2 → blend (2, 0, 5) | [`update`] |
//! | state 2 | `FastDecTimer(&+0x08)` (0x217000) 0 (running) → nothing; else the group list `0x1abfc0[group]` walked in order (every entry, dead ones too, until the end bit): the first member [`wake`] accepts stops the walk; none → state 3; else timer = `ticks(20)` (0x216fb8) | [`update`] (`scheduler::group_ids`, `gold_bolt::fast_dec_timer`) |
//! | state 3 / others | nothing | [`update`] |
//! | 0x2e9d10 | member state ≠ 0xf → refused (0) | [`wake`] |
//! | | position +0x10 (with w, `lq`/`sq`) = the waker's; state 6; +0x94 = class +0x10 (collision); mode `& 0xffbe \| 0x1000` (shown, animated, targetable) | [`wake`] |
//! | | Euler +0x40 = 0 (0x2172f8), z = `rand_angle()` (0x266970); rows +0xc0..+0xef from the Euler (0x217af0 = L01 0x221960: three rows, +0xf0 kept) | [`wake`] (`services::euler_rows`) |
//! | | knockback record K = pvar +0x120: +0x14 drag = 0, +0x24 flags = 5, +0x3d = 3, +0x1c up = 2·dt (0x15ed6c), +0x10 gravity = 20·dt² (0x15ed70), +0x18 speed = 2·dt | [`wake`] |
//! | | `0x26b368(rand_angle(), m, K, 2, 1, 0)` (= L01 0x271418: the flight start, sequence 2 over 1 tick) | [`wake`] (`knock::start`) |
//! | | K+0x08 (the velocity's z) negated (the flight starts downward); accepted (1) | [`wake`] |
//! | | no sound, particle, hit, flag, bolt; no other moby written | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::gold_bolt::fast_dec_timer;
use crate::moby_update::creature::{self as c, knock, DT, DT2};
use crate::moby_update::services::{euler_rows, pv, World};

/// The update in the level06 class table.
pub const UPDATE_FN: u32 = 0x2f_da30;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 1] = [1066];
/// The state the woken family parks in (827's dormant state).
pub const DORMANT: u8 = 0xf;
/// The woken member's knockback record (827 pvar +0x120).
const K: usize = 0x120;

/// Level06 0x2fda30 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    match w.m(id).state {
        0 => {
            let cub = c::pi32(w, id, 4);
            if cub < 0 { return; }
            let h = crate::moby_update::classes::units::hero_pos(w);
            if !crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [h[0], h[1], h[2]], cub) { return; }
            w.mm(id).state = 1;
            if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 5); }
        }
        1 => {
            if w.m(id).anim.flags & 2 == 0 { return; }
            w.mm(id).state = if c::pi32(w, id, 0) < 0 { 3 } else { 2 };
            if w.m(id).anim.seq_b != 2 { w.anim_blend(id, 2, 0, 5); }
        }
        2 => {
            let mut t = c::pi32(w, id, 8);
            let r = fast_dec_timer(&mut t);
            c::set_pi32(w, id, 8, t);
            if r == 0 { return; }
            let g = c::pi32(w, id, 0);
            let list = i8::try_from(g).map(|g| crate::moby_update::scheduler::group_ids(w, g)).unwrap_or_default();
            let at = w.m(id).position;
            let woke = list.into_iter().any(|m| m < w.table.mobys.len() && wake(w, m, at));
            if !woke {
                w.mm(id).state = 3;
                return;
            }
            let t = w.ticks(20);
            c::set_pi32(w, id, 8, t);
        }
        _ => {}
    }
}

/// Level06 `0x2e9d10(member, &pos)`: the 827 family's wake (module table). False when the member is not dormant.
pub fn wake(w: &mut World, id: MobyId, at: [f32; 4]) -> bool {
    if w.m(id).state != DORMANT { return false; }
    let coll = crate::moby_update::classes::units::class_collision(w, w.m(id).o_class);
    {
        let m = w.mm(id);
        m.position = at;
        m.state = 6;
        m.has_collision = coll;
        m.mode = (m.mode & 0xffbe) | mode::TARGETABLE;
        m.rotation = [0.0; 4];
    }
    let z = w.rng.rand_angle();
    w.mm(id).rotation[2] = z;
    let r = euler_rows(pv(w.m(id).rotation));
    {
        let m = w.mm(id);
        for (i, row) in r.iter().enumerate().take(3) { m.rows[i] = row.map(|x| x.to_f32()); }
    }
    if w.m(id).pvars.len() < K + 0x60 { return true; }
    c::set_pi32(w, id, K + knock::k::DRAG, 0);
    c::set_pi32(w, id, K + knock::k::FLAGS, 5);
    c::set_pu8(w, id, K + 0x3d, 3);
    c::set_pf(w, id, K + knock::k::UP, DT + DT);
    c::set_pf(w, id, K + knock::k::GRAVITY, DT2 * 20.0);
    c::set_pf(w, id, K + knock::k::SPEED, DT + DT);
    let a = w.rng.rand_angle();
    knock::start(w, id, K, a, 2, 1, 0);
    let vz = c::pf(w, id, K + 8);
    c::set_pf(w, id, K + 8, -vz);
    true
}
