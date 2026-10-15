//! **Kalebo III's hover racers, class 556** (level16 `0x2d04a0`, census U503; five placed at the race start, named by
//! the board's pvars +0x2c..+0x3c). The level's copy of Rilgar's racers ([`super::rilgar_racer`], whose waypoint,
//! pickup, jump and jump-arc code they share through its [`Layout`]), but they hover themselves (no board of their
//! own), pitch with the track, trail exhaust from two joints, and can be shot: a hit blows them up, adds 250 to
//! Ratchet's race score, and they come back where they fell once out of the camera's view. Read from the level16
//! decomp. Native `f32`.
//!
//! **Pvars** (0x300): +0x29 / +0x2b two bytes of the first block (0 / 1 at the start), +0x60 / +0xe0 / +0x160 / +0x1e0
//! four look-at records (`manip::look`, joint lists 0 / 2 / 1 / 3), +0x260 the point on the track, +0x270 the jump's
//! landing point, +0x280 last tick's position, +0x290 the racing line, +0x294.. the branches (by a branch node's
//! w − 11), +0x2b0 the start lane, +0x2b4 / +0x2b8 the speed and boosted speed, +0x2bc the speed, +0x2c0 the current
//! path (the game's pointer; the port's spline index), +0x2c4 the yaw's spring velocity, +0x2c8 the vertical speed,
//! +0x2cc the racer's number, +0x2dc the pickups' cooldown, +0x2e4 the jump's ticks, +0x2e8 the bob's phase,
//! +0x2f0 (s16) the jump's landing waypoint, +0x2f2 (s16) the ticks since the blast, +0x2f4 (s16) the waypoint,
//! +0x2f6 (s16) the boost timer, +0x2f8 the pitch's spring velocity, +0x2fe (s16) the trick (−1 none).
//!
//! ## Coverage (`0x2d04a0`)
//! | address | what | port |
//! |---|---|---|
//! | top | the big-head cheat on the head's look-at record (2.5, +0x1e0) | [`update`] (`manip::big_head_scale`) |
//! | state 0 | update distance 0x18, hidden (+0x34 \|= 0x41); the start lane's point 0 (`0x2cf9d0`) is the position and the track point; facing along the lane; +0x2b = 1; speeds 17·dt / 24·dt + number·1.55·dt, slowed by 0.8% per finished race past the fifth (gp−0x7dc4) until the Hologuise is owned (0x13d4df), ≥ 80%; the bob's phase random; +0x29 = 0; → 1 | [`update`] |
//! | state 1 | Ratchet on his board (group 0x16): update distance 0xff, the class collision, shown, waypoint 0, boost ticks(180), the motion cleared; odd numbers mirrored (+0x34 \|= 0x8000); hit slot 0xff; → 2 | [`update`] |
//! | state 2 | off the board → 0; a hit (flags 0x30000): `SpawnBeamExplosion(0, 0, 2.5, 1.5, 4, 1, 7, m, …, pos + 0.5 up, 5, 15, 25, −1, 0, 9)`, the break pieces 0x655 + 2 × 0x656 and 0x6fa + 0x6fb, race score 0x13fbf8 += 250, no collision, hidden → 3; else the ride ([`ride`]) | [`update`] |
//! | state 3 | off the board → 0; after ticks(240), once its position is more than 90° off the camera's yaw (0x1671c0 / 0x1671d8): the collision, shown, hit slot 0xff → 2 | [`update`] |
//! | ride | the waypoint (`0x2cf9f8`: a racing-line node with w > 10 branches on `randi(100) > 30`), the pickups (`0x2cfc00`), the jumps (`0x2cff48`, on any path), the jump's arc; the anims (`0x2d0058`); the yaw springs toward the waypoint (0.035, 0.3, 400°/s); the track point moves by the speed along the facing; crates within 0.7 of 3 steps ahead + 0.7 up get a hit (flags 0x30000); gravity and the landing on `GroundHeight(0.5)`; the speed (as Rilgar's, but − dt when ahead of Ratchet); the lean records; the position = the track point + 0.25 + the bob | [`ride`] |
//! | ride, tail | the exhaust: the distance moved since last tick; joint list 4 or 5 by the tick's parity (`0x24aea0`); one `randf` angle (its sideways offsets are overwritten: the velocity is the distance moved, w 120000); boosting: a type-4 puff (0x5032f0d2, ticks(15..18), 30 / 20) at 0.92 × the move and a type-21 spark (15000, ticks(8..17)); else a type-4 puff (gp−0x51f8 / −0x51f4, ticks(10..13), 15 / 15) | [`exhaust`] |
//! | `0x2d0058` | the pitch springs toward the track's slope to the next waypoint (0 in a jump; 0.02, 0.3, 180°/s); idle 0 (back to 0 at a wrap); the jump 5 frame 4; a trick 1..3 frame 3 when the landing is more than ticks(27) away; the landing 5 frame 0x33 (under ticks(5) to go) or 0x1c (at a wrap); the trick's speed 30 / t (0.3..1.7), the jump's (50 − key time) / t | [`anims`] |

#![allow(clippy::neg_cmp_op_on_partial_ord)] // The game's compares, NaN included.

use super::rilgar_racer::{self as r, Layout, DT, DT2};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, ground, turn};
use crate::moby_update::manip;
use crate::moby_update::services::{HeroCall, HitTemplate, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2d_04a0;
pub const CLASSES: [i16; 1] = [556];

mod pv {
    pub const B29: usize = 0x29;
    pub const B2B: usize = 0x2b;
    pub const TRACK: usize = 0x260;
    pub const PREV: usize = 0x280;
    pub const LANE: usize = 0x2b0;
    pub const PATH: usize = 0x2c0;
    pub const NUMBER: usize = 0x2cc;
    pub const PHASE: usize = 0x2e8;
    pub const BLAST: usize = 0x2f2;
    pub const PITCH_VEL: usize = 0x2f8;
    pub const TRICK: usize = 0x2fe;
    pub const LEN: usize = 0x300;
}

const LAYOUT: Layout = Layout {
    track: pv::TRACK, land: 0x270, line: 0x290, branches: 0x294, base: 0x2b4, boosted: 0x2b8, speed: 0x2bc, path: pv::PATH,
    yaw_vel: 0x2c4, vz: 0x2c8, wp: 0x2f4, boost: 0x2f6, cooldown: 0x2dc, jump: 0x2e4, land_wp: 0x2f0,
    land_wp_s16: true, branch_min: 10.0, branch_chance: 30, branch_first: 11, jump_off_line: false,
};
const L: &Layout = &LAYOUT;

/// The Hologuise (item 31, 0x13d4df): the prize that stops the racers slowing down.
const HOLOGUISE: usize = 31;
/// gp−0x51f8 / gp−0x51f4 (0x161a08 / 0x161a0c): the exhaust's colours.
const EXHAUST_C1: u32 = 0x4f00_90f0;
const EXHAUST_C2: u32 = 0x4f00_00f0;
/// The blast: `SpawnBeamExplosion(0, 0, 2.5, 1.5, 4, 1, 7, m, 0x15f580, pos, 5, 15, 25, −1, 0, 9, −1, 0)`.
const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.5, flash2: 1.5, flash_dist: 4.0, scale: 1.0, light: 7.0, streaks: 5, sparks: 15, puffs: 25, debris: 9, sound: -1, shake: false };

/// Level16 `0x2d04a0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::LEN);
    manip::big_head_scale(w, 2.5, id, 0x1e0);
    let on_board = w.hero.group == crate::hero::hoverboard::GROUP;
    match w.m(id).state {
        0 => {
            {
                let m = w.mm(id);
                m.update_dist = 0x18;
                m.mode |= 0x41;
            }
            let lane = c::pi32(w, id, pv::LANE);
            c::set_pi32(w, id, pv::PATH, lane);
            let p0 = r::point(w, id, L, 0);
            w.mm(id).position = p0;
            r::set_track(w, id, L, p0);
            let p1 = r::point(w, id, L, 1);
            let m = w.mm(id);
            m.rotation[0] = 0.0;
            m.rotation[1] = 0.0;
            m.rotation[2] = r::atan(p1[0] - p0[0], p1[1] - p0[1]);
            c::set_pu8(w, id, pv::B2B, 1);
            let k = c::pi32(w, id, pv::NUMBER) as f32 * DT * 1.55;
            let (mut fast, mut base) = (DT * 24.0 + k, DT * 17.0 + k);
            if !w.inventory.owned(HOLOGUISE) {
                let n = ((w.svc.board.records.finished[1] - 5) as f32).max(0.0);
                let f = (1.0 - n * 0.008).max(0.8);
                fast *= f;
                base *= f;
            }
            c::set_pf(w, id, L.boosted, fast);
            c::set_pf(w, id, L.base, base);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            c::set_pu8(w, id, pv::B29, 0);
            w.mm(id).state = 1;
        }
        1 => {
            if !on_board { return; }
            let coll = super::class_collision(w, w.m(id).o_class);
            {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.has_collision = coll;
                m.mode &= 0xffbe;
            }
            c::set_pi16(w, id, L.wp, 0);
            let t = w.ticks(0xb4) as i16;
            c::set_pi16(w, id, L.boost, t);
            for o in [L.vz, L.yaw_vel, L.jump, L.cooldown, L.speed] { c::set_pi32(w, id, o, 0); }
            if c::pi32(w, id, pv::NUMBER) % 2 == 1 { w.mm(id).mode |= 0x8000; }
            let m = w.mm(id);
            m.hit_slot = 0xff;
            m.state = 2;
        }
        2 => {
            if !on_board {
                w.mm(id).state = 0;
                return;
            }
            if w.get_hit(id, 0x3_0000, false).is_some() {
                blast(w, id);
                return;
            }
            w.mm(id).hit_slot = 0xff;
            ride(w, id);
        }
        3 => {
            if !on_board {
                w.mm(id).state = 0;
                return;
            }
            let n = c::pi16(w, id, pv::BLAST).wrapping_add(1);
            c::set_pi16(w, id, pv::BLAST, n);
            if n as i32 <= w.ticks(0xf0) { return; }
            let (p, cam) = (c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)));
            let a = r::atan(p[0] - cam[0], p[1] - cam[1]);
            if c::diff_rots(a, w.camera_yaw) <= std::f32::consts::FRAC_PI_2 { return; }
            let coll = super::class_collision(w, w.m(id).o_class);
            let m = w.mm(id);
            m.state = 2;
            m.has_collision = coll;
            m.mode &= 0xffbe;
            m.hit_slot = 0xff;
        }
        _ => {}
    }
}

/// State 2's hit (module doc): the blast, the pieces, the score; hidden until it respawns (state 3).
fn blast(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    fx::beam_explosion(w, &BLAST, Some(id), [p[0], p[1], p[2] + 0.5, p[3]]);
    let pieces = crate::moby_update::classes::breakables::burst_pieces;
    pieces(w, id, 0x655, 1, 0x656, 2, 2, 0);
    pieces(w, id, 0x6fa, 1, 0x6fb, 1, 1, 0);
    c::set_pi16(w, id, pv::BLAST, 0);
    w.hero_fields_mut().call(HeroCall::RaceScore(250));
    let m = w.mm(id);
    m.state = 3;
    m.has_collision = false;
    m.mode |= 0x41;
}

/// State 2's ride (module doc).
fn ride(w: &mut World, id: MobyId) {
    r::advance(w, id, L);
    r::pickups(w, id, L);
    r::jumps(w, id, L);
    r::jump_arc(w, id, L);
    anims(w, id);
    let q = r::point(w, id, L, c::pi16(w, id, L.wp) as i32);
    let pos = c::pos(w, id);
    let to = r::atan(q[0] - pos[0], q[1] - pos[1]);
    {
        let (mut a, mut v) = (Pf::f(c::yaw(w, id)), Pf::f(c::pf(w, id, L.yaw_vel)));
        crate::hero::physics::turn_spring(Pf::f(to), Pf::f(0.035), Pf::f(0.3), Pf::f(DT * 6.981_317), &mut a, &mut v, 0);
        c::set_yaw(w, id, a.to_f32());
        c::set_pf(w, id, L.yaw_vel, v.to_f32());
    }
    let mut p = r::track(w, id, L);
    if p[2] < q[2] - 5.0 { p[2] = q[2]; }
    let yaw = c::yaw(w, id);
    let speed = c::pf(w, id, L.speed);
    let v = [yaw.cos() * speed, yaw.sin() * speed, 0.0, 0.0];
    p = [p[0] + v[0], p[1] + v[1], p[2] + v[2], p[3]];
    // Crates ahead.
    let centre = [v[0] * 3.0 + p[0], v[1] * 3.0 + p[1], v[2] * 3.0 + p[2] + 0.7, p[3]];
    let pv4 = crate::moby_update::services::pv;
    let listed = crate::moby_update::services::sphere_mobys_in(w.table, w.svc, w.classes, Pf::f(0.7), pv4(centre), 0, Some(id), None);
    if !listed.is_empty() {
        let dir = pv4([v[0] * 7.0, v[1] * 7.0, v[2] * 7.0, 0.0]);
        for m in listed {
            if (500..=540).contains(&w.m(m).o_class) {
                let t = HitTemplate { dir, attacker: Some(id), flags: 0x3_0000, damage: Pf::ONE, w20: 1, ..Default::default() };
                w.deliver_hit(m, &t);
            }
        }
    }
    // The ground.
    let gz = w.ground_height(Pf::b(0x3f00_0000), pv4(p), 0).to_f32();
    let dz = p[2] - gz;
    let mut vz = c::pf(w, id, L.vz);
    if 0.0 < dz || 0.0 < vz {
        vz -= DT2 * 21.0;
    } else if dz.abs() < 0.5 {
        vz = 0.0;
        p[2] = gz;
        c::set_pi32(w, id, L.jump, 0);
    }
    p[2] += vz;
    c::set_pf(w, id, L.vz, vz);
    // The speed.
    let mut speed = c::pf(w, id, L.speed);
    if c::dec_timer_pvar_s16(w, id, L.boost) == 0 {
        turn::approach(c::pf(w, id, L.boosted), DT2 * 20.0, &mut speed);
    } else {
        let mut t = c::pf(w, id, L.base);
        if gz < p[2] { t = DT * 19.0; }
        if w.hero.board.race_ticks < w.ticks(700) { t += DT * 3.0; }
        let n = (c::pi32(w, id, pv::NUMBER) as usize).min(7);
        if (w.hero.board.racer_place[n] as i32) < w.hero.board.place { t -= DT; } else { t += DT; }
        let rate = if t < speed { 7.0 } else { 15.0 };
        turn::approach(t, DT2 * rate, &mut speed);
    }
    c::set_pf(w, id, L.speed, speed);
    if dz < 0.2 && vz < 0.0 {
        p[2] = gz;
        c::set_pi32(w, id, L.jump, 0);
    }
    r::set_track(w, id, L, p);
    // The lean records.
    let lim = DT * 2.617_993_8;
    let mut lean = c::pf(w, id, L.yaw_vel).clamp(-lim, lim);
    if w.m(id).mode & 0x8000 != 0 { lean = -lean; }
    c::set_pf(w, id, 0x248, lean * 30.0);
    c::set_pf(w, id, 0xc8, lean * 20.0);
    c::set_pf(w, id, 0x1c8, lean * 50.0);
    c::set_pf(w, id, 0x144, lean * 40.0);
    manip::look(w, id, id, 0x1e0, 3, 0.02, 0.3);
    manip::look(w, id, id, 0x60, 0, 0.02, 0.3);
    manip::look(w, id, id, 0x160, 1, 0.02, 0.3);
    manip::look(w, id, id, 0xe0, 2, 0.015, 0.3);
    // The position: the track point, 0.25 up, the bob.
    let mut at = p;
    at[2] += 0.25;
    let phase = c::add_rot(c::pf(w, id, pv::PHASE), DT * 6.108_652);
    c::set_pf(w, id, pv::PHASE, phase);
    at[2] += phase.sin() * 0.08;
    w.mm(id).position = at;
    exhaust(w, id);
}

/// The ride's tail (module doc): the exhaust at joint list 4 or 5.
fn exhaust(w: &mut World, id: MobyId) {
    let pos = c::pos(w, id);
    let prev = c::pv4(w, id, pv::PREV);
    let moved = [pos[0] - prev[0], pos[1] - prev[1], pos[2] - prev[2], pos[3] - prev[3]];
    c::set_pv4(w, id, pv::PREV, pos);
    let at = w.joint_point(id, (w.counter & 1) as usize | 4);
    // The sideways offsets (row 2 · cos, row 1 · sin of this angle, dt / 4 long) are summed into the velocity and
    // then overwritten by the move: only the draw is left.
    let _ = w.rng.rand_angle();
    let vel = [moved[0], moved[1], moved[2], f32::from_bits(0x47ea_6000)];
    if c::pi16(w, id, L.boost) == 0 {
        let (a, b) = (w.ticks(10), w.ticks(0xd));
        let life = w.rng.rand_range(a, b);
        fx::part04(w, &crate::particles::type04::Spawn { pos: at, vel, c1: EXHAUST_C1, c2: EXHAUST_C2, life, base: 0xf, growth: 0xf, additive: true });
    } else {
        let slow = c::scale(moved, 0.92);
        let (a, b) = (w.ticks(0xf), w.ticks(0x12));
        let life = w.rng.rand_range(a, b);
        fx::part04(w, &crate::particles::type04::Spawn { pos: at, vel: slow, c1: 0x5032_f0d2, c2: EXHAUST_C2, life, base: 0x1e, growth: 0x14, additive: true });
        let (a, b) = (w.ticks(8), w.ticks(0x11));
        let life = w.rng.rand_range(a, b);
        fx::part21(w, 15000.0, at, vel, EXHAUST_C1, EXHAUST_C2, life, 1);
    }
}

/// Level16 `0x2d0058`: the pitch and the anims (module doc).
fn anims(w: &mut World, id: MobyId) {
    let jump = c::pi32(w, id, L.jump);
    let mut target = 0.0;
    if jump == 0 {
        let p = r::cur_path(w, id, L);
        let wp = c::pi16(w, id, L.wp) as i32;
        let n = r::step(p.len() as i32, wp, 1);
        let (a, b) = (r::pt(&p, wp), r::pt(&p, n));
        target = r::atan(r::dist2(a, b), b[2] - a[2]);
    }
    {
        let (mut a, mut v) = (Pf::f(w.m(id).rotation[1]), Pf::f(c::pf(w, id, pv::PITCH_VEL)));
        crate::hero::physics::turn_spring(Pf::f(target), Pf::f(0.02), Pf::f(0.3), Pf::f(DT * std::f32::consts::PI), &mut a, &mut v, 0);
        w.mm(id).rotation[1] = a.to_f32();
        c::set_pf(w, id, pv::PITCH_VEL, v.to_f32());
    }
    if jump == 0 {
        w.mm(id).anim.speed = 1.0;
        if w.m(id).anim.flags & 2 != 0 && w.m(id).anim.seq_b != 0 {
            let t = w.ticks(8);
            w.anim_blend(id, 0, 0, t);
        }
        return;
    }
    if w.m(id).anim.seq_b != 5 && jump < w.ticks(3) {
        let t = w.ticks(5);
        w.anim_blend(id, 5, 4, t);
        c::set_pi16(w, id, pv::TRICK, -1);
    }
    let g = DT2 * 21.0 * -0.5;
    let (n, root) = r::quad_first(g, c::pf(w, id, L.vz) - g, c::pf(w, id, L.track + 8) - c::pf(w, id, L.land + 8));
    let t = if 0 < n && 0.0 < root { (root as i32) as f32 } else { 30.0 };
    if w.ticks(7) < jump && jump < w.ticks(0xb) && w.m(id).anim.seq_b == 5 && (w.ticks(0x1b) as f32) < t {
        let k = w.rng.rand_range(0, 3) + 1;
        c::set_pi16(w, id, pv::TRICK, k as i16);
        let b = w.ticks(7);
        w.anim_blend(id, k as u8, 3, b);
    }
    if c::pi16(w, id, pv::TRICK) != -1 {
        let a = w.m(id).anim;
        if a.seq_a == a.seq_b {
            let f = ground::key_time(w, id);
            if 7.0 < f && f < 20.0 && t != 0.0 { w.mm(id).anim.speed = (30.0 / t).clamp(0.3, 1.7); }
        }
        if t < w.ticks(5) as f32 {
            let b = w.ticks(5);
            w.anim_blend(id, 5, 0x33, b);
            c::set_pi16(w, id, pv::TRICK, -1);
        } else if w.m(id).anim.flags & 2 != 0 {
            let b = w.ticks(7);
            w.anim_blend(id, 5, 0x1c, b);
            c::set_pi16(w, id, pv::TRICK, -1);
        }
    }
    let a = w.m(id).anim;
    if a.seq_a == a.seq_b && a.seq_a == 5 && t != 0.0 {
        let f = ground::key_time(w, id);
        w.mm(id).anim.speed = (50.0 - f) / t;
    }
}
