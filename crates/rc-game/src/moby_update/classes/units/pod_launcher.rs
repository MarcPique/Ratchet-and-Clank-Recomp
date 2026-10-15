//! The pod launchers, class 1885 (level 09: 4 created, level 13: 4; census U319), the pods 1886 they lob and the
//! hatch that brings a member of the launcher's moby group back to life where a pod lands. Level09 0x30ab80 (the
//! launcher), its hit handler 0x30a9e8, the pod count 0x30aae0, the pod spawn 0x30b218, the pod 0x30b3b0, its delete
//! 0x30b350 and the hatch 0x30a778; level 13's copies (0x30d550, 0x30d3b8, 0x30dd80, 0x30d148) are the same code
//! (masked compare of the whole functions), so `LevelPorts` runs these rows there too. Read from the level09 decomp
//! and disassembly. Native `f32`; the `rand` draws in the game's order.
//!
//! The launcher has health and flashes when hit; at its init it deletes every live member of its group (the creatures
//! it will hatch); then every `+0x8c` ticks, with a target (Ratchet or a decoy found by the target search, re-searched
//! one tick in seven) and fewer than `+0x90` pods and hatchlings alive, it lobs a pod from its joint 0 toward a point
//! in one of its four target cuboids (random, or the one nearest Ratchet) or near itself. A pod grows to full size in
//! flight, bounces off the world and other mobys (a knock sound at most every 12 ticks), keeps 1.8 apart from its
//! siblings, and once it rests (or after 500 ticks) shrinks away while the hatchling (a dead member of the group,
//! revived at the pod) grows in. Killed, the launcher explodes (a beam explosion with its class sound 1) and breaks
//! into the pieces 1733–1735; its bolts come from `SetDeathBits`.
//!
//! **System or not**: the hatch is this family's own (no other class calls 0x30a778; levels 09 and 13 only), not the
//! engine's re-creation `0x302328` (`classes::enemy_spawner`, which throws a *live* member). Shared calls used: the
//! hit resolver and flash (`creature::damage`, `flash`), `SetDeathBits` (`crate_::set_death_bits`), the target search
//! with its cuboid list (`target::acquire_with`, its first consumer), the group walk (`scheduler::group_first`,
//! `group_count`), the lob (`knock::lob_up`), `BreakFxB` / `SpawnBeamExplosion` (`creature::fx`), `CollLine_Fix` /
//! the sphere kernel, `reflect` 0x221570, Ratchet's light, `MobyBuildMatrix`.
//!
//! **Launcher pvars** (0x150, mode 0x20: +0x00 / +0x0c the header's damage / flash pointers): +0x20 the damage record
//! (health +0x20), +0x60 the flash record (+0x67 the red), +0x80 f32 the target range, +0x84 s32 a cuboid (−1 none), +0x88 s32 a path (the
//! target region, −1 none), +0x8c s16 the fire interval, +0x8e s16 the fire timer, +0x90 u8 the most pods and
//! hatchlings, +0x91 u8 their count, +0x92 u8 the aim (0 a random cuboid, else the one nearest Ratchet), +0x93 u8
//! grow (the hatchling grows in from a tenth), +0x94 s32 the group, +0xa0 s32[4] the target cuboids, +0xb0 [20] the
//! pods (pointers in the game; the port keeps moby index + 1, 0 none), +0x100 the target record (0x274df8's out:
//! +0x100 position, +0x110 Euler, +0x120 aim, +0x130 body point, +0x140 the target moby (index + 1), +0x144 the kind).
//! **Pod pvars** (0x80): +0x00 the velocity, +0x10 the launcher (index + 1), +0x14 the hatchling (index + 1), +0x18
//! s16 the life timer, +0x1a u8 grow, +0x1b u8 the knock-sound cooldown, +0x1c u8 its slot in the launcher's list.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x30ab80 | every state: the hit handler 0x30a9e8 | [`update`] ([`hit`]) |
//! | 0x30a9e8 | `MobyGetHitMessage(m, 0xc30000, 0)` (0x279f18 = L01 0x26f320); `0x279f70(m, hit, +0x20, 0, &out, 0, 0, 4)` (= L01 0x26f378) | [`hit`] (`damage::resolve`) |
//! | | out ≥ 2: health −= the hit's damage (record +0x2c); > 0 → red +0x67 = 90, the flash start (0x27cf10 = L01 0x272318); else `SetDeathBits(m, 0, −1)` (0x276df8 = L01 0x26c250), state 2, red 0x78, the flash start | [`hit`] (`flash::start`, `crate_::set_death_bits`) |
//! | | +0xa4 = 0xff; the flash update (0x27cff0 = L01 0x2723f8) | [`hit`] (`flash::update`) |
//! | state 0 | while `0x278d68(&m, group, 0, 0)` (= L01 0x26e150, the first live member) finds one: `DeleteMoby(m)`; → 1 | [`update`] (`scheduler::group_first`) |
//! | state 1 | the hit handler again (a second flash update this tick; the hit was taken by the first) | [`update`] |
//! | | `FastDecTimer(&+0x8e)` (0x21ec48 = L01 0x220ea8) running → nothing | [`update`] |
//! | | `randi(7)` (0x2774d8 = L01 0x26c930) = 0 → the target search `0x27f9f0(+0x80, m, &+0x100, +0x84 ≠ −1 ? (&+0x84, 1) : (0, 0), +0x88 ≠ −1 ? (path points, count) : (0, 0))` (= L01 0x274df8) | [`update`] (`target::acquire_with`) |
//! | | else the target moby +0x140 null or deleted (0xfe / 0xfd) → +0x140 = 0, kind +0x144 = 2; else +0x100 = its position (four words) | [`update`] |
//! | | kind 2 → nothing | [`update`] |
//! | 0x30aae0 | +0x91 = `MobyGroupCount(group, −1)` (0x278c78 = L01 0x26e008); each of the 20 pods alive (class 0x75e, not 0xfe / 0xfd) +1 (byte); the slot = the last free one, −1 none | [`count`] |
//! | state 1 | +0x90 ≤ +0x91 or no free slot → nothing | [`update`] |
//! | | aim 0: k = `rand_range(0, 3)` (0x277518 = L01 0x26c970); the first cuboid ≠ −1 of +0xa0[(j + k) mod 4], j = 0..3; none → p = a random point within max(+0x80, 3) (0x277758: `rand_angle`, radius `(1 − u²)·r`, u = `((rand() >> 16) & 0xfff)·2⁻¹²`, w 0) + position | [`update`] ([`disc_point`]) |
//! | | aim ≠ 0: of +0xa0[0..3] (≠ −1) the cuboid whose centre (+0x30) is nearest Ratchet (0x13f3d0; `VecDistance` < 99840); none → p = (target − position)·0.5, `0x2214a8(3, p)` (clamped to 3) when +0x80 > 3, set to length 3 when shorter than 3, + position | [`update`] |
//! | | a cuboid: p = its centre (four words) + row 0 · `randf(−1, 1)` + row 1 · `randf(−1, 1)` (0x277570 = L01 0x26c9c8) | [`update`] |
//! | | `GroundHeight(0.5, p, 0)` (0x279230 = L01 0x26e618) ≠ 0 → p.z = it | [`update`] |
//! | | v = p − position (w = p's); s = \|v\| · dt · 0.6; v = unit(v) · s | [`update`] |
//! | | start = joint 0's point (0x26f150 = L01 0x2645a8) + unit(v)·0.05 jittered by ±0.025 per axis (0x27f398 = L01 0x2747a0) | [`update`] (`World::joint_point`, `fx::jitter`) |
//! | | v.z = `0x27a6e8(s, −(dt²·9.8), start, p, NULL)` (= L01 0x26faf0, the lob's vertical speed) | [`update`] (`knock::lob_up`) |
//! | | pod = `0x30b218(m, start, v, +0x93, slot)`; +0xb0[slot] = pod (0 when the table is full); timer = `ticks(+0x8c)`; +0x91 += 1 | [`update`] ([`spawn_pod`]) |
//! | state 2 | `SpawnBeamExplosion(0, 0, 8, 4, 18, 1, 15, m, &+0x70, NULL, 30, 8, 20, sound 1, shake, 1 debris, −1, 0)` (0x27df08 = L01 0x273310; its `param_9` (&+0x70) is never read, the NULL centre means the moby's position) | [`update`] (`fx::beam_explosion`) |
//! | | `BreakFxB(0, m, 1733 / 1734 / 1735, position, rotation, 0, 0, zero, zero, zero)` (0x282e18 = L01 0x278ad8; 0x15f580 is the zero vector) | [`update`] (`fx::break_piece`) |
//! | | `DeleteMoby(m)` | [`update`] |
//! | 0x30b218 | `CreateMoby(0x75e)`; none → nothing | [`spawn_pod`] |
//! | | +0x10 = the launcher; position = start, velocity = v (four words each); Euler x, y = `randf(−π, π)` each | [`spawn_pod`] |
//! | | +0x14 = 0, +0x18 = `ticks(500)`; +0x30 = 0xff, +0x32 = 0x40, +0x31 = 1, state 0; scale = class scale · 0.3; +0x1c = slot, +0x1a = grow, +0x1b = 0; `MobyBuildMatrix` | [`spawn_pod`] |
//! | 0x30b3b0 | `FastDecTimer` (byte) on +0x1b (0x21ec78 = L01 0x220ed8); old = position | [`pod_update`] |
//! | pod state 0 | scale += (class scale − scale)·speed·0.05 (0x15ed60); position += velocity; end = position + unit(velocity)·0.2 | [`pod_update`] |
//! | | +0x94 = 0 for the pod and its launcher; `CollLine_Fix(old, end, 4, m, 0)` (0x20efd8 = L01 0x211870) | [`pod_update`] (`World::coll_line`) |
//! | | a hit: position += unit(point − end)·0.215; no moby → velocity reflected off the normal (0x21f340 = L01 0x221570), and with the slope `FastArcTan(n.z, \|n.xy\|)` < 0.6981317 (40°) and no surface id (`CollType` 0x212940 = L01 0x2151d8, −1): velocity ·= 0.5, \|velocity\| < 0.025 → state 1 | [`bounce`] |
//! | | a pod (class 1886): velocity.z ·= `randf(0.9, 1.1)`; another moby: reflected, and with the slope < 40° and no surface id ·= 1.2 | [`bounce`] |
//! | | the cooldown +0x1b = 0 → `PlayClassSound(0, 0, m)` (0x2aa4a8), +0x1b = 12 | [`bounce`] |
//! | | the sphere kernel (0x2100c8 = L01 0x212960: r 0.2 at the position, flags 4, ignore m); a hit: position = the pushed centre + unit(point − pushed centre)·0.015 (w kept [L]), then as the line hit | [`pod_update`] (`World::coll_sphere`), [`bounce`] |
//! | | each other live pod of the launcher (class 1886): d = position − its position, z = 0; \|d\| < 1.8 → position += d·(1.8 − \|d\|)·(1 − 0.5·speed) | [`pod_update`] |
//! | | `FastDecTimer(&+0x18)` out → state 2 | [`pod_update`] |
//! | | Euler x / y = `0x27ddc8(velocity.x / .y)` (= L01 0x2731d0, the wrap into [−π, π)); +0x94 = the class collision for the pod and its launcher; velocity.z −= dt²·9.8; Euler x += speed·0.01, y += speed·0.02 | [`pod_update`] (`flyer::wrap_frac`) |
//! | pod state 1 | z = `GroundHeight(0.5, position, 0)` + 0.2, w = 0; the hatchling = `0x30a778(launcher, position)`; → 2 | [`pod_update`] ([`hatch`]) |
//! | pod state 2 | scale −= scale·speed·0.05; with a hatchling and grow: its scale += (its class scale − its scale)·speed·0.05; scale < class scale · speed · 0.05 → scale = 0.0001, and no hatchling, it deleted, no grow, or its scale ≥ its class scale − 0.01 → the pod's delete 0x30b350 | [`pod_update`] |
//! | every pod state | the blob shadow `0x279ae0(scale·0.3 / class scale, m)` (= L01 0x26eec8) | [`update`] (`crate::shadows::blob`) |
//! | | z < 5 or > 500 → the delete 0x30b350; else x, y, z clamped into [5, 1018] | [`pod_update`] |
//! | 0x30b350 | the launcher alive and class 0x75d → its +0xb0[slot] = 0; `DeleteMoby(pod)` | [`delete_pod`] |
//! | 0x30a778 | no launcher → none; `0x278d68(&m, group, 1, 1)` (the first *dead* member) none → `printf` (n/a), none | [`hatch`] |
//! | | launcher +0x91 += 1; m: position = p (w 0), state 0, +0xbc = 0, mode = class +0x44, scale = class scale, +0x30 = 0xff, +0x32 = 0xff, +0x31 = 1, mode \|= 0x10 with a class glow (+0x40), \|= 0x400 with class +0x0f, +0x36 = 0x7f80, +0x71 = +0x72 = +0xa4 = 0xff; grow → scale = class scale · 0.1 | [`hatch`] |
//! | | the hard cut to sequence 0 frame 0 (0x212ed8 = L01 0x26c5a8) | [`hatch`] (`creature::hard_cut`) |
//! | | the suck record (0x2fcd40 = L01 0x304100): +0x6c = 0, +0x94 = 0, +0x68 (s16) = 0 | [`hatch`] (`react::record`) |
//! | | bolts: m.+0xb4 = min(its, the launcher's), at least 1; the launcher's −= it (u16), 1 when that is negative | [`hatch`] |
//! | | +0x94 = the class collision; Ratchet's light (0x27cc70 = L01 0x272078); `MobyBuildMatrix`; the damage record (0x27bdf0 = L01 0x2711f8): +0x30 = +0x34 = 0, health = (f32) its s16 +0x04 | [`hatch`] (`units::take_hero_light`, `triggers::pvar_record`) |

#![allow(clippy::needless_range_loop)] // the game's per-lane VU writes, spelled out.

use crate::collision_query::CollOutput;
use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::flyer::wrap_frac;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, target, DT, DT2, SPEED};
use crate::moby_update::scheduler::{group_count, group_first, GroupWalk};
use crate::moby_update::services::{pf, pv, reflect, World};
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 9;
/// The launcher's update and class (0x75d).
pub const UPDATE_FN: u32 = 0x30_ab80;
pub const LAUNCHER: i16 = 1885;
pub const CLASSES: [i16; 1] = [LAUNCHER];
/// The pod's update and class (0x75e).
pub const POD_FN: u32 = 0x30_b3b0;
pub const POD: i16 = 1886;
pub const POD_CLASSES: [i16; 1] = [POD];
/// The pieces the launcher breaks into.
pub const PIECES: [i16; 3] = [1733, 1734, 1735];
pub const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 8.0, flash2: 4.0, flash_dist: 18.0, scale: 1.0, light: 15.0, streaks: 30, sparks: 8, puffs: 20, debris: 1, sound: 1, shake: true };

/// The launcher's pvar offsets (module doc).
pub mod lv {
    pub const DAMAGE: usize = 0x20;
    pub const FLASH: usize = 0x60;
    pub const RANGE: usize = 0x80;
    pub const CUBOID: usize = 0x84;
    pub const PATH: usize = 0x88;
    pub const INTERVAL: usize = 0x8c;
    pub const TIMER: usize = 0x8e;
    pub const MAX: usize = 0x90;
    pub const COUNT: usize = 0x91;
    pub const AIM: usize = 0x92;
    pub const GROW: usize = 0x93;
    pub const GROUP: usize = 0x94;
    pub const CUBOIDS: usize = 0xa0;
    pub const PODS: usize = 0xb0;
    pub const TARGET: usize = 0x100;
    pub const SIZE: usize = 0x148;
}

/// The pod's pvar offsets (module doc).
pub mod pd {
    pub const LAUNCHER: usize = 0x10;
    pub const HATCH: usize = 0x14;
    pub const TIMER: usize = 0x18;
    pub const GROW: usize = 0x1a;
    pub const COOL: usize = 0x1b;
    pub const SLOT: usize = 0x1c;
}

/// The pods a launcher keeps.
const SLOTS: usize = 20;

fn gone(w: &World, id: MobyId) -> bool { matches!(w.m(id).state, 0xfe | 0xfd) }

/// A moby pointer field (index + 1, 0 none).
fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    usize::try_from(v - 1).ok().filter(|&m| v > 0 && m < w.table.mobys.len())
}
fn set_ref(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }

fn class_scale(w: &World, id: MobyId) -> f32 { crate::moby_update::classes::units::class_scale(w, w.m(id).o_class) }
fn class_coll(w: &World, id: MobyId) -> bool { crate::moby_update::classes::units::class_collision(w, w.m(id).o_class) }

/// Level09 0x30a9e8, the launcher's hit handler (module table).
pub fn hit(w: &mut World, id: MobyId) {
    let h = w.get_hit(id, 0xc3_0000, false);
    let r = damage::resolve(w, id, h, lv::DAMAGE, 0, 4);
    if r.out5 >= 2 {
        let dmg = r.hit.or(h).map_or(0.0, |h| h.damage.to_f32());
        let hp = c::pf(w, id, lv::DAMAGE) - dmg;
        c::set_pf(w, id, lv::DAMAGE, hp);
        if 0.0 < hp {
            c::set_pu8(w, id, lv::FLASH + 7, 90);
        } else {
            set_death_bits(w, id, 0, -1);
            w.mm(id).state = 2;
            c::set_pu8(w, id, lv::FLASH + 7, 0x78);
        }
        flash::start(w, id, lv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, lv::FLASH);
}

/// Level09 0x30aae0: +0x91 = the group's live members + the live pods; the last free pod slot.
pub fn count(w: &mut World, id: MobyId) -> Option<usize> {
    let g = c::pi32(w, id, lv::GROUP);
    let mut n = group_count(w, g, -1) as u8;
    let mut slot = None;
    for i in 0..SLOTS {
        let alive = moby_ref(w, id, lv::PODS + 4 * i).is_some_and(|p| w.m(p).o_class == POD && !gone(w, p));
        if alive { n = n.wrapping_add(1); } else { slot = Some(i); }
    }
    c::set_pu8(w, id, lv::COUNT, n);
    slot
}

/// Level09 0x277758(r, out): a random point in the disc of radius `r` about the origin (w 0).
pub fn disc_point(w: &mut World, r: f32) -> [f32; 4] {
    let a = w.rng.rand_angle();
    let u = ((w.rng.rand() >> 16) & 0xfff) as f32 * (1.0 / 4096.0);
    let d = (1.0 - u * u) * r;
    [a.cos() * d, a.sin() * d, 0.0, 0.0]
}

fn write_target(w: &mut World, id: MobyId, t: &target::Target) {
    let o = lv::TARGET;
    c::set_pv4(w, id, o, t.pos);
    c::set_pv4(w, id, o + 0x10, t.rot);
    c::set_pv4(w, id, o + 0x20, t.aim);
    c::set_pv4(w, id, o + 0x30, t.body);
    set_ref(w, id, o + 0x40, t.moby);
    c::set_pi32(w, id, o + 0x44, t.kind as i32);
}

/// Level09 0x30ab80, the launcher (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < lv::SIZE { return; }
    hit(w, id);
    match w.m(id).state {
        0 => {
            let g = c::pi32(w, id, lv::GROUP);
            while let Some(m) = group_first(w, g, GroupWalk::Alive) { w.delete_moby(m); }
            w.mm(id).state = 1;
        }
        1 => fire(w, id),
        2 => {
            let at = w.m(id).position;
            fx::beam_explosion(w, &BLAST, Some(id), at);
            let (pos, rot) = (w.m(id).position, w.m(id).rotation);
            for cl in PIECES { fx::break_piece(w, id, cl, pos, rot, 0, 0); }
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// State 1 of 0x30ab80: the target, the aim and the lob (module table).
fn fire(w: &mut World, id: MobyId) {
    hit(w, id);
    let mut t = c::pi16(w, id, lv::TIMER);
    let r = crate::moby_update::services::fast_dec_timer_s16(&mut t);
    c::set_pi16(w, id, lv::TIMER, t);
    if r == 0 { return; }
    if w.rng.randi(7) == 0 {
        let cub = c::pi32(w, id, lv::CUBOID);
        let path = c::pi32(w, id, lv::PATH);
        let list: Vec<i32> = if cub != -1 { vec![cub] } else { Vec::new() };
        // The points go in only with the path (`0 < count` tested by the search: an empty path tests nothing).
        let region = usize::try_from(path).ok().filter(|&i| w.svc.splines.get(i).is_some_and(|s| !s.is_empty()));
        let tg = target::acquire_with(w, id, c::pf(w, id, lv::RANGE), &list, region);
        write_target(w, id, &tg);
    } else {
        match moby_ref(w, id, lv::TARGET + 0x40).filter(|&m| !gone(w, m)) {
            None => {
                c::set_pi32(w, id, lv::TARGET + 0x40, 0);
                c::set_pi32(w, id, lv::TARGET + 0x44, 2);
            }
            Some(m) => {
                let p = w.m(m).position;
                c::set_pv4(w, id, lv::TARGET, p);
            }
        }
    }
    if c::pi32(w, id, lv::TARGET + 0x44) == 2 { return; }
    let slot = count(w, id);
    if c::pu8(w, id, lv::MAX) <= c::pu8(w, id, lv::COUNT) { return; }
    let Some(slot) = slot else { return };
    let pos = w.m(id).position;
    let cuboids: [i32; 4] = std::array::from_fn(|i| c::pi32(w, id, lv::CUBOIDS + 4 * i));
    let shape = |w: &World, c: i32| usize::try_from(c).ok().and_then(|c| w.svc.volumes.cuboids.get(c).copied());
    let mut cub = None;
    let mut p;
    if c::pu8(w, id, lv::AIM) == 0 {
        let k = w.rng.rand_range(0, 3);
        for j in 0..4 {
            let i = (j + k).rem_euclid(4) as usize;
            if cuboids[i] != -1 {
                cub = Some(cuboids[i]);
                break;
            }
        }
        if cub.is_none() {
            let r = c::pf(w, id, lv::RANGE);
            let r = if r < 3.0 { 3.0 } else { r };
            let d = disc_point(w, r);
            p = [d[0] + pos[0], d[1] + pos[1], d[2] + pos[2], d[3]];
        } else {
            p = [0.0; 4];
        }
    } else {
        let h = crate::moby_update::classes::units::hero_pos(w);
        let mut best = 99840.0;
        for &cb in &cuboids {
            if cb == -1 { continue; }
            let Some(s) = shape(w, cb) else { continue };
            let d = c::dist3(h, s.matrix[3]);
            if d < best {
                best = d;
                cub = Some(cb);
            }
        }
        if cub.is_none() {
            let tp = c::pv4(w, id, lv::TARGET);
            let mut v = c::scale(c::sub(tp, pos), 0.5);
            if 3.0 < c::pf(w, id, lv::RANGE) { v = c::clamp_len3(v, 3.0); }
            if c::len3(v) < 3.0 { v = c::set_len3(v, 3.0); }
            p = [v[0] + pos[0], v[1] + pos[1], v[2] + pos[2], v[3]];
        } else {
            p = [0.0; 4];
        }
    }
    if let Some(s) = cub.and_then(|cb| shape(w, cb)) {
        p = s.matrix[3];
        let a = w.rng.randf(-1.0, 1.0);
        for k in 0..3 { p[k] += s.matrix[0][k] * a; }
        let b = w.rng.randf(-1.0, 1.0);
        for k in 0..3 { p[k] += s.matrix[1][k] * b; }
    }
    let gz = w.ground_height(pf(0.5), pv(p), 0).to_f32();
    if gz != 0.0 { p[2] = gz; }
    let mut v = c::sub(p, pos);
    v[3] = p[3];
    let s = c::len3(v) * (DT * 0.6);
    v = c::set_len3(v, s);
    let mut start = w.joint_point(id, 0);
    let mut off = c::set_len3(v, 0.05);
    fx::jitter(w, 0.025, &mut off);
    for k in 0..3 { start[k] += off[k]; }
    let mut time = 0.0;
    v[2] = knock::lob_up(s, -(DT2 * 9.8), start, p, &mut time);
    let grow = c::pu8(w, id, lv::GROW);
    let pod = spawn_pod(w, id, start, v, grow, slot as u8);
    set_ref(w, id, lv::PODS + 4 * slot, pod);
    let n = c::pi16(w, id, lv::INTERVAL) as i32;
    let t = w.ticks(n);
    c::set_pi16(w, id, lv::TIMER, t as i16);
    let cnt = c::pu8(w, id, lv::COUNT).wrapping_add(1);
    c::set_pu8(w, id, lv::COUNT, cnt);
}

/// Level09 0x30b218(launcher, start, velocity, grow, slot): a pod (module table). None when the table is full.
pub fn spawn_pod(w: &mut World, launcher: MobyId, start: [f32; 4], vel: [f32; 4], grow: u8, slot: u8) -> Option<MobyId> {
    let e = w.create_moby(POD)?;
    if w.m(e).pvars.len() < 0x20 { return Some(e); }
    set_ref(w, e, pd::LAUNCHER, Some(launcher));
    w.mm(e).position = start;
    c::set_pv4(w, e, 0, vel);
    let rx = w.rng.randf(-PI, PI);
    w.mm(e).rotation[0] = rx;
    let ry = w.rng.randf(-PI, PI);
    w.mm(e).rotation[1] = ry;
    c::set_pi32(w, e, pd::HATCH, 0);
    let t = w.ticks(500);
    c::set_pi16(w, e, pd::TIMER, t as i16);
    let cs = class_scale(w, e);
    {
        let m = w.mm(e);
        m.update_dist = 0xff;
        m.draw_dist = 0x40;
        m.visible = 1;
        m.state = 0;
        m.scale = cs * 0.3;
    }
    c::set_pu8(w, e, pd::SLOT, slot);
    c::set_pu8(w, e, pd::GROW, grow);
    c::set_pu8(w, e, pd::COOL, 0);
    w.build_matrix(e);
    Some(e)
}

/// The bounce of 0x30b3b0 after a line or sphere hit `o` (module table: the velocity, the rest test, the sound).
fn bounce(w: &mut World, id: MobyId, o: &CollOutput) {
    bounce_velocity(w, id, o, POD);
    if c::pu8(w, id, pd::COOL) == 0 {
        w.play_sound(0, 0, id);
        c::set_pu8(w, id, pd::COOL, 12);
    }
}

/// The velocity part of a pod's bounce (velocity at pvar +0; also Kerwan's pods, [`super::pod_spawner`]): off a pod
/// of class `pod` the vertical speed ·= `randf(0.9, 1.1)`; else reflected off the normal, and with the slope under
/// 40° and no surface id: off the world ·0.5 (under 0.025: state 1, at rest), off another moby ·1.2.
pub fn bounce_velocity(w: &mut World, id: MobyId, o: &CollOutput, pod: i16) {
    let n = [o.normal[0], o.normal[1], o.normal[2], 0.0];
    let slope = || c::atan(n[2], c::len2(n));
    match o.moby {
        Some(m) if w.m(m).o_class == pod => {
            let f = w.rng.randf(0.9, 1.1);
            let vz = c::pf(w, id, 8) * f;
            c::set_pf(w, id, 8, vz);
        }
        hm => {
            let vel = c::pv4(w, id, 0);
            let r = reflect(pv(vel), pv(n)).map(|x| x.to_f32());
            c::set_pv4(w, id, 0, r);
            if slope() < 0.698_131_7 && o.surface_id() == -1 {
                if hm.is_none() {
                    let v = c::scale(r, 0.5);
                    c::set_pv4(w, id, 0, v);
                    if c::len3(v) < 0.025 { w.mm(id).state = 1; }
                } else {
                    c::set_pv4(w, id, 0, c::scale(r, 1.2));
                }
            }
        }
    }
}

/// Level09 0x30b350: the pod leaves its launcher's list and is deleted.
pub fn delete_pod(w: &mut World, id: MobyId) {
    if let Some(l) = moby_ref(w, id, pd::LAUNCHER).filter(|&l| !gone(w, l) && w.m(l).o_class == LAUNCHER) {
        let slot = c::pu8(w, id, pd::SLOT) as usize;
        if w.m(l).pvars.len() >= lv::PODS + 4 * slot + 4 { c::set_pi32(w, l, lv::PODS + 4 * slot, 0); }
    }
    w.delete_moby(id);
}

/// Level09 0x30b3b0, the pod (module doc).
pub fn pod_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let mut cool = c::pu8(w, id, pd::COOL);
    crate::moby_update::services::fast_dec_timer_u8(&mut cool);
    c::set_pu8(w, id, pd::COOL, cool);
    let old = w.m(id).position;
    match w.m(id).state {
        0 => fly(w, id, old),
        1 => {
            let gz = w.ground_height(pf(0.5), pv(w.m(id).position), 0).to_f32();
            {
                let m = w.mm(id);
                m.position[2] = gz + 0.2;
                m.position[3] = 0.0;
            }
            let at = w.m(id).position;
            let h = moby_ref(w, id, pd::LAUNCHER).and_then(|l| hatch(w, l, at));
            set_ref(w, id, pd::HATCH, h);
            w.mm(id).state = 2;
        }
        2 => {
            let f = SPEED * 0.05;
            let s = w.m(id).scale;
            w.mm(id).scale = s - s * f;
            let h = moby_ref(w, id, pd::HATCH);
            let grow = c::pu8(w, id, pd::GROW) != 0;
            if let Some(h) = h.filter(|_| grow) {
                let (hs, hc) = (w.m(h).scale, class_scale(w, h));
                w.mm(h).scale = hs + (hc - hs) * f;
            }
            let cs = class_scale(w, id);
            if w.m(id).scale < cs * (SPEED * 0.05) {
                w.mm(id).scale = 0.0001;
                let done = match h {
                    None => true,
                    Some(h) => gone(w, h) || !grow || class_scale(w, h) - 0.01 <= w.m(h).scale,
                };
                if done { delete_pod(w, id); }
            }
        }
        _ => {}
    }
    // The blob shadow `0x279ae0(scale·0.3 / class scale, m)` (= L01 0x26eec8). The world box:
    let cs = w.classes.info(w.m(id).o_class).map_or(1.0, |i| i.scale);
    let s = w.m(id).scale * 0.3 / cs;
    crate::shadows::blob(w, s, id);
    let p = w.m(id).position;
    if p[2] < 5.0 || 500.0 < p[2] {
        delete_pod(w, id);
        return;
    }
    let m = w.mm(id);
    for k in 0..3 {
        m.position[k] = m.position[k].clamp(5.0, 1018.0);
    }
}

/// Pod state 0 of 0x30b3b0 (module table).
fn fly(w: &mut World, id: MobyId, old: [f32; 4]) {
    let cs = class_scale(w, id);
    let s = w.m(id).scale;
    w.mm(id).scale = s + (cs - s) * (SPEED * 0.05);
    let vel = c::pv4(w, id, 0);
    {
        let m = w.mm(id);
        for k in 0..3 { m.position[k] += vel[k]; }
    }
    let pos = w.m(id).position;
    let u = c::set_len3(vel, 0.2);
    let end = [u[0] + pos[0], u[1] + pos[1], u[2] + pos[2], u[3]];
    let launcher = moby_ref(w, id, pd::LAUNCHER);
    w.mm(id).has_collision = false;
    if let Some(l) = launcher { w.mm(l).has_collision = false; }
    if let Some(o) = w.coll_line(pv(old), pv(end), 4, Some(id)) {
        let pt = [o.point[0], o.point[1], o.point[2], 0.0];
        let d = c::set_len3(c::sub(pt, end), 0.215);
        let m = w.mm(id);
        for k in 0..3 { m.position[k] += d[k]; }
        bounce(w, id, &o);
    }
    let pos = w.m(id).position;
    if let Some(o) = w.coll_sphere(pv(pos), pf(0.2), 4, Some(id)) {
        if let Some(pc) = o.pushed_centre {
            let pt = [o.point[0], o.point[1], o.point[2], 0.0];
            let d = c::set_len3(c::sub(pt, [pc[0], pc[1], pc[2], 0.0]), 0.015);
            let m = w.mm(id);
            for k in 0..3 { m.position[k] = pc[k] + d[k]; }
        }
        bounce(w, id, &o);
    }
    if let Some(l) = launcher {
        for i in 0..SLOTS {
            let Some(e) = moby_ref(w, l, lv::PODS + 4 * i) else { continue };
            if e == id || w.m(e).o_class != POD || gone(w, e) { continue; }
            let pos = w.m(id).position;
            let ep = w.m(e).position;
            let d = [pos[0] - ep[0], pos[1] - ep[1], 0.0, pos[3]];
            let l3 = c::len3(d);
            if l3 < 1.8 {
                let k = (1.8 - l3) * (SPEED * -0.5 + 1.0);
                let m = w.mm(id);
                for j in 0..3 { m.position[j] += d[j] * k; }
            }
        }
    }
    let mut t = c::pi16(w, id, pd::TIMER);
    let r = crate::moby_update::services::fast_dec_timer_s16(&mut t);
    c::set_pi16(w, id, pd::TIMER, t);
    if r != 0 { w.mm(id).state = 2; }
    let vel = c::pv4(w, id, 0);
    let coll = class_coll(w, id);
    {
        let m = w.mm(id);
        m.rotation[0] = wrap_frac(vel[0]);
        m.rotation[1] = wrap_frac(vel[1]);
        m.has_collision = coll;
    }
    if let Some(l) = launcher {
        let lc = class_coll(w, l);
        w.mm(l).has_collision = lc;
    }
    c::set_pf(w, id, 8, vel[2] - DT2 * 9.8);
    let m = w.mm(id);
    m.rotation[0] += SPEED * 0.01;
    m.rotation[1] += SPEED * 0.02;
}

/// Level09 0x30a778(launcher, p): a dead member of the launcher's group revived at `p` (module table). None without
/// a dead member.
pub fn hatch(w: &mut World, launcher: MobyId, p: [f32; 4]) -> Option<MobyId> {
    if w.m(launcher).pvars.len() < lv::SIZE { return None; }
    let g = c::pi32(w, launcher, lv::GROUP);
    let m = group_first(w, g, GroupWalk::Dead)?;
    let n = c::pu8(w, launcher, lv::COUNT).wrapping_add(1);
    c::set_pu8(w, launcher, lv::COUNT, n);
    let grow = c::pu8(w, launcher, lv::GROW) != 0;
    revive(w, launcher, m, p, grow, (0xff, 0xff));
    Some(m)
}

/// The revival of a dead member `m` at `p` by a pod's `launcher` (0x30a778's body; also Kerwan's `0x2bec60`,
/// [`super::pod_spawner`]): state 0, the class's mode, scale and collision, the given update / draw distances, a tenth
/// of the scale with `grow`, the hard cut to sequence 0, the suck record and the damage record reset, the bolts split
/// with the launcher, Ratchet's light, `MobyBuildMatrix`.
pub fn revive(w: &mut World, launcher: MobyId, m: MobyId, p: [f32; 4], grow: bool, (update_dist, draw_dist): (u8, i16)) {
    let info = w.classes.info(w.m(m).o_class).unwrap_or_default();
    {
        let mm = w.mm(m);
        mm.position = [p[0], p[1], p[2], 0.0];
        mm.state = 0;
        mm.cmd = 0;
        mm.mode = info.mode_bits;
        mm.scale = info.scale;
        mm.update_dist = update_dist;
        mm.draw_dist = draw_dist;
        mm.visible = 1;
        if info.glow.is_some() { mm.mode |= mode::GLOW; }
        if info.b0f != 0 { mm.mode |= mode::CLASS_F; }
        mm.occlusion = 0x7f80;
        mm.b71 = 0xff;
        mm.b72 = 0xff;
        mm.hit_slot = 0xff;
        if grow { mm.scale = info.scale * 0.1; }
    }
    c::hard_cut(w, m, 0, 0);
    if let Some(r) = crate::moby_update::creature::react::record(w, m) {
        c::set_pi32(w, m, r + 0x6c, 0);
        c::set_pi32(w, m, r + 0x94, 0);
        c::set_pi16(w, m, r + 0x68, 0);
    }
    let lb = w.m(launcher).b4;
    if lb < w.m(m).b4 { w.mm(m).b4 = lb; }
    if w.m(m).b4 < 1 { w.mm(m).b4 = 1; }
    let left = (w.m(launcher).b4 as u16).wrapping_sub(w.m(m).b4 as u16) as i16;
    w.mm(launcher).b4 = if left < 0 { 1 } else { left };
    w.mm(m).has_collision = info.has_collision;
    crate::moby_update::classes::units::take_hero_light(w, m);
    w.build_matrix(m);
    if let Some(d) = crate::moby_update::triggers::pvar_record(w.m(m)) {
        if d + 0x38 <= w.m(m).pvars.len() {
            c::set_pf(w, m, d + 0x34, 0.0);
            c::set_pf(w, m, d + 0x30, 0.0);
            let hp = c::pi16(w, m, d + 4) as f32;
            c::set_pf(w, m, d, hp);
        }
    }
}
