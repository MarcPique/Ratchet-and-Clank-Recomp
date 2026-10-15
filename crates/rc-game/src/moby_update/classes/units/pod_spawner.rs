//! U134 (census 2026-10-02): class 455, the pod spawners of Kerwan (03: 2 placed), Batalia (08: 3) and Oltanis (14:
//! 2), and the pods 545 they lob. Level03 0x2bef68 (the spawner), its hatch 0x2bec60, its cuboid point 0x2beba0, the pod
//! spawn 0x2c45d8 and the pod 0x2c4718; levels 08 / 14 run the same code (masked compare, census U134), so
//! `LevelPorts` runs these rows there too. Read from the level03 decomp and disassembly. Native `f32`; the `rand`
//! draws in the game's order.
//!
//! The older form of the Gaspar / Gemlik pod launcher ([`super::pod_launcher`]): instead of a moby group, the spawner
//! names up to 18 placed creatures (+0xe8, Kerwan's charging 573s), deletes them at its init and brings them back one
//! per landed pod; it fires only once Ratchet is near, and aims in one of five ways. A pod grows in from a thousandth
//! of its size, bounces off the world and other mobys (a knock sound at most every 12 ticks), keeps 1.8 apart from
//! every moby of the run list, and once it rests (or after 500 ticks) shrinks away while the creature grows in. Killed,
//! the spawner explodes and breaks into the pieces 1733–1735; its bolts come from `SetDeathBits`.
//!
//! **Shared with [`super::pod_launcher`]**: the pod's bounce ([`pod_launcher::bounce_velocity`]) and the revival
//! ([`pod_launcher::revive`]); the rest is this family's own (levels 03 / 08 / 14 only).
//!
//! **Spawner pvars** (0x130): +0x20 the damage record (health 1, +0x29 the column byte), +0x58 / +0x5a bytes, +0x60 the
//! flash record (+0x67 the red), +0x70 (the beam explosion's unused record), +0x82 u8 the trigger (0: within +0x9c; 1:
//! Ratchet in the cuboid +0xd0; else either the cuboid or within +0x90), +0x83 u8 flags (2 the creature grows in from a
//! tenth, 4 it grows while the pod shrinks, 8 a random distance), +0x84 u8 the firing state picked when Ratchet comes
//! near, +0x85 u8 the aim (0 a fan about Ratchet, 1 toward Ratchet, 2 the first unused cuboid, 3 a random cuboid, 4 the
//! cuboid of the count), +0x86 u8 the creatures and pods alive, +0x87 u8 the most, +0x88 s16 the fire timer, +0x8a s16
//! the interval (0: a burst up to the most, then only once all are gone), +0x8c s16 a word the pod keeps (+0x16, not
//! read), +0x90 f32 the range, +0x9c f32 the trigger range, +0xa0 s32[4] the target cuboids, +0xd0 s32 the trigger
//! cuboid, +0xdc f32 the gravity (9.8), +0xe4 f32 the lob's speed factor, +0xe8 s32[18] the creatures (moby indices
//! after the loader's link fixups, −1 none). **Pod pvars** (0x80): +0x00 the velocity, +0x10 the spawner (index + 1),
//! +0x14 s16 the life, +0x16 s16 the spawner's +0x8c, +0x18 the creature (index + 1), +0x1c u32 the spawner's flags,
//! +0x20 s32 the knock-sound cooldown.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2bef68 | drawn last frame and within 28 of the camera (0x166ec0): the shadow probe (0x248ba8 = L01 0x26f020), +0x7f = 0x16 | [`update`] (`shadows::probe_down`) |
//! | | +0x86 = the creatures alive (not 0xfe / 0xfd) + the live pods of the run list (0x15ffe4) | [`count`] |
//! | | d = the distance to Ratchet (0x13f3d0); near = d < +0x9c; the fire timer +0x88 (`FastDecTimer`) done; room = +0x86 < +0x87; in the cuboid +0xd0 (`PointInCuboid`, 0x24e3a8 = L01 0x274820); the trigger by +0x82; interval 0 and any alive → no room | [`update`] |
//! | | `MobyGetHitMessage(m, 0xc30000, 0)`; the resolver (0x248f00 = L01 0x26f378, column 4); out ≥ 2: health −= the damage; > 0 → red 90, the flash start (0x24bea0 = L01 0x272318); else `SetDeathBits(m, 0, −1)`, state 8, red 0x78, the flash start | [`hit`] |
//! | | +0xa4 = 0xff | [`update`] |
//! | state 0 | every live creature `DeleteMoby`; mode \|= 0x1000; +0x5a = 3, +0x58 = 8, +0x29 = 0; → 1 | [`update`] |
//! | state 1 | near → +0x84: 0 / 3 → 2, 1 → 3, 2 → 7, 4 / 5 → 5, else 1 | [`update`] |
//! | states 2..7 | fire (when triggered, the timer done and room) | [`update`] ([`launch`]) |
//! | state 8 | `SpawnBeamExplosion(0, 0, 8, 4, 18, 1, 15, m, &+0x70, NULL, 30, 8, 20, sound 1, shake, …)`; `BreakFxB` 1733 / 1734 / 1735 (0x6c5..0x6c7) at the position and rotation; `DeleteMoby` | [`update`] |
//! | other | → 1 | [`update`] |
//! | | the flash update (0x24bf80 = L01 0x2723f8) | [`update`] |
//! | fire | until (interval 0: +0x87 ≤ +0x86; else the timer running): start = joint 0 (0x23e338 = L01 0x2645a8) | [`launch`] |
//! | | aim 0: a = wrap(2π·+0x86 / (+0x87 + 1) − π) (0x24cd58 = L01 0x2731d0); r = d·0.5 (flag 8: `randf(r, 3)`), at least 3; v = (cos a, sin a, 0)·r clamped to +0x90; p = start + v, z = `GroundHeight(0.5, p, 0)` unless 0 | [`launch`] |
//! | | aim 1: v = (Ratchet − start), z 0, ·0.5; flag 8: length `randf(\|v\|, 3)`; shorter than 3 → length 3; clamped to +0x90; p as aim 0 | [`launch`] |
//! | | aim 2: of the cuboids not used this burst, the first (the "nearest" test measures the spawner's distance to Ratchet, the same for all; each candidate draws its point, 0x2beba0); marked used | [`launch`] ([`cuboid_point`]) |
//! | | aims 3 / 4: the k-th valid cuboid, k = `rand() % n + 1` / `+0x86 % n + 1` (n valid; none: the game traps, the port keeps the last point) | [`launch`] |
//! | | aims 2–4: v = q − start (flag 8: length `randf(\|v\|, 3)`); p = q | [`launch`] |
//! | | s = \|v\|·+0xe4·speed (0x15ed60); v: z 0, length s; q = start + unit(v)·0.05 jittered ±0.025 (0x24e328 = L01 0x2747a0; 0x1f8c28 adds in place); v.z = the lob's vertical speed (0x249678 = L01 0x26faf0, gravity −+0xdc·dt²) | [`launch`] (`knock::lob_up`) |
//! | | pod = 0x2c45d8(m, q, v, +0x8c, +0x83); its +0x30 = 0xff; +0x88 = `ticks(+0x8a)`; +0x86 += 1 | [`launch`] ([`spawn_pod`]) |
//! | 0x2beba0 | a point in cuboid c: (`randf(−1, 1)` ×3) through its rows, + its centre (+0x30) | [`cuboid_point`] |
//! | 0x2c45d8 | `CreateMoby(545)`; +0x10 the spawner; position, velocity; Euler x, y = `randf(−π, π)`; +0x18 = 0; +0x14 = `ticks(500)`; +0x16; +0x30 / +0x32 the spawner's update / draw distances, +0x31 = 1; state 0; scale = class scale · 0.001; +0x1c the flags, +0x20 = 0; `MobyBuildMatrix` | [`spawn_pod`] |
//! | 0x2c4718 | `FastDecTimer` (int) on +0x20; old = position | [`pod_update`] |
//! | pod state 0 | scale += (class scale − scale)·speed·0.05; position += velocity; end = position + unit(velocity)·0.2; the pod's and the spawner's +0x98 = −1, +0x94 = 0 (saved) | [`fly`] |
//! | | `CollLine_Fix(old, end, 4, m, 0)`: position += unit(point − end)·0.215, the bounce; the sphere (0x1ea6c8 = L01 0x212960, r 0.2, flags 4): position = pushed centre + unit(point − pushed centre)·0.015, the bounce; the bounce's sound (class sound 0, 0x22da68) when the cooldown is 0, cooldown 12 | [`fly`] (`pod_launcher::bounce_velocity`) |
//! | | each moby of the run list but the pod, not 0xfe / 0xfd: d = position − its position, z 0; \|d\| < 1.8 → position += d·(1.8 − \|d\|)·(1 − 0.5·speed) | [`fly`] |
//! | | the life +0x14 out → 2; Euler x / y = wrap(velocity x / y); the saved +0x98 / +0x94 back; velocity.z −= 9.8·dt²; Euler x += speed·0.01, y += speed·0.02 | [`fly`] |
//! | pod state 1 | z = `GroundHeight(0.5)` + 0.2, w 0; the creature = 0x2bec60(spawner, position, −1); → 2 | [`pod_update`] ([`hatch`]) |
//! | pod state 2 | scale −= scale·speed·0.05; flag 4 and a creature: its scale += (its class scale − it)·speed·0.05; scale < class scale·speed·0.05 → 0.0001, and no creature, it deleted, no flag 4, or it grown (class scale − 0.01) → `DeleteMoby` | [`pod_update`] |
//! | every pod state | the blob shadow (0x248a50 = L01 0x26eec8, scale·0.3 / class scale); z outside [5, 500] → `DeleteMoby`; else x, y, z clamped into [5, 1018] | [`pod_update`] |
//! | 0x2bec60 | +0x86 = 0; the first deleted creature (class filter −1: any) revived at p (`pod_launcher::revive` with the spawner's update / draw distances, flag 2 the tenth), each other live one counted; the live pods of the run list counted; none revived → `printf` (n/a), none | [`hatch`] |

#![allow(clippy::needless_range_loop)] // the game's per-lane VU writes, spelled out.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::flyer::wrap_frac;
use crate::moby_update::classes::units::pod_launcher;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, DT2, SPEED};
use crate::moby_update::services::{fast_dec_timer_s16, pf, pv, World};
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 3;
/// The spawner's update and class (0x1c7).
pub const UPDATE_FN: u32 = 0x2b_ef68;
pub const SPAWNER: i16 = 455;
pub const CLASSES: [i16; 1] = [SPAWNER];
/// The pod's update and class (0x221).
pub const POD_FN: u32 = 0x2c_4718;
pub const POD: i16 = 545;
pub const POD_CLASSES: [i16; 1] = [POD];
/// The pieces the spawner breaks into.
pub const PIECES: [i16; 3] = [1733, 1734, 1735];

/// The spawner's pvar offsets (module doc).
pub mod sv {
    pub const DAMAGE: usize = 0x20;
    pub const FLASH: usize = 0x60;
    pub const TRIGGER: usize = 0x82;
    pub const FLAGS: usize = 0x83;
    pub const PICK: usize = 0x84;
    pub const AIM: usize = 0x85;
    pub const COUNT: usize = 0x86;
    pub const MAX: usize = 0x87;
    pub const TIMER: usize = 0x88;
    pub const INTERVAL: usize = 0x8a;
    pub const WORD: usize = 0x8c;
    pub const RANGE: usize = 0x90;
    pub const NEAR: usize = 0x9c;
    pub const CUBOIDS: usize = 0xa0;
    pub const CUBOID: usize = 0xd0;
    pub const GRAVITY: usize = 0xdc;
    pub const LOB: usize = 0xe4;
    pub const CREATURES: usize = 0xe8;
    pub const SIZE: usize = 0x130;
}

/// The pod's pvar offsets (module doc).
pub mod pd {
    pub const SPAWNER: usize = 0x10;
    pub const LIFE: usize = 0x14;
    pub const WORD: usize = 0x16;
    pub const CREATURE: usize = 0x18;
    pub const FLAGS: usize = 0x1c;
    pub const COOL: usize = 0x20;
    pub const SIZE: usize = 0x24;
}

/// The creatures a spawner names.
const CREATURES: usize = 18;

fn gone(w: &World, id: MobyId) -> bool { matches!(w.m(id).state, 0xfe | 0xfd) }

/// A moby pointer field of the pod (index + 1, 0 none).
fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    usize::try_from(v - 1).ok().filter(|&m| v > 0 && m < w.table.mobys.len())
}
fn set_ref(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }

/// Creature `i` of the spawner's list (a moby index; −1 none).
fn creature(w: &World, id: MobyId, i: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, sv::CREATURES + 4 * i)).ok().filter(|&m| m < w.table.mobys.len())
}

fn class_scale(w: &World, id: MobyId) -> f32 { crate::moby_update::classes::units::class_scale(w, w.m(id).o_class) }

/// The moby loop's run list (0x15ffe4).
fn run_list(w: &World) -> Vec<MobyId> { crate::moby_update::scheduler::build_active_list(w.table, w.camera, &w.svc.groups).0 }

/// The live pods of the run list.
fn live_pods(w: &World) -> u8 { run_list(w).into_iter().filter(|&m| w.m(m).o_class == POD && !gone(w, m)).count() as u8 }

/// The spawner's count +0x86: the live creatures and the live pods.
fn count(w: &mut World, id: MobyId) {
    let n = (0..CREATURES).filter_map(|i| creature(w, id, i)).filter(|&m| !gone(w, m)).count() as u8;
    let n = n.wrapping_add(live_pods(w));
    c::set_pu8(w, id, sv::COUNT, n);
}

/// The spawner's hits (module table).
fn hit(w: &mut World, id: MobyId) {
    let h = w.get_hit(id, 0xc3_0000, false);
    let r = damage::resolve(w, id, h, sv::DAMAGE, 0, 4);
    if r.out5 >= 2 {
        let dmg = r.hit.or(h).map_or(0.0, |h| h.damage.to_f32());
        let hp = c::pf(w, id, sv::DAMAGE) - dmg;
        c::set_pf(w, id, sv::DAMAGE, hp);
        if 0.0 < hp {
            c::set_pu8(w, id, sv::FLASH + 7, 90);
        } else {
            set_death_bits(w, id, 0, -1);
            w.mm(id).state = 8;
            c::set_pu8(w, id, sv::FLASH + 7, 0x78);
        }
        flash::start(w, id, sv::FLASH);
    }
}

/// Level03 0x2bef68, the spawner (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < sv::SIZE { return; }
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(w.m(id).position, cam) < 28.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x16;
        }
    }
    count(w, id);
    let pos = w.m(id).position;
    let hero = crate::moby_update::classes::units::hero_pos(w);
    let d = c::dist3(pos, hero);
    let near = d < c::pf(w, id, sv::NEAR);
    let range = c::pf(w, id, sv::RANGE);
    let mut t = c::pi16(w, id, sv::TIMER);
    let ready = fast_dec_timer_s16(&mut t) != 0;
    c::set_pi16(w, id, sv::TIMER, t);
    let mut room = c::pu8(w, id, sv::COUNT) < c::pu8(w, id, sv::MAX);
    let cub = c::pi32(w, id, sv::CUBOID);
    let inside = cub != -1 && crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [hero[0], hero[1], hero[2]], cub);
    let trigger = match c::pu8(w, id, sv::TRIGGER) {
        0 => near,
        1 => inside,
        _ => inside || d < range,
    };
    if c::pi16(w, id, sv::INTERVAL) == 0 && c::pu8(w, id, sv::COUNT) != 0 { room = false; }
    hit(w, id);
    w.mm(id).hit_slot = 0xff;
    let mut fire = false;
    match w.m(id).state {
        0 => {
            for i in 0..CREATURES {
                if let Some(m) = creature(w, id, i).filter(|&m| !gone(w, m)) { w.delete_moby(m); }
            }
            w.mm(id).mode |= mode::TARGETABLE;
            c::set_pu8(w, id, 0x5a, 3);
            c::set_pu8(w, id, 0x58, 8);
            c::set_pu8(w, id, sv::DAMAGE + 9, 0);
            w.mm(id).state = 1;
        }
        1 => {
            if near {
                w.mm(id).state = match c::pu8(w, id, sv::PICK) {
                    0 | 3 => 2,
                    1 => 3,
                    2 => 7,
                    4 | 5 => 5,
                    _ => 1,
                };
            }
        }
        2..=7 => fire = true,
        8 => {
            let at = w.m(id).position;
            fx::beam_explosion(w, &pod_launcher::BLAST, Some(id), at);
            let (pos, rot) = (w.m(id).position, w.m(id).rotation);
            for cl in PIECES { fx::break_piece(w, id, cl, pos, rot, 0, 0); }
            w.delete_moby(id);
            return;
        }
        _ => w.mm(id).state = 1,
    }
    if fire && trigger && ready && room { launch(w, id); }
    flash::update(w, id, sv::FLASH);
}

/// Level03 0x2beba0(c, out): a point in cuboid `c` (three `randf(−1, 1)` through its rows, + its centre); None for −1
/// (no draws).
pub fn cuboid_point(w: &mut World, cub: i32) -> Option<[f32; 4]> {
    if cub == -1 { return None; }
    let r = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0)];
    let s = usize::try_from(cub).ok().and_then(|i| w.svc.volumes.cuboids.get(i).copied())?;
    let m = s.matrix;
    Some(std::array::from_fn(|k| if k < 3 { r[0] * m[0][k] + r[1] * m[1][k] + r[2] * m[2][k] + m[3][k] } else { m[3][3] }))
}

/// The firing loop of 0x2bef68 (module table).
fn launch(w: &mut World, id: MobyId) {
    let mut used = [false; 4];
    // The game's stack vectors, kept across the loop (an aim that sets nothing reuses them).
    let (mut v, mut p, mut q) = ([0.0f32; 4], [0.0f32; 4], [0.0f32; 4]);
    loop {
        if c::pi16(w, id, sv::INTERVAL) == 0 {
            if c::pu8(w, id, sv::MAX) <= c::pu8(w, id, sv::COUNT) { break; }
        } else if c::pi16(w, id, sv::TIMER) != 0 {
            break;
        }
        let start = w.joint_point(id, 0);
        let pos = w.m(id).position;
        let hero = crate::moby_update::classes::units::hero_pos(w);
        let flags = c::pu8(w, id, sv::FLAGS);
        let cuboids: [i32; 4] = std::array::from_fn(|i| c::pi32(w, id, sv::CUBOIDS + 4 * i));
        let ground = |w: &mut World, v: [f32; 4]| {
            let mut p = [start[0] + v[0], start[1] + v[1], start[2] + v[2], start[3]];
            let gz = w.ground_height(pf(0.5), pv(p), 0).to_f32();
            if gz != 0.0 { p[2] = gz; }
            p
        };
        match c::pu8(w, id, sv::AIM) {
            0 => {
                let n = c::pu8(w, id, sv::COUNT) as f32;
                let mx = (c::pu8(w, id, sv::MAX) as i32 + 1) as f32;
                let a = wrap_frac((n + n) * PI / mx - PI);
                let mut r = c::dist3(pos, hero) * 0.5;
                if flags & 8 != 0 { r = w.rng.randf(r, 3.0); }
                if r < 3.0 { r = 3.0; }
                let (cs, sn) = (fast_cos(a), fast_sin(a));
                v = c::clamp_len3([cs * r, sn * r, 0.0, v[3]], c::pf(w, id, sv::RANGE));
                p = ground(w, v);
            }
            1 => {
                v = c::sub(hero, start);
                v[2] = 0.0;
                v = c::scale(v, 0.5);
                if flags & 8 != 0 {
                    let l = w.rng.randf(c::len3(v), 3.0);
                    v = c::set_len3(v, l);
                }
                if c::len3(v) < 3.0 { v = c::set_len3(v, 3.0); }
                v = c::clamp_len3(v, c::pf(w, id, sv::RANGE));
                p = ground(w, v);
            }
            aim @ 2..=4 => {
                if aim == 2 {
                    let mut best = 99840.0;
                    let mut pick = None;
                    for i in 0..4 {
                        if used[i] { continue; }
                        let Some(t) = cuboid_point(w, cuboids[i]) else { continue };
                        let dd = c::dist3(pos, hero);
                        if dd < best {
                            q = t;
                            best = dd;
                            pick = Some(i);
                        }
                    }
                    if let Some(i) = pick { used[i] = true; }
                } else {
                    let n = cuboids.iter().filter(|&&cb| cb != -1).count() as i32;
                    if n != 0 {
                        let mut k = if aim == 3 { w.rng.rand() % n + 1 } else { c::pu8(w, id, sv::COUNT) as i32 % n + 1 };
                        for &cb in &cuboids {
                            if cb != -1 { k -= 1; }
                            if k == 0 {
                                if let Some(t) = cuboid_point(w, cb) {
                                    q = t;
                                    break;
                                }
                            }
                        }
                    }
                }
                v = c::sub(q, start);
                if flags & 8 != 0 {
                    let l = w.rng.randf(c::len3(v), 3.0);
                    v = c::set_len3(v, l);
                }
                p = q;
            }
            _ => {}
        }
        let len = c::len3(v);
        v[2] = 0.0;
        let s = len * c::pf(w, id, sv::LOB) * SPEED;
        v = c::set_len3(v, s);
        let mut off = c::set_len3(v, 0.05);
        fx::jitter(w, 0.025, &mut off);
        q = [off[0] + start[0], off[1] + start[1], off[2] + start[2], off[3]];
        let mut time = 0.0;
        v[2] = knock::lob_up(s, -(c::pf(w, id, sv::GRAVITY) * DT2), q, p, &mut time);
        let word = c::pi16(w, id, sv::WORD);
        if let Some(pod) = spawn_pod(w, id, q, v, word, flags) { w.mm(pod).update_dist = 0xff; }
        let n = c::pi16(w, id, sv::INTERVAL) as i32;
        let t = w.ticks(n);
        c::set_pi16(w, id, sv::TIMER, t as i16);
        let cnt = c::pu8(w, id, sv::COUNT).wrapping_add(1);
        c::set_pu8(w, id, sv::COUNT, cnt);
    }
}

fn fast_cos(a: f32) -> f32 { crate::hero::physics::fast_cos(crate::ps2v::Pf::f(a)).to_f32() }
fn fast_sin(a: f32) -> f32 { crate::hero::physics::fast_sin(crate::ps2v::Pf::f(a)).to_f32() }

/// Level03 0x2c45d8(spawner, start, velocity, word, flags): a pod (module table). None when the table is full.
pub fn spawn_pod(w: &mut World, spawner: MobyId, start: [f32; 4], vel: [f32; 4], word: i16, flags: u8) -> Option<MobyId> {
    let e = w.create_moby(POD)?;
    if w.m(e).pvars.len() < pd::SIZE { return Some(e); }
    set_ref(w, e, pd::SPAWNER, Some(spawner));
    w.mm(e).position = start;
    c::set_pv4(w, e, 0, vel);
    let rx = w.rng.randf(-PI, PI);
    w.mm(e).rotation[0] = rx;
    let ry = w.rng.randf(-PI, PI);
    w.mm(e).rotation[1] = ry;
    set_ref(w, e, pd::CREATURE, None);
    let t = w.ticks(500);
    c::set_pi16(w, e, pd::LIFE, t as i16);
    c::set_pi16(w, e, pd::WORD, word);
    let (ud, dd) = (w.m(spawner).update_dist, w.m(spawner).draw_dist);
    let cs = class_scale(w, e);
    {
        let m = w.mm(e);
        m.update_dist = ud;
        m.visible = 1;
        m.draw_dist = dd;
        m.state = 0;
        m.scale = cs * 0.001;
    }
    c::set_pi32(w, e, pd::FLAGS, flags as i32);
    c::set_pi32(w, e, pd::COOL, 0);
    w.build_matrix(e);
    Some(e)
}

/// Level03 0x2bec60(spawner, p, −1): the first deleted creature revived at `p`; the count +0x86 redone (module table).
pub fn hatch(w: &mut World, spawner: MobyId, p: [f32; 4]) -> Option<MobyId> {
    if w.m(spawner).pvars.len() < sv::SIZE { return None; }
    let mut n = 0u8;
    let mut revived = None;
    let grow = c::pu8(w, spawner, sv::FLAGS) & 2 != 0;
    let dists = (w.m(spawner).update_dist, w.m(spawner).draw_dist);
    for i in 0..CREATURES {
        let Some(m) = creature(w, spawner, i) else { continue };
        if gone(w, m) {
            if revived.is_none() {
                revived = Some(m);
                n = n.wrapping_add(1);
                pod_launcher::revive(w, spawner, m, p, grow, dists);
            }
        } else {
            n = n.wrapping_add(1);
        }
    }
    let n = n.wrapping_add(live_pods(w));
    c::set_pu8(w, spawner, sv::COUNT, n);
    revived
}

/// Level03 0x2c4718, the pod (module doc).
pub fn pod_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pd::SIZE { return; }
    let cool = c::pi32(w, id, pd::COOL);
    if cool != 0 { c::set_pi32(w, id, pd::COOL, cool.max(1) - 1); }
    let old = w.m(id).position;
    match w.m(id).state {
        1 => {
            let gz = w.ground_height(pf(0.5), pv(w.m(id).position), 0).to_f32();
            {
                let m = w.mm(id);
                m.position[3] = 0.0;
                m.position[2] = gz + 0.2;
            }
            let at = w.m(id).position;
            let h = moby_ref(w, id, pd::SPAWNER).and_then(|s| hatch(w, s, at));
            set_ref(w, id, pd::CREATURE, h);
            w.mm(id).state = 2;
        }
        0 => fly(w, id, old),
        2 => {
            let f = SPEED * 0.05;
            let s = w.m(id).scale;
            w.mm(id).scale = s - s * f;
            let h = moby_ref(w, id, pd::CREATURE);
            let grow = c::pi32(w, id, pd::FLAGS) & 4 != 0;
            if let Some(h) = h.filter(|_| grow) {
                let (hs, hc) = (w.m(h).scale, class_scale(w, h));
                w.mm(h).scale = hs + (hc - hs) * f;
            }
            let cs = class_scale(w, id);
            if w.m(id).scale < cs * f {
                w.mm(id).scale = 0.0001;
                let done = match h {
                    None => true,
                    Some(h) => gone(w, h) || !grow || class_scale(w, h) - 0.01 <= w.m(h).scale,
                };
                if done {
                    w.delete_moby(id);
                    return;
                }
            }
        }
        _ => {}
    }
    let cs = class_scale(w, id);
    let s = w.m(id).scale * 0.3 / cs;
    crate::shadows::blob(w, s, id);
    let p = w.m(id).position;
    if !(5.0..=500.0).contains(&p[2]) {
        w.delete_moby(id);
        return;
    }
    let m = w.mm(id);
    for k in 0..3 { m.position[k] = m.position[k].clamp(5.0, 1018.0); }
}

/// Pod state 0 of 0x2c4718 (module table).
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
    let spawner = moby_ref(w, id, pd::SPAWNER);
    let saved = |w: &World, m: MobyId| (w.m(m).coll_disable, w.m(m).has_collision);
    let (own, theirs) = (saved(w, id), spawner.map(|s| saved(w, s)));
    for m in std::iter::once(id).chain(spawner) {
        let mm = w.mm(m);
        mm.coll_disable = u32::MAX;
        mm.has_collision = false;
    }
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
    for e in run_list(w) {
        if e == id || gone(w, e) { continue; }
        let pos = w.m(id).position;
        let ep = w.m(e).position;
        let d = [pos[0] - ep[0], pos[1] - ep[1], 0.0, pos[3]];
        let l = c::len3(d);
        if l < 1.8 {
            let k = (1.8 - l) * (SPEED * -0.5 + 1.0);
            let m = w.mm(id);
            for j in 0..3 { m.position[j] += d[j] * k; }
        }
    }
    let mut t = c::pi16(w, id, pd::LIFE);
    if fast_dec_timer_s16(&mut t) != 0 { w.mm(id).state = 2; }
    c::set_pi16(w, id, pd::LIFE, t);
    let vel = c::pv4(w, id, 0);
    {
        let m = w.mm(id);
        m.rotation[0] = wrap_frac(vel[0]);
        m.rotation[1] = wrap_frac(vel[1]);
        (m.coll_disable, m.has_collision) = own;
    }
    if let (Some(s), Some(v)) = (spawner, theirs) { (w.mm(s).coll_disable, w.mm(s).has_collision) = v; }
    c::set_pf(w, id, 8, vel[2] - DT2 * 9.8);
    let m = w.mm(id);
    m.rotation[0] += SPEED * 0.01;
    m.rotation[1] += SPEED * 0.02;
}

/// A bounce of the pod (the velocity, then the knock sound when its cooldown is out).
fn bounce(w: &mut World, id: MobyId, o: &crate::collision_query::CollOutput) {
    pod_launcher::bounce_velocity(w, id, o, POD);
    if c::pi32(w, id, pd::COOL) == 0 {
        w.play_sound(0, 0, id);
        c::set_pi32(w, id, pd::COOL, 12);
    }
}
