//! **Hoven's mine dispensers, class 1281** (level12 `0x307840`, its tick `0x307ab8`, the release `0x3076e0`; census
//! U440; 4 placed). Each owns up to twelve seeker mines (1269, the moby links +0xc0..+0xec), which it deletes at the
//! start. When its target comes within its range (+0xf4, 5 more for `ticks(180)` after a lure) and 3 in height it
//! opens (sequence 1, then 2) and releases the mines one every `ticks(40)` (each revived in its own slot at the
//! dispenser, 0.35 lower, its spawner the dispenser, swerving the other way from the last: the dispenser's command
//! flips), puffing 14 type-28 rings; out of mines it closes (sequence 3, then 0) and stays untargetable.
//!
//! **Pvars** (0x100): the target record at +0x70 (+0xb0 the moby + 1, +0xb4 the kind: 2 none), +0xc0.. the mines,
//! +0xf0 the next one, +0xf4 the range (+0xf8 now), +0xfc s16 the release timer, +0xfe s16 the alert.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x307ab8` | the tick (not in state 0): the lure → alert `ticks(180)`; the range; the target `0x274b78` into +0x70; seen but more than 3 off in z → none; no moby → Ratchet's (his position) | [`tick`] |
//! | `0x307840` | the states (module doc) | [`update`] |
//! | `0x3076e0` | a mine back: at the dispenser (z − 0.35), state 0, command 0, the class's mode and scale, update / draw distance 0xff, drawn, +0x71 / +0x72 0xff, occlusion 0x7f80, no hit, the class's collision, the dispenser's light words, `MobyBuildMatrix`, the spawner; 14 `PartType28Spawn(randf(3·dt, 5·dt), pos, 0)`; its health from its meter | [`release`] |
//!
//! Read from the level12 decomp. Native `f32`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, target};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x30_7840;
pub const CLASSES: [i16; 1] = [1281];

const DT: f32 = c::DT;

mod pv_ {
    pub const LURE: usize = 0x38;
    pub const T_POS: usize = 0x70;
    pub const T_MOBY: usize = 0xb0;
    pub const T_KIND: usize = 0xb4;
    pub const MINES: usize = 0xc0;
    pub const NEXT: usize = 0xf0;
    pub const SIGHT: usize = 0xf4;
    pub const RANGE: usize = 0xf8;
    pub const TIMER: usize = 0xfc;
    pub const ALERT: usize = 0xfe;
    pub const SIZE: usize = 0x100;
}
use pv_ as o;

fn mine(w: &World, id: MobyId, i: i32) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, o::MINES + 4 * i as usize)).ok().filter(|&m| m < w.table.mobys.len())
}
fn alive(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s != 0xfe && s != 0xfd }

/// `0x307ab8(m)`: the tick (module doc).
fn tick(w: &mut World, id: MobyId) {
    if w.m(id).state == 0 { return; }
    if c::pi32(w, id, o::LURE) != 0 {
        let t = w.ticks(0xb4);
        c::set_pi16(w, id, o::ALERT, t as i16);
    }
    c::set_pi32(w, id, o::LURE, 0);
    let sight = c::pf(w, id, o::SIGHT);
    let range = if c::dec_timer_pvar_s16(w, id, o::ALERT) == 0 { sight + 5.0 } else { sight };
    c::set_pf(w, id, o::RANGE, range);
    let t = target::acquire(w, id, range);
    c::set_pv4(w, id, o::T_POS, t.pos);
    c::set_pi32(w, id, o::T_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, o::T_KIND, t.kind as i32);
    if t.kind != 2 && 3.0 < (c::pos(w, id)[2] - t.pos[2]).abs() { c::set_pi32(w, id, o::T_KIND, 2); }
    if c::pi32(w, id, o::T_MOBY) == 0 {
        c::set_pi32(w, id, o::T_MOBY, w.hero_moby.map_or(0, |m| m as i32 + 1));
        let h = super::hero_pos(w);
        c::set_pv4(w, id, o::T_POS, h);
    }
}

/// `0x3076e0(m)`: mine +0xf0 back (module doc).
fn release(w: &mut World, id: MobyId, m: MobyId) {
    let info = w.classes.info(w.m(m).o_class).unwrap_or_default();
    let (pos, light, ambient) = { let d = w.m(id); (d.position, d.light, d.ambient) };
    {
        let mm = w.mm(m);
        mm.position = pos;
        mm.state = 0;
        mm.cmd = 0;
        mm.position[2] -= 0.35;
        mm.mode = info.mode_bits;
        mm.update_dist = 0xff;
        mm.scale = info.scale;
        mm.visible = 1;
        mm.occlusion = 0x7f80;
        mm.hit_slot = 0xff;
        mm.draw_dist = 0xff;
        mm.b71 = 0xff;
        mm.b72 = 0xff;
        mm.has_collision = info.has_collision;
        mm.light = light;
        mm.ambient = ambient;
        mm.delete_tick = 0;
    }
    w.build_matrix(m);
    w.mm(m).parent = Some(id);
    for _ in 0..14 {
        let s = w.rng.randf(DT * 3.0, DT * 5.0);
        c::fx::part28(w, s, pos);
    }
    if let Some(d) = crate::moby_update::triggers::pvar_record(w.m(m)) {
        if d + 8 <= w.m(m).pvars.len() {
            let hp = c::pi16(w, m, d + 4) as f32;
            c::set_pf(w, m, d, hp);
        }
    }
}

fn blend(w: &mut World, id: MobyId, seq: u8) {
    if w.m(id).anim.seq_b != seq {
        let t = w.ticks(2);
        w.anim_blend(id, seq, 0, t);
    }
}

/// Level12 `0x307840` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    tick(w, id);
    match w.m(id).state {
        0 => {
            for i in 0..12 {
                if let Some(m) = mine(w, id, i).filter(|&m| alive(w, m)) { w.delete_moby(m); }
            }
            w.mm(id).state = 1;
        }
        1 => {
            if c::pi32(w, id, o::T_KIND) == 2 { return; }
            if c::dist2(c::pos(w, id), c::pv4(w, id, o::T_POS)) < c::pf(w, id, o::RANGE) { w.mm(id).state = 2; }
        }
        2 => {
            blend(w, id, 1);
            if w.m(id).anim.flags & 2 == 0 { return; }
            blend(w, id, 2);
            w.mm(id).state = 3;
        }
        3 => {
            if c::dec_timer_pvar_s16(w, id, o::TIMER) != 0 {
                let i = c::pi32(w, id, o::NEXT);
                if let Some(m) = mine(w, id, i) {
                    release(w, id, m);
                    w.mm(id).cmd = (w.m(id).cmd == 0) as u8;
                    let t = w.ticks(0x28);
                    c::set_pi16(w, id, o::TIMER, t as i16);
                }
                c::set_pi32(w, id, o::NEXT, i + 1);
            }
            let n = c::pi32(w, id, o::NEXT);
            if n != 12 && mine(w, id, n).is_some() { return; }
            w.mm(id).state = 4;
        }
        4 => {
            w.mm(id).mode &= !mode::TARGETABLE;
            blend(w, id, 3);
            if w.m(id).anim.flags & 2 == 0 { return; }
            blend(w, id, 0);
            w.mm(id).state = 5;
        }
        _ => {}
    }
}
