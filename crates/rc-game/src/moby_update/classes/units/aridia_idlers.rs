//! Aridia's animated props: classes 656 (level02 0x2dca10, census U99: 2 placed), 675 (0x2dd370, U101: 1) and 732
//! (0x2df1a8, U104: 3). Animation only: each plays a clip, holds a pose for a random time (a byte timer in `cmd`,
//! +0xbc), plays the next clip; 732 also hums while it moves and sounds at the end of a clip. Read from the level02
//! decomp. Native `f32`; the `rand` draws in the game's order. They read key B (+0x53, `seq_b`) and the wrap bit
//! (+0x70 bit 1); `fun_00212f90` is `MobyAnimBlend`, `0x210000` the byte `FastDecTimer` (= L01 0x220ed8).
//!
//! | class | key B | what | port |
//! |---|---|---|---|
//! | 656 | 0 | wrapped → blend 1; `cmd` = trunc(scale·`randf(180, 240)`) | [`update_656`] |
//! | | 1 | the timer out and not on 2 → blend 2 | [`update_656`] |
//! | | 2 | wrapped → blend 3; the timer as 0 | [`update_656`] |
//! | | 3 | the timer out and not on 0 → blend 0 | [`update_656`] |
//! | 675 | 0 | wrapped → blend 2 over `ticks(5)`; `cmd` = trunc(scale·`randf(60, 180)`) | [`update_675`] |
//! | | 2 | the timer out and not on 3 → blend 3 (`ticks(5)`) | [`update_675`] |
//! | | 3 | wrapped → blend 7 (`ticks(5)`); the timer as 0 | [`update_675`] |
//! | | 7 | the timer out and not on 0 → blend 0 (`ticks(5)`) | [`update_675`] |
//! | 732 | every tick | anim speed (+0x58) = 0.35 (gp−0x4f98) | [`update_732`] |
//! | | 0 | the timer out: blend 1 (`ticks(10)`, unless on it); `cmd` = 2 (the next rest); the hum (class sound 0, flags 4) → pvar 0 | [`update_732`] |
//! | | 1, 3, 5, 7 | wrapped: +0x54 = 0; blend to `cmd` (unless on it); the hum released; class sound 1; `cmd` = trunc(scale·`randf(180, 240)`) | [`update_732`] |
//! | | 2 | the timer out: `randi(255)` even → blend 7 (`ticks(10)`), `cmd` 0; odd → blend 3, `cmd` 4; the hum | [`update_732`] |
//! | | 4 | the timer out: blend 5 (`ticks(10)`); `cmd` 2; the hum | [`update_732`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::{fast_dec_timer_u8, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 2;
pub const FN_656: u32 = 0x2d_ca10;
pub const FN_675: u32 = 0x2d_d370;
pub const FN_732: u32 = 0x2d_f1a8;
pub const CLASSES_656: [i16; 1] = [656];
pub const CLASSES_675: [i16; 1] = [675];
pub const CLASSES_732: [i16; 1] = [732];
/// gp−0x4f98 (level02): 732's animation speed.
const SPEED_732: f32 = 0.35;

fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn key_b(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }

/// The byte timer in `cmd` (+0xbc): true when out.
fn timer(w: &mut World, id: MobyId) -> bool {
    let mut t = w.m(id).cmd;
    let r = fast_dec_timer_u8(&mut t);
    w.mm(id).cmd = t;
    r != 0
}

/// `cmd` = trunc(`multiply_global_scale(randf(lo, hi))`).
fn rest(w: &mut World, id: MobyId, lo: f32, hi: f32) {
    let r = w.rng.randf(lo, hi);
    let t = crate::moby_update::services::fl(w.svc.timing.scale(Pf::f(r))) as i32;
    w.mm(id).cmd = t as u8;
}

/// Level02 0x2dca10, class 656 (module table).
pub fn update_656(w: &mut World, id: MobyId) {
    match key_b(w, id) {
        1 if timer(w, id) && key_b(w, id) != 2 => { w.anim_blend(id, 2, 0, 0); }
        3 if timer(w, id) && key_b(w, id) != 0 => { w.anim_blend(id, 0, 0, 0); }
        k @ (0 | 2) => {
            if !wrapped(w, id) { return; }
            w.anim_blend(id, if k == 0 { 1 } else { 3 }, 0, 0);
            rest(w, id, 180.0, 240.0);
        }
        _ => {}
    }
}

/// Level02 0x2dd370, class 675 (module table).
pub fn update_675(w: &mut World, id: MobyId) {
    match key_b(w, id) {
        2 if timer(w, id) && key_b(w, id) != 3 => {
            let t = w.ticks(5);
            w.anim_blend(id, 3, 0, t);
        }
        7 if timer(w, id) && key_b(w, id) != 0 => {
            let t = w.ticks(5);
            w.anim_blend(id, 0, 0, t);
        }
        k @ (0 | 3) => {
            if !wrapped(w, id) { return; }
            let t = w.ticks(5);
            w.anim_blend(id, if k == 0 { 2 } else { 7 }, 0, t);
            rest(w, id, 60.0, 180.0);
        }
        _ => {}
    }
}

/// 732's hum: class sound 0 (flags 4) into pvar 0.
fn hum(w: &mut World, id: MobyId) {
    let s = w.play_sound(0, 4, id);
    c::set_pi32(w, id, 0, s);
}

/// 732's blend over `ticks(10)` unless already on `seq`.
fn blend10(w: &mut World, id: MobyId, seq: u8) {
    if key_b(w, id) != seq {
        let t = w.ticks(10);
        w.anim_blend(id, seq, 0, t);
    }
}

/// Level02 0x2df1a8, class 732 (module table).
pub fn update_732(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 4 { return; }
    w.mm(id).anim.speed = SPEED_732;
    match key_b(w, id) {
        0 => {
            if !timer(w, id) { return; }
            blend10(w, id, 1);
            w.mm(id).cmd = 2;
            hum(w, id);
        }
        1 | 3 | 5 | 7 => {
            if !wrapped(w, id) { return; }
            w.mm(id).anim.t = 0.0;
            let next = w.m(id).cmd;
            if key_b(w, id) != next { w.anim_blend(id, next, 0, 0); }
            let v = c::pi32(w, id, 0);
            if v != -1 { w.release_sound(v, id); }
            c::set_pi32(w, id, 0, -1);
            w.play_sound(1, 0, id);
            rest(w, id, 180.0, 240.0);
        }
        2 => {
            if !timer(w, id) { return; }
            if w.rng.randi(0xff) & 1 == 0 {
                blend10(w, id, 7);
                w.mm(id).cmd = 0;
            } else {
                blend10(w, id, 3);
                w.mm(id).cmd = 4;
            }
            hum(w, id);
        }
        4 => {
            if !timer(w, id) { return; }
            blend10(w, id, 5);
            w.mm(id).cmd = 2;
            hum(w, id);
        }
        _ => {}
    }
}
