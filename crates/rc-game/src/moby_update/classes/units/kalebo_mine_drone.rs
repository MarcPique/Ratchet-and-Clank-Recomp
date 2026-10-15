//! Kalebo's mine drones, class 1401 (level 16, 14 placed; census U521): level16 0x2e37a0 with its private helpers
//! 0x2e3e20 (the path setup), 0x2e3fa0 (the path follow), 0x2e4110 (the carried mine), 0x2e4190 (the exhaust), and the
//! two calls into the mines' family it makes: `0x2de298` (create a mine 933 for it to carry) and `0x2de3a8`
//! (release it). Read from the level16 decomp and disassembly. Native `f32`; the `rand` draws in the game's order.
//!
//! A drone waits hidden at the start of its path until Ratchet enters its cuboid, then appears and (mode 0) flies
//! the path carrying a new mine, springs onto the path's last point, drops the mine (the mine's state 1) and
//! flies back along its second path to wait again; mode 1 only plays its appear animation and then turns to face
//! Ratchet; mode 2 faces Ratchet from the start. It blows its exhaust (type-23 glows) while it flies, flinches when
//! Ratchet touches it (`0x13f58c`), and a hit on it or on its mine knocks it out of the air (`SetDeathBits`: its
//! bolts) into a death explosion where it lands.
//!
//! **System or not**: the mine creation is the mines' own family code (level 16 only), ported here with its one
//! caller ([`make_mine`], [`release_mine`]); the mine's update `0x2ddde0` is class 933's ([`super::kalebo_mine`]).
//! Shared calls used: `spline::advance` (0x2726c8), `Spring` 0x270780, `SpringTurn2` 0x26d058, the
//! knockback start / flight (`creature::knock`), `SetDeathBits`, the death explosion `0x273f50`, the hit flash,
//! `PartType23Spawn`, `PointInCuboid`, the hero's contact moby (`Hero::cap_moby`).
//!
//! **Pvars**: +0x60 the flash record (+0x67 the red), +0x70 the knockback record K (0x60), +0xd0 s32 the path cursor's
//! segment, +0xd4 f32 its distance, +0xd8 s32 path A (out), +0xdc s32 path B (back), +0xe0 s32 the cuboid, +0xe4 /
//! +0xe8 / +0xec the springs' velocities (x, y, z), +0xf0 the turn velocity, +0xf4 the carried mine (a pointer in the
//! game; moby index + 1 here), +0xfc s32 the mode (0, 1, 2). Level data words (gp, level16): 0x161d90 the path speed
//! 25, 0x161d94 / 0x161d98 the springs 0.01 / 0.2, 0x161d9c / 0x161da0 the mine's offsets 0.1 / 0.1 along rows 0 / 2,
//! 0x161da4..0x161db0 the exhaust points (−0.5, 0.6), (−0.4, 0.7), 0x161dbc the glow colour 0x7f204080.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2e37c4 | animation flags +0x70 bit 2 and sequence A = B = 1 → `MobyAnimBlend(m, 4, 0, ticks(5))` (0x252f58) | [`update`] |
//! | 0x2e3810 | Ratchet's contact moby (0x13f58c) is this one and neither sequence is 1 → blend (1, 0, `ticks(5)`) (the flinch) | [`update`] (`Hero::cap_moby`) |
//! | 0x2e3864 | `MobyGetHitMessage(m, 0x210000, 0)` (0x255d58 = L01 0x26f320); none → the same on the carried mine | [`update`] |
//! | | a hit, state ≠ 6: K +0x10 gravity 0.008, +0x24 flags 1, +0x1c up 12·dt, +0x14 drag 0.0005, +0x18 speed 24·dt, +0x3d = 0; `0x257e50(atan(pos − Ratchet), m, K, 1, 1, 0)` (= L01 0x271418); red +0x67 = 120 (no flash start: the update fades it); `SetDeathBits(m, 0, −1)` (0x252b48); state 6 | [`update`] (`knock::start`, `crate_::set_death_bits`) |
//! | | +0xa4 = 0xff | [`update`] |
//! | state 0 | mode 2: state 1, turn velocity 0, the hard cut (4, 0) (0x252ea0 = L01 0x26c5a8), carried 0 | [`update`] |
//! | | else path A −1 or empty, (mode 0) path B −1 or empty, or the cuboid −1 → a debug `printf` (n/a) and `DeleteMoby` | [`update`] |
//! | | else the path setup 0x2e3e20; state 2, update distance 0x80, mode \|= 0x41 (hidden, not animated), +0x94 = 0, carried 0 | [`update`] ([`setup`]) |
//! | 0x2e3e20 | path A: point i's w = its distance to point i + 1 (i < count − 1), written into the level's path; cursor 0 / 0; position = A's point 0 (four words); yaw = `FastArcTan(p1 − p0)`; the four velocities 0; mode ≠ 1: the same lengths on path B | [`setup`] |
//! | state 1 | `SpringTurn2(atan(Ratchet − pos), 0.005, 0.2, 0, m, &+0xf0)` (0x253950 = L01 0x26d058); the exhaust | [`update`] (`turn::spring_turn2_pvar`) |
//! | state 2 | Ratchet (0x13f3d0) not in the cuboid (0x25b258 = L01 0x274820) → nothing | [`update`] (`triggers::point_in_cuboid`) |
//! | | mode 1: the hard cut (0, 0); else carried = [`make_mine`], the carry 0x2e4110, the hard cut (2, 0); then mode `&= 0xffbe` (shown, animated), state 4, +0x94 = the class collision | [`update`] |
//! | state 3 | mode 0: the carry; Spring(the last point of path A, 0.01, 0.2, 0, &pos.x / .y / .z, &+0xe4 / +0xe8 / +0xec) (0x2571b8 = L01 0x270780) | [`update`] (`hero::physics::spring`) |
//! | | the animation wrapped (+0x70 bit 2) with A = B: mode 1 → state 1, turn velocity 0, the hard cut (4, 0); else the carried mine released ([`release_mine`]) and cleared, state 5, blend (0, 0, 20), cursor 0 / 0; the exhaust | [`update`] |
//! | state 4 | mode 0: the carry; the path follow ([`follow`]) on path A at its end → state 3, blend (3, 0, 5); the exhaust | [`update`] |
//! | state 5 | the path follow on path B; at its end: state 2, mode \|= 0x41, +0x94 = 0, cursor 0 / 0, the velocities 0, position = A's point 0, yaw = `FastArcTan(p1 − p0)` (no exhaust that tick); else the exhaust | [`update`] |
//! | state 6 | the knockback flight (0x257f90 = L01 0x271558): landed or touched (bits 1 \| 2) → `0x25a988(1, 13, m, pos, −1)` (= L01 0x273f50, the death explosion, no sound) and `DeleteMoby`; else below z 5 → `DeleteMoby` | [`update`] (`knock::update`, `fx::death_explosion`) |
//! | every state left alive | the flash update (0x258e30 = L01 0x2723f8) | [`update`] (`flash::update`) |
//! | 0x2e3fa0 | the path (state 5: B, else A); `0x259100(25·dt, path, &p, &+0xd0, &+0xd4, open)` (= L01 0x2726c8); the three springs toward p; d = p − pos: \|d.x\| > 0.01 and \|d.y\| > 0.01 (`fabs` 0x1fda80) → `SpringTurn2(atan(d), 0.01, 0.2, 0, m, &+0xf0)`; returns the end flag | [`follow`] (`spline::advance`) |
//! | 0x2e4110 | carried ≠ 0: its position = pos + unit(row 0)·0.1 + unit(row 2)·0.1 | [`carry`] |
//! | 0x2e4190 | p = pos + row 0·(−0.5) + row 2·0.6; twice: k = `randi(16)`, `randi(2)` = 0 → +k else −k; `PartType23Spawn(0.2, 1, 0.9, 99840, p, ±k, zero, 0x7f204080)` (0x268440 = L01 0x282060); a record: timer +0x0a = `ticks(12)`, phase +0x24 = 2, +0x2a = 0x7f, +0x2b = the timer | [`exhaust`] (`particles::type23`) |
//! | | p = pos + row 0·(−0.4) + row 2·0.7; t = `ticks(2)`; three: `PartType23Spawn(0.05, 1, 1, size, p, spin, zero, 0x7fffffff)` with size 80000, 60000, 40000 and spin 16, −16, 16; a record: timer = t, rotation +0x08 = `randi(255)`, phase 2, +0x2a = 0x7f, +0x2b = t; t doubles (s16) each time | [`exhaust`] |
//! | 0x2de298 | `CreateMoby(0x3a5)` (933); mine +0x00 = the drone, +0x08 = +0x0c = 0, +0x04 = −1.0, +0x10 = `randf(π/300, π/150)`, +0x14 = `randf(π/450, π/225)`, +0x18 = `randf(π/900, π/450)`; position = the drone's (four words), state 0, yaw = the drone's, +0x30 = 0xff, +0x32 = 0xff, +0x31 = 1; `MobyBuildMatrix` | [`make_mine`] |
//! | 0x2de3a8 | the mine's state = 1 | [`release_mine`] (the mine's own update: [`super::kalebo_mine`]) |

#![allow(clippy::needless_range_loop)] // the game's per-lane writes, spelled out.

use crate::hero::physics::spring;
use crate::moby_runtime::MobyId;
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, flash, fx, knock, turn, DT};
use crate::moby_update::services::{pf, World};
use crate::ps2v::Pf;
use crate::spline::{self, Cursor};

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_37a0;
pub const CLASSES: [i16; 1] = [1401];
/// Level14's copy, class 923 (`0x2fe2a0`; census U509, 2 placed): the same code but for three points ([`Copy`]).
pub const L14_LEVEL: u32 = 14;
pub const L14_FN: u32 = 0x2f_e2a0;
pub const L14_CLASSES: [i16; 1] = [923];

/// Where the copies differ: level16's checks its mine's hits too; level14's (`0x2fe2a0`) flies at 20 on level 16, else
/// 25, or its own speed +0xf8 when not 0 (`0x2fea80`), and its first exhaust glows are 100000 (`0x2feca8`) where
/// level16's are 99840.
#[derive(Clone, Copy)]
struct Copy {
    mine_hits: bool,
    own_speed: bool,
    flame: f32,
}
const L16: Copy = Copy { mine_hits: true, own_speed: false, flame: 99840.0 };
const L14: Copy = Copy { mine_hits: false, own_speed: true, flame: 100_000.0 };
/// The mine the drone carries (0x3a5).
pub const MINE: i16 = 933;

/// Pvar offsets (module doc).
pub mod pv {
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const SEG: usize = 0xd0;
    pub const DIST: usize = 0xd4;
    pub const PATH_A: usize = 0xd8;
    pub const PATH_B: usize = 0xdc;
    pub const CUBOID: usize = 0xe0;
    pub const VEL: usize = 0xe4;
    pub const TURN: usize = 0xf0;
    pub const MINE: usize = 0xf4;
    /// Level14's copy: its own speed (0: the level's).
    pub const OWN_SPEED: usize = 0xf8;
    pub const MODE: usize = 0xfc;
    pub const SIZE: usize = 0x100;
}

/// Level16 gp words (module doc).
const SPEED: f32 = 25.0;
const SPRING_K: f32 = 0.01;
const SPRING_D: f32 = 0.2;
const MINE_ROW0: f32 = 0.1;
const MINE_ROW2: f32 = 0.1;
const GLOW: u32 = 0x7f20_4080;

fn mine(w: &World, id: MobyId) -> Option<MobyId> {
    let v = c::pi32(w, id, pv::MINE);
    usize::try_from(v - 1).ok().filter(|&m| v > 0 && m < w.table.mobys.len())
}

fn path(w: &World, i: i32) -> Option<Vec<[f32; 4]>> {
    let s = w.svc.splines.get(usize::try_from(i).ok()?)?;
    Some(s.iter().map(|p| p.map(f32::from_bits)).collect())
}

fn path_ok(w: &World, i: i32) -> bool { i != -1 && path(w, i).is_some_and(|p| !p.is_empty()) }

/// The animation wrapped (+0x70 bit 2) with sequence A = B.
fn wrapped_on_same(w: &World, id: MobyId) -> bool {
    let a = &w.m(id).anim;
    a.flags & 2 != 0 && a.seq_a == a.seq_b
}

/// Level16 0x2e37a0 (module doc).
pub fn update(w: &mut World, id: MobyId) { update_copy(w, id, L16) }

/// Level14 0x2fe2a0: class 923 ([`Copy`]).
pub fn update_l14(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::SIZE);
    update_copy(w, id, L14)
}

fn update_copy(w: &mut World, id: MobyId, v: Copy) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let a = w.m(id).anim;
    if a.flags & 2 != 0 && a.seq_a == a.seq_b && a.seq_a == 1 {
        let t = w.ticks(5);
        w.anim_blend(id, 4, 0, t);
    }
    let a = w.m(id).anim;
    if w.hero.cap_moby == Some(id) && a.seq_b != 1 && a.seq_a != 1 {
        let t = w.ticks(5);
        w.anim_blend(id, 1, 0, t);
    }
    let mut hit = w.get_hit(id, 0x21_0000, false).is_some();
    if !hit && v.mine_hits {
        if let Some(m) = mine(w, id) { hit = w.get_hit(m, 0x21_0000, false).is_some(); }
    }
    if hit && w.m(id).state != 6 {
        let k = pv::K;
        c::set_pf(w, id, k + knock::k::GRAVITY, f32::from_bits(0x3c03_126f));
        c::set_pi32(w, id, k + knock::k::FLAGS, 1);
        c::set_pf(w, id, k + knock::k::UP, DT * 12.0);
        c::set_pf(w, id, k + knock::k::DRAG, f32::from_bits(0x3a03_126f));
        c::set_pf(w, id, k + knock::k::SPEED, DT * 24.0);
        c::set_pu8(w, id, k + 0x3d, 0);
        let h = crate::moby_update::classes::units::hero_pos(w);
        let p = w.m(id).position;
        let ang = c::atan(p[0] - h[0], p[1] - h[1]);
        knock::start(w, id, k, ang, 1, 1, 0);
        c::set_pu8(w, id, pv::FLASH + 7, 120);
        set_death_bits(w, id, 0, -1);
        w.mm(id).state = 6;
    }
    w.mm(id).hit_slot = 0xff;
    let md = c::pi32(w, id, pv::MODE);
    match w.m(id).state {
        0 => {
            if md == 2 {
                w.mm(id).state = 1;
                c::set_pf(w, id, pv::TURN, 0.0);
                c::hard_cut(w, id, 4, 0);
                c::set_pi32(w, id, pv::MINE, 0);
            } else {
                let ok = path_ok(w, c::pi32(w, id, pv::PATH_A))
                    && (md == 1 || path_ok(w, c::pi32(w, id, pv::PATH_B)))
                    && c::pi32(w, id, pv::CUBOID) != -1;
                if !ok {
                    w.delete_moby(id);
                    return;
                }
                setup(w, id);
                let m = w.mm(id);
                m.state = 2;
                m.update_dist = 0x80;
                m.mode |= 0x41;
                m.has_collision = false;
                c::set_pi32(w, id, pv::MINE, 0);
            }
        }
        1 => {
            let h = crate::moby_update::classes::units::hero_pos(w);
            let p = w.m(id).position;
            let ang = c::atan(h[0] - p[0], h[1] - p[1]);
            turn::spring_turn2_pvar(w, id, ang, f32::from_bits(0x3ba3_d70a), f32::from_bits(0x3e4c_cccd), 0.0, pv::TURN);
            exhaust(w, id, v.flame);
        }
        2 => {
            let h = crate::moby_update::classes::units::hero_pos(w);
            if crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [h[0], h[1], h[2]], c::pi32(w, id, pv::CUBOID)) {
                if md == 1 {
                    c::hard_cut(w, id, 0, 0);
                } else {
                    let at = w.m(id).position;
                    let mn = make_mine(w, id, at);
                    c::set_pi32(w, id, pv::MINE, mn.map_or(0, |m| m as i32 + 1));
                    carry(w, id);
                    c::hard_cut(w, id, 2, 0);
                }
                let coll = crate::moby_update::classes::units::class_collision(w, w.m(id).o_class);
                let m = w.mm(id);
                m.mode &= 0xffbe;
                m.state = 4;
                m.has_collision = coll;
            }
        }
        3 => {
            if md == 0 { carry(w, id); }
            if let Some(last) = path(w, c::pi32(w, id, pv::PATH_A)).and_then(|p| p.last().copied()) { spring_to(w, id, last); }
            if wrapped_on_same(w, id) {
                if md == 1 {
                    w.mm(id).state = 1;
                    c::set_pf(w, id, pv::TURN, 0.0);
                    c::hard_cut(w, id, 4, 0);
                } else {
                    if let Some(m) = mine(w, id) {
                        release_mine(w, m);
                        c::set_pi32(w, id, pv::MINE, 0);
                    }
                    w.mm(id).state = 5;
                    w.anim_blend(id, 0, 0, 20);
                    c::set_pf(w, id, pv::DIST, 0.0);
                    c::set_pi32(w, id, pv::SEG, 0);
                }
            }
            exhaust(w, id, v.flame);
        }
        4 => {
            if md == 0 { carry(w, id); }
            if follow(w, id, v) {
                w.mm(id).state = 3;
                w.anim_blend(id, 3, 0, 5);
            }
            exhaust(w, id, v.flame);
        }
        5 => {
            if !follow(w, id, v) {
                exhaust(w, id, v.flame);
            } else {
                {
                    let m = w.mm(id);
                    m.state = 2;
                    m.mode |= 0x41;
                    m.has_collision = false;
                }
                c::set_pf(w, id, pv::DIST, 0.0);
                c::set_pi32(w, id, pv::SEG, 0);
                for o in [pv::TURN, pv::VEL + 8, pv::VEL + 4, pv::VEL] { c::set_pf(w, id, o, 0.0); }
                if let Some(a) = path(w, c::pi32(w, id, pv::PATH_A)) {
                    let p0 = a.first().copied().unwrap_or([0.0; 4]);
                    let p1 = a.get(1).copied().unwrap_or([0.0; 4]);
                    let m = w.mm(id);
                    m.position = p0;
                    m.rotation[2] = c::atan(p1[0] - p0[0], p1[1] - p0[1]);
                }
            }
        }
        6 => {
            let r = knock::update(w, id, pv::K);
            if r & 3 != 0 {
                let p = w.m(id).position;
                fx::death_explosion(w, 1.0, 13.0, Some(id), p, -1);
                w.delete_moby(id);
                return;
            }
            if w.m(id).position[2] < 5.0 {
                w.delete_moby(id);
                return;
            }
        }
        _ => {}
    }
    flash::update(w, id, pv::FLASH);
}

/// Level16 0x2e3e20, the path setup (module table).
pub fn setup(w: &mut World, id: MobyId) {
    measure(w, c::pi32(w, id, pv::PATH_A));
    c::set_pi32(w, id, pv::SEG, 0);
    c::set_pf(w, id, pv::DIST, 0.0);
    if let Some(a) = path(w, c::pi32(w, id, pv::PATH_A)) {
        let p0 = a.first().copied().unwrap_or([0.0; 4]);
        let p1 = a.get(1).copied().unwrap_or([0.0; 4]);
        let m = w.mm(id);
        m.position = p0;
        m.rotation[2] = c::atan(p1[0] - p0[0], p1[1] - p0[1]);
    }
    for o in [pv::TURN, pv::VEL + 8, pv::VEL + 4, pv::VEL] { c::set_pf(w, id, o, 0.0); }
    if c::pi32(w, id, pv::MODE) != 1 { measure(w, c::pi32(w, id, pv::PATH_B)); }
}

/// Point i's w = `VecDistance(p[i], p[i+1])` for i < count − 1, in the level's path `i` (0x2e3e20's loops).
fn measure(w: &mut World, i: i32) {
    let Some(pts) = path(w, i) else { return };
    let n = pts.len();
    for k in 0..n.saturating_sub(1) {
        let d = c::dist3(pts[k], pts[k + 1]);
        w.svc.splines[i as usize][k][3] = d.to_bits();
    }
}

/// `Spring(t, 0.01, 0.2, 0, &pos.k, &vel.k)` (0x2571b8 = L01 0x270780) on x, y, z toward `t`.
fn spring_to(w: &mut World, id: MobyId, t: [f32; 4]) {
    for k in 0..3 {
        let mut x = Pf::f(w.m(id).position[k]);
        let mut v = Pf::f(c::pf(w, id, pv::VEL + 4 * k));
        spring(Pf::f(t[k]), pf(SPRING_K), pf(SPRING_D), Pf::ZERO, &mut x, &mut v);
        w.mm(id).position[k] = x.to_f32();
        c::set_pf(w, id, pv::VEL + 4 * k, v.to_f32());
    }
}

/// Level16 0x2e3fa0, the path follow (module table): whether the path's end was reached.
fn follow(w: &mut World, id: MobyId, v: Copy) -> bool {
    let mut speed = SPEED;
    if v.own_speed {
        if w.svc.level == 16 { speed = 20.0; }
        let own = c::pf(w, id, pv::OWN_SPEED);
        if own != 0.0 { speed = own; }
    }
    let i = if w.m(id).state == 5 { c::pi32(w, id, pv::PATH_B) } else { c::pi32(w, id, pv::PATH_A) };
    let pts = path(w, i).unwrap_or_default();
    let mut cur = Cursor { seg: c::pi32(w, id, pv::SEG), t: c::pf(w, id, pv::DIST) };
    let (p, ended) = spline::advance(&pts, false, speed * DT, &mut cur);
    c::set_pi32(w, id, pv::SEG, cur.seg);
    c::set_pf(w, id, pv::DIST, cur.t);
    let p = [p[0], p[1], p[2], 0.0];
    spring_to(w, id, p);
    let pos = w.m(id).position;
    let d = [p[0] - pos[0], p[1] - pos[1]];
    if 0.01 < d[0].abs() && 0.01 < d[1].abs() {
        turn::spring_turn2_pvar(w, id, c::atan(d[0], d[1]), SPRING_K, SPRING_D, 0.0, pv::TURN);
    }
    ended
}

/// Level16 0x2e4110: the carried mine at pos + unit(row 0)·0.1 + unit(row 2)·0.1.
pub fn carry(w: &mut World, id: MobyId) {
    let Some(m) = mine(w, id) else { return };
    let (pos, rows) = (w.m(id).position, w.m(id).rows);
    let a = c::set_len3(rows[0], MINE_ROW0);
    let b = c::set_len3(rows[2], MINE_ROW2);
    let mm = w.mm(m);
    for k in 0..3 { mm.position[k] = pos[k] + a[k] + b[k]; }
}

#[allow(clippy::too_many_arguments)]
fn glow(w: &mut World, jitter: f32, lo: f32, hi: f32, size: f32, p: [f32; 4], spin: i32, rgba: u32) -> Option<usize> {
    let Some(sys) = w.particles.as_deref_mut() else { fx::part_unported(w, 23); return None };
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let r = crate::particles::type23::spawn(sys, w.rng, jitter, lo, hi, size, p, spin, [0.0; 4], rgba);
    if r.is_none() { w.svc.fx.part_failed += 1; }
    r
}

/// Level16 0x2e4190, the exhaust (module table).
pub fn exhaust(w: &mut World, id: MobyId, flame: f32) {
    use crate::particles::rec;
    let (pos, rows) = (w.m(id).position, w.m(id).rows);
    let at = |a: f32, b: f32| -> [f32; 4] { std::array::from_fn(|k| if k == 3 { pos[3] } else { pos[k] + rows[0][k] * a + rows[2][k] * b }) };
    let p = at(-0.5, 0.6);
    for _ in 0..2 {
        let k = w.rng.randi(16);
        let spin = if w.rng.randi(2) == 0 { k } else { -k };
        if let Some(i) = glow(w, 0.2, 1.0, 0.9, flame, p, spin, GLOW) {
            let t = w.ticks(12);
            let r = &mut w.particles.as_deref_mut().unwrap().pool.recs[i];
            rec::set_i16(r, 0xa, t as i16);
            rec::set_u32(r, 0x24, 2);
            r[0x2a] = 0x7f;
            r[0x2b] = t as u8;
        }
    }
    let p = at(-0.4, 0.7);
    let mut t = w.ticks(2) as i16;
    let (mut size, mut spin) = (80000.0, 0x10);
    for _ in 0..3 {
        if let Some(i) = glow(w, 0.05, 1.0, 1.0, size, p, spin, 0x7fff_ffff) {
            let rot = w.rng.randi(0xff) as u8;
            let r = &mut w.particles.as_deref_mut().unwrap().pool.recs[i];
            rec::set_i16(r, 0xa, t);
            r[8] = rot;
            rec::set_u32(r, 0x24, 2);
            r[0x2a] = 0x7f;
            r[0x2b] = t as u8;
        }
        size -= 20000.0;
        t = t.wrapping_shl(1);
        spin = -spin;
    }
}

/// Level16 0x2de298(drone, &pos): a mine 933 for the drone to carry (module table). None when the table is
/// full.
pub fn make_mine(w: &mut World, drone: MobyId, at: [f32; 4]) -> Option<MobyId> {
    let m = w.create_moby(MINE)?;
    if w.m(m).pvars.len() < 0x1c { return Some(m); }
    c::set_pi32(w, m, 0, drone as i32 + 1);
    c::set_pi32(w, m, 8, 0);
    c::set_pi32(w, m, 0xc, 0);
    c::set_pf(w, m, 4, -1.0);
    let a = w.rng.randf(f32::from_bits(0x3c2b_92a6), f32::from_bits(0x3cab_92a6));
    c::set_pf(w, m, 0x10, a);
    let b = w.rng.randf(f32::from_bits(0x3be4_c388), f32::from_bits(0x3c64_c388));
    c::set_pf(w, m, 0x14, b);
    let d = w.rng.randf(f32::from_bits(0x3b64_c388), f32::from_bits(0x3be4_c388));
    c::set_pf(w, m, 0x18, d);
    let yaw = w.m(drone).rotation[2];
    {
        let mm = w.mm(m);
        mm.position = at;
        mm.state = 0;
        mm.rotation[2] = yaw;
        mm.update_dist = 0xff;
        mm.draw_dist = 0xff;
        mm.visible = 1;
    }
    w.build_matrix(m);
    Some(m)
}

/// Level16 0x2de3a8(mine): its state = 1 (the release).
pub fn release_mine(w: &mut World, m: MobyId) { w.mm(m).state = 1; }
