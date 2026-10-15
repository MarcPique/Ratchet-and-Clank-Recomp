//! Batalia's circling fighters, class 438: level08 0x2de848 with its trail 0x2dec90 (census U274; 25 created
//! instances). Each flies its own closed path at 45 u/s with a fixed random height offset, leaving a trail of type-2
//! blobs while drawn; a hit with flag 0x10000 (the turret's shot) blows it up in the air and sends it tumbling down
//! under gravity until it hits the world (a second blast) or drops below z 10. Shot down while Ratchet is mounted
//! (hero state 0x32: Batalia's turret 440) the first time, it awards a skill point. Read from the level08 decomp and
//! the disassembly of both functions; the level data words from the level08 overlay (gp 0x166c00). Native `f32`.
//!
//! **Pvar block**: +0x00 the path (index into the level's paths, −1 none), +0x04 the path parameter (a fraction of
//! the path in the placement, points after state 0), +0x08 the step per tick, +0x0c the height offset, +0x10 the
//! fall velocity.
//!
//! | address | what | port |
//! |---|---|---|
//! | entry | the old position kept (the trail's and the blast's velocity) | [`update`] |
//! | state 0 | path −1 → `DeleteMoby`; step = 45 (gp−0x51bc)·dt / `VecDistance(p0, p1)` (0x221360; the second call, to the last point, is unused); t ·= count; offset = `randf_sym(0, 5)` (0x26ca28); +0x30 = 0xff; → 1, then state 1 | [`update`] |
//! | state 1 | `0x277d40(t, path, closed, &pos, &rot, 0)` (`path::pose`); rot.y ·= 0.5; z += offset; t += step; t > count → t −= count | [`update`] |
//! | state 2 | vel.z −= 15·dt²; rot.x = rot.x + `fast_add_rotations(rot.x, 7.330383·dt)` (the game's own sum, not a wrap); rot.y = −`FastArcTan(len2(vel), vel.z)`; pos += vel; z < 10 → delete; `coll_sphere(1.25, pos, 0, self)` (0x212960) hit → the blast, delete | [`fall`] |
//! | the blast | `SpawnBeamExplosion(0, 0, 10, 5, 9, 1, 15, m, vel, pos, 20, 40, 16, −1, 1, 1, −1, 0)` (0x273310): no damage sphere, 20 streaks, 40 spark pairs, 16 puffs, debris 1, shake, no sound | [`BLAST`] (`fx::beam_explosion`) |
//! | tail | +0x94 (collision) = 0 when x, y or z < 4, else the class's collision (class +0x10) | [`update`] |
//! | tail, hit | `MobyGetHitMessage(m, 0x10000, 0)` (0x26f320) and state ≠ 2: **Ratchet mounted (0x1413d4 = 0x32) and skill point 0x13d414 not earned → earned, `PlayLevelSoundAtMoby(1, 0, 0)`, `ShowBanner(0x53d6, −1)`**; vel = pos − old; vel.z += 2·dt; the blast; → 2 | [`update`]; the skill point through `story::award_skill_point` |
//! | tail | +0xa4 = 0xff (the hit record dropped) | [`update`] |
//! | 0x2dec90 (drawn, +0x31 ≠ 0) | d = pos − old; point = rows·(−2.3, 0, 0.3) + pos; v1 = 0.75·d, w `randf(0.333, 0.8)`; v2 = 0.5·d + `rand_vec(0, 1.5·dt)`, w `randf(1, 1.7)`; c1 = `FastTweenColor(randf(0, 1), 0x600080ff, 0x6020a0c0)`, c2 = `FastTweenColor(randf(0, 1), 0x20802040, 0x20601020)`; phases `trunc(scale(23·randf(0, 1) + 1))`, `trunc(scale(10·(randf(−.5, .5) + 1)))`, `trunc(scale(7·(randf(−.5, .5) + 1)))`; `PartType02Spawn(…, −1)` (0x27dc98); the record's byte 9 = 8 − 0x70 | [`trail`] |
//! | | no class sound, light, save flag besides the skill point; the blasts' own effects (`fx::beam_explosion`) | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{beam_explosion, Beam};
use crate::moby_update::creature::{add, add_rot, atan, pf, pi32, pv4, set_pf, set_pv4, sub, DT, DT2};
use crate::moby_update::services::{pf as to_pf, pv, World};

/// The update in the level08 class table.
pub const UPDATE_FN: u32 = 0x2d_e848;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 1] = [438];

/// The hit flag that shoots one down.
pub const SHOT_FLAG: u32 = 0x1_0000;
/// Level08 gp−0x51bc: the speed along the path (u/s).
pub const SPEED: f32 = 45.0;
/// The height offset's range (`randf_sym(0, 5)`).
const OFFSET: f32 = 5.0;
/// The fall's gravity (·dt²) and the tumble (rad/s).
const GRAVITY: f32 = 15.0;
const TUMBLE: f32 = f32::from_bits(0x40ea_927f);
/// Below this height a falling one is deleted.
const FLOOR: f32 = 10.0;
/// The crash sphere.
const CRASH_R: f32 = 1.25;
/// The collision is off below 4 on any axis.
const EDGE: f32 = 4.0;

/// Both blasts (module doc).
pub const BLAST: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 10.0, flash2: 5.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 20, sparks: 40, puffs: 16, debris: 1, sound: -1, shake: true };

mod pvo {
    pub const PATH: usize = 0x00;
    pub const T: usize = 0x04;
    pub const STEP: usize = 0x08;
    pub const OFFSET: usize = 0x0c;
    pub const VEL: usize = 0x10;
}

/// The trail's level data (level08 gp−0x51b8..−0x5180).
mod trail_k {
    pub const V1: f32 = 0.75;
    pub const V2: f32 = 0.5;
    pub const JITTER: f32 = 1.5;
    pub const T: [i32; 3] = [23, 10, 7];
    pub const T_SPREAD: f32 = 0.5;
    pub const W1: (f32, f32) = (f32::from_bits(0x3eaa_7efa), f32::from_bits(0x3f4c_cccd));
    pub const W2: (f32, f32) = (1.0, f32::from_bits(0x3fd9_999a));
    pub const C1: (u32, u32) = (0x6000_80ff, 0x6020_a0c0);
    pub const C2: (u32, u32) = (0x2080_2040, 0x2060_1020);
    pub const OFFSET: [f32; 4] = [f32::from_bits(0xc013_3333), 0.0, f32::from_bits(0x3e99_999a), 0.0];
    /// The blob row (`super::super::engine_trail`): byte 9 = trunc(8) − 0x70.
    pub const BLOB: super::super::engine_trail::Blob = super::super::engine_trail::Blob { k1: V1, k2: V2, jitter: JITTER, w1: W1, w2: W2, c1: C1, c2: C2, t: T, spread: T_SPREAD, byte9: -0x70, size_div: 1.0 };
}

fn len2(v: [f32; 4]) -> f32 { (v[0] * v[0] + v[1] * v[1]).sqrt() }
fn dist3(a: [f32; 4], b: [f32; 4]) -> f32 { let d = sub(a, b); (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() }

/// Level08 0x2de848 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let old = w.m(id).position;
    let path_i = pi32(w, id, pvo::PATH);
    let pts = usize::try_from(path_i).ok().and_then(|i| w.svc.splines.get(i)).cloned().unwrap_or_default();
    let count = pts.len() as i32 as f32;
    let mut state = w.m(id).state;
    if state == 0 {
        if path_i == -1 { w.delete_moby(id); return; }
        let q = |k: usize| pts.get(k).map(|p| p.map(f32::from_bits)).unwrap_or([0.0; 4]);
        let d = dist3(q(0), q(1));
        set_pf(w, id, pvo::STEP, SPEED * DT / d);
        let t = pf(w, id, pvo::T) * count;
        set_pf(w, id, pvo::T, t);
        let off = w.rng.randf_sym(0.0, OFFSET);
        set_pf(w, id, pvo::OFFSET, off);
        let m = w.mm(id);
        m.state = 1;
        m.update_dist = 0xff;
        state = 1;
    }
    match state {
        1 => {
            let t = pf(w, id, pvo::T);
            let (pos, rot) = crate::path::pose(&pts, true, t, true);
            let off = pf(w, id, pvo::OFFSET);
            let m = w.mm(id);
            m.position = pos;
            m.rotation = rot;
            m.rotation[1] *= 0.5;
            m.position[2] += off;
            let t = t + pf(w, id, pvo::STEP);
            set_pf(w, id, pvo::T, if count < t { t - count } else { t });
        }
        2 if !fall(w, id) => return,
        _ => {}
    }
    // The tail.
    let p = w.m(id).position;
    let coll = !(p[0] < EDGE || p[1] < EDGE || p[2] < EDGE) && super::class_collision(w, w.m(id).o_class);
    w.mm(id).has_collision = coll;
    if w.get_hit(id, SHOT_FLAG, false).is_some() && w.m(id).state != 2 {
        // Shot down from the turret (Ratchet in state 0x32): skill point 0x13d414.
        if w.hero.state == 0x32 { crate::moby_update::story::award_skill_point(w, crate::moby_update::story::skill_index(0x13_d414)); }
        let mut v = sub(p, old);
        v[2] += DT + DT;
        set_pv4(w, id, pvo::VEL, v);
        beam_explosion(w, &BLAST, Some(id), p);
        w.mm(id).state = 2;
    }
    w.mm(id).hit_slot = 0xff;
    if w.m(id).visible != 0 { trail(w, id, old); }
}

/// State 2 (module doc). False when it was deleted.
fn fall(w: &mut World, id: MobyId) -> bool {
    let mut v = pv4(w, id, pvo::VEL);
    v[2] -= DT2 * GRAVITY;
    set_pv4(w, id, pvo::VEL, v);
    let rx = w.m(id).rotation[0];
    let r = add_rot(rx, DT * TUMBLE);
    w.mm(id).rotation[0] = rx + r;
    w.mm(id).rotation[1] = -atan(len2(v), v[2]);
    let p = add(w.m(id).position, v);
    w.mm(id).position = p;
    if p[2] < FLOOR {
        w.delete_moby(id);
        return false;
    }
    if w.coll_sphere(pv(p), to_pf(CRASH_R), 0, Some(id)).is_some() {
        beam_explosion(w, &BLAST, Some(id), p);
        w.delete_moby(id);
        return false;
    }
    true
}

/// `0x2dec90(m, old)`: one trail blob (module doc; the shared shape `super::engine_trail`).
fn trail(w: &mut World, id: MobyId, old: [f32; 4]) {
    use trail_k as k;
    let m = w.m(id);
    let d = sub(m.position, old);
    let r = &m.rows;
    let o = k::OFFSET;
    let local: [f32; 4] = std::array::from_fn(|l| if l == 3 { 0.0 } else { r[0][l] * o[0] + r[1][l] * o[1] + r[2][l] * o[2] });
    let point = add(local, m.position);
    super::engine_trail::blob(w, &k::BLOB, point, d);
}
