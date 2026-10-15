//! Kerwan's path movers, classes 868, 905, 928: level03 0x294c08 (census U126; 12 created instances). A platform that
//! swings to and fro along the straight line from its path's first point to its last over `ticks(240)` (constant
//! acceleration, then deceleration), waits, and turns back; a second mode (states 3 / 4, not entered from the init)
//! runs point to point, easing to a cruise speed and braking near a stop. It carries its riders every tick. 868 and
//! 905 keep class 905's loop sound 0 playing while they move. Read from the level03 decomp of 0x294c08 (its private
//! callee 0x295190 is `jr ra`); native `f32`.
//!
//! **Pvar block**: +0x60 the platform block (`CarryRiders`), +0xa0 the step, +0xb0 s32 the point, +0xb4 s8 the
//! direction (1 / −1), +0xc0 the path (`0x1b05b0[+0xe0]`; the port reads +0xe0), +0xe4 the speed, +0xe8 s32 the wait
//! timer, +0xec the cruise speed, +0xf0 s32 the wait (ticks), +0xf4 the braking distance, +0xf8 reversed, +0xfc the
//! scale factor, +0x100 s32 the start timer, +0x104 the loop-sound slot, +0x108 stop at every point, +0x10c s32 the
//! tick, +0x110 the length, +0x114 the acceleration.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x294c08 | the old position and rotation kept; `0x295190(m)` | [`update`] (0x295190 is `jr ra`: n/a) |
//! | state 0 | path = +0xe0; direction = +0xf8 ≠ 0 ? −1 : 1; position = the last point (forward) or the first; point = count − 1 / 0; scale = class scale · +0xfc; loop slot −1; `FastDecTimer(+0x100)` out → length = \|first − last\|, tick 0, acceleration = 4·length / (`ticks(240)`²), state 1 | [`update`] |
//! | state 1 | 868 (0x364), 905 (0x389): loop not alive (`0x27a108`) → `PlayClassSoundAs(0, 4, m, 905)` (`0x27a4d8` = L01 0x2a16c0) | [`update`] (`World::play_sound_as`) |
//! | | tick += 1; s = a/2·n² (first half) or length − a/2·(T − n)², T = `ticks(240)`; reversed → length − s; position = first + unit(last − first)·s (the first swing starts at the end state 0 did not put it at: a one-tick jump, the game's) | [`update`] |
//! | | n ≥ T: direction = −direction, the loop released, slot −1, state 2, wait timer = `ticks(+0xf0)` | [`update`] |
//! | state 2 | `FastDecTimer(+0xe8)` out → tick 0, state 1 | [`update`] |
//! | state 3 | next point = (point + count + direction) % count; the end = count − 1 (forward) / 0; step = next − position; distance to the next (+0x108 ≠ 0) or to the end < +0xf4 → `Approach(0.5·dt, (cruise²/+0xf4)·dt²/2, &speed)` (0x24a2b0 = L01 0x270728), else `Approach(cruise·dt, same, &speed)`; step clamped to the speed (0x24e178 = L01 0x2745f0); position += step | [`update`] (`turn::approach`, `clamp_len3`) |
//! | | within 2·speed of the next: point = next; at the end or +0x108 ≠ 0, with a wait ≥ 1: the loop released, slot −1, state 4, wait timer = `ticks(+0xf0)` | [`update`] |
//! | state 4 | `FastDecTimer(+0xe8)` out → state 3 | [`update`] |
//! | every state | `CarryRiders(+0x60, position − old, old rotation, rotation)` (0x24ee28 = L01 0x2755f8) | `triggers::carry_riders` |
//! | | no particle, light, hit, save flag or other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::gold_bolt::fast_dec_timer;
use crate::moby_update::creature::turn::approach;
use crate::moby_update::creature::{add, clamp_len3, dist3, len3, scale, sub, DT, DT2};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::triggers;

pub const UPDATE_FN: u32 = 0x29_4c08;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 3] = [868, 905, 928];
/// The class whose sound table the loop plays (905, 0x389).
pub const SOUND_CLASS: i16 = 0x389;

pub mod pv {
    pub const BLOCK: usize = 0x60;
    pub const STEP: usize = 0xa0;
    pub const POINT: usize = 0xb0;
    pub const DIR: usize = 0xb4;
    pub const PATH: usize = 0xe0;
    pub const SPEED: usize = 0xe4;
    pub const WAIT_T: usize = 0xe8;
    pub const CRUISE: usize = 0xec;
    pub const WAIT: usize = 0xf0;
    pub const BRAKE: usize = 0xf4;
    pub const REVERSED: usize = 0xf8;
    pub const SCALE: usize = 0xfc;
    pub const START_T: usize = 0x100;
    pub const SLOT: usize = 0x104;
    pub const EVERY: usize = 0x108;
    pub const TICK: usize = 0x10c;
    pub const LENGTH: usize = 0x110;
    pub const ACCEL: usize = 0x114;
}

fn path(w: &World, id: MobyId) -> Vec<[f32; 4]> {
    usize::try_from(p::i32(&w.m(id).pvars, pv::PATH)).ok().and_then(|i| w.svc.splines.get(i)).map(|s| s.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}

fn timer(w: &mut World, id: MobyId, o: usize) -> i32 {
    let mut t = p::i32(&w.m(id).pvars, o);
    let r = fast_dec_timer(&mut t);
    p::set_i32(&mut w.mm(id).pvars, o, t);
    r
}

fn release(w: &mut World, id: MobyId) {
    let slot = p::i32(&w.m(id).pvars, pv::SLOT);
    if slot != -1 { w.release_sound(slot, id); }
}

/// Level03 0x294c08 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x118 { return; }
    let (old, rot_old) = (w.m(id).position, w.m(id).rotation);
    let pts = path(w, id);
    if !pts.is_empty() {
        step(w, id, &pts);
    } else if w.m(id).state <= 4 {
        // The game would read the table entry as it is; a missing path leaves it where it is [L].
        w.svc.unported("kerwan mover: no path");
    }
    let (pos, rot) = (w.m(id).position, w.m(id).rotation);
    let d = add(scale(old, -1.0), pos);
    triggers::carry_riders(&mut w.mm(id).pvars, pv::BLOCK, d, rot_old, rot);
}

fn step(w: &mut World, id: MobyId, pts: &[[f32; 4]]) {
    let n = pts.len();
    let (first, last) = (pts[0], pts[n - 1]);
    let t240 = w.ticks(0xf0);
    let st = w.m(id).state;
    match st {
        0 => {
            let dir: i8 = if p::i32(&w.m(id).pvars, pv::REVERSED) != 0 { -1 } else { 1 };
            let cs = super::class_scale(w, w.m(id).o_class);
            let m = w.mm(id);
            m.pvars[pv::DIR] = dir as u8;
            m.position = if dir >= 0 { last } else { first };
            p::set_i32(&mut m.pvars, pv::POINT, if dir >= 0 { n as i32 - 1 } else { 0 });
            m.scale = cs * p::ff(&m.pvars, pv::SCALE);
            p::set_i32(&mut m.pvars, pv::SLOT, -1);
            if timer(w, id, pv::START_T) == 0 { return; }
            let len = dist3(first, last);
            let (a, b) = (w.ticks(0xf0), w.ticks(0xf0));
            let m = w.mm(id);
            p::set_ff(&mut m.pvars, pv::LENGTH, len);
            p::set_i32(&mut m.pvars, pv::TICK, 0);
            p::set_ff(&mut m.pvars, pv::ACCEL, (len * 4.0) / (a * b) as f32);
            m.state = 1;
        }
        1 => {
            let oc = w.m(id).o_class;
            if oc == 0x389 || oc == 0x364 {
                let slot = p::i32(&w.m(id).pvars, pv::SLOT);
                if !w.sound_alive(slot, id) {
                    let h = w.play_sound_as(0, 4, id, SOUND_CLASS);
                    p::set_i32(&mut w.mm(id).pvars, pv::SLOT, h);
                }
            }
            let k = p::i32(&w.m(id).pvars, pv::TICK) + 1;
            p::set_i32(&mut w.mm(id).pvars, pv::TICK, k);
            let (mut a, mut s0, mut j) = (p::ff(&w.m(id).pvars, pv::ACCEL), 0.0, k);
            if t240 / 2 < k {
                a = -a;
                s0 = p::ff(&w.m(id).pvars, pv::LENGTH);
                j = t240 - k;
            }
            let mut s = s0 + a * 0.5 * (j * j) as f32;
            if (w.m(id).pvars[pv::DIR] as i8) < 0 { s = p::ff(&w.m(id).pvars, pv::LENGTH) - s; }
            let v = sub(last, first);
            let l = len3(v);
            let v = if l == 0.0 { v } else { scale(v, s / l) };
            w.mm(id).position = add(first, v);
            if k < t240 { return; }
            let m = w.mm(id);
            m.pvars[pv::DIR] = (m.pvars[pv::DIR] as i8).wrapping_neg() as u8;
            release(w, id);
            wait(w, id, 2);
        }
        2 => {
            if timer(w, id, pv::WAIT_T) == 0 { return; }
            let m = w.mm(id);
            p::set_i32(&mut m.pvars, pv::TICK, 0);
            m.state = 1;
        }
        3 => {
            let dir = w.m(id).pvars[pv::DIR] as i8 as i32;
            let end = if dir >= 0 { n as i32 - 1 } else { 0 };
            let next = (p::i32(&w.m(id).pvars, pv::POINT) + n as i32 + dir).rem_euclid(n as i32);
            let pos = w.m(id).position;
            let to = sub(pts[next as usize], pos);
            let every = p::i32(&w.m(id).pvars, pv::EVERY) != 0;
            let stop = if every { next } else { end };
            let d = dist3(pos, pts[stop as usize]);
            let (cruise, brake) = (p::ff(&w.m(id).pvars, pv::CRUISE), p::ff(&w.m(id).pvars, pv::BRAKE));
            let mut sp = p::ff(&w.m(id).pvars, pv::SPEED);
            let target = if d < brake { DT * 0.5 } else { cruise * DT };
            approach(target, ((cruise * cruise) / brake) * DT2 * 0.5, &mut sp);
            let stepv = clamp_len3(to, sp);
            let pos = add(pos, stepv);
            {
                let m = w.mm(id);
                p::set_ff(&mut m.pvars, pv::SPEED, sp);
                p::set_v4f(&mut m.pvars, pv::STEP, stepv);
                m.position = pos;
            }
            if sp + sp <= dist3(pos, pts[next as usize]) { return; }
            p::set_i32(&mut w.mm(id).pvars, pv::POINT, next);
            if (next != end && !every) || p::i32(&w.m(id).pvars, pv::WAIT) < 1 { return; }
            release(w, id);
            wait(w, id, 4);
        }
        4 if timer(w, id, pv::WAIT_T) != 0 => w.mm(id).state = 3,
        _ => {}
    }
}

/// LAB_00295104: slot −1, the state, wait timer = `ticks(+0xf0)`.
fn wait(w: &mut World, id: MobyId, state: u8) {
    let t = w.ticks(p::i32(&w.m(id).pvars, pv::WAIT));
    let m = w.mm(id);
    p::set_i32(&mut m.pvars, pv::SLOT, -1);
    m.state = state;
    p::set_i32(&mut m.pvars, pv::WAIT_T, t);
}
