//! **Batalia's gunships, classes 444, 462 and 463** (level08 `0x2e24e8`, `0x2e8cd0`, `0x2e9b70`; census U276, U278,
//! U279; placed #340, #347, #348, #349, the turret 440's targets), and **their parts** (441–443, 445, 448–451, 464,
//! 465: `0x2e16c0`). One update per class, the same shape, read here through a [`Variant`] table:
//!
//! * 0: the mission (+0xb0) done → deleted. Else the attack path (+0x14 → +0x10) and the intro path (+0x44 → +0x40),
//!   the idle-missile timer `ticks(randf(300, 600))`, mode 0x4000, at the intro path's first point; the parts are
//!   built (`0x2e1c98`: each a new moby placed so its joint sits on its parent's joint, the hero's light), → 2,
//!   facing point 1.
//! * 2 (the intro loop): along the intro path at 5 a second (+0x30 the point), the yaw blended between the segments
//!   (444: turned toward the next point at 20°·dt² / 90° a second instead); the idle missile (`batalia_bomber` state 5)
//!   from the launcher's joint 1 when the timer runs out. 462 with +0x138 set: in cutscene 0 at its tick 2 (game mode
//!   2) jumps to the intro path's point 15.
//! * 3 (the turret's wave begins): → 1 at the attack path's start, facing point 1 (444: pitch 0).
//! * 1 (the attack): `SpringTurn2` (0.004, 0.3, 0.02) along the attack path (+0 the point, +4 its direction) at 5 a
//!   second; the hits; the turret's command (+0xbc) launches a missile at it (`0x2de3e0`, pitch `randf(10°, 30°)`,
//!   yaw away from the turret plus `randf(−15°, 45°)` kept off ±0.34, a fifth of the class scale), registered in
//!   the turret's eight slots.
//! * The hits (+0x10000 on any record, the hit slot cleared): the hull / the body / a pod score 1 / 2 / 4 (the
//!   highest once; sound 1 / 1 / 2); the counter (+0x28c / +0x13c / +0x17c) past 7 / 7 / 11 knocks off the pod hit
//!   (else a random one still on; none left → 99) with the death explosion (2, 13), flung out from its parent at
//!   10 a second.
//! * 99: every part flung from the hull at 5 a second with the death explosion (3, 13), the hull's (5, 13), deleted.
//!
//! The engine loop (sound 3, flags 4) is kept alive in +0x28e / +0x13e / +0x17e. Each tick the parts are aligned
//! (`0x2e2078` / `0x2e8ba0` / `0x2e9a18`: the hull's yaw, joint onto parent joint), the rotors spun at 30° a second
//! (444: the turret part faces Ratchet), and their matrices built (463: the mirrored ones' row 1 negated).
//!
//! **The parts** (`0x2e16c0`): attached (0) they do nothing; knocked off (1): a random spin (±90° a second on x and
//! y) → 2: falling at 10.8·dt²·0.5, a smoke spark (type 11) one tick in three, deleted out of [8, 500]; under z 15.2 a
//! splash of 50 puffs (type 2) → 3: falling on until out of bounds.
//!
//! Read from the level08 decomp; the constants from the overlay (gp 0x166c00: 0x161c00.., 0x161d24..). [L] The
//! splash puffs' first velocity starts from whatever the stack held when no smoke spark was made that tick (its xy
//! gain is 0); the port starts from zero. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2e24e8` / `0x2e8cd0` / `0x2e9b70` | the gunships (module doc) | [`update`] |
//! | `0x2e1d70` / `0x2e8788` / `0x2e9558` | the parts built ([`Variant::recs`], in [`Variant::order`]) | `build` |
//! | `0x2e2250` / `0x2e88d8` / `0x2e9758` | the hits | `hits` |
//! | `0x2e2078` / `0x2e8ba0` / `0x2e9a18` | the parts aligned | `align` |
//! | `0x2e1c98(parent, class, part joint, parent joint)` | a part | `part` |
//! | `0x2e16c0` | the parts | [`part_update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx;
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist3, set_len3, sub, sub_rot, turn, DT, DT2, V};
use crate::moby_update::services::{pf, pv, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 8;
pub const BIG_FN: u32 = 0x2e_24e8;
pub const MID_FN: u32 = 0x2e_8cd0;
pub const SMALL_FN: u32 = 0x2e_9b70;
pub const PART_FN: u32 = 0x2e_16c0;
pub const BIG_CLASSES: [i16; 1] = [444];
pub const MID_CLASSES: [i16; 1] = [462];
pub const SMALL_CLASSES: [i16; 1] = [463];
pub const PART_CLASSES: [i16; 10] = [441, 442, 443, 445, 448, 449, 450, 451, 464, 465];
/// The joint lists the alignment reads (the hulls and every part).
pub const JOINT_CLASSES: [i16; 13] = [444, 462, 463, 441, 442, 443, 445, 448, 449, 450, 451, 464, 465];

/// gp−0x4fc8 / −0x4edc / −0x4ecc: the speed; −0x4fc4 / −0x4ed8 / −0x4ec8: the rotor spin (degrees a second).
const SPEED: f32 = 5.0;
const SPIN: f32 = 30.0;
/// gp−0x4fbc / −0x4fb8: 444's intro turn (degrees, ·dt² / ·dt).
const TURN_ACC: f32 = 20.0;
const TURN_MAX: f32 = 90.0;
const DEG: f32 = 0.017_453_292;

mod pvo {
    pub const NODE: usize = 0x00;
    pub const DIR: usize = 0x04;
    pub const PATH: usize = 0x14;
    pub const INTRO_NODE: usize = 0x30;
    pub const INTRO: usize = 0x44;
    pub const RECS: usize = 0x60;
    /// 462: the cutscene jump.
    pub const JUMP: usize = 0x138;
}

/// The part pvars: velocity, spin x / y, the unused timer.
mod ppo {
    pub const VEL: usize = 0x00;
    pub const SPIN_X: usize = 0x10;
    pub const SPIN_Y: usize = 0x14;
    pub const TIMER: usize = 0x18;
    pub const LEN: usize = 0x20;
}

/// A part record (+0x60 + 16·i: part, parent, part joint, parent joint; record 0 the hull). `spawn` is the moby the
/// part is first placed against when it differs from `parent` (444's launcher).
#[derive(Clone, Copy)]
struct Rec {
    class: i16,
    parent: usize,
    spawn: usize,
    part_joint: usize,
    parent_joint: usize,
    mirror: bool,
    small: bool,
}

const fn r(class: i16, parent: usize, part_joint: usize, parent_joint: usize) -> Rec {
    Rec { class, parent, spawn: parent, part_joint, parent_joint, mirror: false, small: false }
}
const fn mirrored(x: Rec) -> Rec { Rec { mirror: true, ..x } }
const fn small(x: Rec) -> Rec { Rec { small: true, ..x } }
const fn spawned(x: Rec, spawn: usize) -> Rec { Rec { spawn, ..x } }
const POD: Rec = r(0x1bb, 0, 0, 0);

/// The per-class table. (gp−0x4fb4 / −0x4ed0 / −0x4ec0, a switch that stops the class, are 0 in the shipped data.)
pub struct Variant {
    recs: &'static [Rec],
    order: &'static [usize],
    pods: (usize, usize, usize),
    turret: usize,
    turn_v: usize,
    timer: usize,
    timer16: bool,
    counter: usize,
    sound: usize,
    len: usize,
    /// The hit scores' bounds (record < a: 1, < b: 2, else 4), the counter's wrap.
    bounds: (usize, usize),
    wrap: i32,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Big,
    Mid,
    Small,
}

impl Variant {
    fn big(&self) -> bool { self.kind == Kind::Big }
}

/// 444's records: 1, 2 the wings (2 mirrored), 3 the turret, 4 the launcher (placed against the hull, then on 6),
/// 5 the cockpit on 9, 6 the arm on 7, 7, 8, 9 the rotor hubs, 10–17 / 18–25 / 26–33 the pods on 7 / 8 / 9.
const BIG_RECS: [Rec; 34] = {
    let mut t = [POD; 34];
    t[1] = r(0x1c2, 0, 0, 3);
    t[2] = mirrored(r(0x1c2, 0, 0, 4));
    t[3] = r(0x1c1, 0, 0, 2);
    t[4] = r(0x1b9, 6, 0, 0);
    t[4] = spawned(t[4], 0);
    t[5] = r(0x1bd, 9, 0, 0);
    t[6] = r(0x1ba, 7, 1, 1);
    t[7] = r(0x1c0, 0, 0, 0);
    t[8] = r(0x1c0, 0, 1, 1);
    t[9] = r(0x1c0, 8, 1, 0);
    let mut i = 0;
    while i < 8 {
        t[10 + i] = r(0x1bb, 7, 0, i + 2);
        t[18 + i] = r(0x1bb, 8, 0, i + 2);
        t[26 + i] = r(0x1bb, 9, 0, i + 2);
        i += 1;
    }
    t
};
const BIG_ORDER: [usize; 33] = [3, 1, 2, 7, 8, 9, 5, 6, 4, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33];

/// 462's: 1 the cockpit (¾ scale), 2 the arm, 3 the rotor hub, 4 the launcher on 2, 5–12 the pods on 3.
const MID_RECS: [Rec; 13] = {
    let mut t = [POD; 13];
    t[1] = small(r(0x1bd, 0, 0, 2));
    t[2] = r(0x1ba, 0, 1, 1);
    t[3] = r(0x1c0, 0, 1, 0);
    t[4] = r(0x1b9, 2, 0, 0);
    let mut i = 0;
    while i < 8 {
        t[5 + i] = r(0x1bb, 3, 0, i + 2);
        i += 1;
    }
    t
};
const MID_ORDER: [usize; 12] = [3, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/// 463's: 1 the cockpit (¾), 2 the arm on 3, 3 the rotor hub, 4 the launcher on 2, 5 / 6 the fins (6 mirrored),
/// 7 / 8 the guns on 2 (8 mirrored), 9–16 the pods on 3.
const SMALL_RECS: [Rec; 17] = {
    let mut t = [POD; 17];
    t[1] = small(r(0x1bd, 0, 0, 2));
    t[2] = r(0x1ba, 3, 1, 1);
    t[3] = r(0x1c0, 0, 0, 0);
    t[4] = r(0x1b9, 2, 0, 0);
    t[5] = r(0x1d0, 0, 0, 1);
    t[6] = mirrored(r(0x1d0, 0, 0, 1));
    t[7] = r(0x1d1, 2, 0, 0);
    t[8] = mirrored(r(0x1d1, 2, 0, 0));
    let mut i = 0;
    while i < 8 {
        t[9 + i] = r(0x1bb, 3, 0, i + 2);
        i += 1;
    }
    t
};
const SMALL_ORDER: [usize; 16] = [3, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

const BIG: Variant = Variant { recs: &BIG_RECS, order: &BIG_ORDER, pods: (10, 24, 24), turret: 0x280, turn_v: 0x284, timer: 0x288, timer16: false, counter: 0x28c, sound: 0x28e, len: 0x290, bounds: (7, 10), wrap: 8, kind: Kind::Big };
const MID: Variant = Variant { recs: &MID_RECS, order: &MID_ORDER, pods: (5, 8, 8), turret: 0x130, turn_v: 0x134, timer: 0x13a, timer16: true, counter: 0x13c, sound: 0x13e, len: 0x140, bounds: (3, 5), wrap: 8, kind: Kind::Mid };
const SMALL: Variant = Variant { recs: &SMALL_RECS, order: &SMALL_ORDER, pods: (9, 8, 8), turret: 0x170, turn_v: 0x174, timer: 0x178, timer16: false, counter: 0x17c, sound: 0x17e, len: 0x180, bounds: (3, 9), wrap: 12, kind: Kind::Small };

fn variant(w: &World, id: MobyId) -> &'static Variant {
    match w.m(id).o_class {
        444 => &BIG,
        462 => &MID,
        _ => &SMALL,
    }
}

fn path(w: &World, i: i32) -> Vec<V> {
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map(|v| v.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}
fn rec(w: &World, id: MobyId, i: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, pvo::RECS + 16 * i) - 1).ok().filter(|&m| m < w.table.mobys.len())
}
fn set_rec(w: &mut World, id: MobyId, i: usize, m: Option<MobyId>) { c::set_pi32(w, id, pvo::RECS + 16 * i, m.map_or(0, |m| m as i32 + 1)); }
fn turret(w: &World, id: MobyId, v: &Variant) -> Option<MobyId> { usize::try_from(c::pi32(w, id, v.turret)).ok().filter(|&m| m < w.table.mobys.len()) }

/// Level08 `0x2e24e8` / `0x2e8cd0` / `0x2e9b70` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let v = variant(w, id);
    story::pvars(w, id, v.len);
    let slot = c::pi16(w, id, v.sound) as i32;
    if !w.sound_alive(slot, id) {
        let s = w.play_sound(3, 4, id);
        c::set_pi16(w, id, v.sound, s as i16);
    }
    match w.m(id).state {
        0 => init(w, id, v),
        1 => attack(w, id, v),
        2 => intro(w, id, v),
        3 => {
            w.mm(id).state = 1;
            let pts = path(w, c::pi32(w, id, pvo::PATH));
            if let Some(&p0) = pts.first() {
                c::set_pos(w, id, p0);
                if let Some(p1) = pts.get(1) { w.mm(id).rotation[2] = atan(p1[0] - p0[0], p1[1] - p0[1]); }
            }
            if v.big() { w.mm(id).rotation[0] = 0.0; }
            align(w, id, v);
        }
        99 => {
            let me = c::pos(w, id);
            for i in 1..v.recs.len() {
                let Some(m) = rec(w, id, i) else { continue };
                let p = c::pos(w, m);
                fx::death_explosion(w, 3.0, 13.0, Some(id), p, -1);
                knock(w, m, set_len3(sub(p, me), DT * 5.0));
                set_rec(w, id, i, None);
            }
            fx::death_explosion(w, 5.0, 13.0, Some(id), me, -1);
            w.delete_moby(id);
        }
        _ => {}
    }
}

fn idle_timer(w: &mut World, id: MobyId, v: &Variant) {
    let f = w.rng.randf(300.0, 600.0);
    let t = w.ticks(f as i32);
    if v.timer16 { c::set_pi16(w, id, v.timer, t as i16) } else { c::set_pi32(w, id, v.timer, t) }
}

fn init(w: &mut World, id: MobyId, v: &Variant) {
    if story::mission_done(w, w.m(id).mission as i32) {
        w.delete_moby(id);
        return;
    }
    idle_timer(w, id, v);
    w.mm(id).mode |= 0x4000;
    let pts = path(w, c::pi32(w, id, pvo::INTRO));
    if let Some(&p0) = pts.first() { c::set_pos(w, id, p0); }
    build(w, id, v);
    w.mm(id).state = 2;
    if let Some(p1) = pts.get(1) {
        let me = c::pos(w, id);
        w.mm(id).rotation[2] = atan(p1[0] - me[0], p1[1] - me[1]);
    }
}

/// `0x2e1c98(parent, class, part joint, parent joint)`.
fn part(w: &mut World, parent: MobyId, class: i16, part_joint: usize, parent_joint: usize) -> Option<MobyId> {
    let m = w.create_moby(class)?;
    let hero = w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient));
    let rot = w.m(parent).rotation;
    {
        let mo = w.mm(m);
        mo.draw_dist = 0xff;
        mo.update_dist = 0xff;
        mo.visible = 1;
        mo.mode |= 0x4000;
        if let Some((l, a)) = hero {
            mo.light = l;
            mo.ambient = a;
        }
        mo.rotation = rot;
    }
    let a = w.joint_point(m, part_joint);
    let b = w.joint_point(parent, parent_joint);
    let p = sub(b, a);
    w.mm(m).position = [p[0], p[1], p[2], w.m(m).position[3]];
    w.build_matrix(m);
    Some(m)
}

fn build(w: &mut World, id: MobyId, v: &Variant) {
    set_rec(w, id, 0, Some(id));
    for &i in v.order {
        let x = v.recs[i];
        let Some(spawn) = (if x.spawn == 0 { Some(id) } else { rec(w, id, x.spawn) }) else { continue };
        let m = part(w, spawn, x.class, x.part_joint, x.parent_joint);
        if let Some(m) = m {
            if x.mirror { w.mm(m).mode |= 0x8000; }
            if x.small { w.mm(m).scale = super::class_scale(w, x.class) * 0.75; }
        }
        set_rec(w, id, i, m);
        let parent = if x.parent == 0 { Some(id) } else { rec(w, id, x.parent) };
        let o = pvo::RECS + 16 * i;
        c::set_pi32(w, id, o + 4, parent.map_or(0, |m| m as i32 + 1));
        c::set_pi32(w, id, o + 8, x.part_joint as i32);
        c::set_pi32(w, id, o + 12, x.parent_joint as i32);
    }
}

/// The alignment (module doc).
fn align(w: &mut World, id: MobyId, v: &Variant) {
    let yaw = w.m(id).rotation[2];
    for i in 1..v.recs.len() {
        let Some(m) = rec(w, id, i) else { continue };
        let o = pvo::RECS + 16 * i;
        let parent = usize::try_from(c::pi32(w, id, o + 4) - 1).ok().filter(|&p| p < w.table.mobys.len()).unwrap_or(id);
        w.mm(m).rotation[2] = yaw;
        let a = w.joint_point(m, c::pi32(w, id, o + 8) as usize);
        let b = w.joint_point(parent, c::pi32(w, id, o + 12) as usize);
        let d = sub(b, a);
        let p = add(w.m(m).position, d);
        w.mm(m).position = p;
    }
    let spin = SPIN * DEG * DT;
    let spins: &[(usize, f32)] = if v.big() { &[(7, 1.0), (8, -1.0), (9, 1.0)] } else { &[(3, 1.0)] };
    for &(i, s) in spins {
        if let Some(m) = rec(w, id, i) {
            let x = add_rot(w.m(m).rotation[0], spin * s);
            w.mm(m).rotation[0] = x;
        }
    }
    if v.big() {
        if let Some(m) = rec(w, id, 3) {
            let h = super::hero_pos(w);
            let p = c::pos(w, m);
            w.mm(m).rotation[2] = atan(h[0] - p[0], h[1] - p[1]);
        }
    }
    for i in 1..v.recs.len() {
        let Some(m) = rec(w, id, i) else { continue };
        w.build_matrix(m);
        if v.kind == Kind::Small && w.m(m).mode & 0x8000 != 0 {
            let r1 = w.m(m).rows[1];
            w.mm(m).rows[1] = r1.map(|x| -x);
        }
    }
}

/// A part knocked off: its velocity, state 1.
fn knock(w: &mut World, m: MobyId, vel: V) {
    story::pvars(w, m, ppo::LEN);
    c::set_pv4(w, m, ppo::VEL, vel);
    w.mm(m).state = 1;
}

/// Record `i` knocked off its parent (the death explosion (2, 13), 10 a second).
fn knock_rec(w: &mut World, id: MobyId, i: usize) {
    let Some(m) = rec(w, id, i) else { return };
    let parent = usize::try_from(c::pi32(w, id, pvo::RECS + 16 * i + 4) - 1).ok().filter(|&p| p < w.table.mobys.len()).unwrap_or(id);
    let p = c::pos(w, m);
    fx::death_explosion(w, 2.0, 13.0, Some(id), p, -1);
    let d = sub(p, c::pos(w, parent));
    knock(w, m, set_len3(d, DT * 10.0));
    set_rec(w, id, i, None);
}

/// The hits (module doc).
fn hits(w: &mut World, id: MobyId, v: &Variant) {
    let (mut score, mut sound, mut pod) = (0, -1, 0usize);
    for i in 0..v.recs.len() {
        let Some(m) = rec(w, id, i) else { continue };
        if w.get_hit(m, 0x1_0000, false).is_none() { continue; }
        if i < v.bounds.0 {
            if score < 1 {
                score = 1;
                sound = 1;
            }
        } else if i < v.bounds.1 {
            if score < 2 {
                score = 2;
                sound = 1;
            }
        } else if score < 4 {
            score += 4;
            sound = 2;
            pod = i;
        }
        w.mm(m).hit_slot = 0xff;
    }
    let n = (c::pi16(w, id, v.counter) as i32 + score) as i16;
    c::set_pi16(w, id, v.counter, n);
    if v.wrap - 1 < n as i32 {
        if v.big() { sound = 0; }
        c::set_pi16(w, id, v.counter, (n as i32 % v.wrap) as i16);
        if pod != 0 {
            knock_rec(w, id, pod);
            return;
        }
        let r = w.rng.randf(0.0, 23.0) as i32;
        let (base, count, modulo) = v.pods;
        let found = (0..count as i32).map(|k| ((k + r) % modulo as i32) as usize + base).find(|&i| rec(w, id, i).is_some());
        match found {
            Some(i) => {
                knock_rec(w, id, i);
                return;
            }
            None => w.mm(id).state = 99,
        }
    }
    if 0 <= sound { w.play_sound(sound, 0, id); }
}

/// The step toward path point `q` at the speed (`< 0.5` reaches it): the move vector.
fn step(w: &mut World, id: MobyId, q: V) -> (V, bool) {
    let me = c::pos(w, id);
    let d = c::clamp_len3(sub(q, me), SPEED * DT);
    let reached = dist3(me, q) < 0.5;
    c::set_pos(w, id, add(me, d));
    (d, reached)
}

fn intro(w: &mut World, id: MobyId, v: &Variant) {
    if v.kind == Kind::Mid && w.svc.game_mode == 2 && c::pi16(w, id, pvo::JUMP) != 0 {
        let at = w.svc.cinematic.scene.as_ref().is_some_and(|s| s.id == 0 && s.tick == 2);
        if at {
            let pts = path(w, c::pi32(w, id, pvo::INTRO));
            if let (Some(&p15), Some(p11)) = (pts.get(15), pts.get(11)) {
                c::set_pos(w, id, p15);
                c::set_pi32(w, id, pvo::INTRO_NODE, 15);
                w.mm(id).rotation[2] = atan(p11[0] - p15[0], p11[1] - p15[1]);
            }
        }
    }
    let pts = path(w, c::pi32(w, id, pvo::INTRO));
    let n = pts.len() as i32;
    if n == 0 { return; }
    // The placed node is −1: the first segment starts at the path's header (the original reads it as point −1),
    // reached at once, its weight 0.
    let node = c::pi32(w, id, pvo::INTRO_NODE);
    let i = node.rem_euclid(n);
    let j = (i + 1) % n;
    let (pi, pj, pk) = (pts[i as usize], pts[j as usize], pts[((i + 2) % n) as usize]);
    let me = c::pos(w, id);
    if v.big() {
        let mut yaw = w.m(id).rotation[2];
        let mut vel = c::pf(w, id, v.turn_v);
        let acc = TURN_ACC * DEG * DT2;
        turn::turn_toward(atan(pj[0] - me[0], pj[1] - me[1]), acc, acc, TURN_MAX * DEG * DT, &mut yaw, &mut vel);
        w.mm(id).rotation[2] = yaw;
        c::set_pf(w, id, v.turn_v, vel);
    } else {
        let a1 = atan(pj[0] - pi[0], pj[1] - pi[1]);
        let a2 = atan(pk[0] - pj[0], pk[1] - pj[1]);
        let d = sub_rot(a1, a2);
        let f = if node < 0 { 0.0 } else { dist3(me, pj) / dist3(pi, pj) };
        w.mm(id).rotation[2] = add_rot(d * f, a2);
    }
    let (mv, reached) = step(w, id, pj);
    if reached { c::set_pi32(w, id, pvo::INTRO_NODE, j); }
    align(w, id, v);
    let fired = if v.timer16 { c::dec_timer_pvar_s16(w, id, v.timer) } else { c::dec_timer_pvar_i32(w, id, v.timer) };
    if fired == 0 { return; }
    idle_timer(w, id, v);
    let Some(launcher) = rec(w, id, 4) else { return };
    let at = w.joint_point(launcher, 1);
    let pitch = w.rng.randf(f32::from_bits(0xbf06_0a92), f32::from_bits(0xbe86_0a92));
    let yaw = w.rng.rand_angle();
    let vel = set_len3(mv, (SPEED + SPEED) * DT);
    if let Some(m) = super::batalia_bomber::spawn_missile(w, pitch, yaw, at, None, vel) {
        let r = w.m(launcher).rotation[2];
        let mo = w.mm(m);
        mo.state = 5;
        mo.rotation[2] = r;
        mo.rotation[1] = 0.0;
    }
}

fn attack(w: &mut World, id: MobyId, v: &Variant) {
    let pts = path(w, c::pi32(w, id, pvo::PATH));
    let n = pts.len() as i32;
    if n == 0 { return; }
    let dir = c::pi32(w, id, pvo::DIR) as i8 as i32;
    let i = (c::pi32(w, id, pvo::NODE) + n + dir).rem_euclid(n);
    let q = pts[i as usize];
    let me = c::pos(w, id);
    turn::spring_turn2_pvar(w, id, atan(q[0] - me[0], q[1] - me[1]), f32::from_bits(0x3b83_126f), f32::from_bits(0x3e99_999a), f32::from_bits(0x3ca3_d70a), v.turn_v);
    let small = v.kind == Kind::Small;
    let mut mv = [0.0; 4];
    let go = |w: &mut World| {
        let (d, reached) = step(w, id, q);
        if reached { c::set_pi32(w, id, pvo::NODE, i); }
        d
    };
    if small { mv = go(w); }
    hits(w, id, v);
    align(w, id, v);
    if !small { mv = go(w); }
    if w.m(id).cmd == 0 { return; }
    w.mm(id).cmd = 0;
    let Some(t) = turret(w, id, v) else { return };
    let launcher = if v.big() && w.rng.randi(0xff) & 1 != 0 { rec(w, id, 3) } else { rec(w, id, 4) };
    let at = launcher.map_or(c::pos(w, id), |l| w.joint_point(l, 1));
    let pitch = w.rng.randf(f32::from_bits(0x3e32_b8c2), f32::from_bits(0x3f06_0a92));
    let tp = w.m(t).position;
    let me = c::pos(w, id);
    let away = add_rot(atan(tp[0] - me[0], tp[1] - me[1]), std::f32::consts::PI);
    let off = w.rng.randf(f32::from_bits(0xbe86_0a92), f32::from_bits(0x3f49_0fdb));
    let f = add_rot(away, off);
    let yaw = if 0.0 < f && f < 0.34 {
        0.34
    } else if f <= 0.0 && -0.34 < f {
        -0.34
    } else {
        f
    };
    let vel = set_len3(mv, (SPEED + SPEED) * DT);
    let Some(m) = super::batalia_bomber::spawn_missile(w, pitch, yaw, at, Some(t), vel) else { return };
    w.mm(m).scale = super::class_scale(w, 435) / 5.0;
    super::batalia_turret::register_missile(w, t, m);
}

/// Level08 `0x2e16c0`: the parts (module doc).
pub fn part_update(w: &mut World, id: MobyId) {
    let s = w.m(id).state;
    if s == 0 || 3 < s { return; }
    story::pvars(w, id, ppo::LEN);
    if s == 1 {
        let k = DT * std::f32::consts::FRAC_PI_2;
        let x = w.rng.randf(-k, k);
        c::set_pf(w, id, ppo::SPIN_X, x);
        let y = w.rng.randf(-k, k);
        c::set_pf(w, id, ppo::SPIN_Y, y);
        let (a, b) = (w.ticks(0x5a), w.ticks(0x96));
        let t = w.rng.randf(a as f32, b as f32) as i32;
        c::set_pi32(w, id, ppo::TIMER, t);
        w.mm(id).state = 2;
    }
    let mut vel = c::pv4(w, id, ppo::VEL);
    vel[2] -= DT2 * 10.8 * 0.5;
    c::set_pv4(w, id, ppo::VEL, vel);
    let p = add(c::pos(w, id), vel);
    c::set_pos(w, id, p);
    let (sx, sy) = (c::pf(w, id, ppo::SPIN_X), c::pf(w, id, ppo::SPIN_Y));
    {
        let m = w.mm(id);
        m.rotation[0] = add_rot(m.rotation[0], sx);
        m.rotation[1] = add_rot(m.rotation[1], sy);
    }
    if s == 3 {
        let keep = 8.0 <= p[0] && 8.0 <= p[1] && 8.0 <= p[2] && p[0] <= 500.0 && p[1] <= 500.0;
        if !keep { w.delete_moby(id); }
        return;
    }
    if p[0] < 8.0 || p[1] < 8.0 || 500.0 < p[0] || 500.0 < p[1] || 500.0 < p[2] {
        w.delete_moby(id);
        return;
    }
    if w.rng.randi(3) == 0 {
        const SMOKE: [u32; 2] = [0x2f00_0000, 0x3f00_0000];
        let c2 = SMOKE[w.rng.randi(2) as usize & 1];
        let (a, b) = (w.ticks(0xf), w.ticks(0x14));
        let t1 = w.rng.rand_range(a, b);
        let (a, b) = (w.ticks(0x19), w.ticks(0x1e));
        let t2 = w.rng.rand_range(a, b);
        let speed = w.rng.randf(8.0, 16.0) * DT;
        w.part11(pf(100_000.0), pf(speed), pv(p), pv([0.0; 4]), 0x2f3f_3f7f, c2, t1, t2, 0, 0);
    }
    if p[2] < 15.2 {
        splash(w, p);
        w.mm(id).state = 3;
    }
}

/// gp−0x5000..−0x4fd4: the splash.
fn splash(w: &mut World, p: V) {
    const COUNT: i32 = 50;
    const COLOUR: u32 = 0x40c4_c889;
    const UP: f32 = 4.0;
    const DOWN: f32 = -1.0;
    const OUT1: f32 = 0.0;
    const OUT2: f32 = 2.0;
    const SIZE1: f32 = 3.0;
    const SIZE2: f32 = 6.0;
    const LIFE: f32 = 30.0;
    let (mut v1, mut v2) = ([0.0f32; 4], [0.0f32; 4]);
    for _ in 0..COUNT {
        let a = w.rng.rand_angle();
        let x = w.rng.randf(-3.0, 3.0);
        let y = w.rng.randf(-3.0, 3.0);
        let at = [p[0] + x, p[1] + y, p[2], p[3]];
        v1[2] = UP * DT;
        v1[0] += a.cos() * OUT1;
        v1[1] += a.sin() * OUT1;
        v2[2] = DOWN * DT;
        v2[0] += a.cos() * OUT2;
        v2[1] += a.sin() * OUT2;
        v1[3] = SIZE1;
        v2[3] = SIZE2;
        let mut t = [0; 3];
        for x in &mut t {
            let f = w.rng.randf(LIFE, LIFE + LIFE);
            *x = w.ticks(f as i32);
        }
        fx::part02(w, &crate::particles::type02::Spawn { pos: at, v1, v2, c1: COLOUR, c2: COLOUR, t, def: -1 });
    }
}
