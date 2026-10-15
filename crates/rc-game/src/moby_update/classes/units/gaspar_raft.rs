//! **Gaspar's lava raft, class 1766** (level09 `0x309c08`, census U313; one placed: #819 on path 62, cuboids 27 /
//! 46). It waits at one end of its path, bobbing (0.14·, 30°/s) and rocking (5°, 18°/s / 28°/s), until Ratchet
//! stands on it (or, once ridden, walks into the cuboid of the far end: +0xb4 going forward, +0xb0 going back); then
//! it sails the path to the other end with its loop sound, its paddle wheel turning (manipulator +0x60 about x by the
//! distance sailed) and a wake on the lava (type 45) away from the ends, and rests there while he is on it.
//!
//! **States**: 0: update distance 0xff, the yaw offset +0xbc = π, the voice −1; with +0xa0 the manipulator on joint
//! list 0 and → 1 at the path's start (its mission done: at its end, going back); every tick of 0 the yaw + π. 1: the
//! rocking eases in (+0xcc → 1 by 1%); boarded once (+0xc8) and Ratchet in the far end's cuboid, or Ratchet on it
//! (+0xc8 = 1) → 3. 2: as 1 until Ratchet is off it → 1. 3: the loop sound (re-requested one tick in eight, by the
//! moby's slot); with the yaw offset 0 (it faces its way) Ratchet entering the cuboid of the end it came from, both
//! ends more than 8 away, turns it back (+0xb8 negated, the offset π); the follower `0x30a298` at its end: the voice
//! released, the rocking 0, → 2, the direction flipped, the next point the far end.
//!
//! **The follower** (`0x30a298`): the rocking eases out (·0.005); toward the current point (7·speed higher) by the
//! speed +0xa8; the yaw eased (·(1 − rock)·0.02) to the point's bearing + the offset, the pitch and roll eased to 0
//! (·0.02); within twice the speed the next point (done past either end); the speed springs (3·dt², at most 10·dt)
//! toward the distance to the end it heads for; a point with w = 42 zeroes the yaw offset; the paddle phase +0xc0 +=
//! speed; one tick in seven away from the ends (8, xy) a wake ring at the lava's height 25.01 (`PartType45Spawn(speed·3
//! / (10·dt), 12600, p ± 0.5, 0, 0x40103080)`).
//!
//! Every tick the riders are carried (`CarryRiders`, the platform block +0x20). Read from the level09 decomp and
//! disassembly (the follower's target). Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x309c08` | the raft (module doc) | [`update`] |
//! | `0x30a298` | the follower | `follow` |
//! | `0x30a238(a, b, k, &out)` | `out = a + (b − a)·k` (rotations) | `ease` |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, add, add_rot, atan, set_len3, sub, sub_rot, turn, V, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::story;
use crate::moby_update::triggers;
use crate::moby_update::services::World;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 9;
pub const UPDATE_FN: u32 = 0x30_9c08;
pub const CLASSES: [i16; 1] = [1766];

const LEN: usize = 0xe0;
/// gp−0x4e08: the target's lift per unit of speed.
const LIFT: f32 = 7.0;
const LAVA_Z: f32 = f32::from_bits(0x41c8_147b);

mod pvo {
    pub const BLOCK: usize = 0x20;
    pub const MANIP: usize = 0x60;
    pub const HAS_MANIP: usize = 0xa0;
    pub const SPEED: usize = 0xa8;
    pub const PATH: usize = 0xac;
    pub const BACK_CUB: usize = 0xb0;
    pub const FWD_CUB: usize = 0xb4;
    pub const DIR: usize = 0xb8;
    pub const NODE: usize = 0xba;
    pub const YAW_OFF: usize = 0xbc;
    pub const PADDLE: usize = 0xc0;
    pub const VOICE: usize = 0xc4;
    pub const BOARDED: usize = 0xc8;
    pub const ROCK: usize = 0xcc;
    pub const WOB_A: usize = 0xd0;
    pub const WOB_B: usize = 0xd4;
    pub const BOB: usize = 0xd8;
    pub const BOB_PREV: usize = 0xdc;
}

fn path(w: &World, id: MobyId) -> Vec<V> {
    usize::try_from(c::pi32(w, id, pvo::PATH)).ok().and_then(|i| w.svc.splines.get(i)).map(|v| v.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}
fn ease(a: f32, b: f32, k: f32) -> f32 { add_rot(sub_rot(b, a) * k, a) }
fn dist_xy(a: V, b: V) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }
fn in_cub(w: &World, id: MobyId, o: usize) -> bool {
    let k = c::pi32(w, id, o);
    k != -1 && w.in_cuboid(w.hero_point(), k)
}

/// The rocking of states 1 and 2.
fn rock(w: &mut World, id: MobyId) {
    let r = c::pf(w, id, pvo::ROCK);
    let r = r + (1.0 - r) * 0.01;
    c::set_pf(w, id, pvo::ROCK, r);
    super::bob(w, id, r * 0.14, DT * std::f32::consts::FRAC_PI_6, pvo::BOB, pvo::BOB_PREV);
    super::wobble(w, id, r * f32::from_bits(0x3db2_b8c2), DT * f32::from_bits(0x3ea0_d97c), DT * f32::from_bits(0x3efa_35dd), pvo::WOB_A, pvo::WOB_B);
}

/// Level09 `0x309c08` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, LEN);
    let old = c::pos(w, id);
    let rot_old = w.m(id).rotation;
    let pts = path(w, id);
    let n = pts.len();
    match w.m(id).state {
        0 => {
            w.mm(id).update_dist = 0xff;
            c::set_pf(w, id, pvo::YAW_OFF, PI);
            c::set_pi32(w, id, pvo::VOICE, -1);
            c::set_pi32(w, id, pvo::BOARDED, 0);
            c::set_pf(w, id, pvo::ROCK, 0.0);
            if c::pi32(w, id, pvo::HAS_MANIP) != 0 && 2 <= n {
                manip::attach(w, id, 0, id, pvo::MANIP);
                w.mm(id).state = 1;
                let done = story::mission_done(w, w.m(id).mission as i32);
                let at = if done {
                    c::set_pi16(w, id, pvo::DIR, -1);
                    c::set_pi16(w, id, pvo::NODE, n as i16 - 2);
                    pts[n - 1]
                } else {
                    c::set_pi16(w, id, pvo::NODE, 1);
                    c::set_pi16(w, id, pvo::DIR, 1);
                    pts[0]
                };
                c::set_pos(w, id, at);
                let q = pts[c::pi16(w, id, pvo::NODE) as usize];
                w.mm(id).rotation[2] = atan(q[0] - at[0], q[1] - at[1]);
            }
            let z = add_rot(w.m(id).rotation[2], PI);
            w.mm(id).rotation[2] = z;
        }
        1 => {
            rock(w, id);
            if c::pi32(w, id, pvo::BOARDED) != 0 {
                let dir = c::pi16(w, id, pvo::DIR);
                if (1 <= dir && in_cub(w, id, pvo::FWD_CUB)) || (dir < 0 && in_cub(w, id, pvo::BACK_CUB)) { w.mm(id).state = 3; }
            }
            if w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 {
                c::set_pi32(w, id, pvo::BOARDED, 1);
                w.mm(id).state = 3;
            }
        }
        2 => {
            rock(w, id);
            if w.hero.ground_moby != Some(id) { w.mm(id).state = 1; }
        }
        3 if 2 <= n => travel(w, id, &pts),
        _ => {}
    }
    let p = c::pos(w, id);
    let rot_new = w.m(id).rotation;
    let delta = [p[0] - old[0], p[1] - old[1], p[2] - old[2], p[3]];
    triggers::carry_riders(&mut w.mm(id).pvars, pvo::BLOCK, delta, rot_old, rot_new);
}

fn travel(w: &mut World, id: MobyId, pts: &[V]) {
    let n = pts.len();
    // One tick in eight by the moby's slot (the game: `(moby address >> 8) & 7` = slot + 0x15ff).
    if (w.counter & 7) as usize == (id + 0x15ff) & 7 {
        let v = c::pi32(w, id, pvo::VOICE);
        if v == -1 || !w.sound_alive(v, id) {
            let s = w.play_sound(0, 4, id);
            c::set_pi32(w, id, pvo::VOICE, s);
        }
    }
    if c::pf(w, id, pvo::YAW_OFF) == 0.0 {
        let p = c::pos(w, id);
        let far = 8.0 < dist_xy(p, pts[n - 1]) && 8.0 < dist_xy(p, pts[0]);
        let dir = c::pi16(w, id, pvo::DIR);
        let flip = if dir < 0 {
            (in_cub(w, id, pvo::FWD_CUB) && far).then_some(1)
        } else {
            (1 <= dir && in_cub(w, id, pvo::BACK_CUB) && far).then_some(-1)
        };
        if let Some(d) = flip {
            c::set_pi16(w, id, pvo::DIR, d);
            c::set_pf(w, id, pvo::YAW_OFF, PI);
        }
    }
    if follow(w, id, pts) {
        let v = c::pi32(w, id, pvo::VOICE);
        if v != -1 { w.release_sound(v, id); }
        c::set_pi32(w, id, pvo::VOICE, -1);
        c::set_pf(w, id, pvo::ROCK, 0.0);
        c::set_pf(w, id, pvo::BOB_PREV, 0.0);
        w.mm(id).state = 2;
        let s = c::pi16(w, id, pvo::DIR);
        c::set_pf(w, id, pvo::YAW_OFF, PI);
        c::set_pi16(w, id, pvo::DIR, -s);
        c::set_pi16(w, id, pvo::NODE, if -s < 1 { n as i16 - 1 } else { 0 });
    }
}

/// `0x30a298`: the follower (module doc); true at the path's end.
fn follow(w: &mut World, id: MobyId, pts: &[V]) -> bool {
    let n = pts.len() as i32;
    let r = c::pf(w, id, pvo::ROCK);
    c::set_pf(w, id, pvo::ROCK, r + (0.0 - r) * 0.005);
    let node = (c::pi16(w, id, pvo::NODE) as i32).clamp(0, n - 1);
    let speed = c::pf(w, id, pvo::SPEED);
    let pt = pts[node as usize];
    let tgt = [pt[0], pt[1], pt[2] + speed * LIFT, pt[3]];
    let mut p = c::pos(w, id);
    p = add(p, set_len3(sub(tgt, p), speed));
    c::set_pos(w, id, p);
    let yaw = add_rot(atan(pt[0] - p[0], pt[1] - p[1]), c::pf(w, id, pvo::YAW_OFF));
    let rock = c::pf(w, id, pvo::ROCK);
    let rr = w.m(id).rotation;
    let k = f32::from_bits(0x3ca3_d70a);
    w.mm(id).rotation[2] = ease(rr[2], yaw, (1.0 - rock) * 0.02);
    w.mm(id).rotation[0] = ease(rr[0], 0.0, k);
    w.mm(id).rotation[1] = ease(rr[1], 0.0, k);
    let near = c::dist3(tgt, p) < speed + speed;
    let mut node = node;
    let (done, end) = if c::pi16(w, id, pvo::DIR) < 1 {
        if near { node -= 1; }
        (node == -1, pts[0])
    } else {
        if near { node += 1; }
        (node == n, pts[n as usize - 1])
    };
    c::set_pi16(w, id, pvo::NODE, node as i16);
    let mut x = 0.0;
    let mut v = c::pf(w, id, pvo::SPEED);
    turn::spring(c::dist3(p, end), DT2 * 3.0, DT2 * 3.0, DT * 10.0, &mut x, &mut v);
    c::set_pf(w, id, pvo::SPEED, v);
    if (0..n).contains(&node) && pts[node as usize][3] == 42.0 { c::set_pf(w, id, pvo::YAW_OFF, 0.0); }
    let paddle = add_rot(c::pf(w, id, pvo::PADDLE), v);
    c::set_pf(w, id, pvo::PADDLE, paddle);
    manip::set_axis(w, id, id, pvo::MANIP, paddle, 0);
    let p = c::pos(w, id);
    if 8.0 < dist_xy(p, pts[n as usize - 1]) && 8.0 < dist_xy(p, pts[0]) && w.rng.randi(7) == 0 {
        let mut q = p;
        crate::moby_update::creature::fx::jitter(w, 0.5, &mut q);
        q[2] = LAVA_Z;
        let size = (v * 3.0) / (DT * 10.0);
        if let Some(sys) = w.particles.as_deref_mut() {
            crate::particles::type45::spawn45(sys, w.rng, size, 12600.0, q, 0x4010_3080);
        }
    }
    done
}
