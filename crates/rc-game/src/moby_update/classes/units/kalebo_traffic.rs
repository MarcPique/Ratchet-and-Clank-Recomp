//! Kalebo's air traffic, classes 1667–1671: level16 0x2e76b8 (census U523; 90 created instances). A group of
//! vehicles spaced along one path: the first member's init places the whole group (shuffled) at even t, each
//! following the one placed before it; every tick a vehicle rides the path at its own speed (eased toward a target
//! speed), holds back from the one ahead, and, while out of view, jumps ahead to a new out-of-view spot. The odd
//! classes (1667, 1669, 1671) trail exhaust puffs (type 22) while drawn near the camera; the even ones (1668, 1670)
//! keep their class loop sound 0 playing. Read from the level16 decomp of 0x2e76b8, 0x2e7ba0 and 0x2e7a30 and the
//! level's data words (gp−0x4d98..−0x4d58); native `f32`.
//!
//! **Pvar block**: +0x00 s32 the path (`0x1b0930[i]`), +0x04 f32 t, +0x08 f32 the speed (segments per tick), +0x0c
//! f32 count − 1, +0x10 the vehicle ahead (a moby pointer; here its index), +0x14 f32 the height offset, +0x18 f32
//! the target speed, +0x1c s16 the re-place counter, +0x1e s16 the loop-sound slot.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2e76b8 | odd class: the position before the move is kept for the exhaust; even class: `SoundIsAlive(m, +0x1e)` else +0x1e = `PlayClassSound(0, 4, m)` | [`update`] |
//! | state 0 | group −1 → `DeleteMoby`; else 0x2e7ba0 | [`init_group`] |
//! | 0x2e7ba0 | the group's list (`0x1abcc0[g]`); path −1 → nothing; each member in a random order (`randi(n − 1)` over the first n − 1, the last one last): update distance 0xff, collision off, draw distance 250 (gp−0x4d70), scale ·= 1.5 (gp−0x4d6c), ahead = the member placed before (the list's last for the first), count − 1, t = k·(count − 1)/n for k = n…1, height `randf(4.25, −4.25)` (gp−0x4d84 / −0x4d80), speed = target = `randf(5, 15)` (gp−0x4d90 / −0x4d8c) · dt, state 1 | [`init_group`] |
//! | state 1 | `0x25e338(t, path, 0, &pos, &rot, 0)` ([`crate::path::pose`]); z += height; t += speed; `Approach(target, 10 (gp−0x4d88) · dt², &speed)` | [`fly`] |
//! | | gap = ahead.t − t (+ count − 1 when negative); gap < 3 (gp−0x4d98) and ahead.target < target → the targets swap; gap < 1.5 → both speeds `Approach` their targets by 50 · dt² | [`fly`] |
//! | | not drawn: the loop sound alive → counter = 30; else gap > 6 (gp−0x4d94) + 1 → a new height `randf(4.25, −4.25)`; new t = t + 100 (gp−0x4d74) · dt when the one ahead is not drawn or the counter is 0, else ahead.t − `randf(3, 6)` and counter −= 1; wrapped at count − 1; `0x25e338(new t, …, 1)` (no rotation) + the height, radius 2, `FastBSphereCheck(250, …)` = −1 → t, height and a new target speed `randf(5, 15)` · dt are taken | [`fly`] (`BSphereView::culled`; no view: out of view) |
//! | | drawn → counter = 30; t wrapped at count − 1 | [`fly`] |
//! | 0x2e7a30 (odd classes) | drawn and tick parity = the moby slot's; camera (0x1671c0) within 75: one type-22 puff at pos − 0.5·row 0, velocity `0x277b50(1 (gp−0x4d5c) · dt, angle, angle)` + row 0 · 0.4 (gp−0x4d60) · the distance moved, size 1.5 (gp−0x4d58) · 210000, colours 0x60d0d080 / 0x00801000 (gp−0x4d68 / −0x4d64), life 35 ticks | [`exhaust`] (`projectile::part22`) |
//! | | no hit, light or save flag | — |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::polar;
use crate::moby_update::creature::turn::approach;
use crate::moby_update::creature::{add, dist3, pf, pi32, set_pf, DT, DT2};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{pvar as p, World};
use crate::particles::type22;
use crate::path::pose;

pub const UPDATE_FN: u32 = 0x2e_76b8;
pub const REFERENCE_LEVEL: u32 = 16;
pub const CLASSES: [i16; 5] = [1667, 1668, 1669, 1670, 1671];

/// The level16 data words the code reads (gp = 0x166c00).
pub mod k {
    /// gp−0x4d98: the gap below which the targets swap.
    pub const SWAP_GAP: f32 = 3.0;
    /// gp−0x4d94: the gap (+ 1) above which an unseen vehicle re-places itself.
    pub const REPLACE_GAP: f32 = 6.0;
    /// gp−0x4d90 / −0x4d8c: the target speed range, units per second.
    pub const SPEED: (f32, f32) = (5.0, 15.0);
    /// gp−0x4d88: the speed ease per tick (· dt²).
    pub const EASE: f32 = 10.0;
    /// gp−0x4d84 / −0x4d80: the height range (as the code orders it).
    pub const HEIGHT: (f32, f32) = (4.25, -4.25);
    /// gp−0x4d74: the jump ahead (· dt).
    pub const JUMP: f32 = 100.0;
    /// gp−0x4d70: the draw distance and the view radius.
    pub const DRAW_DIST: i16 = 250;
    /// gp−0x4d6c: the scale factor.
    pub const SCALE: f32 = 1.5;
    /// gp−0x4d68 / −0x4d64: the puff colours.
    pub const PUFF_C1: u32 = 0x60d0_d080;
    pub const PUFF_C2: u32 = 0x0080_1000;
    /// gp−0x4d60: the puff's share of the distance moved.
    pub const PUFF_DRIFT: f32 = 0.4;
    /// gp−0x4d5c: the puff's spread speed (· dt).
    pub const PUFF_SPREAD: f32 = 1.0;
    /// gp−0x4d58: the puff size (· 210000).
    pub const PUFF_SIZE: f32 = 1.5;
}

pub mod pv {
    pub const PATH: usize = 0x00;
    pub const T: usize = 0x04;
    pub const SPEED: usize = 0x08;
    pub const LAST: usize = 0x0c;
    pub const AHEAD: usize = 0x10;
    pub const HEIGHT: usize = 0x14;
    pub const TARGET: usize = 0x18;
    pub const COUNTER: usize = 0x1c;
    pub const SOUND: usize = 0x1e;
}

fn path(w: &World, id: MobyId) -> Vec<[u32; 4]> {
    usize::try_from(pi32(w, id, pv::PATH)).ok().and_then(|i| w.svc.splines.get(i)).cloned().unwrap_or_default()
}

/// Level16 0x2e76b8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let odd = w.m(id).o_class & 1 == 1;
    let before = w.m(id).position;
    let mut sound_on = false;
    if !odd {
        let slot = p::i16(&w.m(id).pvars, pv::SOUND) as i32;
        if w.sound_alive(slot, id) {
            sound_on = true;
        } else {
            let h = w.play_sound(0, 4, id);
            p::set_i16(&mut w.mm(id).pvars, pv::SOUND, h as i16);
        }
    }
    match w.m(id).state {
        0 => {
            if w.m(id).group == -1 {
                w.delete_moby(id);
                return;
            }
            init_group(w, id);
        }
        1 => fly(w, id, sound_on),
        _ => {}
    }
    if odd { exhaust(w, id, before); }
}

/// 0x2e7ba0 (module doc): the whole group placed along the path.
pub fn init_group(w: &mut World, id: MobyId) {
    let list = group_ids(w, w.m(id).group);
    if list.is_empty() || pi32(w, id, pv::PATH) == -1 { return; }
    let n = list.len();
    let last = (path(w, id).len() as f32) - 1.0;
    let mut slots: Vec<Option<MobyId>> = list.iter().map(|&m| Some(m)).collect();
    let mut prev = list[n - 1];
    let mut k = n as i32;
    for (i, &member) in list.iter().enumerate() {
        let m = if i == n - 1 {
            member
        } else {
            let mut j = w.rng.randi((n - 1) as i32) as usize;
            while slots[j].is_none() { j = (j + 1) % (n - 1); }
            slots[j].take().unwrap()
        };
        let t = k as f32 * (last / n as f32);
        let height = w.rng.randf(k::HEIGHT.0, k::HEIGHT.1);
        let speed = w.rng.randf(k::SPEED.0, k::SPEED.1) * DT;
        let Some(mm) = w.table.mobys.get_mut(m) else { continue };
        if mm.pvars.len() < 0x20 { continue; }
        mm.update_dist = 0xff;
        mm.has_collision = false;
        mm.draw_dist = k::DRAW_DIST;
        mm.scale *= k::SCALE;
        p::set_i32(&mut mm.pvars, pv::AHEAD, prev as i32);
        p::set_ff(&mut mm.pvars, pv::LAST, last);
        p::set_ff(&mut mm.pvars, pv::T, t);
        p::set_ff(&mut mm.pvars, pv::HEIGHT, height);
        p::set_ff(&mut mm.pvars, pv::TARGET, speed);
        p::set_ff(&mut mm.pvars, pv::SPEED, speed);
        mm.state = 1;
        prev = m;
        k -= 1;
    }
}

/// State 1 (module doc).
fn fly(w: &mut World, id: MobyId, sound_on: bool) {
    let pts = path(w, id);
    let ahead = pi32(w, id, pv::AHEAD);
    let Some(ahead) = usize::try_from(ahead).ok().filter(|&a| w.table.mobys.get(a).is_some_and(|m| m.pvars.len() >= 0x20)) else { return };
    let last = pf(w, id, pv::LAST);
    let t0 = pf(w, id, pv::T);
    let (pos, rot) = pose(&pts, false, t0, true);
    let height = pf(w, id, pv::HEIGHT);
    {
        let m = w.mm(id);
        m.position = pos;
        m.rotation = rot;
        m.position[2] += height;
    }
    let mut speed = pf(w, id, pv::SPEED);
    let mut t = t0 + speed;
    let mut target = pf(w, id, pv::TARGET);
    approach(target, k::EASE * DT2, &mut speed);
    let mut gap = pf(w, ahead, pv::T) - t;
    if gap < 0.0 { gap += last; }
    if gap < k::SWAP_GAP {
        let at = pf(w, ahead, pv::TARGET);
        if at < target {
            set_pf(w, ahead, pv::TARGET, target);
            target = at;
        }
    }
    if gap < 1.5 {
        approach(target, 50.0 * DT2, &mut speed);
        let mut asp = pf(w, ahead, pv::SPEED);
        approach(pf(w, ahead, pv::TARGET), 50.0 * DT2, &mut asp);
        set_pf(w, ahead, pv::SPEED, asp);
    }
    let mut counter = p::i16(&w.m(id).pvars, pv::COUNTER);
    let mut height = height;
    if w.m(id).visible == 0 {
        if sound_on {
            counter = 30;
        } else if k::REPLACE_GAP + 1.0 < gap {
            let dz = w.rng.randf(k::HEIGHT.0, k::HEIGHT.1);
            let mut nt = if w.m(ahead).visible == 0 || counter == 0 {
                t + k::JUMP * DT
            } else {
                let d = w.rng.randf(k::SWAP_GAP, k::REPLACE_GAP);
                counter -= 1;
                pf(w, ahead, pv::T) - d
            };
            if nt < 0.0 { nt += last; }
            let (mut q, _) = pose(&pts, false, nt, false);
            q[2] += dz;
            q[3] = 2.0;
            let out = w.view.map(|v| v.culled(k::DRAW_DIST as f32, q)).unwrap_or(true);
            if out {
                let ts = w.rng.randf(k::SPEED.0, k::SPEED.1) * DT;
                t = nt;
                height = dz;
                target = ts;
            }
        }
    } else {
        counter = 30;
    }
    if last < t { t -= last; }
    let m = w.mm(id);
    p::set_ff(&mut m.pvars, pv::T, t);
    p::set_ff(&mut m.pvars, pv::SPEED, speed);
    p::set_ff(&mut m.pvars, pv::TARGET, target);
    p::set_ff(&mut m.pvars, pv::HEIGHT, height);
    p::set_i16(&mut m.pvars, pv::COUNTER, counter);
}

/// The tick-parity phase of a moby slot: the game compares the tick counter with the moby's address >> 8 (the
/// mobys are 0x100 apart from 0x15ffd8, so slot i has phase i + 0x15ff).
pub fn slot_phase(id: MobyId) -> u64 { id as u64 + 0x15ff }

/// 0x2e7a30 (module doc): the exhaust puff of the odd classes.
fn exhaust(w: &mut World, id: MobyId, before: [f32; 4]) {
    if w.m(id).visible == 0 || (w.counter & 1) != (slot_phase(id) & 1) { return; }
    let m = w.m(id);
    let pos = m.position;
    let cam = w.camera_point();
    if dist3(pos, [cam[0], cam[1], cam[2], 0.0]) >= 75.0 { return; }
    let moved = dist3(pos, before);
    let row0 = rc_formats::moby_light::rotation_rows([m.rotation[0], m.rotation[1], m.rotation[2]])[0].map(f32::from_bits);
    let drift = row0.map(|x| x * (k::PUFF_DRIFT * moved));
    let l = (row0[0] * row0[0] + row0[1] * row0[1] + row0[2] * row0[2]).sqrt();
    let back = if l == 0.0 { [0.0; 4] } else { row0.map(|x| x * (-0.5 / l)) };
    let at = add(back, pos);
    let (a, b) = (w.rng.rand_angle(), w.rng.rand_angle());
    let vel = add(polar(k::PUFF_SPREAD * DT, a, b), drift);
    let life = w.ticks(0x23);
    crate::moby_update::creature::projectile::part22(w, &type22::Spawn { size: k::PUFF_SIZE * 210_000.0, pos: [at[0], at[1], at[2], pos[3]], vel, c1: k::PUFF_C1, c2: k::PUFF_C2, life });
}
