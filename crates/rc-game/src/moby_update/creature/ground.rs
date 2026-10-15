//! Ground probes and animation timing for creatures (level01 `GroundHeight` 0x26e618, `CollType` 0x2151d8,
//! `0x2765b0`, `MobyAnimKeyTime` 0x263920).

use super::V;
use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pv, World};

/// A `GroundHeight` probe with the surface the line hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// The hit z, 0 on a miss (what `GroundHeight` returns).
    pub z: f32,
    /// `CollType()` right after: the hit's surface id (`kind & 0x1f`, −1 for 0x1f, a moby primitive or a miss).
    /// On a miss the game reads the collision output the previous query left (memory leftover, not modelled: −1).
    pub surface: i32,
    pub hit: bool,
}

/// `GroundHeight(up, pos, fl)` 0x26e618: `CollLine_Fix((x, y, z + up), (x, y, 0.01), fl | 2, none)`.
pub fn ground(w: &World, p: V, up: f32, fl: u32) -> Ground {
    let a = [p[0], p[1], p[2] + up, p[3]];
    let b = [p[0], p[1], 0.01, p[3]];
    match w.coll_line(pv(a), pv(b), fl | 2, None) {
        Some(o) => Ground { z: o.point[2], surface: o.surface_id(), hit: true },
        None => Ground { z: 0.0, surface: -1, hit: false },
    }
}

/// `round_float_to_decimal_places(x, n)` 0x276530: `trunc((x + 1/(2·10ⁿ))·10ⁿ) / 10ⁿ`.
pub fn round_places(x: f32, n: u32) -> f32 {
    let k = 10i32.pow(n) as f32;
    (((x + 1.0 / (k + k)) * k) as i32) as f32 / k
}

/// `MobyAnimKeyTime` 0x263920: the current key time in frames (key times are stored ×16): frame A's time at
/// `t = 0`; interpolated with frame B's while both keys are of one sequence and A ≤ B; else A's time + t. A snapshot
/// key A reads frame B's header.
pub fn key_time(w: &World, id: MobyId) -> f32 {
    let m = w.m(id);
    let Some(class) = w.classes.anim(m.o_class) else { return 0.0 };
    key_time_of(class, &m.anim)
}

/// [`key_time`] on any animation state of `class` (the hand items' too: the Suck Cannon's firing key).
pub fn key_time_of(class: &rc_formats::moby_anim::MobyAnimClass, s: &rc_formats::moby_anim::AnimState) -> f32 {
    let time = |seq: u8, f: u8| class.frame(seq, f).map(|fr| fr.header.time as i32 as f32).unwrap_or(0.0);
    let ta = if s.seq_a == rc_formats::moby_anim::SNAPSHOT_SEQ { time(s.seq_b, s.frame_b) } else { time(s.seq_a, s.frame_a) };
    if s.t == 0.0 { return ta * 0.0625; }
    if s.seq_a == s.seq_b && s.frame_a <= s.frame_b {
        let tb = time(s.seq_b, s.frame_b);
        return (ta * (1.0 - s.t) + tb * s.t) * 0.0625;
    }
    ta * 0.0625 + s.t
}

/// `0x2765b0(frame, moby)`: the key time crossed `frame` this tick: `frame ≤ k` and
/// `round4(k − frame) < round4(speed·rate)` (moby +0x58 · +0x5c).
pub fn passed_frame(w: &World, id: MobyId, frame: f32) -> bool {
    let k = key_time(w, id);
    let a = round_places(k - frame, 4);
    let s = &w.m(id).anim;
    let b = round_places(s.speed * s.rate, 4);
    frame <= k && a < b
}
