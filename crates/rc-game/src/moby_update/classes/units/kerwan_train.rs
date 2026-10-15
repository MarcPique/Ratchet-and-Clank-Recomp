//! U123: class 822, Kerwan's train (level03 `0x292e98`, the only copy: 2 placed). The name is descriptive [L]. Instance
//! #4 is the **locomotive** and the train's controller; #5 has no paths and only rewrites four header words every tick.
//! The locomotive pulls a chain of joint-carried cars 1210 (+0xa0, each car's pvar +0xc0 the next) behind an invisible
//! lead, the flyer 845 (+0xc4) that runs the shared flyer driver (`classes::flyer::driver`, level03 `0x293c78` =
//! level01 `FlyerPathDriver` 0x2f5168) on the train's path: the lead is kept +0xdc (20) ahead (its speed is the slack),
//! the locomotive moves at +0xc8 toward it (yaw and pitch on springs), and each car is pulled to 10 behind the joint 1
//! of the one in front. Everything on the locomotive and the cars rides it (`CarryRiders`, platform blocks +0x60 /
//! the cars' +0x20).
//!
//! The run: parked at the start of path B (+0xa8); Ratchet enters cuboid +0xb4 → the ride (`cmd` 4): the music (track
//! 4, stinger 7), the "talked" word of its talk slot = 1, the cutaway camera 737 #907 (+0x100) aimed at Ratchet for
//! `ticks(860)`, the engine loop (class sound 1) and a horn (class sound 0) every 900–1398 ticks, the map's train
//! predicate on; the train follows path A (+0xa4), speeding up over 5 s to +0xc0 (10 u/s); a car Ratchet stands on gets
//! "talked" 3; falling off (60 airborne ticks, 10 below the path) reloads the level. Stepping onto the locomotive ends the
//! ride: one trooper 574 of its eight links (+0xe0..) still about is sent away (state 0xe), the second cutaway 737 #906
//! (+0xd8) is aimed at path C's start (+0xac) with its Ratchet cuboid moved behind it, the train jumps to path C at
//! double speed (`cmd` 5, music 0 / 8) and brakes to a stop at the point whose w is 1 (+0xd4); then the cutaway ends,
//! `cmd` 6 releases the infobot riding it (`classes::infobot`), and the train waits until Ratchet enters cuboid +0xb0 to
//! start over (`cmd` 1). Troopers in state 0x10 get 0x11 then (no 574 has those states: no effect).
//!
//! **Pvars** (0x110): +0x20 / +0x24 / +0x28 / +0x3e header words, +0x60 the platform block (flags +0x9c = 3), +0xa0 the
//! first car, +0xa4 / +0xa8 / +0xac paths A / B / C, +0xb0 / +0xb4 the restart / start cuboids, +0xb8 the moby's +0xa8
//! (its id, for the game's debug prints), +0xbc the distance from the locomotive to its joint 0 (xy), +0xc0 the top speed
//! (u/s at init, then per tick), +0xc4 the lead 845, +0xc8 the speed, +0xcc / +0xd0 the yaw / pitch spring velocities,
//! +0xd4 path C's stop point, +0xd8 the arrival cutaway 737, +0xdc the lead's distance (20), +0xe0..+0xfc eight trooper
//! links, +0x100 the ride cutaway 737, +0x104 the horn timer, +0x108 the engine loop's voice, +0x10c the airborne count.
//! The level03 word 0x161368 (gp−0x5898) 10.0 is the gap between the cars.
//!
//! ## Coverage
//!
//! **The update** `0x292e98`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | +0xb8 = moby +0xa8 (u16) | the id for `STUB_printf` | [`update`] |
//! | +0xa4 = −1: +0x3e 0xd (s16), +0x28 4, +0x20 0, +0x24 0 (s16); done | instance #5 | [`update`] |
//! | +0xc4 = −1: `STUB_printf`, `DeleteMoby` | | [`update`] |
//! | 0x15fdfc (the map's train predicate, `map::MapState::g15fdfc`) = 0 | | [`update`] |
//! | 0: the header words; +0xbc = xy distance to joint 0 (`0x23e338` = `0x2645a8`); +0xdc 20; +0xc0 ·= dt, +0xc8 = it; path C: +0xd4 = the first point whose w is 1 (−1 none); `cmd` 1; block flags 3; the lead's mode \|= 6 | init | [`init`] |
//! | 1: [`reset`] at path A's point 0 (heading 0); lead +0x74 = A, +0xfc = +0xc0; `FlyerPathDriver(lead)`; 2 | | [`update`] (`flyer::driver`) |
//! | 2: [`reset`] at path B's point 0 facing point 1; 3; lead +0x74 = A, +0x60 = 1, +0x70 = A's points, +0xc8 = 0 | parked | [`update`] |
//! | 3: [`stand`]; Ratchet (0x13f3d0) in cuboid +0xb4 (`0x24e3a8`), a ride cutaway, his movement group (0x1413dc) 0, 1, 9 or 0xc → 4: the predicate, `MusicRequestTrack(4, 7)` (`0x253820` = `0x27a248`), `0x254a10(m, 1)` (= `0x27b438`, the talked word), the cutaway's `cmd` 1, its target yaw / pitch at Ratchet from the camera (0x166ec0), its hold `ticks(860)`; horn timer `ticks(30)`; voice −1 | the start | [`update`] (`interact::set_talked`, `SoundSink::music_request`) |
//! | 4: +0xc8 += (+0xc0 / 5)·dt (≤ +0xc0); the predicate; [`drive`]; the loop `PlayClassSound(1, 4, m)` kept (`0x27a108` = `SoundIsAlive`); horn: `FastDecTimer(+0x104)` out → `PlayClassSound(0, 0, m)`, `rand_range(ticks(900), ticks(400))` (`0x246700`) | the ride | [`update`], [`engine_sounds`] |
//! | 4: Ratchet airborne (0x13f65e ≠ 0) and not in group 3: +0x10c += 1; beyond `ticks(60)` and 10 below the lead's path's nearest point (`0x24c9b0`, 1000 / 5) → the loop released (owner-guarded), voice −1, `FadeToBlack(ticks(16))`, 0x141401 = 1 (the reload); else +0x10c = 0 | falling off | [`update`] (`cinematic::fade_to_black`, `HeroCall::Reload`) |
//! | 4: the ride cutaway's `cmd` ≠ 0 → its target at Ratchet; Ratchet's ground moby a 822 / 1210 → `0x254a10(it, 3)` | | [`update`] |
//! | 4: Ratchet on the locomotive (`0x251598` = `HeroOnMoby`), paths C and the arrival cutaway set, the ride cutaway done: the first live trooper link not waiting (`0x2c6c60`) → `0x2c6c90` (state 0xe); +0xd4 = −1 → `STUB_printf`; else the arrival cutaway's `cmd` 1, target at path C's point 0, its hero cuboid (P[10]) at point 0 − (+0xdc + 4) along the first segment, z + 2.7, Euler (0, 0, heading), hold `ticks(10000)`; [`reset`] at path C's point 0; lead +0x74 = C, +0x60 1, +0xf0 1.0; +0xc0 ·= 2, lead +0xfc = it; `FlyerPathDriver(lead)`; 5; `MusicRequestTrack(0, 8)` | the arrival | [`arrive`] (`kerwan_trooper::waiting` / `send_away`) |
//! | 5: [`drive`]; the predicate; the sounds as 4; the lead past +0xd4: +0xdc 0, +0xc8 −= (+0xc0 / 1.9)·dt (≥ 0); stopped → the cutaway's `cmd` 0, 6, +0xc0 ·= ½, the loop released, voice −1; the cutaway's target at Ratchet | braking | [`update`] |
//! | 6: Ratchet in cuboid +0xb0 → troopers 574 in state 0x10 → 0x11, +0xc8 = +0xc0, +0xdc 20, 1; else [`stand`] | the restart | [`update`] |
//!
//! **[`reset`]** `0x292578(heading, m, p)`: the lead at p; the locomotive +0xdc behind it (along −heading), Euler y 0, z
//! heading + π; `CarryRiders(+0x60, Δ, old, new)` (`0x24ee28` = `0x2755f8`); the spring velocities 0; the cars one after
//! another: their old pose recorded, each at 10 + its +0xc4 behind the joint 1 of the one in front (the first behind the
//! locomotive's point +0xbc behind it), at the locomotive's height, yaw toward that point; `0x20d580` / `0x20def8` (the
//! matrix, `MobyBuildMatrix`), mode \|= 6, its `CarryRiders(+0x20)`, its pose recorded; the lead's state 0, mode \|= 6.
//!
//! **[`drive`]** `0x292890`: +0x72 = 0xff; lead +0xfc = +0xdc − the xy distance to it (≥ 0); `FlyerPathDriver(lead)`;
//! moved by `0x251130(+0xc8, yaw + π, pitch)` (= `0x277b50`, the polar vector) renormalised to +0xc8; yaw `SpringTurn`
//! (`0x246c80` = `0x26cef0`) to the lead's heading + π and pitch to atan(xy distance, Δz), both with k = +0xc8·60 / +0xc0:
//! (0.25k°·dt², 0.5k°·dt², k·dt); each car (+0x72 0xff): pulled to 10 (xy, at that height) behind the joint 1 of the
//! one in front (the first: the locomotive's joint 0) less its +0xc4, yaw toward it, pitch −atan(xy distance, Δz), the
//! matrix, mode \|= 6, its joint 1, `CarryRiders`, the pose recorded; then the locomotive's `CarryRiders`; lead mode \|= 6.
//!
//! **[`stand`]** `0x292d10`: +0x72 = 0x40; each car +0x72 = 0x40 and its `CarryRiders` with the move since the last
//! record; the locomotive's `CarryRiders` with no move; lead mode \|= 6.
//!
//! **Called by others:** the infobot 750 (`classes::infobot`) rides the locomotive (+0x54 = #4) and leaves on `cmd` 6.
//!
//! **Not ported:** none of the train's own code. The lead's flyer driver lacks its level-3 combat block (G-CLS-015). **Not
//! the game's, noted [L]:** the fall test reads the lead's path by its index (+0x74; the game: its pointer +0x70, the
//! same path); a failed nearest-point search leaves the game's output stale (the port: no reload that tick).

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::flyer;
use crate::moby_update::classes::units::kerwan_trooper;
use crate::moby_update::creature::{self as c, turn};
use crate::moby_update::interact;
use crate::moby_update::services::World;
use crate::moby_update::triggers;

/// The update in the level03 class table.
pub const UPDATE_FN: u32 = 0x29_2e98;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 1] = [822];
/// The joint lists read: the locomotive's 0, the cars' 1.
pub const JOINTS: [i16; 2] = [822, CAR];
/// The cars (0x4ba) and the lead (0x34d).
pub const CAR: i16 = 1210;
pub const LEAD: i16 = 845;
/// The lead's update (the shared flyer driver's level03 copy) and its class.
pub const LEAD_FN: u32 = 0x29_3c78;
pub const LEAD_CLASSES: [i16; 1] = [LEAD];
/// Level03 0x161368: the gap between the cars.
pub const GAP: f32 = 10.0;

/// Pvar offsets (module doc).
pub mod pv {
    pub const BLOCK: usize = 0x60;
    pub const BLOCK_FLAGS: usize = 0x9c;
    pub const CARS: usize = 0xa0;
    pub const PATH_A: usize = 0xa4;
    pub const PATH_B: usize = 0xa8;
    pub const PATH_C: usize = 0xac;
    pub const RESTART: usize = 0xb0;
    pub const START: usize = 0xb4;
    pub const ID: usize = 0xb8;
    pub const JOINT_D: usize = 0xbc;
    pub const TOP: usize = 0xc0;
    pub const LEAD: usize = 0xc4;
    pub const SPEED: usize = 0xc8;
    pub const YAW_V: usize = 0xcc;
    pub const PITCH_V: usize = 0xd0;
    pub const STOP: usize = 0xd4;
    pub const ARRIVAL_CAM: usize = 0xd8;
    pub const LEAD_D: usize = 0xdc;
    pub const TROOPERS: usize = 0xe0;
    pub const RIDE_CAM: usize = 0x100;
    pub const HORN: usize = 0x104;
    pub const VOICE: usize = 0x108;
    pub const AIR: usize = 0x10c;
    pub const SIZE: usize = 0x110;
    /// A car's pvars: +0x20 its block, +0xa0 / +0xb0 its recorded pose, +0xc0 the next car, +0xc4 its joint distance.
    pub const CAR_BLOCK: usize = 0x20;
    pub const CAR_POS: usize = 0xa0;
    pub const CAR_ROT: usize = 0xb0;
    pub const CAR_NEXT: usize = 0xc0;
    pub const CAR_JOINT_D: usize = 0xc4;
    /// The lead's flyer pvars written: +0x60 the point, +0x74 the path, +0xf0 t, +0xfc the speed.
    pub const LEAD_IDX: usize = 0x60;
    pub const LEAD_PATH: usize = 0x74;
    pub const LEAD_T: usize = 0xf0;
    pub const LEAD_SPEED: usize = 0xfc;
}

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&m| m < w.table.mobys.len()) }
fn alive(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s != 0xfe && s != 0xfd }
fn spline(w: &World, i: i32) -> Option<usize> { usize::try_from(i).ok().filter(|&p| p < w.svc.splines.len()) }
fn point(w: &World, p: Option<usize>, i: usize) -> c::V { p.and_then(|p| w.svc.splines[p].get(i)).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }
fn hero(w: &World) -> c::V { crate::moby_update::classes::units::hero_pos(w) }
fn camera(w: &World) -> c::V { w.camera.map(|x| f32::from_bits(x.0)) }
fn rot3(r: c::V) -> [f32; 4] { [r[0], r[1], r[2], r[3]] }
/// The pvar block's room for a car's fields.
fn car_ok(w: &World, car: MobyId) -> bool { w.m(car).pvars.len() >= 0xc8 }

/// A cutaway 737's camera target (P[5] / P[6] = pvar +0x14 / +0x18) at `p` from the camera.
fn aim_cutaway(w: &mut World, cam: MobyId, p: c::V) {
    if w.m(cam).pvars.len() < 0x30 { return; }
    let e = camera(w);
    c::set_pf(w, cam, 0x18, c::atan(p[0] - e[0], p[1] - e[1]));
    c::set_pf(w, cam, 0x14, -c::atan(c::dist2(e, p), p[2] - e[2]));
}

/// `CarryRiders(block, pos − old, old rot, rot)` for moby `m`.
fn carry(w: &mut World, m: MobyId, block: usize, old_pos: c::V, old_rot: c::V) {
    let d = c::sub(c::pos(w, m), old_pos);
    let r = w.m(m).rotation;
    triggers::carry_riders(&mut w.mm(m).pvars, block, d, old_rot, r);
}

/// A car's carry with the move since its last record, then the record (pos +0xa0, rot +0xb0).
fn carry_car(w: &mut World, car: MobyId) {
    let (op, or) = (c::pv4(w, car, pv::CAR_POS), c::pv4(w, car, pv::CAR_ROT));
    carry(w, car, pv::CAR_BLOCK, op, or);
    let (p, r) = (c::pos(w, car), w.m(car).rotation);
    c::set_pv4(w, car, pv::CAR_POS, p);
    c::set_pv4(w, car, pv::CAR_ROT, rot3(r));
}

/// The cars in order (+0xa0, then each car's +0xc0), stopping at −1 (and at a repeat).
fn cars(w: &World, id: MobyId) -> Vec<MobyId> {
    let mut out = Vec::new();
    let mut next = link(w, id, pv::CARS);
    while let Some(car) = next {
        if out.contains(&car) || !car_ok(w, car) { break; }
        out.push(car);
        next = link(w, car, pv::CAR_NEXT);
    }
    out
}

fn lead_mode(w: &mut World, id: MobyId) {
    if let Some(l) = link(w, id, pv::LEAD) { w.mm(l).mode |= 6; }
}

/// `0x292578(heading, m, p)` (module doc).
pub fn reset(w: &mut World, id: MobyId, heading: f32, p: c::V) {
    let Some(lead) = link(w, id, pv::LEAD) else { return };
    let (old_pos, old_rot) = (c::pos(w, id), w.m(id).rotation);
    w.mm(lead).position = p;
    let (cs, sn) = c::cs(heading);
    let back = |l: f32| [cs * -l, sn * -l, 0.0, 0.0];
    let ld = c::pf(w, id, pv::LEAD_D);
    c::set_pos(w, id, c::add(p, back(ld)));
    {
        let m = w.mm(id);
        m.rotation[1] = 0.0;
        m.rotation[2] = c::add_rot(heading, std::f32::consts::PI);
    }
    carry(w, id, pv::BLOCK, old_pos, old_rot);
    c::set_pf(w, id, pv::YAW_V, 0.0);
    c::set_pf(w, id, pv::PITCH_V, 0.0);
    let jd = c::pf(w, id, pv::JOINT_D);
    let mut prev = c::add(back(jd), c::pos(w, id));
    let z = c::pos(w, id)[2];
    for car in cars(w, id) {
        let (cp, cr) = (c::pos(w, car), w.m(car).rotation);
        c::set_pv4(w, car, pv::CAR_POS, cp);
        c::set_pv4(w, car, pv::CAR_ROT, rot3(cr));
        let d = c::add(back(GAP), prev);
        let cd = c::pf(w, car, pv::CAR_JOINT_D);
        let mut np = c::add(back(cd), d);
        np[2] = z;
        c::set_pos(w, car, np);
        {
            let m = w.mm(car);
            m.rotation[1] = 0.0;
            m.rotation[2] = c::atan(d[0] - np[0], d[1] - np[1]);
        }
        w.build_matrix(car);
        w.mm(car).mode |= 6;
        prev = w.joint_point(car, 1);
        carry_car(w, car);
    }
    w.mm(lead).state = 0;
    w.mm(lead).mode |= 6;
}

/// `0x292890` (module doc).
pub fn drive(w: &mut World, id: MobyId) {
    let Some(lead) = link(w, id, pv::LEAD) else { return };
    w.mm(id).b72 = 0xff;
    let (old_pos, old_rot) = (c::pos(w, id), w.m(id).rotation);
    if c::pi32(w, id, pv::CARS) == -1 {
        // `STUB_printf("no cars", +0xb8)`, `DeleteMoby`.
        w.delete_moby(id);
        return;
    }
    let slack = (c::pf(w, id, pv::LEAD_D) - c::dist2(c::pos(w, id), c::pos(w, lead))).max(0.0);
    if w.m(lead).pvars.len() > pv::LEAD_SPEED + 4 { c::set_pf(w, lead, pv::LEAD_SPEED, slack); }
    flyer::driver(w, lead);
    let sp = c::pf(w, id, pv::SPEED);
    let r = w.m(id).rotation;
    let v = c::set_len3(c::fx::polar(sp, c::add_rot(r[2], std::f32::consts::PI), r[1]), sp);
    let p = c::add(c::pos(w, id), [v[0], v[1], v[2], 0.0]);
    c::set_pos(w, id, p);
    let lp = c::pos(w, lead);
    let k = (sp * 60.0) / c::pf(w, id, pv::TOP);
    let deg = 0.017_453_292_f32;
    let (acc, damp, max) = (k * 0.25 * deg * c::DT2, k * 0.5 * deg * c::DT2, k * c::DT);
    let to = c::add_rot(c::atan(lp[0] - p[0], lp[1] - p[1]), std::f32::consts::PI);
    let mut yv = c::pf(w, id, pv::YAW_V);
    let yaw = turn::spring_turn(r[2], to, acc, damp, max, &mut yv);
    c::set_pf(w, id, pv::YAW_V, yv);
    w.mm(id).rotation[2] = yaw;
    let pt = c::atan(c::dist2(p, lp), lp[2] - p[2]);
    let mut pvv = c::pf(w, id, pv::PITCH_V);
    let pitch = turn::spring_turn(r[1], pt, acc, damp, max, &mut pvv);
    c::set_pf(w, id, pv::PITCH_V, pvv);
    w.mm(id).rotation[1] = pitch;
    let mut prev = w.joint_point(id, 0);
    for car in cars(w, id) {
        w.mm(car).b72 = 0xff;
        let j0 = w.joint_point(car, 0);
        let mut t = c::add(c::set_len2(c::sub(j0, prev), GAP), prev);
        t[2] = prev[2];
        let cp = c::pos(w, car);
        let d = c::sub(t, cp);
        let dl = c::len3(d);
        let mv = c::set_len3(d, dl - c::pf(w, car, pv::CAR_JOINT_D));
        let np = c::add(mv, cp);
        c::set_pos(w, car, np);
        {
            let m = w.mm(car);
            m.rotation[2] = c::atan(t[0] - np[0], t[1] - np[1]);
            m.rotation[1] = -c::atan(c::dist2(np, prev), prev[2] - np[2]);
        }
        w.build_matrix(car);
        w.mm(car).mode |= 6;
        prev = w.joint_point(car, 1);
        carry_car(w, car);
    }
    carry(w, id, pv::BLOCK, old_pos, old_rot);
    w.mm(lead).mode |= 6;
}

/// `0x292d10` (module doc).
pub fn stand(w: &mut World, id: MobyId) {
    w.mm(id).b72 = 0x40;
    let (p, r) = (c::pos(w, id), w.m(id).rotation);
    if c::pi32(w, id, pv::CARS) == -1 {
        w.delete_moby(id);
        return;
    }
    for car in cars(w, id) {
        w.mm(car).b72 = 0x40;
        carry_car(w, car);
    }
    carry(w, id, pv::BLOCK, p, r);
    lead_mode(w, id);
}

/// The loop and the horn of states 4 / 5.
fn engine_sounds(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, pv::VOICE);
    if !w.sound_alive(v, id) {
        let s = w.play_sound(1, 4, id);
        c::set_pi32(w, id, pv::VOICE, s);
    }
    if c::dec_timer_pvar_i32(w, id, pv::HORN) != 0 {
        w.play_sound(0, 0, id);
        let (a, b) = (w.ticks(900), w.ticks(400));
        let t = w.rng.rand_range(a, b);
        c::set_pi32(w, id, pv::HORN, t);
    }
}

/// The loop released when it still plays for the train (`release_voice_slot`), voice −1.
fn release(w: &mut World, id: MobyId) {
    let s = c::pi32(w, id, pv::VOICE);
    if s != -1 && w.sound_owner(s) == Some(id) && w.sound_alive(s, id) { w.release_sound(s, id); }
    c::set_pi32(w, id, pv::VOICE, -1);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    header(w, id);
    let jp = w.joint_point(id, 0);
    let d = c::dist2(jp, c::pos(w, id));
    c::set_pf(w, id, pv::JOINT_D, d);
    c::set_pf(w, id, pv::LEAD_D, 20.0);
    let top = c::pf(w, id, pv::TOP) * c::DT;
    c::set_pf(w, id, pv::SPEED, top);
    c::set_pf(w, id, pv::TOP, top);
    if let Some(pc) = spline(w, c::pi32(w, id, pv::PATH_C)) {
        c::set_pi32(w, id, pv::STOP, -1);
        let pts: Vec<f32> = w.svc.splines[pc].iter().map(|q| f32::from_bits(q[3])).collect();
        if let Some(i) = pts.iter().position(|&x| x == 1.0) { c::set_pi32(w, id, pv::STOP, i as i32); }
    }
    w.mm(id).cmd = 1;
    c::set_pi32(w, id, pv::BLOCK_FLAGS, 3);
    lead_mode(w, id);
}

/// The header words both instances rewrite.
fn header(w: &mut World, id: MobyId) {
    c::set_pi32(w, id, 0x20, 0);
    c::set_pi16(w, id, 0x24, 0);
    c::set_pi16(w, id, 0x3e, 0xd);
    c::set_pu8(w, id, 0x28, 4);
}

/// The arrival on stepping onto the locomotive (state 4, module doc).
fn arrive(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::PATH_C) == -1 || c::pi32(w, id, pv::ARRIVAL_CAM) == -1 { return; }
    if link(w, id, pv::RIDE_CAM).is_some_and(|m| w.m(m).cmd != 0) { return; }
    for i in 0..8 {
        let Some(t) = link(w, id, pv::TROOPERS + 4 * i) else { continue };
        if !alive(w, t) || kerwan_trooper::waiting(w, t) { continue; }
        kerwan_trooper::send_away(w, t);
        break;
    }
    if c::pi32(w, id, pv::STOP) == -1 {
        // `STUB_printf("no stop point")`.
        return;
    }
    let pc = spline(w, c::pi32(w, id, pv::PATH_C));
    let (p0, p1) = (point(w, pc, 0), point(w, pc, 1));
    let h = c::atan(p1[0] - p0[0], p1[1] - p0[1]);
    if let Some(cam) = link(w, id, pv::ARRIVAL_CAM) {
        w.mm(cam).cmd = 1;
        aim_cutaway(w, cam, p0);
        if w.m(cam).pvars.len() >= 0x30 {
            let cub = c::pi32(w, cam, 0x28);
            let ld = c::pf(w, id, pv::LEAD_D) + 4.0;
            let v = std::sync::Arc::make_mut(&mut w.svc.volumes);
            if let Some(s) = usize::try_from(cub).ok().and_then(|i| v.cuboids.get_mut(i)) {
                let (cs, sn) = c::cs(h);
                s.matrix[3] = [cs * -ld + p0[0], sn * -ld + p0[1], p0[2] + 2.7, s.matrix[3][3] + p0[3]];
                s.euler = [0.0, 0.0, h];
                s.unused_7c = 0.0;
            }
            let t = w.ticks(10000);
            c::set_pi32(w, cam, 0x2c, t);
        }
    }
    reset(w, id, h, p0);
    if let Some(lead) = link(w, id, pv::LEAD) {
        let pcv = c::pi32(w, id, pv::PATH_C);
        c::set_pi32(w, lead, pv::LEAD_PATH, pcv);
        c::set_pi32(w, lead, pv::LEAD_IDX, 1);
        c::set_pf(w, lead, pv::LEAD_T, 1.0);
        let top = c::pf(w, id, pv::TOP) * 2.0;
        c::set_pf(w, id, pv::TOP, top);
        c::set_pf(w, lead, pv::LEAD_SPEED, top);
        flyer::driver(w, lead);
    }
    w.mm(id).cmd = 5;
    if let Some(s) = w.sound.as_deref_mut() { s.music_request(0, 8); }
}

/// `0x292e98`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let uid = (w.m(id).uid_hi & 0xffff) as i32;
    c::set_pi32(w, id, pv::ID, uid);
    if c::pi32(w, id, pv::PATH_A) == -1 {
        header(w, id);
        return;
    }
    let Some(lead) = link(w, id, pv::LEAD) else {
        // `STUB_printf("no lead")`, `DeleteMoby`.
        w.delete_moby(id);
        return;
    };
    if w.m(lead).pvars.len() < 0x150 { return; }
    w.svc.map.g15fdfc = 0;
    match w.m(id).cmd {
        0 => init(w, id),
        1 => {
            let pa = spline(w, c::pi32(w, id, pv::PATH_A));
            let p = point(w, pa, 0);
            reset(w, id, 0.0, p);
            let a = c::pi32(w, id, pv::PATH_A);
            c::set_pi32(w, lead, pv::LEAD_PATH, a);
            let top = c::pf(w, id, pv::TOP);
            c::set_pf(w, lead, pv::LEAD_SPEED, top);
            flyer::driver(w, lead);
            w.mm(id).cmd = 2;
        }
        2 => {
            let pb = spline(w, c::pi32(w, id, pv::PATH_B));
            let (p0, p1) = (point(w, pb, 0), point(w, pb, 1));
            reset(w, id, c::atan(p1[0] - p0[0], p1[1] - p0[1]), p0);
            w.mm(id).cmd = 3;
            let a = c::pi32(w, id, pv::PATH_A);
            c::set_pi32(w, lead, pv::LEAD_PATH, a);
            c::set_pi32(w, lead, pv::LEAD_IDX, 1);
            // lead +0x70 = A's points (the game's pointer; the port's driver reads +0x74).
            c::set_pf(w, id, pv::SPEED, 0.0);
        }
        3 => {
            stand(w, id);
            if !w.in_cuboid(hero3(w), c::pi32(w, id, pv::START)) { return; }
            let Some(cam) = link(w, id, pv::RIDE_CAM) else { return };
            let g = w.hero.group as u32;
            if 1 < g && g != 9 && g != 0xc { return; }
            w.mm(id).cmd = 4;
            w.svc.map.g15fdfc = 1;
            if let Some(s) = w.sound.as_deref_mut() { s.music_request(4, 7); }
            interact::set_talked(w, id, 1);
            w.mm(cam).cmd = 1;
            let h = hero(w);
            aim_cutaway(w, cam, h);
            if w.m(cam).pvars.len() >= 0x30 {
                let t = w.ticks(0x35c);
                c::set_pi32(w, cam, 0x2c, t);
            }
            let t = w.ticks(30);
            c::set_pi32(w, id, pv::HORN, t);
            c::set_pi32(w, id, pv::VOICE, -1);
        }
        4 => {
            let top = c::pf(w, id, pv::TOP);
            let s = (c::pf(w, id, pv::SPEED) + (top / 5.0) * c::DT).min(top);
            c::set_pf(w, id, pv::SPEED, s);
            w.svc.map.g15fdfc = 1;
            drive(w, id);
            if w.m(id).state >= 0x80 { return; }
            engine_sounds(w, id);
            if w.hero.air_ticks == 0 || w.hero.group == 3 {
                c::set_pi32(w, id, pv::AIR, 0);
            } else {
                let path = spline(w, c::pi32(w, lead, pv::LEAD_PATH));
                let h = hero(w);
                let near = path.and_then(|p| {
                    let pts: Vec<[f32; 4]> = w.svc.splines[p].iter().map(|q| q.map(f32::from_bits)).collect();
                    crate::spline::nearest(&pts, false, 1000.0, 5.0, 0.0, [h[0], h[1], h[2]])
                });
                let n = c::pi32(w, id, pv::AIR) + 1;
                c::set_pi32(w, id, pv::AIR, n);
                if let Some((q, _)) = near {
                    if w.ticks(60) < n && h[2] < q[2] - 10.0 {
                        release(w, id);
                        let t = w.ticks(16);
                        crate::cinematic::fade_to_black(w, t);
                        w.hero_fields_mut().call(crate::moby_update::services::HeroCall::Reload);
                        return;
                    }
                }
            }
            if let Some(cam) = link(w, id, pv::RIDE_CAM) {
                if w.m(cam).cmd != 0 {
                    let h = hero(w);
                    aim_cutaway(w, cam, h);
                }
            }
            if let Some(g) = w.hero.ground_moby.filter(|&g| g < w.table.mobys.len()) {
                if matches!(w.m(g).o_class, 822 | CAR) { interact::set_talked(w, g, 3); }
            }
            if w.hero_on_moby(id) { arrive(w, id); }
        }
        5 => {
            drive(w, id);
            if w.m(id).state >= 0x80 { return; }
            w.svc.map.g15fdfc = 1;
            engine_sounds(w, id);
            if c::pi32(w, id, pv::STOP) <= c::pi32(w, lead, pv::LEAD_IDX) {
                c::set_pf(w, id, pv::LEAD_D, 0.0);
                let s = (c::pf(w, id, pv::SPEED) - (c::pf(w, id, pv::TOP) / 1.9) * c::DT).max(0.0);
                c::set_pf(w, id, pv::SPEED, s);
                if s == 0.0 {
                    if let Some(cam) = link(w, id, pv::ARRIVAL_CAM) { w.mm(cam).cmd = 0; }
                    w.mm(id).cmd = 6;
                    let top = c::pf(w, id, pv::TOP) * 0.5;
                    c::set_pf(w, id, pv::TOP, top);
                    release(w, id);
                }
            }
            if let Some(cam) = link(w, id, pv::ARRIVAL_CAM) {
                let h = hero(w);
                aim_cutaway(w, cam, h);
            }
        }
        6 => {
            if !w.in_cuboid(hero3(w), c::pi32(w, id, pv::RESTART)) {
                stand(w, id);
                return;
            }
            for i in 0..8 {
                let Some(t) = link(w, id, pv::TROOPERS + 4 * i) else { continue };
                let m = w.m(t);
                if alive(w, t) && m.o_class == kerwan_trooper::CLASSES[0] && m.state == 0x10 { w.mm(t).state = 0x11; }
            }
            let top = c::pf(w, id, pv::TOP);
            c::set_pf(w, id, pv::SPEED, top);
            c::set_pf(w, id, pv::LEAD_D, 20.0);
            w.mm(id).cmd = 1;
        }
        _ => {}
    }
}

fn hero3(w: &World) -> [f32; 3] { let h = hero(w); [h[0], h[1], h[2]] }

/// The lead 845's own update (level03 `0x293c78`, the flyer driver): the locomotive sets its mode bit 2 at its init, so
/// the moby loop never runs it after the first tick.
pub fn lead_update(w: &mut World, id: MobyId) { flyer::driver(w, id); }
