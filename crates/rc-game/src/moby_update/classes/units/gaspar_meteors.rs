//! **Gaspar's meteor shower, classes 263–267** (level09 `0x2eb970`, census U301; one placed: #141, class 265, the
//! controller on path 69) and **the dust they kick up, class 417** (`0x2f0040`). One update for the rocks, by mode
//! (+0x4c):
//!
//! * **0, the controller** (cmd +0xbc 0): once (+0x48), the speed 10·dt, the path's centre (+0x30: the points' mean,
//!   z the first point's − 10), 23 meteors already part way down (`0x2ea778`, each moved `randf(0, d − 32)` toward
//!   its target) and 50 rocks along the path (`0x2eaa50`); then a new meteor when `randi(ticks(40))` is 0, a new rock
//!   every `ticks(15) + randi(ticks(30))` while fewer than 50. It rides its own path, back to the start at the end.
//! * **0, the drifting rocks** (cmd 1): along the parent's path at `(f/20 + 10)·dt` (f = their offset, `randf(−30,
//!   30)`, across the path), faded in over the first segment and out over the last (alpha `dot·128/len`), deleted
//!   at the end (the parent's count − 1). Every rock is drawn at its path point + its offset + the camera − the
//!   path's centre: the shower moves with the camera.
//! * **1, a meteor** (`0x2ea778`): from (1000, 500, 300..350) + a random point within 250 of (256, 256) toward the
//!   ground below that point (`GroundHeight`, at least 25) at `randf(110, 130)·dt`, tumbling. Drawn at most 100 from
//!   the camera (scaled down by the distance). Below 100 it probes 25 ahead (`CollLine` flags 4): a hit above 26.5
//!   bursts it there; reaching its target above 26.5 bursts it on the ground; at the sea (25) it splashes (five dust
//!   puffs 417 a tick under z 27, slowing by 0.9, the splash sound 0 of class 263) and sinks. A burst: 8 type-11
//!   sparks, 10 debris rocks (mode 2), the class-0xc1 sound 6 at volume 0x800, and within 15 of the camera (Ratchet not
//!   in state 0x32) the camera shakes `(64 − d)/100` for `ticks(10)`. Then it counts its state up: burst, 3 more spark
//!   ticks; splashed, two puffs a tick for `ticks(10)` hidden.
//! * **2, debris** (`0x2ea528`): out from the burst at 50·dt plus 0.3 of the meteor's motion, falling at 20·dt²,
//!   slowing 2% a tick, fading 2 a tick.
//!
//! **417** (`0x2efe48` makes it: a puff around the splash, out at `randf(4, 12)·scale` (by how far it lies off the
//! meteor's heading), its anim at half speed): moves by its velocity, slowing 2% a tick, deleted when its
//! animation ends (+0x70 bit 2).
//!
//! Read from the level09 decomp. [L] The splash puffs' centre xy is the last probe point (the game reads its stack
//! slot, which only the probe writes; on the probe's skipped paths the meteor's own xy). The camera position is
//! `World::camera` (level09 0x166f40). Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2eb970` | the rocks (module doc) | [`update`] |
//! | `0x2ea778` / `0x2eaa50` / `0x2ea528` | a meteor / a drifting rock / a debris rock | `meteor` / `rock` / `debris` |
//! | `0x2eaea8` / `0x2ead30` | the meteor's / the debris' update | `fall` / `debris_update` |
//! | `0x294ae0(speed, m, path, &pos, node, dir)` | along a path: past the next point's distance → the next node | `follow` |
//! | `0x2efe48(lo, hi, m, at, v)` / `0x2f0040` | the dust puff 417 / its update | `dust` / [`dust_update`] |

use crate::follow_camera::{ShakeAxis, ShakeRequest};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, add, add_rot, atan, len3, set_len3, sub, V, DT, DT2};
use crate::moby_update::services::{pf, pv, World};
use crate::moby_update::story;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 9;
pub const UPDATE_FN: u32 = 0x2e_b970;
pub const DUST_FN: u32 = 0x2f_0040;
pub const CLASSES: [i16; 5] = [263, 264, 265, 266, 267];
pub const DUST_CLASSES: [i16; 1] = [417];

const ROCK: i16 = 0x107;
const DUST: i16 = 0x1a1;
const LEN: usize = 0x60;
/// 0x15ed60 (the speed, 1.0).
const SPEED: f32 = 1.0;
/// 0x208b00 / 0x208b18: the sparks' colours.
const C1: [u32; 6] = [0x4f00_8fff, 0x4f00_8fff, 0x4f00_7fff, 0x4f00_6fff, 0x2fff_ffff, 0x2fff_ffff];
const C2: [u32; 6] = [0x2f00_5f7f, 0x2f00_4f7f, 0x2f00_3f7f, 0x2f00_004f, 0x2f00_0000, 0x3f00_0000];

mod pvo {
    pub const POS: usize = 0x00;
    pub const SPEED: usize = 0x0c;
    pub const PATH: usize = 0x10;
    pub const NODE: usize = 0x14;
    pub const COUNT: usize = 0x18;
    pub const TIMER: usize = 0x1c;
    pub const AT: usize = 0x20;
    pub const CENTRE: usize = 0x30;
    pub const SPIN_X: usize = 0x40;
    pub const SPIN_Y: usize = 0x44;
    pub const INIT: usize = 0x48;
    pub const MODE: usize = 0x4c;
    pub const PARENT: usize = 0x50;
    pub const SCALE: usize = 0x50;
    pub const VEL: usize = 0x54;
    pub const SOUND: usize = 0x58;
}

fn path(w: &World, i: i32) -> Vec<V> {
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map(|v| v.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}
fn cam(w: &World) -> V {
    let c = w.camera.map(|x| f32::from_bits(x.0));
    [c[0], c[1], c[2], 0.0]
}
fn scale(v: V, k: f32) -> V { v.map(|x| x * k) }
fn new_rock(w: &mut World) -> Option<MobyId> {
    let k = w.rng.randi(4) as i16;
    let m = w.create_moby(ROCK + k)?;
    story::pvars(w, m, LEN);
    Some(m)
}

/// The draw position at most 100 from the camera, the scale shrunk with it.
fn place(w: &mut World, id: MobyId, p: V, base: f32, always: bool) {
    let cm = cam(w);
    let rel = sub(p, cm);
    let l = len3(rel);
    let (pos, s) = if always || l <= 100.0 { (p, base) } else { (add(set_len3(rel, 100.0), cm), base * (100.0 / l)) };
    let m = w.mm(id);
    m.position = pos;
    m.scale = s;
}

/// `0x2ea778`: a meteor.
fn meteor(w: &mut World) -> Option<MobyId> {
    let m = new_rock(w)?;
    let r = w.rng.randf(0.0, 250.0);
    let a = w.rng.randf(-PI, PI);
    let mut at = [a.cos() * r + 256.0, a.sin() * r + 256.0, 100.0, 0.0];
    let z = w.rng.randf(200.0, 250.0);
    let p = [1000.0 + at[0], 500.0 + at[1], z + at[2], 1.0];
    c::set_pv4(w, m, pvo::POS, p);
    let k = w.rng.randf(0.5, 1.0);
    let s = w.m(m).scale * k;
    w.mm(m).scale = s;
    let lim = f32::from_bits(0x3c80_adfd);
    let sx = w.rng.randf(-lim, lim);
    c::set_pf(w, m, pvo::SPIN_X, sx);
    let sy = w.rng.randf(-lim, lim);
    c::set_pf(w, m, pvo::SPIN_Y, sy);
    at[2] = f32::from_bits(w.ground_height(pf(0.5), pv(at), 0).0).max(25.0);
    c::set_pv4(w, m, pvo::AT, at);
    {
        let mo = w.mm(m);
        mo.update_dist = 0xff;
        mo.visible = 1;
        mo.draw_dist = 0xff;
    }
    c::set_pi32(w, m, pvo::MODE, 1);
    let v = w.rng.randf(110.0, 130.0) * DT;
    c::set_pf(w, m, pvo::VEL, v);
    c::set_pf(w, m, pvo::SCALE, s);
    let dx = w.rng.randf(-100.0, 100.0);
    let dy = w.rng.randf(-100.0, 100.0);
    let mut p = c::pv4(w, m, pvo::POS);
    p[0] += dx;
    p[1] += dy;
    c::set_pv4(w, m, pvo::POS, p);
    place(w, m, p, s, false);
    c::set_pi32(w, m, pvo::SOUND, -1);
    w.build_matrix(m);
    Some(m)
}

/// `0x2eaa50`: a rock drifting along the controller's path.
fn rock(w: &mut World, parent: MobyId) -> Option<MobyId> {
    let m = new_rock(w)?;
    let pp = w.m(parent).position;
    w.mm(m).position = pp;
    let n = c::pi32(w, parent, pvo::COUNT) + 1;
    c::set_pi32(w, parent, pvo::COUNT, n);
    c::set_pi32(w, m, pvo::PARENT, parent as i32 + 1);
    c::set_pi32(w, m, pvo::INIT, 1);
    let pi = c::pi32(w, parent, pvo::PATH);
    c::set_pi32(w, m, pvo::PATH, pi);
    c::set_pi32(w, m, pvo::NODE, 0);
    {
        let mo = w.mm(m);
        mo.visible = 1;
        mo.cmd = 1;
        mo.update_dist = 0xff;
        mo.draw_dist = 0xff;
        mo.ambient = [0x80, 0x80, 0x80, mo.ambient[3]];
        mo.alpha = 0;
    }
    let k = w.rng.randf(f32::from_bits(0x3ecc_cccd), f32::from_bits(0x3f59_999a));
    w.mm(m).scale *= k;
    let lim = f32::from_bits(0x3bab_92a6);
    let sx = w.rng.randf(-lim, lim);
    c::set_pf(w, m, pvo::SPIN_X, sx);
    let sy = w.rng.randf(-lim, lim);
    c::set_pf(w, m, pvo::SPIN_Y, sy);
    let pts = path(w, pi);
    let n = pts.len();
    if 2 <= n {
        let f = w.rng.randf(-30.0, 30.0);
        let d_end = sub(pts[n - 1], pts[n - 2]);
        let mut a = cross(sub(pts[1], pts[0]), d_end);
        a[2] = 0.0;
        let mut off = set_len3(a, f);
        let b = cross(a, sub(pts[n - 1], pts[0]));
        let r = w.rng.randf(0.0, 5.0);
        off = add(off, set_len3(b, r));
        c::set_pv4(w, m, pvo::POS, off);
        c::set_pv4(w, m, pvo::AT, pts[0]);
        let centre = c::pv4(w, parent, pvo::CENTRE);
        c::set_pv4(w, m, pvo::CENTRE, centre);
        let p = sub(add(add(pts[0], off), cam(w)), centre);
        w.mm(m).position = p;
        c::set_pf(w, m, pvo::SPEED, (f / 20.0 + 10.0) * DT);
        w.mm(m).scale *= f / 100.0 + 1.0;
    }
    w.build_matrix(m);
    Some(m)
}

fn cross(a: V, b: V) -> V { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0], 0.0] }

/// `0x2ea528`: a debris rock from meteor `m` (its motion `v`).
fn debris(w: &mut World, m: MobyId, v: V) {
    let Some(d) = new_rock(w) else { return };
    let x = w.rng.randf(-2.0, 2.0);
    let y = w.rng.randf(-2.0, 2.0);
    let z = w.rng.randf(-2.0, 2.0);
    let o = [x, y, z, 0.0];
    let at = add(o, c::pv4(w, m, pvo::POS));
    c::set_pv4(w, d, pvo::AT, at);
    let vel = scale(add(set_len3(o, DT * 50.0), v), SPEED * -0.7 + 1.0);
    c::set_pv4(w, d, pvo::POS, vel);
    {
        let mo = w.mm(d);
        mo.update_dist = 0xff;
        mo.visible = 1;
        mo.draw_dist = 0xff;
        mo.alpha = 0x7f;
    }
    c::set_pi32(w, d, pvo::MODE, 2);
    let k = w.rng.randf(0.125, 0.25);
    let s = w.m(m).scale * k;
    w.mm(d).scale = s;
    c::set_pf(w, d, pvo::SCALE, s);
    let lim = f32::from_bits(0x3d56_7750);
    let sx = w.rng.randf(-lim, lim);
    c::set_pf(w, d, pvo::SPIN_X, sx);
    let sy = w.rng.randf(-lim, lim);
    c::set_pf(w, d, pvo::SPIN_Y, sy);
    place(w, d, at, s, false);
}

/// `0x2efe48`: a dust puff 417 around `at`, `v` the meteor's motion.
fn dust(w: &mut World, lo: f32, hi: f32, at: V, v: V) {
    let Some(m) = w.create_moby(DUST) else { return };
    story::pvars(w, m, 0x10);
    let a = w.rng.randf(-PI, PI);
    let r = w.rng.randf(lo, hi);
    let d = c::diff_rots(atan(v[0], v[1]), a);
    let mut p = [a.cos() * r, a.sin() * r, 0.0, 0.0];
    let mut vel = set_len3(p, d * DT);
    vel = add(vel, set_len3(v, (PI - d) * (PI - d) * 0.25 * DT));
    vel[2] = 0.0;
    p = add(add(p, at), v);
    vel = scale(vel, f32::from_bits(0x3eb3_3333));
    c::set_pv4(w, m, 0, vel);
    {
        let mo = w.mm(m);
        mo.position = [p[0], p[1], p[2], mo.position[3]];
        mo.anim.speed *= 0.5;
        mo.rotation[2] = atan(vel[0], vel[1]);
        mo.update_dist = 0xff;
        mo.draw_dist = 0xff;
        mo.visible = 1;
        mo.scale *= (PI - d) + (PI - d) + 1.0;
    }
    w.build_matrix(m);
}

/// Level09 `0x2f0040`: the dust puff 417.
pub fn dust_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x10);
    let v = c::pv4(w, id, 0);
    let p = add(w.m(id).position, v);
    w.mm(id).position = p;
    c::set_pv4(w, id, 0, scale(v, SPEED * f32::from_bits(0xbca3_d700) + 1.0));
    if w.m(id).anim.flags & 2 != 0 { w.delete_moby(id); }
}

/// Eight (or three) type-11 sparks at `p` (jittered by `jit` when some), the base velocity `base`.
fn sparks(w: &mut World, n: usize, size: f32, p: V, jit: Option<f32>, base: V, near: f32) {
    for _ in 0..n {
        let s = w.rng.randf(8.0, 10.0) * DT;
        let at = match jit {
            Some(j) => {
                let x = w.rng.randf(-j, j);
                let y = w.rng.randf(-j, j);
                let z = w.rng.randf(-j, j);
                add([x, y, z, 0.0], p)
            }
            None => p,
        };
        let c1 = C1[w.rng.randi(6) as usize % 6];
        let c2 = C2[w.rng.randi(6) as usize % 6];
        let (a, b) = (w.ticks(0xf), w.ticks(0x14));
        let t1 = w.rng.rand_range(a, b);
        let (a, b) = (w.ticks(0x19), w.ticks(0x1e));
        let t2 = w.rng.rand_range(a, b);
        w.part11(pf(size), pf(s - near * DT), pv(at), pv(base), c1, c2, t1, t2, 0, 0);
    }
}

/// The burst: the class-0xc1 sound 6 at volume 0x800.
fn burst_sound(w: &mut World, id: MobyId) {
    let s = w.play_sound_as(6, 0, id, 0xc1);
    c::set_pi32(w, id, pvo::SOUND, s);
    if s != 0 { w.set_volume(s, 0x800); }
}

/// `0x2eaea8`: a meteor (module doc).
fn fall(w: &mut World, id: MobyId) {
    let mut p = c::pv4(w, id, pvo::POS);
    let at = c::pv4(w, id, pvo::AT);
    let d = sub(at, p);
    let ahead = set_len3(d, 25.0);
    let vel = set_len3(d, c::pf(w, id, pvo::VEL));
    p = add(p, vel);
    c::set_pv4(w, id, pvo::POS, p);
    let cm = cam(w);
    let l = len3(sub(p, cm));
    let base = c::pf(w, id, pvo::SCALE);
    place(w, id, p, base, p[2] <= 27.0);
    let x = add_rot(w.m(id).rotation[0], c::pf(w, id, pvo::SPIN_X));
    let y = add_rot(x, c::pf(w, id, pvo::SPIN_Y));
    w.mm(id).rotation[0] = x;
    w.mm(id).rotation[1] = y;
    let near = if l < 7.0 { 7.0 - l } else { 0.0 };
    let state = w.m(id).state;
    if state != 0 {
        w.mm(id).state = state.wrapping_add(1);
        if at[2] == 25.0 {
            w.mm(id).draw_dist = 0;
            w.mm(id).visible = 0;
            let puff = [p[0], p[1], 25.0, p[3]];
            for _ in 0..2 { dust(w, base * 4.0, base * 12.0, puff, vel); }
            if w.ticks(10) < w.m(id).state as i32 { w.delete_moby(id); }
            return;
        }
        sparks(w, 3, f32::from_bits(0x4992_7c00), p, Some(1.0), scale(vel, 0.25), near);
        if 3 <= w.m(id).state { w.delete_moby(id); }
        return;
    }
    let mut probe = (p[0], p[1]);
    let mut burst = false;
    let mut far = true;
    if p[2] < 100.0 && p[0] < 500.0 && p[1] < 500.0 {
        let x = w.rng.randf(-5.0, 5.0);
        let y = w.rng.randf(-5.0, 5.0);
        let z = w.rng.randf(-5.0, 5.0);
        let a = add([x, y, z, 0.0], p);
        probe = (a[0], a[1]);
        let b = add(a, ahead);
        let hit = w.coll_line(pv(a), pv(b), 4, Some(id));
        if let Some(h) = hit.filter(|h| 26.5 < h.point[2]) {
            let _ = h;
            far = false;
            burst = true;
            sparks(w, 8, f32::from_bits(0x48c3_5000), p, None, scale(vel, 0.1), near);
        }
    }
    if far {
        if at[2] == 25.0 && p[2] < 27.0 {
            let puff = [probe.0, probe.1, 25.0, p[3]];
            for _ in 0..5 { dust(w, base * 4.0, base * 12.0, puff, vel); }
            let v = c::pf(w, id, pvo::VEL) * (SPEED * f32::from_bits(0xbdcc_cccd) + 1.0);
            c::set_pf(w, id, pvo::VEL, v);
            if c::pi32(w, id, pvo::SOUND) < 1 {
                let s = w.play_sound_as(0, 0, id, ROCK);
                c::set_pi32(w, id, pvo::SOUND, s);
            }
        }
        if at[2] <= p[2] { return; }
        if at[2] <= 26.5 {
            w.mm(id).state = state.wrapping_add(1);
            return;
        }
        burst = true;
        sparks(w, 8, f32::from_bits(0x4992_7c00), p, Some(3.0), scale(vel, 0.5), near);
    }
    if burst {
        for _ in 0..10 { debris(w, id, vel); }
        burst_sound(w, id);
    }
    if w.hero.state != 0x32 {
        let d = len3(sub(p, cm));
        if d < 15.0 {
            let t = w.ticks(10);
            w.shake_camera(ShakeRequest { axis: ShakeAxis::Up, amp: (64.0 - d) * 0.01, ticks: t });
        }
    }
    w.mm(id).state = state.wrapping_add(1);
}

/// `0x2ead30`: a debris rock.
fn debris_update(w: &mut World, id: MobyId) {
    if w.m(id).alpha < 3 {
        w.delete_moby(id);
        return;
    }
    w.mm(id).alpha -= 2;
    let mut v = scale(c::pv4(w, id, pvo::POS), SPEED * f32::from_bits(0xbca3_d700) + 1.0);
    v[2] -= DT2 * 20.0;
    c::set_pv4(w, id, pvo::POS, v);
    let p = add(c::pv4(w, id, pvo::AT), v);
    c::set_pv4(w, id, pvo::AT, p);
    let base = c::pf(w, id, pvo::SCALE);
    place(w, id, p, base, false);
    let x = add_rot(w.m(id).rotation[0], c::pf(w, id, pvo::SPIN_X));
    let y = add_rot(x, c::pf(w, id, pvo::SPIN_Y));
    w.mm(id).rotation[0] = x;
    w.mm(id).rotation[1] = y;
}

/// `0x294ae0(speed, m, path, &pos, node, dir)`.
fn follow(pts: &[V], speed: f32, me: V, node: usize) -> (V, usize) {
    let n = pts.len();
    let next = |i: usize| if i == n - 1 { 0 } else { i + 1 };
    let mut node = node;
    let mut j = next(node);
    let mut d = sub(pts[j], pts[node]);
    let seg = len3(d);
    d = set_len3(d, 1.0);
    let mut along = c::dot3(sub(me, pts[node]), d) + speed;
    if seg < along {
        let k = next(j);
        d = set_len3(sub(pts[k], pts[j]), 1.0);
        along -= seg;
        node = j;
        j = k;
    }
    let _ = j;
    (add(set_len3(d, along), pts[node]), node)
}

/// Level09 `0x2eb970` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, LEN);
    match c::pi32(w, id, pvo::MODE) {
        2 => return debris_update(w, id),
        1 => return fall(w, id),
        _ => {}
    }
    let pi = c::pi32(w, id, pvo::PATH);
    let pts = path(w, pi);
    if w.m(id).cmd == 0 {
        if c::pi32(w, id, pvo::INIT) == 0 {
            c::set_pf(w, id, pvo::SPEED, DT * 10.0);
            c::set_pv4(w, id, pvo::CENTRE, [0.0; 4]);
            if 0 <= pi && !pts.is_empty() {
                let mut s = [0.0f32; 4];
                for q in &pts { s = add(s, *q); }
                let mut centre = scale(s, 1.0 / pts.len() as f32);
                centre[2] = pts[0][2] - 10.0;
                c::set_pv4(w, id, pvo::CENTRE, centre);
                for _ in 0..23 {
                    let Some(m) = meteor(w) else { continue };
                    let p = c::pv4(w, m, pvo::POS);
                    let d = sub(c::pv4(w, m, pvo::AT), p);
                    let r = w.rng.randf(0.0, len3(d) - 32.0);
                    c::set_pv4(w, m, pvo::POS, add(p, set_len3(d, r)));
                }
                for _ in 0..50 {
                    let Some(m) = rock(w, id) else { continue };
                    let k = w.rng.randi(pts.len() as i32 - 2).max(0) as usize;
                    let t = w.rng.randf(0.0, 1.0);
                    let at = add(scale(sub(pts[k + 1], pts[k]), t), pts[k]);
                    c::set_pi32(w, m, pvo::NODE, k as i32);
                    c::set_pv4(w, m, pvo::AT, at);
                    let off = c::pv4(w, m, pvo::POS);
                    let centre = c::pv4(w, m, pvo::CENTRE);
                    let p = sub(add(add(pts[0], off), cam(w)), centre);
                    w.mm(m).position = p;
                }
            }
            c::set_pi32(w, id, pvo::INIT, 1);
        }
        if w.m(id).cmd == 0 {
            let t = w.ticks(0x28);
            if w.rng.randi(t) == 0 { meteor(w); }
        }
    }
    w.mm(id).update_dist = 0xff;
    w.mm(id).draw_dist = 0xff;
    if pi < 0 || pts.len() < 2 { return; }
    let at = c::pv4(w, id, pvo::AT);
    w.mm(id).position = at;
    if w.m(id).cmd == 0 && c::pi32(w, id, pvo::COUNT) < 50 && c::dec_timer_pvar_i32(w, id, pvo::TIMER) != 0 {
        let a = w.ticks(0xf);
        let b = w.ticks(0x1e);
        let r = w.rng.randi(b);
        c::set_pi32(w, id, pvo::TIMER, a + r);
        rock(w, id);
    }
    let x = add_rot(w.m(id).rotation[0], c::pf(w, id, pvo::SPIN_X));
    let y = add_rot(w.m(id).rotation[1], c::pf(w, id, pvo::SPIN_Y));
    w.mm(id).rotation[0] = x;
    w.mm(id).rotation[1] = y;
    let n = pts.len();
    let node = (c::pi32(w, id, pvo::NODE).max(0) as usize).min(n - 1);
    let (mut p, mut node) = follow(&pts, c::pf(w, id, pvo::SPEED), w.m(id).position, node);
    if node == n - 1 {
        if w.m(id).cmd == 0 {
            node = 0;
            p = pts[0];
        } else {
            if let Some(par) = usize::try_from(c::pi32(w, id, pvo::PARENT) - 1).ok().filter(|&m| m < w.table.mobys.len()) {
                if LEN <= w.m(par).pvars.len() {
                    let k = c::pi32(w, par, pvo::COUNT) - 1;
                    c::set_pi32(w, par, pvo::COUNT, k);
                }
            }
            w.delete_moby(id);
        }
    }
    c::set_pi32(w, id, pvo::NODE, node as i32);
    let edge = if node == 0 {
        Some((pts[1], pts[0]))
    } else if node == n - 2 {
        Some((pts[n - 2], pts[n - 1]))
    } else {
        None
    };
    let alpha = match edge {
        Some((a, b)) => {
            let d = sub(a, b);
            let l = len3(d);
            let dot = c::dot3(sub(p, b), set_len3(d, 1.0)).max(0.0);
            let k = ((dot * 128.0) / l) as i32 as u32 as u8;
            if k <= 0x80 { k } else { 0x80 }
        }
        None => 0x80,
    };
    w.mm(id).alpha = alpha;
    c::set_pv4(w, id, pvo::AT, p);
    let off = c::pv4(w, id, pvo::POS);
    let centre = c::pv4(w, id, pvo::CENTRE);
    let pos = sub(add(add(p, off), cam(w)), centre);
    w.mm(id).position = [pos[0], pos[1], pos[2], w.m(id).position[3]];
}
