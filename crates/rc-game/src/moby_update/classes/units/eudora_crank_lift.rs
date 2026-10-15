//! Eudora's crank lifts, class 434 (level 04, 3 created instances): level04 0x2c6bb8 (census U155). A lift driven by
//! a bolt crank (class 280, `classes::bolt_crank`): its frozen animation (sequence 1, speed 0) is posed at
//! t = 1 − the crank's progress while Ratchet holds the crank, and its joint 0 carries a second moby (the platform he
//! rides). Wound up (progress 1) it sinks back by itself as soon as Ratchet stands on that platform (only with +0x0c
//! clear), unwinding the crank with it (half a unit of t a second; the crank's progress and angle rewritten). Its class loop sound 0 runs while it moves and sound 1 plays at either
//! end. Read from the level04 decomp of 0x2c6bb8. Native `f32`.
//!
//! **Pvar block**: +0x00 s32 the crank, +0x04 s32 the carried moby, +0x08 s32 the voice slot, +0x0c s32 (non-zero: it
//! never sinks by itself).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2c6bb8 | no pvar block → `DeleteMoby` | [`update`] |
//! | state 0 | blend sequence 1, frame 0, `ticks(300)` (`moby_set_anim_snapshot`); → 1; anim speed +0x58 = 0; slot −1 | [`update`] (`World::anim_blend`) |
//! | state 1 | the crank (−1 or not class 0x118 → only the tail): held (its state 3) → t (+0x54) = 1 − progress; else progress ≠ 1 → 2; else with a carried moby and +0x0c = 0, Ratchet's movement group < 2, 9 or 0xc and his ground moby (0x13f64c) the carried one → 2 | [`update`] |
//! | | t unchanged → the voice released (`SoundIsAlive` 0x27e820, `release_voice_slot` when it owns it), slot −1; t changed and the progress 0 or 1 → the voice released, slot −1, `PlayClassSound(1, 0, m)`; else the voice not alive → slot = `PlayClassSound(0, 4, m)` (loop) | [`sound`] |
//! | state 2 | t = min(t + 0.5·dt, 1); the crank (class 0x118): held → 1, t = 1 − progress; else progress = 1 − t, angle +0x18 = progress·turns (+0x30)·2π; then the voice as in state 1 | [`update`] |
//! | tail | carried moby ≠ −1 → its position = this moby's joint 0 point (0x242500 = L01 0x2645a8) | [`update`] (`World::joint_point`) |
//! | | no particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{pvar as p, World};

/// The update in the level04 class table.
pub const UPDATE_FN: u32 = 0x2c_6bb8;
pub const REFERENCE_LEVEL: u32 = 4;
pub const CLASSES: [i16; 1] = [434];
/// The bolt crank's class.
pub const CRANK: i16 = 0x118;
const TAU: f32 = 6.283_185_5;

fn crank(w: &World, id: MobyId) -> Option<MobyId> {
    let k = usize::try_from(c::pi32(w, id, 0)).ok()?;
    let m = w.table.mobys.get(k)?;
    (m.o_class == CRANK && m.pvars.len() >= 0x34).then_some(k)
}

/// The voice: `t` against the tick's old `t0`, the crank's progress `prog` (module doc).
fn sound(w: &mut World, id: MobyId, t: f32, t0: f32, prog: f32) {
    let slot = c::pi32(w, id, 8);
    if t == t0 || prog == 1.0 || prog == 0.0 {
        if w.sound_alive(slot, id) {
            w.release_sound(slot, id);
            c::set_pi32(w, id, 8, -1);
        }
        if t != t0 { w.play_sound(1, 0, id); }
        return;
    }
    if !w.sound_alive(slot, id) {
        let s = w.play_sound(0, 4, id);
        c::set_pi32(w, id, 8, s);
    }
}

/// Level04 0x2c6bb8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.is_empty() {
        w.delete_moby(id);
        return;
    }
    if w.m(id).pvars.len() < 0x10 { return; }
    let t0 = w.m(id).anim.t;
    match w.m(id).state {
        0 => {
            let t = w.ticks(300);
            w.anim_blend(id, 1, 0, t);
            let m = w.mm(id);
            m.state = 1;
            m.anim.speed = 0.0;
            c::set_pi32(w, id, 8, -1);
        }
        1 => {
            if let Some(k) = crank(w, id) {
                let prog = p::ff(&w.m(k).pvars, 0);
                if w.m(k).state == 3 {
                    w.mm(id).anim.t = 1.0 - prog;
                } else if prog != 1.0 {
                    w.mm(id).state = 2;
                } else {
                    let carried = c::pi32(w, id, 4);
                    let g = w.hero.group;
                    let on = carried != -1 && c::pi32(w, id, 0xc) == 0 && (g < 2 || g == 9 || g == 0xc) && w.hero.ground_moby == usize::try_from(carried).ok();
                    if on { w.mm(id).state = 2; }
                }
                let t = w.m(id).anim.t;
                sound(w, id, t, t0, prog);
            }
        }
        2 => {
            let t = (t0 + DT * 0.5).min(1.0);
            w.mm(id).anim.t = t;
            if c::pi32(w, id, 0) != -1 {
                let Some(k) = crank(w, id) else { tail(w, id); return };
                if w.m(k).state == 3 {
                    let prog = p::ff(&w.m(k).pvars, 0);
                    w.mm(id).state = 1;
                    w.mm(id).anim.t = 1.0 - prog;
                } else {
                    let prog = 1.0 - t;
                    let turns = p::ff(&w.m(k).pvars, 0x30);
                    let pv = &mut w.mm(k).pvars;
                    p::set_ff(pv, 0, prog);
                    p::set_ff(pv, 0x18, prog * turns * TAU);
                }
                let prog = p::ff(&w.m(k).pvars, 0);
                let t = w.m(id).anim.t;
                sound(w, id, t, t0, prog);
            }
        }
        _ => {}
    }
    tail(w, id);
}

fn tail(w: &mut World, id: MobyId) {
    let carried = c::pi32(w, id, 4);
    if let Some(m) = usize::try_from(carried).ok().filter(|&m| m < w.table.mobys.len()) {
        let pt = w.joint_point(id, 0);
        w.mm(m).position = pt;
    }
}
