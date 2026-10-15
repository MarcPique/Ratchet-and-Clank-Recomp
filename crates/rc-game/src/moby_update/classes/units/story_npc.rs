//! The inline patterns the levels' story NPCs share (docs/plan/progression.md `## story`). Not a system of the game:
//! every NPC class (Aridia's 786 / 788, Kerwan's 890 / 909, Rilgar's 918 / 919 / 925, …) compiles the same few
//! lines from one source template into its own update; they are written once here because they repeat byte for
//! byte with only their constants and offsets changing (the head look-at is `talking_npc::look_at_layout` with a
//! layout row per class).
//!
//! * [`idle_anim`]: the idle fidgets: while a "rest" sequence plays, a timer (`randf(1200, 2400)` scaled ticks) picks
//!   a fidget sequence, blended over `ticks(10)`; a fidget at its end (anim flags +0x70 & 2) goes back.
//! * [`talk_radius`]: the talk radius 255 while the current node is auto and Ratchet is in the NPC's cuboid.
//! * [`place_grounded`]: the scene-end place a few units along the NPC's row 0, grounded, facing it.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::interact::talk;
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

/// One NPC's idle rule (the constants of its copy of the template).
#[derive(Clone, Copy)]
pub struct Idle {
    /// The timer's pvar offset and width (s16 `FastDecTimer__FRs` or s32 `__FRi`).
    pub timer: usize,
    pub short: bool,
    /// The sequences in which the timer runs (786 / 788 / 919 / 925: seq 0; the race girl 918: 0 and 2).
    pub rest: fn(u8) -> bool,
    /// The fidget the timer picks (may draw `randi`).
    pub pick: fn(&mut World) -> u8,
    /// At a sequence's end (+0x70 & 2, when the timer did not fire): the sequence to go to (None: stay).
    pub at_end: fn(&mut World, u8) -> Option<u8>,
}

/// The idle fidgets (module doc): the timer first while resting; when it fires: a new wait
/// `trunc(scale · randf(1200, 2400))` and the pick (blended over `ticks(10)` unless already on it); otherwise, at the
/// end of the sequence, `at_end`'s sequence (unless already on it).
pub fn idle_anim(w: &mut World, id: MobyId, r: &Idle) {
    let seq = w.m(id).anim.seq_b;
    if (r.rest)(seq) {
        let fired = if r.short { c::dec_timer_pvar_s16(w, id, r.timer) } else { c::dec_timer_pvar_i32(w, id, r.timer) };
        if fired != 0 {
            let t = w.svc.timing.scale(Pf::f(w.rng.randf(1200.0, 2400.0))).to_f32() as i32;
            if r.short { c::set_pi16(w, id, r.timer, t as i16) } else { c::set_pi32(w, id, r.timer, t) }
            let s = (r.pick)(w);
            if w.m(id).anim.seq_b != s {
                let n = w.ticks(10);
                w.anim_blend(id, s, 0, n);
            }
            return;
        }
    }
    if w.m(id).anim.flags & 2 == 0 { return; }
    let Some(s) = (r.at_end)(w, seq) else { return };
    if w.m(id).anim.seq_b != s {
        let n = w.ticks(10);
        w.anim_blend(id, s, 0, n);
    }
}

/// The rest rule of most NPCs: only sequence 0.
pub fn rest0(s: u8) -> bool { s == 0 }

/// The end rule of most NPCs: any fidget goes back to 0.
pub fn back0(_: &mut World, s: u8) -> Option<u8> { (s != 0).then_some(0) }

/// The talk radius: 255 while the current node is auto (talk +0x08 = 1) and Ratchet is in the cuboid of pvar
/// `cuboid`, else `r` (talk block at `base`).
pub fn talk_radius(w: &mut World, id: MobyId, base: usize, cuboid: usize, r: f32) {
    let auto = p::u8(&w.m(id).pvars, base + talk::AUTO) == 1 && story::hero_in(w, p::i32(&w.m(id).pvars, cuboid));
    p::set_ff(&mut w.mm(id).pvars, base + talk::RADIUS, if auto { 255.0 } else { r });
}

/// The grounded scene-end place (`0x16cea6` clear: the level's copy of 0x16cd26): `d` along the NPC's row 0, its z the
/// ground under it probed from `up` above (`GroundHeight(0.5, …)`), facing the NPC (yaw + 0x40490fd0). Only while no
/// place is pending ([`crate::moby_update::interact::Interact::scene_end_place`]) [L: the game tests its flag, which
/// the scene player clears at the start of a scene].
pub fn place_grounded(w: &mut World, id: MobyId, d: f32, up: f32) {
    if w.svc.interact.scene_end_place.is_some() { return; }
    let m = w.m(id);
    let r = c::set_len3(m.rows[0], d);
    let mut q = [m.position[0] + r[0], m.position[1] + r[1], m.position[2] + r[2] + up, m.position[3]];
    q[2] = w.ground_height(Pf::f(0.5), q.map(Pf::f), 0).to_f32();
    let yaw = c::add_rot(c::yaw(w, id), f32::from_bits(0x4049_0fd0));
    w.svc.interact.scene_end_place = Some(([q[0], q[1], q[2]], yaw));
}

/// The levels' "child on a joint" helper (level12 `0x27b9c0(parent, child, list)`; Hoven's 282 carries its prop with
/// it): the child's position = joint list `list`'s point (`moby_attach_to_joint`), its animation advanced
/// (`MobyAnimAdvance`), its matrix built, its rows = the joint's rows with normalised columns (`0x275060`), its sphere
/// (`MobyAnimSphereLerp`); hidden with the parent (mode 0x41 follows the parent's bit 1); mode |= 6 (no update, the
/// matrix kept).
pub fn attach_child(w: &mut World, parent: MobyId, child: MobyId, list: usize) {
    let mx = w.joint_matrix(parent, list);
    w.mm(child).position = mx[3];
    crate::moby_update::anim_sound::advance(w, child);
    w.build_matrix(child);
    let mut rows = [mx[0], mx[1], mx[2]].map(|r| r.map(f32::to_bits));
    rc_formats::moby_anim::normalise_columns(&mut rows);
    let m = w.mm(child);
    for (r, q) in m.rows.iter_mut().zip(rows.iter()) { *r = q.map(f32::from_bits); }
    crate::moby_update::creature::react::sphere_lerp(w, child);
    let hidden = w.m(parent).mode & crate::moby_runtime::mode::HIDDEN != 0;
    let m = w.mm(child);
    if hidden { m.mode |= 0x41 } else { m.mode &= !0x41 }
    m.mode |= crate::moby_runtime::mode::NO_UPDATE | crate::moby_runtime::mode::KEEP_MATRIX;
}
