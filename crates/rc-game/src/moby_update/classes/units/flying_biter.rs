//! U407 (census 2026-09-29): class 63, the flying biters of Gemlik (level13 `0x2b50d8`, 58 created). The same
//! creature header as the pack biters 193 (`super::pack_biter`: damage record +0x20, flash +0x110, knockback +0x120,
//! suck record +0x60, walker +0x180) but its own code (another source: different states, sequences and tests), so its
//! own port. Most of them fly (+0x20a = 0): they hover over home (6 up) or along their flight path (+0x220), pick random
//! points near home to fly to, land when a target comes within range + 4 (1 in 19 a tick) and then fight on foot as the
//! pack biters do — face the target, charge with a swerve, bite (key 13: a hit of 1), go home — and take off again when
//! the target is gone. The ground ones (+0x20a = 1) stay on foot. A weapon hit knocks one back (health 1: the second
//! kills) or kills it (the death flight, `SetDeathBits`, then the death explosion); a fall below z 5 kills it (the
//! Gemlik pits). A cuboid (+0x22c) can bound where it sees a target. While it flies and is drawn it carries a
//! particle of type 68 (level13 `0x280400`: a flickering additive ribbon trailing its joint list 0, colours
//! 0x80808080 / 0x10808080; `crate::particles::type68`). The Suck Cannon takes it (the level-01 critter table shape, held 7).
//!
//! **Pvars** (0x270): +0x20 the damage record (+0x26 s16 the class's hit cooldown), +0x38 the lure, +0x40 the flight
//! velocity, +0x60 the suck record, +0x110 the flash, +0x120 the knockback record, +0x180 the walker, +0x198 the
//! vertical speed, +0x1d0 home (w: the range it was placed with), +0x1e0 the flight point, +0x1f0 the turn velocity,
//! +0x1f4 / +0x1f8 the swerve, +0x1fc the range, +0x200 the alert range, +0x204 the walk speed, +0x208 (s16, counted
//! down, read by nothing here), +0x20a (s16) on foot, +0x20c the flight-point timer, +0x210 the swerve timer, +0x214
//! (s16) the alert, +0x216 (s16) / +0x218 the leash (state 0xd), +0x21c the walkway path, +0x220 the flight path,
//! +0x224 its point, +0x228 the type-68 particle, +0x22c the sight cuboid (−1 none), +0x230 the big-head cheat's.
//!
//! ## Coverage (level13 `0x2b50d8` and its private helpers `0x2b4fb8`, `0x2b4f28`, `0x2b4ff0`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x26f7a8(2.5, m, 0, +0x230)` | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn, within 28 of the camera: `0x2661a8` (= `0x26f020`), +0x7f = 0x16 | the shadow probe | [`update`] (`shadows::probe_down`) |
//! | game mode 2 (0x15f5c4): +0x94 = 0, mode \| 0x41, done; else the class's collision, mode & ~0x41 | hidden in cutscenes | [`update`] |
//! | `0x26bd00(range + 4, m)` (= `0x274b78`); no moby, or not alerted with a cuboid +0x22c the target is outside (`0x26b9a8` = `PointInCuboid`) → no target (zero) | the target search (decoys win), the sight box | [`sight`] (`target::acquire`, `World::in_cuboid`) |
//! | lure +0x38 → +0x214 = `ticks(240)`; `FastDecTimer_s16(+0x208)`; state ≠ 0: +0x214 counting → range +0x200, else +0x1dc | the Taunter; the alert range | [`update`] |
//! | `0x2664a8` / `0x266500` (col 4); `FastDecTimer_s16(+0x26)`; a hit while +0x26 < `ticks(45)`, not dying: class 0xb0 / 0xb1 with the cooldown running deals 0; group call `0x265000(group, 1)` | every weapon's hit record through the resolver; the pack joins in | [`hit`] (`damage::resolve`, `attack::group_command`) |
//! | damage ≠ 0: K zoff 0.5, radius 0x202, gravity 0.0055, drag 0.0011, speeds 3·dt / 7·dt (gp−0x586c / −0x5870), flags 9, +0x15d 0; health −= damage | | [`hit`] |
//! | health left: `0x266bd0` (= `0x26fa48`) the push's heading and speeds, `0x2685a0(…, K, (rand() & 1) + 9, 1, 0)`, keys 5 / 10, state 6, the particle off, flash 0xfa, +0x26 `ticks(60)`, flash start | the knockback | [`hit`] (`knock::aim`, `knock::start`) |
//! | none: untargetable, 10·dt / 5.5·dt, the same aim and start, keys 11 / 18, state 99, particle off, flash 0x78, flash start, `SetDeathBits(m, 0, −1)` | the death flight, the bolts and the save bit | [`hit`] (`crate_::set_death_bits`) |
//! | +0xa4 = 0xff | | [`update`] |
//! | case 0: home (w = range); on foot → 1 (blend 3), flying → 0xe, z + 6 (blend 0); D fields, +0x58 8 / +0x5a 4, the walker (radius 0x38d, probes 2 / 2, top speed +0x204·dt, flags \| 0x30), `rand() & 1` mirror, +0xd0 gp−0x5860, the flight path's nearest point (`0x281d70` = `0x28b510`) | init | [`init`] (+0xd0: `react::SEQS_63`) |
//! | case 1: within range + 4 and 8 in z, 1 in 19 → 2 (blend 4); +0xbc, 1 in 19 → the charge; flying kind beyond range + 6 → take off (0x10) | on foot, idle | [`idle`] |
//! | case 2: `SpringTurn2(atan(pos − target), 0.02, 0.3, 0.1)` (it turns its back to the target); as 193's (range, 8 in z, 1 in 19 → group call and the charge; +0xbc); sequence 4 wrapped → blend 3 | face | [`face`] |
//! | case 3: aim = target or `0x282010(path, target)`; turn (+ swerve, 0.05, 0.3, 0.2); `0x264918(1, m, J, 2·dir)`; swerve timer; not near (1.5; the gold chicken 0x10e alive within 2.3 counts as near) or blocked: steep or blocked → back to the old xy; far from home (range + 4) or 8 in z → home 5 unless +0xbc; else group call; near → bite 4 (blend 6) | the charge | [`charge`] (`walker::step`, `path::toward`) |
//! | case 4: turn; not blending, key 13 (±0.2), within 1.5 in z: within 2 (xy; 2.6 for a live gold chicken) and facing 15° → `0x265c30(1, target, m, 1, target + 0.75 z, 0.2·dir)` (= `0x26eaa8`); wrapped and beyond 1.5 → 3 (blend 5); gravity 9.8·dt² onto `GroundHeight(0.5, fl 0x120)` | the bite on Ratchet (his moby's hit record: the hero's intake) | [`bite`] (`attack::hit_moby`) |
//! | case 5: aim = home or `0x282010(path, home)`; turn (0.02, 0.3, 0.1), step; within 2 of home → 1 (blend 3); the target near home (range) → group call, 3; +0xbc → 3; blocked → old xy, 0xc (blend 4) | go home | [`home`] |
//! | case 6: `0x2686e0` (= `0x271558`) landed → K velocity 0, 3 (blend 5), done; below z 5 → `SetDeathBits`, deleted | knockback; the pit | [`update`] (`knock::update`) |
//! | case 7: particle off; `0x2ffa30` (= `0x305260`) done → 0xe (blend 0), record state 0 | the Suck Cannon's carried update | [`update`] (`react::carried`) |
//! | case 8: within range − 4 and 8 in z, 1 in 19 → 2, z + 1 (blend 4) | (entered by no code here) | [`update`] |
//! | case 10: turn toward home; the target near home (range, 8 in z) or +0xbc = 2 → group call `(group, 2)`, blend (`rand() & 1`) + 9, a lob home (`0x266c78` = `0x26faf0`: speed 7·dt, gravity 0.008), flags 0xd, `0x2685a0(yaw, …)`, keys 5 / 10, particle on, 0xb | (entered by no code here) | [`update`] (`knock::lob_up`) |
//! | case 0xb: landed → particle off, 3 with the charge's swerve (blend 5 from frame 8), done; below z 5 → `SetDeathBits`, deleted | the lob | [`update`] |
//! | case 0xc: as 193's stuck (blocked → wrap blend 3 / 4 with two `randi(4)`; then 1 in 99 → take off 0x10) | stuck | [`stuck`] |
//! | case 0xd: blend 5; walk +0x204·dt ahead sliding round Ratchet's and other 63s within 2; gravity (fl 0x120); the leash → stay, else 5 | (entered by no code here) | [`leashed`] |
//! | case 0xe: particle on; ground 1 below: a target within range + 4 (xy) and 10 in z, 1 in 19 → land 0x11 (particle off); +0xbc and 1 in 4 → the swerve, land; no flight path: timer `ticks(rand_range(40, 120))` → a point `randf(0, 4)` from home and `randf(5, 7)` up with ground more than 1 below (10 tries), 0xf; a path → its next point (within 0.25), 0x12 | fly: hover | [`hover`] |
//! | case 0xf: turn (0.03, 0.6, 0.09) toward the point; anim speed 20·\|v\| + 1; speed `Approach` to 4·dt (6·dt²) or dt near (8·dt²); `coll_sphere(1.2)` free → move, else stop (0xe); arrived → 0xe; particle on | fly to a point | [`fly_to`] (`World::coll_sphere`) |
//! | case 0x10: vz `Approach` 8·dt (dt above 3) at 10.8·dt²; `0x264580(0.6, 0.5, 0, m, vel, 0)` (= `0x26d610`); anim speed 6 − 5·h/6; 6 up → 0xe; particle on | take off | [`update`] (`walker::move_collide`) |
//! | case 0x11: vz `Approach` −8·dt; the move; within 8·dt·45 of the ground → blend 2 (5); landed → the swerve, 3 (blend 5) | land | [`update`] |
//! | case 0x12: turn (0.018, 0.8, 0.09); v = point − pos ≤ 4·dt; anim speed; `coll_sphere(1.2)` free → move, else `ticks(120)` and a random point (10 tries), 0xf; at the point → the next (wrap), z ± 0.2; ground 1 below: a target near (range + 4, 10 in z, 1 in 19) → land; else +0xbc and 1 in 4 → the swerve, land; particle on | fly the path | [`fly_path`]; the near test reads a stack vector the tick may not have written (the random point's offset after a collision, else stale): the port uses the target's offset [L] |
//! | case 99: untargetable; `0x2686e0` & 0x140 → the rate slot `0x2b4ec0(0x1613b0, 3, ticks(1)·60)` (= `0x2efbf8`), a free one → `0x26b0d8(0.5, 13, m, pos, 4)` (= `0x273f50`: 3 spark pairs, 2 flashes, shake, class sound 4, light); deleted; below z 0 → deleted | the death | [`update`] (`fx::rate_slot`, `fx::death_explosion`) |
//! | tail: `0x269580` (= `0x2723f8`) flash update; outside [2, 1021]³ → deleted | | [`update`] |
//! | `0x2b4ff0` (cases 1–5): ground (fl 0) within 0.3 below the feet → K vz −= 9.8·dt², z += vz, not below; below the ground → z += 9.8·dt², K velocity 0; else K velocity 0 | settle | [`settle`] |
//! | `0x2b4f28` / `0x2b4fb8` | the particle on (drawn: `0x280400(0.2, 0.2, 0.8, m, 0, 0x80808080, 0x10808080, 0x19)`) / off (`0x273410` = `KillPart`) | [`glow_on`] (`particles::type68`: the ribbon lives and draws), [`glow_off`] |
//! | table (level13 0x1f6174): 0x2b7980 / 0x2b79d0 / 0x2b7a20 (bounce: class sound 2) / 0x2b7ad0 (held 7, refused → 1), 0x2b7b00 (+0x60), `DeleteMoby` | the Suck Cannon: the level-01 critter shape | `react::slot_*` with `react::CRITTER` and the class's sequence table |
//!
//! Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, ground, knock, react, target, turn, walker};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2b_50d8;
pub const REFERENCE_LEVEL: u32 = 13;
pub const CLASSES: [i16; 1] = [63];
/// The gold Morph-o-Ray chicken (a decoy the bite reaches a little farther).
const GOLD_CHICKEN: i16 = 0x10e;
/// The particle type the flight carries (level13 `0x280400`).
pub const GLOW_PART: u8 = 68;

pub mod pv {
    pub const D: usize = 0x20;
    pub const COOLDOWN: usize = 0x26;
    pub const LURE: usize = 0x38;
    pub const VEL: usize = 0x40;
    pub const SUCK: usize = 0x60;
    pub const FLASH: usize = 0x110;
    pub const K: usize = 0x120;
    pub const J: usize = 0x180;
    pub const VZ: usize = 0x198;
    pub const HOME: usize = 0x1d0;
    pub const POINT: usize = 0x1e0;
    pub const TURN_V: usize = 0x1f0;
    pub const SWERVE: usize = 0x1f4;
    pub const SWERVE_DEG: usize = 0x1f8;
    pub const RANGE: usize = 0x1fc;
    pub const ALERT_RANGE: usize = 0x200;
    pub const SPEED: usize = 0x204;
    pub const T208: usize = 0x208;
    pub const ON_FOOT: usize = 0x20a;
    pub const POINT_T: usize = 0x20c;
    pub const SWERVE_T: usize = 0x210;
    pub const ALERT: usize = 0x214;
    pub const LEASH_R: usize = 0x216;
    pub const LEASH: usize = 0x218;
    pub const PATH: usize = 0x21c;
    pub const FLIGHT: usize = 0x220;
    pub const FLIGHT_I: usize = 0x224;
    pub const PART: usize = 0x228;
    pub const CUBOID: usize = 0x22c;
    pub const SIZE: usize = 0x234;
}

pub mod st {
    pub const INIT: u8 = 0;
    pub const IDLE: u8 = 1;
    pub const FACE: u8 = 2;
    pub const CHARGE: u8 = 3;
    pub const BITE: u8 = 4;
    pub const HOME: u8 = 5;
    pub const KNOCKED: u8 = 6;
    pub const HELD: u8 = 7;
    pub const LOB: u8 = 0xb;
    pub const STUCK: u8 = 0xc;
    pub const LEASHED: u8 = 0xd;
    pub const HOVER: u8 = 0xe;
    pub const FLY_TO: u8 = 0xf;
    pub const TAKE_OFF: u8 = 0x10;
    pub const LAND: u8 = 0x11;
    pub const FLY_PATH: u8 = 0x12;
    pub const DYING: u8 = 99;
}

/// Level13 gp−0x586c / −0x5870 (0x161394 / 0x161390): the knockback's out and up speeds; gp−0x5868 / −0x5864: the
/// lob's speed 7 and gravity 0.008.
const KNOCK_OUT: f32 = 3.0;
const KNOCK_UP: f32 = 7.0;
const LOB_SPEED: f32 = 7.0;
const LOB_GRAVITY: f32 = 0.008;

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
/// `MobyAnimBlend(m, s, 0, ticks(n))` behind `if (m+0x53 != s)`.
fn blend(w: &mut World, id: MobyId, s: u8, n: i32) {
    if seq(w, id) != s {
        let t = w.ticks(n);
        w.anim_blend(id, s, 0, t);
    }
}
fn range(w: &World, id: MobyId) -> f32 { c::pf(w, id, pv::RANGE) }
fn group_call(w: &mut World, id: MobyId, v: u8) {
    let g = w.m(id).group;
    if g != -1 { attack::group_command(w, g, v); }
}
fn spline(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn aim(w: &World, id: MobyId, to: c::V) -> c::V {
    match spline(w, id, pv::PATH) {
        Some(p) => crate::path::toward(&w.svc.splines[p], c::pos(w, id), to),
        None => to,
    }
}
fn turn_to(w: &mut World, id: MobyId, h: f32, acc: f32, damp: f32, max: f32) { turn::spring_turn2_pvar(w, id, h, acc, damp, max, pv::TURN_V); }
fn heading(w: &World, id: MobyId, to: c::V) -> f32 { let p = c::pos(w, id); c::atan(to[0] - p[0], to[1] - p[1]) }
/// `0x264918(1, m, J, 2·(cos yaw, sin yaw, 0), &out)`: the direction itself as the step's target point.
fn step(w: &mut World, id: MobyId) -> u32 {
    let (co, si) = c::cs(c::yaw(w, id));
    let mut out = [0.0; 4];
    walker::step(w, id, pv::J, 1.0, [co + co, si + si, 0.0, 0.0], &mut out)
}
fn ground_at(w: &World, p: c::V, fl: u32) -> f32 { ground::ground(w, p, 0.5, fl).z }
fn zero_vel(w: &mut World, id: MobyId) { c::set_pv4(w, id, pv::VEL, [0.0; 4]); }

/// `0x2b4fb8`: the flight particle off (`KillPart`).
pub fn glow_off(w: &mut World, id: MobyId) {
    let h = c::pi32(w, id, pv::PART);
    if h != 0 {
        if let Some(p) = w.particles.as_deref_mut() { p.kill_part((h - 1) as usize); }
        c::set_pi32(w, id, pv::PART, 0);
    }
}

/// `0x2b4f28`: drawn → the flight particle on (`0x280400(0.2, 0.2, 0.8, m, 0, 0x80808080, 0x10808080, 0x19)`: the
/// type-68 ribbon on joint list 0, `crate::particles::type68`; its record's slot is kept), else off.
pub fn glow_on(w: &mut World, id: MobyId) {
    if w.m(id).visible == 0 { return glow_off(w, id); }
    if c::pi32(w, id, pv::PART) != 0 { return; }
    *w.svc.fx.part_spawns.entry(GLOW_PART).or_default() += 1;
    let at = w.joint_point(id, 0);
    let (draw_dist, o_class) = (w.m(id).draw_dist, w.m(id).o_class);
    let s = crate::particles::type68::Spawn { w1: 0.2, k: 0.2, len: 0.8, moby: id, list: 0, rgba1: 0x8080_8080, rgba2: 0x1080_8080, jitter: 0x19, draw_dist, o_class, at };
    let slot = w.particles.as_deref_mut().and_then(|p| crate::particles::type68::spawn(p, s));
    if slot.is_none() { w.svc.fx.part_failed += 1; }
    c::set_pi32(w, id, pv::PART, slot.map_or(0, |s| s as i32 + 1));
}

/// The charge's swerve (level13 0x2b6998 and its inline copies): `randi(256) & 1` flips it, `ticks(randf(30, 90))`.
fn swerve(w: &mut World, id: MobyId) {
    if w.rng.randi(0x100) & 1 != 0 {
        let d = c::pf(w, id, pv::SWERVE_DEG);
        c::set_pf(w, id, pv::SWERVE_DEG, -d);
    }
    let s = c::pf(w, id, pv::SWERVE_DEG) * 0.017_453_292;
    c::set_pf(w, id, pv::SWERVE, s);
    let f = w.rng.randf(30.0, 90.0);
    let t = w.ticks(f as i32);
    c::set_pi32(w, id, pv::SWERVE_T, t);
}
fn charge_start(w: &mut World, id: MobyId) {
    swerve(w, id);
    set_state(w, id, st::CHARGE);
    blend(w, id, 5, 3);
    w.mm(id).cmd = 0;
}

/// The target search and the sight box (module doc).
fn sight(w: &mut World, id: MobyId) -> target::Target {
    let t = target::acquire(w, id, range(w, id) + 4.0);
    let cub = c::pi32(w, id, pv::CUBOID);
    let hidden = t.moby.is_none() || (c::pi16(w, id, pv::ALERT) == 0 && cub != -1 && !w.in_cuboid([t.pos[0], t.pos[1], t.pos[2]], cub));
    if hidden { target::Target { kind: t.kind, ..Default::default() } } else { t }
}

/// The hit (module doc).
fn hit(w: &mut World, id: MobyId) {
    let h = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, h, pv::D, 0, 4);
    c::dec_timer_pvar_s16(w, id, pv::COOLDOWN);
    let cool = c::pi16(w, id, pv::COOLDOWN);
    if (cool as i32) >= w.ticks(0x2d) || state(w, id) == st::DYING { return; }
    let Some(r) = res.hit else { return };
    let mut dmg = f32::from_bits(r.damage.0);
    if r.attacker.is_some_and(|a| (w.m(a).o_class as u16).wrapping_sub(0xb0) < 2) && cool != 0 { dmg = 0.0; }
    group_call(w, id, 1);
    if dmg == 0.0 { return; }
    let k = pv::K;
    c::set_pf(w, id, k + knock::k::ZOFF, 0.5);
    c::set_pi32(w, id, k + knock::k::RADIUS, 0x202);
    let hp = c::pf(w, id, pv::D) - dmg;
    c::set_pf(w, id, k + knock::k::GRAVITY, f32::from_bits(0x3bb4_3958));
    c::set_pf(w, id, k + knock::k::DRAG, f32::from_bits(0x3a90_2de0));
    c::set_pf(w, id, k + knock::k::SPEED, KNOCK_OUT * c::DT);
    c::set_pf(w, id, k + knock::k::UP, KNOCK_UP * c::DT);
    c::set_pi32(w, id, k + knock::k::FLAGS, 9);
    c::set_pf(w, id, pv::D, hp);
    c::set_pu8(w, id, k + 0x3d, 0);
    let dir = r.dir.map(|x| f32::from_bits(x.0));
    let alive = 0.0 < hp;
    if !alive {
        w.mm(id).mode &= !mode::TARGETABLE;
        c::set_pf(w, id, k + knock::k::SPEED, c::DT * 10.0);
        c::set_pf(w, id, k + knock::k::UP, c::DT * 5.5);
    }
    let (mut sp, mut up) = (c::pf(w, id, k + knock::k::SPEED), c::pf(w, id, k + knock::k::UP));
    let a = knock::aim(dir, &mut sp, &mut up);
    c::set_pf(w, id, k + knock::k::SPEED, sp);
    c::set_pf(w, id, k + knock::k::UP, up);
    let s = (w.rng.rand() & 1) as u8 + 9;
    knock::start(w, id, k, a, s, 1, 0);
    if alive {
        c::set_pf(w, id, k + knock::k::KEY_APEX, 5.0);
        c::set_pf(w, id, k + knock::k::KEY_LAND, 10.0);
        set_state(w, id, st::KNOCKED);
        glow_off(w, id);
        c::set_pu8(w, id, pv::FLASH + 7, 0xfa);
        let t = w.ticks(0x3c);
        c::set_pi16(w, id, pv::COOLDOWN, t as i16);
        flash::start(w, id, pv::FLASH);
    } else {
        c::set_pf(w, id, k + knock::k::KEY_APEX, 11.0);
        c::set_pf(w, id, k + knock::k::KEY_LAND, 18.0);
        set_state(w, id, st::DYING);
        glow_off(w, id);
        c::set_pu8(w, id, pv::FLASH + 7, 0x78);
        flash::start(w, id, pv::FLASH);
        set_death_bits(w, id, 0, -1);
    }
}

/// Case 0.
fn init(w: &mut World, id: MobyId) {
    let mut h = c::pos(w, id);
    h[3] = range(w, id);
    c::set_pv4(w, id, pv::HOME, h);
    if c::pi16(w, id, pv::ON_FOOT) == 0 {
        set_state(w, id, st::HOVER);
        w.mm(id).position[2] += 6.0;
        if seq(w, id) != 0 { w.anim_blend(id, 0, 0, 0); }
    } else {
        set_state(w, id, st::IDLE);
        if seq(w, id) != 3 { w.anim_blend(id, 3, 0, 0); }
    }
    c::set_pi16(w, id, pv::ALERT, 0);
    c::set_pi16(w, id, pv::T208, 0);
    c::set_pu8(w, id, pv::D + 9, 0);
    c::set_pf(w, id, pv::D, 1.0);
    c::set_pi16(w, id, pv::D + 4, 1);
    c::set_pu8(w, id, 0x58, 8);
    c::set_pu8(w, id, 0x5a, 4);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    let sp = c::pf(w, id, pv::SPEED) * c::DT;
    c::set_pi32(w, id, pv::J, 0x38d);
    c::set_pf(w, id, pv::J + 8, 2.0);
    c::set_pf(w, id, pv::J + 0x24, sp);
    let f = c::pi32(w, id, pv::J + 0x38) | 0x30;
    c::set_pi32(w, id, pv::J + 0x38, f);
    c::set_pf(w, id, pv::J + 0xc, 2.0);
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
    c::set_pi32(w, id, pv::SUCK + react::rec::SEQS, react::seq_table_id(react::SEQS_63));
    if let Some(p) = spline(w, id, pv::FLIGHT) {
        let i = crate::path::nearest_at_distance(&w.svc.splines[p], 0.0, c::pos(w, id));
        c::set_pi32(w, id, pv::FLIGHT_I, i);
    }
}

/// `0x2b4ff0` (module doc).
fn settle(w: &mut World, id: MobyId) {
    let g = ground_at(w, c::pos(w, id), 0);
    let z = w.m(id).position[2];
    if g < z + 0.3 {
        let vz = c::pf(w, id, pv::K + 8) - c::DT2 * 9.8;
        c::set_pf(w, id, pv::K + 8, vz);
        let z = z + vz;
        w.mm(id).position[2] = if z < g { g } else { z };
    } else {
        if z < g { w.mm(id).position[2] = z + c::DT2 * 9.8; }
        c::set_pv4(w, id, pv::K, [0.0; 4]);
    }
}

/// Case 1.
fn idle(w: &mut World, id: MobyId, t: &target::Target) {
    let d = c::sub(t.pos, c::pos(w, id));
    let l = c::len3(d);
    if l < range(w, id) + 4.0 && (w.m(id).position[2] - t.pos[2]).abs() < 8.0 && w.rng.randi(0x13) == 0 {
        set_state(w, id, st::FACE);
        blend(w, id, 4, 3);
        return;
    }
    if w.m(id).cmd == 1 && w.rng.randi(0x13) == 0 {
        charge_start(w, id);
        return;
    }
    if range(w, id) + 6.0 < l && c::pi16(w, id, pv::ON_FOOT) == 0 {
        zero_vel(w, id);
        set_state(w, id, st::TAKE_OFF);
        blend(w, id, 0, 3);
    }
}

/// Case 2.
fn face(w: &mut World, id: MobyId, t: &target::Target) {
    let p = c::pos(w, id);
    turn_to(w, id, c::atan(p[0] - t.pos[0], p[1] - t.pos[1]), 0.02, 0.3, 0.1);
    let l = c::len3(c::sub(t.pos, p));
    let mut go = false;
    if l < range(w, id) && (p[2] - t.pos[2]).abs() < 8.0 && w.rng.randi(0x13) == 0 {
        group_call(w, id, 1);
        go = true;
    } else if w.m(id).cmd == 1 && w.rng.randi(0x13) == 0 {
        go = true;
    }
    if go { charge_start(w, id); }
    if seq(w, id) == 4 && wrapped(w, id) {
        let tk = w.ticks(3);
        w.anim_blend(id, 3, 0, tk);
    }
}

/// A live gold chicken target within `r` (xy) counts as near for the charge and the bite.
fn chicken_near(w: &World, t: &target::Target, d2: f32, r: f32) -> bool {
    t.moby.is_some_and(|m| {
        let o = w.m(m);
        o.o_class == GOLD_CHICKEN && o.state != crate::moby_runtime::state::DELETED && o.state != crate::moby_runtime::state::DELETED_STATIC && d2 < r
    })
}

fn to_bite(w: &mut World, id: MobyId) {
    set_state(w, id, st::BITE);
    blend(w, id, 6, 3);
    w.mm(id).cmd = 0;
}

/// Case 3; `old` the position before the states.
fn charge(w: &mut World, id: MobyId, t: &target::Target, old: c::V) {
    let p = c::pos(w, id);
    let (dz, dxy) = ((p[2] - t.pos[2]).abs(), c::dist2(p, t.pos));
    let a = aim(w, id, t.pos);
    let h = heading(w, id, a) + c::pf(w, id, pv::SWERVE);
    turn_to(w, id, h, 0.05, 0.3, 0.2);
    let r = step(w, id);
    if c::dec_timer_pvar_i32(w, id, pv::SWERVE_T) != 0 {
        let f = w.rng.randf(30.0, 90.0);
        let tk = w.ticks(f as i32);
        c::set_pi32(w, id, pv::SWERVE_T, tk);
        let s = c::pf(w, id, pv::SWERVE);
        c::set_pf(w, id, pv::SWERVE, -s);
    }
    let e = c::sub(t.pos, c::pv4(w, id, pv::HOME));
    let d2 = c::dist2(c::pos(w, id), t.pos);
    let near = d2 < 1.5 || chicken_near(w, t, d2, 2.3);
    if !near || r & 2 != 0 {
        if 1.0 < dz / dxy || r & 2 != 0 {
            let m = w.mm(id);
            m.position[0] = old[0];
            m.position[1] = old[1];
        }
        let z = w.m(id).position[2];
        if range(w, id) + 4.0 <= c::len3(e) || 8.0 <= (z - t.pos[2]).abs() {
            if w.m(id).cmd != 1 { set_state(w, id, st::HOME); }
        } else {
            group_call(w, id, 1);
        }
        w.mm(id).cmd = 0;
    } else {
        to_bite(w, id);
    }
}

/// Case 4.
fn bite(w: &mut World, id: MobyId, t: &target::Target) {
    let p = c::pos(w, id);
    turn_to(w, id, c::atan(t.pos[0] - p[0], t.pos[1] - p[1]), 0.05, 0.3, 0.2);
    let a = w.m(id).anim;
    let kt = ground::key_time(w, id);
    let p = c::pos(w, id);
    let dz = (p[2] - t.pos[2]).abs();
    let d2 = c::dist2(p, t.pos);
    let diff = c::diff_rots(c::atan(t.pos[0] - p[0], t.pos[1] - p[1]), c::yaw(w, id));
    if a.seq_a == a.seq_b && (kt - 13.0).abs() < 0.2 && dz < 1.5 && (d2 < 2.0 || chicken_near(w, t, d2, 2.6)) && diff < 0.261_799_4 {
        let (co, si) = c::cs(c::yaw(w, id));
        let dir = [co * 0.2, si * 0.2, 0.0, 0.0];
        let at = [t.pos[0], t.pos[1], t.pos[2] + 0.75, t.pos[3]];
        if let Some(m) = t.moby { attack::hit_moby(w, m, id, 1.0, 1, at, dir); }
    }
    if wrapped(w, id) && 1.5 < c::dist2(c::pos(w, id), t.pos) {
        set_state(w, id, st::CHARGE);
        blend(w, id, 5, 3);
    }
    let g = ground_at(w, c::pos(w, id), 0x120);
    let vz = c::pf(w, id, pv::VZ) - c::DT2 * 9.8;
    c::set_pf(w, id, pv::VZ, vz);
    let z = w.m(id).position[2] + vz;
    w.mm(id).position[2] = z;
    if z < g {
        c::set_pf(w, id, pv::VZ, 0.0);
        w.mm(id).position[2] = g;
    }
}

/// Case 5; `old` the position before the states.
fn home(w: &mut World, id: MobyId, t: &target::Target, old: c::V) {
    let hm = c::pv4(w, id, pv::HOME);
    let a = aim(w, id, hm);
    let h = heading(w, id, a);
    turn_to(w, id, h, 0.02, 0.3, 0.1);
    let r = step(w, id);
    if c::dist2(c::pos(w, id), hm) < 2.0 {
        set_state(w, id, st::IDLE);
        blend(w, id, 3, 3);
    }
    let e = c::sub(t.pos, hm);
    let go = if c::len3(e) < range(w, id) {
        group_call(w, id, 1);
        true
    } else {
        w.m(id).cmd == 1
    };
    if go {
        set_state(w, id, st::CHARGE);
        blend(w, id, 5, 3);
    } else if r & 2 != 0 {
        let m = w.mm(id);
        m.position[0] = old[0];
        m.position[1] = old[1];
        set_state(w, id, st::STUCK);
        blend(w, id, 4, 3);
    }
    settle(w, id);
    w.mm(id).cmd = 0;
}

/// Case 0xc.
fn stuck(w: &mut World, id: MobyId, t: &target::Target) {
    let p = c::pos(w, id);
    let (dz, dxy) = ((p[2] - t.pos[2]).abs(), c::dist2(p, t.pos));
    let hm = c::pv4(w, id, pv::HOME);
    let a = aim(w, id, hm);
    let h = heading(w, id, a);
    turn_to(w, id, h, 0.05, 0.3, 0.2);
    let r = step(w, id);
    let d2 = c::dist2(c::pos(w, id), t.pos);
    let steep = 1.0 < dz / dxy;
    if !(1.5 <= d2 || steep) { return to_bite(w, id); }
    if !steep && r & 2 == 0 {
        set_state(w, id, st::HOME);
        blend(w, id, 5, 3);
        w.mm(id).cmd = 0;
        return;
    }
    if wrapped(w, id) {
        w.rng.randi(4);
        let s = if w.rng.randi(4) != 0 { 3 } else { 4 };
        let tk = w.ticks(3);
        w.anim_blend(id, s, 0, tk);
    }
    if w.rng.randi(99) == 0 {
        zero_vel(w, id);
        set_state(w, id, st::TAKE_OFF);
        blend(w, id, 0, 3);
    }
    w.mm(id).cmd = 0;
}

/// Case 0xd.
fn leashed(w: &mut World, id: MobyId) {
    let a = w.m(id).anim;
    if a.seq_a != 5 && a.seq_b != 5 {
        let tk = w.ticks(3);
        w.anim_blend(id, 5, 0, tk);
    }
    let yaw = c::yaw(w, id);
    let sp = c::pf(w, id, pv::SPEED) * c::DT;
    let p = c::pos(w, id);
    let mut to = [p[0] + yaw.cos() * sp, p[1] + yaw.sin() * sp, p[2], p[3]];
    for o in 0..w.table.mobys.len() {
        let m = &w.table.mobys[o];
        if m.state == crate::moby_runtime::state::END { break; }
        if o == id || m.state >= 0x80 || !(m.o_class == 0 || m.o_class == 0x3f) { continue; }
        let op = m.position;
        if c::dist2(to, op) < 2.0 {
            let me = c::pos(w, id);
            let hd = c::atan(op[0] - me[0], op[1] - me[1]);
            let mut tr = c::DT * std::f32::consts::FRAC_PI_2;
            if c::sub_rot(c::yaw(w, id), hd) <= 0.0 { tr = -tr; }
            let y = c::add_rot(c::yaw(w, id), tr);
            c::set_yaw(w, id, y);
            to = c::add(c::set_len3(c::sub(me, op), 2.0), op);
            to[2] = me[2];
        }
    }
    c::set_pos(w, id, to);
    let g = ground_at(w, c::pos(w, id), 0x120);
    let vz = c::pf(w, id, pv::VZ) - c::DT2 * 9.8;
    c::set_pf(w, id, pv::VZ, vz);
    let z = w.m(id).position[2] + vz;
    w.mm(id).position[2] = z;
    if z < g {
        c::set_pf(w, id, pv::VZ, 0.0);
        w.mm(id).position[2] = g;
    }
    let l = c::pi32(w, id, pv::LEASH);
    if l > 0 && ((l - 1) as usize) < w.table.mobys.len() && c::dist2(c::pos(w, id), c::pos(w, (l - 1) as usize)) <= c::pi16(w, id, pv::LEASH_R) as f32 {
        w.mm(id).cmd = 0;
        return;
    }
    set_state(w, id, st::HOME);
    blend(w, id, 5, 3);
    w.mm(id).cmd = 0;
}

/// Land now (the swerve drawn when `with_swerve`).
fn land(w: &mut World, id: MobyId, with_swerve: bool) {
    if with_swerve { swerve(w, id); }
    blend(w, id, 0, 3);
    glow_off(w, id);
    set_state(w, id, st::LAND);
    if with_swerve {
        zero_vel(w, id);
        w.mm(id).cmd = 0;
    }
}

/// A random flight point near home with ground more than 1 below (10 tries; `r` the radius).
fn pick_point(w: &mut World, id: MobyId, r: f32) {
    for _ in 0..10 {
        let d = w.rng.randf(0.0, r);
        let a = w.rng.rand_angle();
        let up = w.rng.randf(5.0, 7.0);
        let h = c::pv4(w, id, pv::HOME);
        let q = [a.cos() * d + h[0], a.sin() * d + h[1], up + h[2], h[3]];
        c::set_pv4(w, id, pv::POINT, q);
        if 1.0 < ground_at(w, q, 0x120) { break; }
    }
    blend(w, id, 0, 3);
    set_state(w, id, st::FLY_TO);
}

/// The flight path's point `i`.
fn path_point(w: &World, p: usize, i: i32) -> c::V { w.svc.splines[p].get(i.max(0) as usize).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }

/// Case 0xe.
fn hover(w: &mut World, id: MobyId, t: &target::Target) {
    glow_on(w, id);
    let d = c::sub(t.pos, c::pos(w, id));
    let g = ground_at(w, c::pos(w, id), 0x120);
    if 1.0 < g {
        if c::len2(d) < range(w, id) + 4.0 && (w.m(id).position[2] - t.pos[2]).abs() < 10.0 && w.rng.randi(0x13) == 0 {
            land(w, id, false);
            return;
        }
        if w.m(id).cmd == 1 && w.rng.randi(4) == 0 {
            land(w, id, true);
            return;
        }
    }
    match spline(w, id, pv::FLIGHT) {
        None => {
            if c::dec_timer_pvar_i32(w, id, pv::POINT_T) != 0 {
                let n = w.rng.rand_range(0x28, 0x78);
                let tk = w.ticks(n);
                c::set_pi32(w, id, pv::POINT_T, tk);
                pick_point(w, id, 4.0);
            }
        }
        Some(p) => {
            let mut i = c::pi32(w, id, pv::FLIGHT_I);
            if c::dist3(c::pos(w, id), path_point(w, p, i)) < 0.25 {
                i += 1;
                if w.svc.splines[p].len() as i32 <= i { i = 0; }
                c::set_pi32(w, id, pv::FLIGHT_I, i);
            }
            let q = path_point(w, p, i);
            c::set_pv4(w, id, pv::POINT, q);
            set_state(w, id, st::FLY_PATH);
        }
    }
}

/// `coll_sphere(1.2, to, 0, m)` (0x1fc748 = level01 0x212960): free → the moby moves there.
fn fly_step(w: &mut World, id: MobyId, to: c::V) -> bool {
    let hit = w.coll_sphere(to.map(crate::ps2v::Pf::f), crate::ps2v::Pf::f(1.2), 0, Some(id)).is_some();
    if !hit { c::set_pos(w, id, to); }
    !hit
}

/// Case 0xf.
fn fly_to(w: &mut World, id: MobyId) {
    let q = c::pv4(w, id, pv::POINT);
    let h = heading(w, id, q);
    turn_to(w, id, h, f32::from_bits(0x3cf5_c28f), f32::from_bits(0x3f19_999a), f32::from_bits(0x3db8_51ec));
    let mut s = c::len3(c::pv4(w, id, pv::VEL));
    w.mm(id).anim.speed = s * 20.0 + 1.0;
    let v = c::sub(q, c::pos(w, id));
    if 1.0 < c::len3(v) { turn::approach(c::DT * 4.0, c::DT2 * 6.0, &mut s); } else { turn::approach(c::DT, c::DT2 * 8.0, &mut s); }
    let v = c::clamp_len3(v, s);
    c::set_pv4(w, id, pv::VEL, v);
    let to = c::add(v, c::pos(w, id));
    let mut arrived = false;
    if !fly_step(w, id, to) {
        zero_vel(w, id);
        blend(w, id, 0, 3);
        set_state(w, id, st::HOVER);
    }
    let d = c::dist3(c::pos(w, id), q);
    if d < s || s == 0.0 { arrived = true; }
    if arrived {
        blend(w, id, 0, 3);
        set_state(w, id, st::HOVER);
    }
    glow_on(w, id);
}

/// Case 0x12.
fn fly_path(w: &mut World, id: MobyId, t: &target::Target) {
    let Some(p) = spline(w, id, pv::FLIGHT) else { return glow_on(w, id) };
    let q = c::pv4(w, id, pv::POINT);
    let h = heading(w, id, q);
    turn_to(w, id, h, f32::from_bits(0x3c93_74bc), f32::from_bits(0x3f4c_cccd), f32::from_bits(0x3db8_51ec));
    let v = c::clamp_len3(c::sub(q, c::pos(w, id)), c::DT * 4.0);
    c::set_pv4(w, id, pv::VEL, v);
    let to = c::add(v, c::pos(w, id));
    w.mm(id).anim.speed = c::len3(v) * 20.0 + 1.0;
    // [L] the near test's vector (module doc): the random point's offset after a collision, else the target's.
    let mut near_v = c::sub(t.pos, c::pos(w, id));
    if !fly_step(w, id, to) {
        let tk = w.ticks(0x78);
        c::set_pi32(w, id, pv::POINT_T, tk);
        pick_point(w, id, 3.0);
        near_v = c::sub(c::pv4(w, id, pv::POINT), c::pv4(w, id, pv::HOME));
    }
    let q = c::pv4(w, id, pv::POINT);
    if c::dist3(c::pos(w, id), q) < c::len3(c::pv4(w, id, pv::VEL)) {
        let mut i = c::pi32(w, id, pv::FLIGHT_I) + 1;
        if w.svc.splines[p].len() as i32 <= i { i = 0; }
        c::set_pi32(w, id, pv::FLIGHT_I, i);
        let mut nq = path_point(w, p, i);
        nq[2] += w.rng.randf(-0.2, 0.2);
        c::set_pv4(w, id, pv::POINT, nq);
        let g = ground_at(w, c::pos(w, id), 0x120);
        if 1.0 < g {
            let z = w.m(id).position[2];
            let near = c::len2(near_v) < range(w, id) + 4.0 && (z - t.pos[2]).abs() < 10.0 && w.rng.randi(0x13) == 0;
            if near {
                blend(w, id, 0, 3);
                zero_vel(w, id);
                set_state(w, id, st::LAND);
                glow_off(w, id);
            } else if w.m(id).cmd == 1 && w.rng.randi(4) == 0 {
                land(w, id, true);
            }
        }
    }
    glow_on(w, id);
}

/// Level13 `0x2b50d8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    // 0x26f7a8(2.5, m, 0, +0x230): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 0, id, 0x230);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 28.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x16;
    }
    if w.svc.game_mode == 2 {
        let m = w.mm(id);
        m.has_collision = false;
        m.mode |= mode::HIDDEN | mode::NO_ANIM;
        return;
    }
    let o = w.m(id).o_class;
    let coll = crate::moby_update::classes::units::class_collision(w, o);
    w.mm(id).has_collision = coll;
    w.mm(id).mode &= !(mode::HIDDEN | mode::NO_ANIM);
    let t = sight(w, id);
    if c::pi32(w, id, pv::LURE) != 0 {
        let tk = w.ticks(0xf0);
        c::set_pi16(w, id, pv::ALERT, tk as i16);
    }
    c::set_pi32(w, id, pv::LURE, 0);
    c::dec_timer_pvar_s16(w, id, pv::T208);
    if state(w, id) != st::INIT {
        let r = if c::dec_timer_pvar_s16(w, id, pv::ALERT) == 0 { c::pf(w, id, pv::ALERT_RANGE) } else { c::pf(w, id, pv::HOME + 0xc) };
        c::set_pf(w, id, pv::RANGE, r);
    }
    hit(w, id);
    w.mm(id).hit_slot = 0xff;
    let old = c::pos(w, id);
    match state(w, id) {
        st::INIT => init(w, id),
        st::IDLE => { idle(w, id, &t); settle(w, id); }
        st::FACE => { face(w, id, &t); settle(w, id); }
        st::CHARGE => { charge(w, id, &t, old); settle(w, id); }
        st::BITE => { bite(w, id, &t); settle(w, id); }
        st::HOME => home(w, id, &t, old),
        st::KNOCKED => {
            if knock::update(w, id, pv::K) & knock::res::LANDED != 0 {
                c::set_pv4(w, id, pv::K, [0.0; 4]);
                set_state(w, id, st::CHARGE);
                blend(w, id, 5, 3);
                return;
            }
            if w.m(id).position[2] < 5.0 {
                set_death_bits(w, id, 0, -1);
                w.delete_moby(id);
                return;
            }
        }
        st::HELD => {
            glow_off(w, id);
            if react::carried(w, id, pv::K) != 0 {
                set_state(w, id, st::HOVER);
                if seq(w, id) != 0 { w.anim_blend(id, 0, 0, 0); }
                c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
            }
        }
        8 => {
            let d = c::sub(t.pos, c::pos(w, id));
            if c::len3(d) < range(w, id) - 4.0 && (w.m(id).position[2] - t.pos[2]).abs() < 8.0 && w.rng.randi(0x13) == 0 {
                set_state(w, id, st::FACE);
                w.mm(id).position[2] += 1.0;
                blend(w, id, 4, 3);
            }
        }
        10 => {
            let hm = c::pv4(w, id, pv::HOME);
            let hd = heading(w, id, hm);
            turn_to(w, id, hd, 0.05, 0.3, 0.2);
            let near = c::len3(c::sub(t.pos, hm)) < range(w, id) && (hm[2] - t.pos[2]).abs() < 8.0;
            if near || w.m(id).cmd == 2 {
                group_call(w, id, 2);
                let s = (w.rng.rand() & 1) as u8 + 9;
                if seq(w, id) != s {
                    let s2 = (w.rng.rand() & 1) as u8 + 9;
                    let tk = w.ticks(3);
                    w.anim_blend(id, s2, 0, tk);
                }
                let k = pv::K;
                c::set_pf(w, id, k + knock::k::DRAG, 0.0);
                c::set_pf(w, id, k + knock::k::GRAVITY, LOB_GRAVITY);
                c::set_pf(w, id, k + knock::k::SPEED, LOB_SPEED * c::DT);
                let mut time = 0.0;
                let up = knock::lob_up(LOB_SPEED * c::DT, -LOB_GRAVITY, c::pos(w, id), hm, &mut time);
                c::set_pu8(w, id, k + 0x3d, 0);
                c::set_pf(w, id, k + knock::k::UP, up);
                c::set_pi32(w, id, k + knock::k::FLAGS, 0xd);
                let s = (w.rng.rand() & 1) as u8 + 9;
                let y = c::yaw(w, id);
                knock::start(w, id, k, y, s, 1, 0);
                c::set_pf(w, id, k + knock::k::KEY_APEX, 5.0);
                c::set_pf(w, id, k + knock::k::KEY_LAND, 10.0);
                glow_on(w, id);
                set_state(w, id, st::LOB);
            }
        }
        st::LOB => {
            if knock::update(w, id, pv::K) & knock::res::LANDED != 0 {
                glow_off(w, id);
                c::set_pv4(w, id, pv::K, [0.0; 4]);
                set_state(w, id, st::CHARGE);
                swerve(w, id);
                if seq(w, id) != 5 {
                    let tk = w.ticks(3);
                    w.anim_blend(id, 5, 8, tk);
                }
                return;
            }
            if w.m(id).position[2] < 5.0 {
                set_death_bits(w, id, 0, -1);
                w.delete_moby(id);
                return;
            }
        }
        st::STUCK => stuck(w, id, &t),
        st::LEASHED => leashed(w, id),
        st::HOVER => hover(w, id, &t),
        st::FLY_TO => fly_to(w, id),
        st::TAKE_OFF => {
            let g = ground_at(w, c::pos(w, id), 0x120);
            let mut vz = c::pf(w, id, pv::VEL + 8);
            let h = w.m(id).position[2] - g;
            if h < 3.0 { turn::approach(c::DT * 8.0, c::DT2 * 10.8, &mut vz); } else { turn::approach(c::DT, c::DT2 * 10.8, &mut vz); }
            c::set_pf(w, id, pv::VEL + 8, vz);
            let mut v = c::pv4(w, id, pv::VEL);
            walker::move_collide(w, id, 0.6, 0.5, 0.0, &mut v, 0);
            c::set_pv4(w, id, pv::VEL, v);
            let z = w.m(id).position[2];
            w.mm(id).anim.speed = 6.0 - ((z - g) / 6.0) * 5.0;
            if g + 6.0 <= z {
                w.mm(id).position[2] = g + 6.0;
                blend(w, id, 0, 3);
                set_state(w, id, st::HOVER);
                zero_vel(w, id);
            }
            glow_on(w, id);
        }
        st::LAND => {
            let g = ground_at(w, c::pos(w, id), 0x120);
            let mut vz = c::pf(w, id, pv::VEL + 8);
            turn::approach(-(c::DT * 8.0), c::DT2 * 10.8, &mut vz);
            c::set_pf(w, id, pv::VEL + 8, vz);
            let mut v = c::pv4(w, id, pv::VEL);
            walker::move_collide(w, id, 0.6, 0.5, 0.0, &mut v, 0);
            c::set_pv4(w, id, pv::VEL, v);
            let z = w.m(id).position[2];
            if seq(w, id) != 2 && z - g <= c::DT * 8.0 * 45.0 {
                let tk = w.ticks(5);
                w.anim_blend(id, 2, 0, tk);
            }
            if g >= w.m(id).position[2] {
                w.mm(id).position[2] = g;
                swerve(w, id);
                set_state(w, id, st::CHARGE);
                zero_vel(w, id);
                blend(w, id, 5, 3);
            }
        }
        st::FLY_PATH => fly_path(w, id, &t),
        st::DYING => {
            w.mm(id).mode &= !mode::TARGETABLE;
            let r = knock::update(w, id, pv::K);
            if r & 0x140 == 0 {
                if w.m(id).position[2] < 0.0 { w.delete_moby(id); return; }
            } else {
                // The level's 3-slot table (level13 0x1613b0; level01's 0x161a80 is the critters'): one per level.
                let now = w.counter as i32;
                let dur = (w.ticks(1) as f32 * 60.0) as i32;
                let slot = fx::rate_slot(&mut w.svc.creatures.death_fx_slots, now, dur);
                if -1 < slot {
                    let p = c::pos(w, id);
                    fx::death_explosion(w, 0.5, 13.0, Some(id), p, 4);
                }
                w.delete_moby(id);
                return;
            }
        }
        _ => {}
    }
    if state(w, id) >= 0x80 { return; }
    flash::update(w, id, pv::FLASH);
    let p = c::pos(w, id);
    if (0..3).any(|k| p[k] < 2.0 || 1021.0 < p[k]) { w.delete_moby(id); }
}
