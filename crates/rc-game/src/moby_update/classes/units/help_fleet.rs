//! The Fleet's help director, class 1470 (census U615; level17 `0x2f26d0`, one instance). Read from the disassembly (no
//! decomp). Besides its hints it awards two skill points: Ratchet under water in its cuboid +0x60, then out through
//! +0x64 / +0x68 (skill point 0x1c, 0x13d424); and every guard 44 and crew 1382 down outside the cuboid +0x78 (skill
//! point 0x1b, 0x13d423; the moby table scanned 30 slots a tick).
//!
//! **Pvars** (0x80): +0x00 the barrier hint's cuboid, +0x0c its ticks there, +0x10 the barrier (class 78), +0x20 /
//! +0x24 / +0x28 the water cuboids, +0x2c their ticks, +0x30 the cuboid last hinted, +0x38 the barrier hint's wait,
//! +0x3c the look's trigger cuboid, +0x40 / +0x44 its two look points (cuboids), +0x48 (checked only), +0x4c done,
//! +0x50, +0x58 the mission, +0x5c the look's timer, +0x60 / +0x64 / +0x68 the dive cuboids, +0x6c the dive state,
//! +0x70 / +0x74 the scan's slot and count, +0x78 the party cuboid.
//!
//! | address | what | port |
//! |---|---|---|
//! | entry | dive 0: Ratchet in +0x60 in group 0x11 (under water) → 1; 1: state 0x76 / 0x32 → 0; in +0x64 or +0x68 → skill point 0x1c (when not earned: = 1, `PlayLevelSoundAtMoby(1, 0, 0)`, `ShowBanner(0x53d6, −1)`) | [`update`] |
//! | | skill point 0x1b not earned: 30 slots (+0x70 on) of the moby table: class 0x2c or 0x566 alive (state < 0x80) outside +0x78 → +0x74 += 1; past the table's end: +0x74 = 0 → the skill point; +0x70 = +0x74 = 0 | [`update`] |
//! | 0 | update 0xff; +0x38 = `ticks(600)`; +0x4c = 0, or 1 (and a debug print) with any of +0x3c..+0x48 −1; → 1 | [`update`] |
//! | 1 | in +0x00: +0x0c += 1 (and, not the cuboid last hinted, it is the candidate), else +0x0c = 0, +0x30 = −1; the barrier (+0x10, −1 none) not class 78, deleted or in state 3 → nothing; +0x0c > +0x38, a candidate, group ok, box idle, item 31 owned: +0x38 = `ticks(1800)`, record 0x7d never shown → `Help_Request(15002, 0x7d)`; +0x30 = it, +0x0c = 0 | [`update`] |
//! | | in +0x20, box idle, item 4 (the Hydro-Pack) not owned, record 0x82 not shown on this level → `Help_Request(15005, 0x82)` | [`update`] |
//! | | the first of +0x20..+0x28 Ratchet is in: +0x2c += 1 unless state 0x35, else +0x2c = 0, +0x30 = −1; +0x2c > `ticks(3600)`, not the cuboid last hinted, in the water (`0x22dea8`), box idle, item 4 owned → `Help_Request(20014, 0x78)`, +0x30 = it, +0x2c = 0 | [`update`] |
//! | | mission +0x58 (0xff none) done → +0x4c = 1; +0x4c clear and in +0x3c: the box closed (`0x2258b0`); record 0x8b never shown → `Help_Request(17000, 0x8b)`; the box busy → +0x50 = 0, → 2, +0x5c = `ticks(180)`, `SetState(0x72, 0)` | [`update`] |
//! | 2 | the follow camera's stick off (`0x313858`) and turn toward cuboid +0x44's centre (`0x313b48(1.5°, 0, ·)`); the camera's yaw within 2° of it → 3, +0x5c = `ticks(60)` | [`update`] |
//! | 3 | stick off; the timer +0x5c run out: the turn toward +0x40's centre; within 2° → the body's idle (`0x223380`), +0x4c = 1, → 1 | [`update`] |

use super::hints::{group_ok, idle, in_cuboid, mission_done, owned, pvi, request, set_pvi, ticks};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::{HeroCall, World};
use crate::moby_update::story;

pub const UPDATE_FN: u32 = 0x2f_26d0;
pub const REFERENCE_LEVEL: u32 = 17;
pub const CLASSES: [i16; 1] = [1470];

const SIZE: usize = 0x80;
/// The skill points (0x13d423 / 0x13d424).
const SKILL_PARTY: usize = story::skill_index(0x13_d423);
const SKILL_DIVE: usize = story::skill_index(0x13_d424);
const SKILL_BANNER: i32 = 0x53d6;
const GUARD: i16 = 0x2c;
const CREW: i16 = 0x566;
const BARRIER: i16 = 0x4e;
const HOLOGUISE: usize = 31;
const HYDRO_PACK: usize = 4;
/// 1.5° (0x3cd67750) and 2° (0x3d0efa35).
const TURN_RATE: f32 = f32::from_bits(0x3cd6_7750);
const LOOKING: f32 = f32::from_bits(0x3d0e_fa35);

/// The skill point `k` set when not earned, the jingle and the banner (`ShowBanner(0x53d6, −1)`).
fn award(w: &mut World, k: usize) {
    if story::skill_point(w, k) { return; }
    story::set_skill_point(w, k);
    w.play_level_sound(1, 0, None);
    crate::cinematic::show_banner(w, SKILL_BANNER, -1);
}

fn mask(w: &World, r: usize) -> u32 { w.svc.help.records.help[r].mask }

/// The centre of cuboid `c`.
fn centre(w: &World, c: i32) -> Option<[f32; 3]> { story::cuboid(w, c).map(|(p, _)| p) }

/// The follow camera turned toward cuboid `c`'s centre; whether its yaw is within 2° of it.
fn look_at(w: &mut World, c: i32) -> bool {
    let Some(p) = centre(w, c) else { return false };
    crate::cinematic::follow_turn_toward_point(w, TURN_RATE, 0.0, p);
    let cam = w.camera_point();
    c::diff_rots(w.camera_yaw, c::atan(p[0] - cam[0], p[1] - cam[1])) < LOOKING
}

fn stick_off(w: &mut World) { w.svc.cinematic.calls.push(crate::cinematic::CinematicCall::FollowStickOff); }

/// The moby table scan (module doc): 30 slots a tick.
fn scan(w: &mut World, id: MobyId) {
    for _ in 0..30 {
        let i = pvi(w, id, 0x70);
        if let Some(m) = usize::try_from(i).ok().and_then(|i| w.table.mobys.get(i)) {
            if (m.o_class == GUARD || m.o_class == CREW) && m.state < 0x80 {
                let p = m.position;
                if !w.in_cuboid([p[0], p[1], p[2]], pvi(w, id, 0x78)) { set_pvi(w, id, 0x74, pvi(w, id, 0x74) + 1); }
            }
        }
        set_pvi(w, id, 0x70, i + 1);
        if ((i + 1) as usize) < w.table.mobys.len() { continue; }
        if pvi(w, id, 0x74) == 0 && !story::skill_point(w, SKILL_PARTY) { award(w, SKILL_PARTY); }
        set_pvi(w, id, 0x70, 0);
        set_pvi(w, id, 0x74, 0);
    }
}

/// Level17 0x2f26d0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, SIZE);
    match pvi(w, id, 0x6c) {
        0 => {
            if in_cuboid(w, id, 0x60) && w.hero.group == 0x11 { set_pvi(w, id, 0x6c, 1); }
        }
        1 => {
            if w.hero.state == 0x76 || w.hero.state == 0x32 {
                set_pvi(w, id, 0x6c, 0);
            } else if in_cuboid(w, id, 0x64) || in_cuboid(w, id, 0x68) {
                award(w, SKILL_DIVE);
            }
        }
        _ => {}
    }
    if !story::skill_point(w, SKILL_PARTY) { scan(w, id); }
    match w.m(id).state {
        0 => {
            w.mm(id).update_dist = 0xff;
            set_pvi(w, id, 0x38, ticks(600));
            let broken = [0x44, 0x40, 0x3c, 0x48].iter().any(|&o| pvi(w, id, o) == -1);
            w.mm(id).pvars[0x4c] = broken as u8;
            w.mm(id).state = 1;
        }
        1 => hints(w, id),
        2 => {
            stick_off(w);
            if look_at(w, pvi(w, id, 0x44)) {
                w.mm(id).state = 3;
                set_pvi(w, id, 0x5c, ticks(60));
            }
        }
        3 => {
            stick_off(w);
            if c::dec_timer_pvar_i32(w, id, 0x5c) != 0 && look_at(w, pvi(w, id, 0x40)) {
                w.hero_fields_mut().call(HeroCall::BodyIdle);
                w.mm(id).pvars[0x4c] = 1;
                w.mm(id).state = 1;
            }
        }
        _ => {}
    }
}

/// State 1 (module doc).
fn hints(w: &mut World, id: MobyId) {
    // The barrier hint (+0x00 / +0x10).
    let cub = pvi(w, id, 0);
    let mut cand = -1;
    let mut barrier = -1;
    if in_cuboid(w, id, 0) {
        if pvi(w, id, 0x30) != cub {
            cand = cub;
            barrier = pvi(w, id, 0x10);
        }
        set_pvi(w, id, 0x0c, pvi(w, id, 0x0c) + 1);
    } else {
        set_pvi(w, id, 0x0c, 0);
        set_pvi(w, id, 0x30, -1);
    }
    let barrier_up = barrier == -1
        || usize::try_from(barrier).ok().and_then(|i| w.table.mobys.get(i)).is_some_and(|m| m.o_class == BARRIER && !matches!(m.state, 0xfe | 0xfd | 3));
    if barrier_up && pvi(w, id, 0x38) < pvi(w, id, 0x0c) && cand != -1 && group_ok(w) && idle(w) && owned(w, HOLOGUISE) {
        set_pvi(w, id, 0x38, ticks(1800));
        if (mask(w, 0x7d) as i32) >= 0 { request(w, 0x3a9a, 0x7d); }
        set_pvi(w, id, 0x30, cand);
        set_pvi(w, id, 0x0c, 0);
    }
    // The Hydro-Pack hints (+0x20..+0x28).
    if in_cuboid(w, id, 0x20) && idle(w) && !owned(w, HYDRO_PACK) && mask(w, 0x82) & (1 << w.svc.level) == 0 {
        request(w, 0x3a9d, 0x82);
    }
    let mut cand = -1;
    let inside = (0..3).map(|k| pvi(w, id, 0x20 + 4 * k)).find(|&c| w.in_cuboid(w.hero_point(), c));
    if let Some(c) = inside {
        if pvi(w, id, 0x30) != c { cand = c; }
    }
    if inside.is_some() && w.hero.state != 0x35 {
        set_pvi(w, id, 0x2c, pvi(w, id, 0x2c) + 1);
    } else {
        set_pvi(w, id, 0x2c, 0);
        set_pvi(w, id, 0x30, -1);
    }
    if ticks(3600) < pvi(w, id, 0x2c) && cand != -1 && crate::help::hero_in_water(w.hero.state, w.hero.group) && idle(w) && owned(w, HYDRO_PACK) {
        request(w, 0x4e2e, 0x78);
        set_pvi(w, id, 0x30, cand);
        set_pvi(w, id, 0x2c, 0);
    }
    // The look (+0x3c, +0x44 then +0x40).
    let m = pvi(w, id, 0x58);
    if m != 0xff && mission_done(w, m) { w.mm(id).pvars[0x4c] = 1; }
    if w.m(id).pvars[0x4c] != 0 || !in_cuboid(w, id, 0x3c) { return; }
    w.svc.help.kill();
    if (mask(w, 0x8b) as i32) >= 0 { request(w, 0x4268, 0x8b); }
    if idle(w) { return; }
    set_pvi(w, id, 0x50, 0);
    w.mm(id).state = 2;
    set_pvi(w, id, 0x5c, ticks(180));
    crate::cinematic::hero_state(w, 0x72, false);
}
