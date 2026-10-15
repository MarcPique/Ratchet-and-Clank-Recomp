//! **Hoven's carrier, class 1274** (level12 `0x3069d0`, census U407, one instance): the ship the turret 1267 shoots
//! down. It is assembled from 17 parts (the records of its pvar block: the guns 1278, the plates 1277 / 1280 and the
//! hull pieces), rises when Ratchet comes near, flies its path to a hover point once the turret game is on (or Ratchet
//! enters its cuboid) and fights with five guns ([`battle`], `0x3054b0`) that shoot the class-184 shots
//! ([`super::gun_shot`]) at the camera (the turret) or at Ratchet on foot; each gun takes 10 points of shells (0.2 a
//! shell through the part it sits on); a gun down costs the carrier 20 of its 100 and drops its part ([`fling`],
//! [`fall`]); the last one sets the turret's last stand (0x161f24) and the death: the camera glides between two
//! cuboids, every part falls, the scene 7 plays and the carrier deletes itself, which is the turret's win. Read from
//! the level12 decomp and disassembly (`0x3069d0`, `0x3054b0`, `0x306350`, `0x3064e8`, `0x306828`); the level data
//! from the overlay (gp 0x166c00; the gun-part table 0x1fb670). Native `f32`.
//!
//! **System or not.** Per-class code (clusters of one), on the shared pieces: the child records
//! (`triggers::record_children` / `place_children` = level12 `0x27b268` / `0x27b370`, level18's `0x265250` /
//! `0x265358`), the bob and the wobble (`units::bob` / `units::wobble`; `0x27b1a0` is level18's `0x265188`), the
//! springs (`hero::physics::{spring, turn_spring}`, `turn::{spring, spring_turn2_pvar, approach, approach_rot}`), the
//! look-at manipulator (`manip::look`), the gun shot ([`super::gun_shot`]), the script camera's targets and
//! `CameraScript2(3)` (`cinematic`), the scene player (`cinematic::start_scene`).
//!
//! **Pvar block** (0x630): 17 child records of 0x30 (+0x00 offset, +0x10 Euler, +0x20 moby index) at +0x000; per fall
//! slot k (0..17, 17 = the carrier): +0x338 + 16k the fall speed, +0x450 + 16k / +0x454 + 16k the spins (the second
//! also a spring velocity), +0x45c + 16k a spring rate, +0x570 + 4k the fall's gravity; +0x5c0 the path, +0x5c8 /
//! +0x5cc the bob, +0x5d0..+0x5e0 the five guns' record indices, +0x5e4 / +0x5e8 the wobble, +0x5ec the battle cuboid,
//! +0x5f0 the speed, +0x5f4 the turn velocity, +0x5f8 (s16) the path point, +0x5fa (s16) the death ticks, +0x5fc (s16)
//! the health (100), +0x604 the engine voice, +0x608 / +0x60c the death camera's cuboids, +0x610 / +0x614 its blend,
//! +0x618 the rest height, +0x61c / +0x620 the raise cuboid and timer, +0x624 the hide-from-camera cuboid, +0x628 the
//! near cuboid, +0x62c the height spring's velocity. The guns (1278, no update of their own) keep: +0x00 their look-at
//! record (+0x60 its pitch target), +0x80 health, +0x84 the hit timer, +0x8c the sweep side, +0x90 (s16) aimed, +0x92 /
//! +0x93 (u8) the voice timers, +0x94 the sweep's velocity, +0x98 / +0x9c / +0x9e (s16) the fire timers, +0x9a (s16) the
//! barrel.
//!
//! **Level words** ([`lw`]): 0x161f00 / 0x161f04 / 0x161f20 / 0x161f24 / 0x1619a0 (the turret's, `hoven_turret::lw`),
//! 0x1619b4 the battle on (the class 336 hides and ignores hits while it is set), 0x161f34 the engagement count (the
//! guns fire at Ratchet on foot past `ticks(1200)` near the carrier), 0x161f40.. the falling pose (position, Euler) and
//! 0x161f60.. the flown pose of the carrier while it falls, 0x1fb688[18] the falling parts (0 none, −1 gone, else the
//! moby; [17] = 1: the carrier itself).
//!
//! ## Coverage (`0x3069d0`)
//! | address | what | port |
//! |---|---|---|
//! | entry | state ≠ 0 and the carrier falls (0x1fb6cc): position / Euler = the flown pose 0x161f60 / 0x161f70 | [`update`] |
//! | state 0 | 0x1619b4 = 0; mission (+0xb0) done: the 17 parts and the carrier deleted | [`update`] |
//! | state 0 | 0x161f24 = 0; the records in its frame (`0x27b268`); position = the path's first point; +0x618 = z; yaw toward point 1; Ratchet below z 52: z + 3; the five guns' health = 10; 0x161f40 = −1; 0x1fb688[0..17] = 0; +0x604 = −1; +0x5fc = 100; → 1 | [`update`] |
//! | state 1 | the camera in cuboid +0x624: carrier and parts hidden (mode \|= 0x41), else shown | [`update`] |
//! | state 1 | Ratchet below z 52 or in cuboid +0x628: the height springs (`Spring(+0x618 + 3 / 10, 0.03, 0.3, 2.7·dt)`), 19 while the raise timer +0x620 runs (`ticks(90)` while Ratchet is in cuboid +0x61c); else `Approach(+0x618, 2·dt)` | [`update`] (`hero::physics::spring`, `turn::approach`) |
//! | states 1..3 | the bob (`0x27b120(0.7, 70°·dt)`) and the wobble (`0x27b1a0(2°, 35°·dt, 30°·dt)`), the parts placed (`0x27b370`) | [`update`] (`units::bob`, `units::wobble`, `triggers::place_children`) |
//! | state 1 | Ratchet in cuboid +0x5ec or the turret game on (0x161f00): 0x1619b4 = 1, the guns' hit records dropped, → 2 | [`update`] |
//! | state 2 | speed +0x5f0: `Approach(8·dt, 10·dt²)` until 11 points from the end, then `Approach(0, 8·dt²)` and at 0 → 3; position += the segment's direction · speed; yaw springs (`SpringTurn2`, 0.0015, 0.3, 10°·dt) toward the segment (3.0545 rad in the last 20 points); the next point when within 5 (xy) or behind (> 90°) | [`update`] |
//! | state 3 | 0x161f34 + 1; yaw springs to 3.0545 (`SpringAngle`, mode −1) | [`update`] (`hero::physics::turn_spring`) |
//! | states 2 / 3 | the battle `0x3054b0` | [`battle`] |
//! | state 4, +0xbc = 0 | the camera targets = cuboid +0x608 (centre, Euler); 0x1619a0 = 1; the letterbox on | [`update`] (`cinematic::camera_targets`, `letterbox`) |
//! | state 4, +0xbc = 1 | blend +0x610 → 1 (`0x274860(1, 0.666·dt², 0.666·dt², 0.4·dt)`): the camera between the cuboids' centres; its Euler x = cuboid A's (the game adds a zeroed stack word, not the blend), y / z blended; at 1: +0xbc = 2, the letterbox off, `CameraScript2(3)` | [`update`] (`turn::spring`, `cinematic::camera_script2`) |
//! | state 4, +0xbc = 2 | each part: alive → deleted (its record −1); a falling one (0x1fb688) still alive → deleted; 0x1619b4 = 0; `DialogStreamStart(7)`; the carrier deleted | [`update`] |
//! | state 4, first tick | every record's moby → 0x1fb688 (unless set), records −1; the carrier falls (0x1fb6cc = 1); 18 flings (`0x306350`) | [`update`], [`fling`] |
//! | state 4 | +0x5fa + 1; the fall `0x3064e8` | [`update`], [`fall`] |
//! | tail | the glow `0x306828` | [`glow`] |
//! | tail | states 1..3: the engine loop `PlayClassSound(0, 4, self)` once (+0x604); else released (`release_voice_slot` of an owned slot) | [`update`] |
//!
//! ## Coverage (`0x3054b0`, per gun g = 0..4 with a record)
//! | address | what | port |
//! |---|---|---|
//! | 0x3054b0 | pitch target (+0x60) = atan(xy distance, Δz) from the gun's point (z + 1.5) to the camera (z − 0.7); `0x27aef8(0.008, 0.3, gun, gun, 0)` | [`battle`] (`manip::look`) |
//! | 0x3054b0 | yaw to Ratchet in the carrier's frame (atan − π/2 − carrier yaw); not aimed: the record's yaw `0x274af0`s there at (30° + 10°·g)·dt, within 5° → aimed, side +1 (g odd: −1); aimed: `SpringAngle(yaw + side·32°, 0.022, 0.3, (15° + 3°·g)·dt)` on it, within 1° → side flips | [`battle`] (`turn::approach_rot`, `hero::physics::turn_spring`) |
//! | 0x3054b0 | engaged = `ticks(1200)` < 0x161f34, Ratchet within 80 (xy) and 4 (z) of the carrier; beyond 90 or 7: 0x161f34 = 0 | [`battle`] |
//! | 0x3054b0 | the turret game on or engaged, gun alive: +0x98 / +0x9c − 1; +0x98 = 0: within 4° of Ratchet's yaw → +0x9e = `ticks(20)`; +0x9e running: the voice (+0x92 running, +0x93 out → `PlayClassSound(0, 0, gun)`, +0x93 = `ticks(9)`); +0x9c out → a shot | [`battle`] |
//! | 0x3054b0 | the shot: muzzle = joint list (+0x9a + 1) (+0x9a toggles); the aim: Ratchet's body point (z + `randf_sym(0, 0.5)`) on foot, else the camera (z + `randf(−1.5, 0.3)`), z + `randf(±0.3·clamp(0x161f20 − 3, 0, ticks(5)))`; vel = polar(80·dt, gun yaw + π/2, pitch to it); `0x2d4150(gun, muzzle, vel, ticks(150))`; made: +0x92 = `ticks(20)`, nine puffs (`PartType04Spawn(muzzle, randf_sym³(0, 1.7·dt), 0x7f000000 \| g·0x100 \| r, 0xff, ticks(rand_range(15, 30)), 0x37, rand_range(180, 270), 1)`, r = `rand_range(170, 240)`, g = `rand_range(110, 200)`); +0x9c = `ticks(7)` + clamp(0x161f20, 0, `ticks(11)`) | [`fire`] (`gun_shot::spawn`, `fx::part04`) |
//! | 0x3054b0 | the turret alive (0x161f04 = 0), gun alive, hit timer +0x84 out: the hit (0x50000) on the gun (1.0), else on its part (0.2; the table 0x1fb670 = 1, 9, 17, 10, 11; 17 = the carrier; its hit record dropped); none → nothing | [`battle`] |
//! | 0x3054b0 | a hit with the game off: 0x161f34 = `ticks(4000)` (the guns engage Ratchet); on: +0x84 = `ticks(20)`, health −= it; ≤ 0: carrier health − 20 (≥ 0), `PlayClassSound(1, 0, gun)`; health left: the gun's part falls (17: the carrier) and is flung; gun 4: every plate 1277 falls too; 130 puffs (`PartType22Spawn(420000, gun + (r·cos a, r·sin a, randf(0.1, 0.5)), (s·cos a, s·sin a, randf(dt, 5.5·dt)), 0x500a1414, 0x0a0a0a, rand_range(ticks(90), ticks(180)))`, r = `randf(0.1, 0.8)`, s = `randf(0.7·dt, 1.5·dt)`); the gun's collision off, hidden | [`battle`] (`projectile::part22`) |
//! | 0x3054b0 | the gun's hit record dropped | [`battle`] |
//! | 0x3054b0 | 0 < health < 10: k = (9 − trunc(health))/2; k + 3 puffs (`PartType22Spawn(420000, gun + (r·cos a, r·sin a, 0.7), (s·cos a, s·sin a, randf(7·dt, 17·dt)·k·1.5/5), 0x460a00aa \| rand_range(70, 205)·0x100, 0x0a0a46, rand_range(ticks(25), ticks(55)))`, r = `randf(0.1, 0.4)`, s = `randf(0.5·dt, 3·dt)`); three dark puffs (`(503999, gun + …0.1..0.8, (…0.1·dt..0.4·dt, randf(2.4·dt, 5.5·dt)), 0x460a1414, 0x0a0a0a, rand_range(ticks(40), ticks(70)))`) | [`smoke`] |
//! | 0x3054b0 end | the fall `0x3064e8`; health < 1 and the turret alive: 0x161f24 = 1, → 4, +0xbc = 0 | [`battle`] |
//! | 0x306350 | the fall slot k (17: the carrier): gravity `randf(3.7·dt², 5·dt²)`, spins `randf_sym(3°·dt, 8°·dt)` / `randf_sym(3°·dt, 10°·dt)`; a plate 1277: rate `randf(0.001, 0.0045)`, gravity `randf(5.5·dt², 9·dt²)`; class 1280: gravity 13·dt² | [`fling`] |
//! | 0x3064e8 | each falling slot: the carrier: its falling pose kept apart from its flown pose (0x161f40 / 0x161f60); below z 20: a part deleted (−1), the carrier stops (the loop ends); else fall speed −= gravity, z += it; 1280: rot.x += 20°·dt, `SpringAngle(π/2, 0.002, 0.3)` on rot.y; 1277: rot.x += spin, `SpringAngle(−π/2, rate, 0.3)` on rot.y; the carrier: rot.x −= 50°·dt, rot.y += spin, y −= 4·dt; others: y −= 4·dt, rot.x / rot.y += the spins; `MobyBuildMatrix` | [`fall`] (the game also writes into the deleted part's slot this tick: n/a) |
//! | 0x306828 | every part but the guns (the carrier too): glow (+0x90) = 0x80 \| (0x46 + trunc(10·s)) \| min(255, 0xb4 + trunc(70·s)) ×2, s = sin(2π·(tick % `ticks(170)`)/`ticks(170)` − π); mode \|= 0x10 | [`glow`] (a record of −1 makes the game write before the moby array: skipped) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::fx::{part04, polar};
use crate::moby_update::creature::projectile::part22;
use crate::moby_update::creature::{self as c, add_rot, atan, diff_rots, dist2, sub_rot, DT, DT2};
use crate::moby_update::services::{fl, pf as to_pf, World};
use crate::moby_update::triggers::{place_children, record_children, CHILD_RECORD};
use crate::ps2v::Pf;
use super::hoven_turret::lw as tw;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x30_69d0;
pub const CLASSES: [i16; 1] = [1274];
/// The guns' joint lists (their muzzles) are read: the gun class 1278.
pub const JOINTS: [i16; 1] = [GUN];

const GUN: i16 = 0x4fe;
const PLATE: i16 = 0x4fd;
const BLOCK: i16 = 0x500;
const CARRIER: i16 = 0x4fa;

/// The parts (the records) and the fall slots (17: the carrier).
const PARTS: usize = 17;
const SELF_SLOT: usize = 17;
/// Level12 0x1fb670: each gun's part.
const GUN_PART: [usize; 5] = [1, 9, 17, 10, 11];

/// The level words of the carrier (module doc).
pub mod lw {
    pub const BATTLE: u32 = 0x16_19b4;
    pub const ENGAGE: u32 = 0x16_1f34;
    pub const FALL_POS: u32 = 0x16_1f40;
    pub const FALL_ROT: u32 = 0x16_1f50;
    pub const FLOWN_POS: u32 = 0x16_1f60;
    pub const FLOWN_ROT: u32 = 0x16_1f70;
    /// 0x1fb688[18].
    pub const FALLING: u32 = 0x1f_b688;
    /// The letterbox 0x15f404 is `Services::creatures.cutscene` (`cinematic::letterbox`).
    pub const _LETTERBOX: u32 = 0x15_f404;
}

pub mod pvo {
    pub const FALL_V: usize = 0x338;
    pub const SPIN_A: usize = 0x450;
    pub const SPIN_B: usize = 0x454;
    pub const RATE: usize = 0x45c;
    pub const GRAVITY: usize = 0x570;
    pub const PATH: usize = 0x5c0;
    pub const BOB_PREV: usize = 0x5c8;
    pub const BOB_PHASE: usize = 0x5cc;
    pub const GUNS: usize = 0x5d0;
    pub const WOB_A: usize = 0x5e4;
    pub const WOB_B: usize = 0x5e8;
    pub const BATTLE_CUBOID: usize = 0x5ec;
    pub const SPEED: usize = 0x5f0;
    pub const TURN_V: usize = 0x5f4;
    pub const POINT: usize = 0x5f8;
    pub const DEATH: usize = 0x5fa;
    pub const HEALTH: usize = 0x5fc;
    pub const VOICE: usize = 0x604;
    pub const CAM_A: usize = 0x608;
    pub const CAM_B: usize = 0x60c;
    pub const BLEND: usize = 0x610;
    pub const BLEND_V: usize = 0x614;
    pub const REST_Z: usize = 0x618;
    pub const RAISE_CUBOID: usize = 0x61c;
    pub const RAISE_T: usize = 0x620;
    pub const HIDE_CUBOID: usize = 0x624;
    pub const NEAR_CUBOID: usize = 0x628;
    pub const Z_V: usize = 0x62c;
    pub const LEN: usize = 0x630;
}

/// The guns' pvars.
pub mod gpo {
    pub const PITCH_T: usize = 0x60;
    pub const HEALTH: usize = 0x80;
    pub const HIT_T: usize = 0x84;
    pub const SIDE: usize = 0x8c;
    pub const AIMED: usize = 0x90;
    pub const VOICE_A: usize = 0x92;
    pub const VOICE_B: usize = 0x93;
    pub const SWEEP_V: usize = 0x94;
    pub const DELAY: usize = 0x98;
    pub const BARREL: usize = 0x9a;
    pub const SHOT_T: usize = 0x9c;
    pub const HOLD_T: usize = 0x9e;
    pub const LEN: usize = 0xa0;
}

/// The level data: the wobble (gp−0x4cd0 2°, −0x4cd8 35°, −0x4cd4 30°).
mod k {
    pub const WOBBLE: f32 = f32::from_bits(0x3d0e_fa35);
    pub const WOBBLE_A: f32 = 35.0;
    pub const WOBBLE_B: f32 = 30.0;
    /// 0x15ed64: the springs' rate (1.0).
    pub const RATE: f32 = 1.0;
    /// The end heading.
    pub const END_YAW: f32 = f32::from_bits(0x4043_7a14);
    pub const LOW: f32 = 52.0;
}

const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const DEG: f32 = 0.017_453_292;

fn word(w: &World, a: u32) -> u32 { w.svc.units.word(a) }
fn set_word(w: &mut World, a: u32, v: u32) { w.svc.units.set_word(a, v) }
fn falling(w: &World, k: usize) -> i32 { word(w, lw::FALLING + 4 * k as u32) as i32 }
fn set_falling(w: &mut World, k: usize, v: i32) { set_word(w, lw::FALLING + 4 * k as u32, v as u32) }
fn get_v4(w: &World, a: u32) -> [f32; 4] { std::array::from_fn(|i| f32::from_bits(word(w, a + 4 * i as u32))) }
fn set_v4(w: &mut World, a: u32, v: [f32; 4]) { for (i, x) in v.iter().enumerate() { set_word(w, a + 4 * i as u32, x.to_bits()); } }

/// The record k's moby index (+0x20 + k·0x30).
fn rec_index(w: &World, id: MobyId, k: usize) -> i32 { c::pi32(w, id, k * CHILD_RECORD + 0x20) }
fn set_rec_index(w: &mut World, id: MobyId, k: usize, v: i32) { c::set_pi32(w, id, k * CHILD_RECORD + 0x20, v) }
fn rec_moby(w: &World, id: MobyId, k: usize) -> Option<MobyId> { crate::moby_update::story::link(w, rec_index(w, id, k)) }
fn in_cuboid(w: &World, p: [f32; 3], c0: i32) -> bool { c0 >= 0 && w.in_cuboid(p, c0) }

/// Level12 `0x3069d0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pvo::LEN);
    if w.m(id).state != 0 && falling(w, SELF_SLOT) != 0 {
        let (pos, rot) = (get_v4(w, lw::FLOWN_POS), get_v4(w, lw::FLOWN_ROT));
        let m = w.mm(id);
        m.position = pos;
        m.rotation = rot;
    }
    let st = w.m(id).state;
    match st {
        0 if !init(w, id) => return,
        1 => approach(w, id),
        2 => {
            bob_wobble(w, id, DT);
            if fly(w, id) {
                place_children(w, id, 0, PARTS);
                battle(w, id);
            }
        }
        3 => {
            let n = word(w, lw::ENGAGE).wrapping_add(1);
            set_word(w, lw::ENGAGE, n);
            let mut a = to_pf(w.m(id).rotation[2]);
            let mut v = to_pf(c::pf(w, id, pvo::TURN_V));
            crate::hero::physics::turn_spring(to_pf(k::END_YAW), to_pf(k::RATE * 0.0015), to_pf(k::RATE * 0.3), to_pf(DT * 0.174_532_92), &mut a, &mut v, -1);
            w.mm(id).rotation[2] = fl(a);
            c::set_pf(w, id, pvo::TURN_V, fl(v));
            bob_wobble(w, id, DT);
            place_children(w, id, 0, PARTS);
            battle(w, id);
        }
        4 if !death(w, id) => return,
        _ => {}
    }
    glow(w, id);
    let st = w.m(id).state;
    if (1..=3).contains(&st) {
        if c::pi32(w, id, pvo::VOICE) == -1 {
            let s = w.play_sound(0, 4, id);
            c::set_pi32(w, id, pvo::VOICE, s);
        }
    } else {
        let s = c::pi32(w, id, pvo::VOICE);
        if s != -1 && w.sound_owner(s) == Some(id) { w.release_sound(s, id); }
        c::set_pi32(w, id, pvo::VOICE, -1);
    }
}

/// The bob and the wobble (states 1..3).
fn bob_wobble(w: &mut World, id: MobyId, rate: f32) {
    super::bob(w, id, f32::from_bits(0x3f33_3333), rate * f32::from_bits(0x3f9c_61aa), pvo::BOB_PHASE, pvo::BOB_PREV);
    super::wobble(w, id, k::WOBBLE, k::WOBBLE_A * DEG * DT, k::WOBBLE_B * DEG * DT, pvo::WOB_A, pvo::WOB_B);
}

/// The path's points (`0x1b0930[+0x5c0]`).
fn path(w: &World, id: MobyId) -> Vec<[f32; 4]> {
    let i = c::pi32(w, id, pvo::PATH);
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map(|v| v.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}

/// State 0. False when the carrier was deleted.
fn init(w: &mut World, id: MobyId) -> bool {
    set_word(w, lw::BATTLE, 0);
    if super::hints::mission_done(w, w.m(id).mission as i32) {
        for k in 0..PARTS {
            if rec_index(w, id, k) != -1 {
                if let Some(m) = rec_moby(w, id, k) { w.delete_moby(m); }
            }
        }
        w.delete_moby(id);
        return false;
    }
    set_word(w, tw::LAST_STAND, 0);
    record_children(w, id, 0, PARTS);
    let pts = path(w, id);
    let q = |i: usize| pts.get(i).copied().unwrap_or([0.0; 4]);
    w.mm(id).position = q(0);
    let z = w.m(id).position[2];
    c::set_pf(w, id, pvo::REST_Z, z);
    w.mm(id).rotation[2] = atan(q(1)[0] - q(0)[0], q(1)[1] - q(0)[1]);
    if w.hero_point()[2] < k::LOW { w.mm(id).position[2] += 3.0; }
    for g in 0..5 {
        let r = c::pi32(w, id, pvo::GUNS + 4 * g);
        if let Some(gm) = usize::try_from(r).ok().filter(|&r| r < PARTS).and_then(|r| rec_moby(w, id, r)) {
            crate::moby_update::story::pvars(w, gm, gpo::LEN);
            c::set_pf(w, gm, gpo::HEALTH, 10.0);
        }
    }
    set_word(w, lw::FALL_POS, (-1.0f32).to_bits());
    for k in 0..=SELF_SLOT { set_falling(w, k, 0); }
    c::set_pi32(w, id, pvo::VOICE, -1);
    c::set_pi16(w, id, pvo::HEALTH, 100);
    w.mm(id).state = 1;
    true
}

/// State 1: the carrier waits by its first point.
fn approach(w: &mut World, id: MobyId) {
    let cam = w.camera_point();
    let hide = in_cuboid(w, cam, c::pi32(w, id, pvo::HIDE_CUBOID));
    let set = |w: &mut World, m: MobyId| if hide { crate::moby_update::story::hide(w, m) } else { crate::moby_update::story::show(w, m) };
    set(w, id);
    for k in 0..PARTS {
        if rec_index(w, id, k) != -1 {
            if let Some(m) = rec_moby(w, id, k) { set(w, m); }
        }
    }
    let hero = w.hero_point();
    let near = in_cuboid(w, hero, c::pi32(w, id, pvo::NEAR_CUBOID));
    if hero[2] <= k::LOW || near {
        let mut f = if near { 10.0 } else { 3.0 };
        if in_cuboid(w, hero, c::pi32(w, id, pvo::RAISE_CUBOID)) {
            let t = w.ticks(90);
            c::set_pi32(w, id, pvo::RAISE_T, t);
        }
        if c::dec_timer_pvar_i32(w, id, pvo::RAISE_T) == 0 { f = 19.0; }
        let t = c::pf(w, id, pvo::REST_Z) + f;
        let mut z = to_pf(w.m(id).position[2]);
        let mut v = to_pf(c::pf(w, id, pvo::Z_V));
        crate::hero::physics::spring(to_pf(t), to_pf(k::RATE * 0.03), to_pf(k::RATE * 0.3), to_pf(DT * 2.7), &mut z, &mut v);
        w.mm(id).position[2] = fl(z);
        c::set_pf(w, id, pvo::Z_V, fl(v));
    } else {
        let mut z = w.m(id).position[2];
        c::turn::approach(c::pf(w, id, pvo::REST_Z), DT + DT, &mut z);
        w.mm(id).position[2] = z;
    }
    // Both branches pass dt to the bob (`fVar14`).
    bob_wobble(w, id, DT);
    place_children(w, id, 0, PARTS);
    if in_cuboid(w, hero, c::pi32(w, id, pvo::BATTLE_CUBOID)) || word(w, tw::ACTIVE) != 0 {
        set_word(w, lw::BATTLE, 1);
        for g in 0..5 {
            let r = c::pi32(w, id, pvo::GUNS + 4 * g);
            if let Some(gm) = usize::try_from(r).ok().filter(|&r| r < PARTS).and_then(|r| rec_moby(w, id, r)) { w.mm(gm).hit_slot = 0xff; }
        }
        w.mm(id).state = 2;
    }
}

/// State 2's flight. False when it moved on to state 3 (the tail is skipped).
fn fly(w: &mut World, id: MobyId) -> bool {
    let pts = path(w, id);
    let n = pts.len() as i32;
    if n == 0 { return true; }
    let i = c::pi16(w, id, pvo::POINT) as i32;
    let mut sp = c::pf(w, id, pvo::SPEED);
    if i < n - 11 {
        c::turn::approach(DT * 8.0, DT2 * 10.0, &mut sp);
        c::set_pf(w, id, pvo::SPEED, sp);
    } else {
        c::turn::approach(0.0, DT2 * 8.0, &mut sp);
        c::set_pf(w, id, pvo::SPEED, sp);
        if sp == 0.0 {
            w.mm(id).state = 3;
            return false;
        }
    }
    let ii = i.rem_euclid(n) as usize;
    let next = ((i + 1) % n).rem_euclid(n) as usize;
    let d = c::sub(pts[next], pts[ii]);
    let d = c::set_len3([d[0], d[1], d[2], 0.0], sp);
    let pos = w.m(id).position;
    w.mm(id).position = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2], pos[3] + d[3]];
    let mut yaw = atan(pts[next][0] - pts[ii][0], pts[next][1] - pts[ii][1]);
    if n - 20 < i { yaw = k::END_YAW; }
    c::turn::spring_turn2_pvar(w, id, yaw, k::RATE * 0.0015, k::RATE * 0.3, DT * 0.174_532_92, pvo::TURN_V);
    let pos = w.m(id).position;
    let mut advance = true;
    if 5.0 <= dist2(pos, pts[ii]) {
        let a = atan(pts[ii][0] - pos[0], pts[ii][1] - pos[1]);
        if diff_rots(w.m(id).rotation[2], a) <= HALF_PI { advance = false; }
    }
    if advance { c::set_pi16(w, id, pvo::POINT, ((i + 1) % n) as i16); }
    true
}

/// State 4. False when the carrier deleted itself.
fn death(w: &mut World, id: MobyId) -> bool {
    let (ca, cb) = (c::pi32(w, id, pvo::CAM_A), c::pi32(w, id, pvo::CAM_B));
    if ca != -1 && cb != -1 {
        match w.m(id).cmd {
            0 => {
                w.mm(id).cmd = 1;
                if let Some((pos, e)) = crate::moby_update::story::cuboid(w, ca) { crate::cinematic::camera_targets(w, Some(pos), Some(e)); }
                set_word(w, tw::CARRIER_CAM, 1);
                crate::cinematic::letterbox(w, true);
            }
            1 => {
                if c::pf(w, id, pvo::BLEND) < 1.0 {
                    let (mut t, mut v) = (c::pf(w, id, pvo::BLEND), c::pf(w, id, pvo::BLEND_V));
                    c::turn::spring(1.0, DT2 * 0.666, DT2 * 0.666, DT * 0.4, &mut t, &mut v);
                    c::set_pf(w, id, pvo::BLEND, t);
                    c::set_pf(w, id, pvo::BLEND_V, v);
                    if let (Some((pa, ea)), Some((pb, eb))) = (crate::moby_update::story::cuboid(w, ca), crate::moby_update::story::cuboid(w, cb)) {
                        let pos: [f32; 3] = std::array::from_fn(|i| pa[i] + (pb[i] - pa[i]) * t);
                        // The x angle adds the zeroed stack word (the game's own slip), y and z the blended difference.
                        let e = [add_rot(ea[0], 0.0), add_rot(ea[1], sub_rot(eb[1], ea[1]) * t), add_rot(ea[2], sub_rot(eb[2], ea[2]) * t)];
                        crate::cinematic::camera_targets(w, Some(pos), Some(e));
                    }
                } else {
                    w.mm(id).cmd = 2;
                    crate::cinematic::letterbox(w, false);
                    crate::cinematic::camera_script2(w, 3);
                }
            }
            2 => {
                for k in 0..PARTS {
                    if rec_index(w, id, k) == -1 {
                        let f = falling(w, k);
                        if 0 < f {
                            if let Some(m) = crate::moby_update::story::link(w, f) { if w.m(m).state < 0x80 { w.delete_moby(m); } }
                        }
                    } else {
                        if let Some(m) = rec_moby(w, id, k) { if w.m(m).state < 0x80 { w.delete_moby(m); } }
                        set_rec_index(w, id, k, -1);
                    }
                }
                set_word(w, lw::BATTLE, 0);
                crate::cinematic::start_scene(w, 7, false);
                w.delete_moby(id);
                return false;
            }
            _ => {}
        }
    }
    if c::pi16(w, id, pvo::DEATH) == 0 {
        for k in 0..PARTS {
            if falling(w, k) == 0 {
                let r = rec_index(w, id, k);
                set_falling(w, k, r);
            }
            set_rec_index(w, id, k, -1);
        }
        set_falling(w, SELF_SLOT, 1);
        for k in 0..=SELF_SLOT { fling(w, id, k); }
    }
    let n = c::pi16(w, id, pvo::DEATH).wrapping_add(1);
    c::set_pi16(w, id, pvo::DEATH, n);
    fall(w, id);
    true
}

/// The falling slot `k`'s moby (17: the carrier).
fn slot_moby(w: &World, id: MobyId, k: usize) -> Option<MobyId> {
    if k == SELF_SLOT { return Some(id); }
    let f = falling(w, k);
    if f <= 0 { return None; }
    crate::moby_update::story::link(w, f)
}

/// `0x306350(carrier, k)`: a part (or the carrier) flung (module doc).
pub fn fling(w: &mut World, id: MobyId, k: usize) {
    let target = slot_moby(w, id, k);
    let g = w.rng.randf(DT2 * 3.7, DT2 * 5.0);
    let go = pvo::GRAVITY + 4 * k;
    c::set_pf(w, id, go, g);
    let a = w.rng.randf_sym(DT * f32::from_bits(0x3d56_7750), DT * f32::from_bits(0x3e0e_fa35));
    c::set_pf(w, id, pvo::SPIN_A + 16 * k, a);
    let b = w.rng.randf_sym(DT * f32::from_bits(0x3d56_7750), DT * f32::from_bits(0x3e32_b8c2));
    c::set_pf(w, id, pvo::SPIN_B + 16 * k, b);
    match target.map(|t| w.m(t).o_class) {
        Some(PLATE) => {
            let r = w.rng.randf(f32::from_bits(0x3a83_126f), f32::from_bits(0x3b93_74bc));
            c::set_pf(w, id, pvo::RATE + 16 * k, r);
            let g = w.rng.randf(DT2 * 5.5, DT2 * 9.0);
            c::set_pf(w, id, go, g);
        }
        Some(BLOCK) => c::set_pf(w, id, go, DT2 * 13.0),
        _ => {}
    }
}

/// `0x3064e8`: the falling parts (module doc).
pub fn fall(w: &mut World, id: MobyId) {
    for k in 0..=SELF_SLOT {
        let f = falling(w, k);
        if f == 0 { continue; }
        if k != SELF_SLOT && f == -1 { continue; }
        let Some(m) = slot_moby(w, id, k) else { continue };
        if k == SELF_SLOT {
            if f32::from_bits(word(w, lw::FALL_POS)) == -1.0 {
                let (pos, rot) = (w.m(m).position, w.m(m).rotation);
                set_v4(w, lw::FALL_POS, pos);
                set_v4(w, lw::FALL_ROT, rot);
            }
            let (pos, rot) = (w.m(m).position, w.m(m).rotation);
            set_v4(w, lw::FLOWN_POS, pos);
            set_v4(w, lw::FLOWN_ROT, rot);
            let (fp, fr) = (get_v4(w, lw::FALL_POS), get_v4(w, lw::FALL_ROT));
            let mo = w.mm(m);
            mo.position = fp;
            mo.rotation = fr;
        }
        if w.m(m).position[2] < 20.0 {
            if k == SELF_SLOT { return; }
            w.delete_moby(m);
            set_falling(w, k, -1);
            continue;
        }
        let vz = c::pf(w, id, pvo::FALL_V + 16 * k) - c::pf(w, id, pvo::GRAVITY + 4 * k);
        c::set_pf(w, id, pvo::FALL_V + 16 * k, vz);
        w.mm(m).position[2] += vz;
        let (sa, sb) = (pvo::SPIN_A + 16 * k, pvo::SPIN_B + 16 * k);
        match w.m(m).o_class {
            BLOCK => {
                let rx = w.m(m).rotation[0];
                w.mm(m).rotation[0] = add_rot(rx, DT * f32::from_bits(0x3eb2_b8c2));
                spring_angle(w, id, m, HALF_PI, f32::from_bits(0x3b03_126f), sb);
            }
            PLATE => {
                let rx = w.m(m).rotation[0];
                w.mm(m).rotation[0] = add_rot(rx, c::pf(w, id, sa));
                let rate = c::pf(w, id, pvo::RATE + 16 * k);
                spring_angle(w, id, m, -HALF_PI, rate, sb);
            }
            CARRIER => {
                let rx = w.m(m).rotation[0];
                w.mm(m).rotation[0] = add_rot(rx, DT * f32::from_bits(0xbf5f_66f3));
                let ry = w.m(m).rotation[1];
                w.mm(m).rotation[1] = add_rot(ry, c::pf(w, id, sb));
                w.mm(m).position[1] += DT * -4.0;
            }
            _ => {
                w.mm(m).position[1] += DT * -4.0;
                let rx = w.m(m).rotation[0];
                w.mm(m).rotation[0] = add_rot(rx, c::pf(w, id, sa));
                let ry = w.m(m).rotation[1];
                w.mm(m).rotation[1] = add_rot(ry, c::pf(w, id, sb));
            }
        }
        if k == SELF_SLOT {
            let (pos, rot) = (w.m(m).position, w.m(m).rotation);
            set_v4(w, lw::FALL_POS, pos);
            set_v4(w, lw::FALL_ROT, rot);
        }
        w.build_matrix(m);
    }
}

/// `SpringAngle(target, k, 0.3, 0, &rot.y, &vel, 0)` on part `m` with the velocity in the carrier's pvar `vel`.
fn spring_angle(w: &mut World, id: MobyId, m: MobyId, target: f32, kk: f32, vel: usize) {
    let mut a = to_pf(w.m(m).rotation[1]);
    let mut v = to_pf(c::pf(w, id, vel));
    crate::hero::physics::turn_spring(to_pf(target), to_pf(kk), to_pf(f32::from_bits(0x3e99_999a)), Pf::ZERO, &mut a, &mut v, 0);
    w.mm(m).rotation[1] = fl(a);
    c::set_pf(w, id, vel, fl(v));
}

/// `0x306828`: the parts' glow (module doc).
pub fn glow(w: &mut World, id: MobyId) {
    for k in 0..=PARTS {
        let m = if k == PARTS { Some(id) } else { rec_moby(w, id, k) };
        let Some(m) = m else { continue };
        if w.m(m).o_class == GUN { continue; }
        let n = w.ticks(0xaa);
        if n == 0 { continue; }
        let f = ((w.counter as i64).rem_euclid(n as i64)) as f32 / n as f32;
        let s = ((f + f) * std::f32::consts::PI + -std::f32::consts::PI).sin();
        let a = (s * 70.0) as i32;
        let b = (s * 10.0) as i32;
        let r = (a + 0xb4).min(0xff) as u32;
        let g = (a + 0xb4).min(0xff) as u32;
        let mo = w.mm(m);
        mo.glow = ((b + 0x46) as u32) << 16 | 0x8000_0000 | g << 8 | r;
        mo.mode |= mode::GLOW;
    }
}

/// The five guns' moby and their record (`0x3054b0`'s loop).
fn gun(w: &World, id: MobyId, g: usize) -> Option<(usize, MobyId)> {
    let r = c::pi32(w, id, pvo::GUNS + 4 * g);
    if r == -1 { return None; }
    let r = usize::try_from(r).ok().filter(|&r| r < PARTS)?;
    Some((r, rec_moby(w, id, r)?))
}

/// `0x3054b0`: the guns (module doc).
pub fn battle(w: &mut World, id: MobyId) {
    for g in 0..5 {
        let Some((r, gm)) = gun(w, id, g) else { continue };
        crate::moby_update::story::pvars(w, gm, gpo::LEN);
        let cam = w.camera_point();
        let cam4 = [cam[0], cam[1], cam[2] - 0.7, 0.0];
        let mut t = w.m(gm).position;
        t[2] += 1.5;
        let pitch = atan(dist2(t, cam4), cam4[2] - t[2]);
        c::set_pf(w, gm, gpo::PITCH_T, pitch);
        crate::moby_update::manip::look(w, gm, gm, 0, 0, k::RATE * 0.008, k::RATE * 0.3);
        let hero = w.hero_point();
        let gp = w.m(gm).position;
        let yh = sub_rot(add_rot(atan(hero[0] - gp[0], hero[1] - gp[1]), -HALF_PI), w.m(id).rotation[2]);
        let yo = r * CHILD_RECORD + 0x18;
        if c::pi16(w, gm, gpo::AIMED) == 0 {
            let mut y = c::pf(w, id, yo);
            c::turn::approach_rot(yh, DT * f32::from_bits(0x3f06_0a92) + DT * f32::from_bits(0x3e32_b8c2) * g as f32, &mut y);
            c::set_pf(w, id, yo, y);
            let res = crate::moby_update::creature::sub_rot(yh, y);
            if res < f32::from_bits(0x3db2_b8c2) {
                c::set_pi16(w, gm, gpo::AIMED, 1);
                c::set_pf(w, gm, gpo::SIDE, if g & 1 != 0 { -1.0 } else { 1.0 });
            }
        } else {
            let tgt = add_rot(yh, c::pf(w, gm, gpo::SIDE) * f32::from_bits(0x3f0e_fa35));
            let mut a = to_pf(c::pf(w, id, yo));
            let mut v = to_pf(c::pf(w, gm, gpo::SWEEP_V));
            let max = DT * f32::from_bits(0x3e86_0a92) + g as f32 * DT * f32::from_bits(0x3d56_7750);
            let res = crate::hero::physics::turn_spring(to_pf(tgt), to_pf(k::RATE * 0.022), to_pf(k::RATE * 0.3), to_pf(max), &mut a, &mut v, 0);
            c::set_pf(w, id, yo, fl(a));
            c::set_pf(w, gm, gpo::SWEEP_V, fl(v));
            if fl(res).abs() < DEG {
                let s = c::pf(w, gm, gpo::SIDE);
                c::set_pf(w, gm, gpo::SIDE, -s);
            }
        }
        let cp = w.m(id).position;
        let d = dist2(cp, [hero[0], hero[1], hero[2], 0.0]);
        let dz = (hero[2] - cp[2]).abs();
        let t1200 = w.ticks(0x4b0);
        let engaged = t1200 < word(w, lw::ENGAGE) as i32 && d < 80.0 && dz < 4.0;
        if 90.0 < d || 7.0 < dz { set_word(w, lw::ENGAGE, 0); }
        let active = word(w, tw::ACTIVE) != 0;
        if (active || engaged) && 0.0 < c::pf(w, gm, gpo::HEALTH) {
            c::dec_timer_pvar_s16(w, gm, gpo::DELAY);
            c::dec_timer_pvar_s16(w, gm, gpo::SHOT_T);
            if c::pi16(w, gm, gpo::DELAY) == 0 {
                if diff_rots(c::pf(w, id, yo), yh) < f32::from_bits(0x3d8e_fa35) {
                    let t = w.ticks(20) as i16;
                    c::set_pi16(w, gm, gpo::HOLD_T, t);
                }
                if c::dec_timer_pvar_s16(w, gm, gpo::HOLD_T) == 0 {
                    if dec_u8(w, gm, gpo::VOICE_A) == 0 && dec_u8(w, gm, gpo::VOICE_B) != 0 {
                        w.play_sound(0, 0, gm);
                        let t = w.ticks(9) as u8;
                        c::set_pu8(w, gm, gpo::VOICE_B, t);
                    }
                    if c::dec_timer_pvar_s16(w, gm, gpo::SHOT_T) != 0 { fire(w, gm); }
                }
            }
        }
        if word(w, tw::DEAD) == 0 && 0.0 < c::pf(w, gm, gpo::HEALTH) && c::dec_timer_pvar_i32(w, gm, gpo::HIT_T) != 0 {
            take_hit(w, id, g, gm);
        }
        w.mm(gm).hit_slot = 0xff;
        let h = c::pf(w, gm, gpo::HEALTH);
        if 0.0 < h && h < 10.0 { smoke(w, gm, h); }
    }
    fall(w, id);
    if c::pi16(w, id, pvo::HEALTH) < 1 && word(w, tw::DEAD) == 0 {
        set_word(w, tw::LAST_STAND, 1);
        w.mm(id).state = 4;
        w.mm(id).cmd = 0;
    }
}

/// `FUN_00221e98` (L01 0x220ed8) on a gun's u8 timer.
fn dec_u8(w: &mut World, id: MobyId, o: usize) -> i32 {
    let mut t = c::pu8(w, id, o);
    let r = crate::moby_update::services::fast_dec_timer_u8(&mut t);
    c::set_pu8(w, id, o, t);
    r
}

/// A gun's shot (module doc).
fn fire(w: &mut World, gm: MobyId) {
    let b = c::pi16(w, gm, gpo::BARREL);
    let muzzle = w.joint_point(gm, (b + 1) as usize);
    c::set_pi16(w, gm, gpo::BARREL, (b == 0) as i16);
    let yaw = add_rot(w.m(gm).rotation[2], HALF_PI);
    let mut tgt;
    if word(w, tw::ACTIVE) == 0 {
        let bp = w.hero_body_point();
        tgt = [bp[0], bp[1], bp[2], fl(w.hero.body_point[3])];
        tgt[2] += w.rng.randf_sym(0.0, 0.5);
    } else {
        let cp = w.camera_point();
        tgt = [cp[0], cp[1], cp[2], fl(w.camera[3])];
        tgt[2] += w.rng.randf(-1.5, f32::from_bits(0x3e99_999a));
    }
    let mut n = word(w, tw::MOUNTS) as i32 - 3;
    let t5 = w.ticks(5);
    if t5 < n { n = t5; }
    if n < 0 { n = 0; }
    tgt[2] += w.rng.randf(n as f32 * -0.3, n as f32 * 0.3);
    let pitch = atan(dist2(muzzle, tgt), tgt[2] - muzzle[2]);
    let vel = polar(DT * 80.0, yaw, pitch);
    let life = w.ticks(0x96);
    if super::gun_shot::spawn(w, gm, muzzle, vel, life).is_some() {
        let t = w.ticks(20) as u8;
        c::set_pu8(w, gm, gpo::VOICE_A, t);
        for _ in 0..9 {
            let v = [w.rng.randf_sym(0.0, DT * 1.7), w.rng.randf_sym(0.0, DT * 1.7), w.rng.randf_sym(0.0, DT * 1.7), 0.0];
            let r = w.rng.rand_range(0xaa, 0xf0) as u32;
            let g = w.rng.rand_range(0x6e, 200) as u32;
            let tl = w.rng.rand_range(0xf, 0x1e);
            let life = w.ticks(tl);
            let x = w.rng.rand_range(0xb4, 0x10e);
            part04(w, &crate::particles::type04::Spawn { pos: muzzle, vel: v, c1: g << 8 | r | 0x7f00_0000, c2: 0xff, life, base: 0x37, growth: x as i16, additive: true });
        }
    }
    let t7 = w.ticks(7);
    let mut m = word(w, tw::MOUNTS) as i32;
    let t11 = w.ticks(0xb);
    if t11 < m { m = t11; }
    if m < 0 { m = 0; }
    c::set_pi16(w, gm, gpo::SHOT_T, (t7 + m) as i16);
}

/// A gun's hit (module doc).
fn take_hit(w: &mut World, id: MobyId, g: usize, gm: MobyId) {
    let mut f = 1.0;
    let mut hit = w.get_hit(gm, 0x5_0000, false);
    if hit.is_none() {
        let t = GUN_PART[g];
        let parent = if t == SELF_SLOT { Some(id) } else { rec_moby(w, id, t).filter(|_| rec_index(w, id, t) != -1) };
        if let Some(pm) = parent {
            f = 0.2;
            hit = w.get_hit(pm, 0x5_0000, false);
            w.mm(pm).hit_slot = 0xff;
        }
        if hit.is_none() { return; }
    }
    if word(w, tw::ACTIVE) == 0 {
        let t = w.ticks(4000);
        set_word(w, lw::ENGAGE, t as u32);
        return;
    }
    let t = w.ticks(20);
    c::set_pi32(w, gm, gpo::HIT_T, t);
    let h = c::pf(w, gm, gpo::HEALTH) - f;
    c::set_pf(w, gm, gpo::HEALTH, h);
    if 0.0 < h { return; }
    let hp = (c::pi16(w, id, pvo::HEALTH).wrapping_sub(0x14)).max(0);
    c::set_pi16(w, id, pvo::HEALTH, hp);
    w.play_sound(1, 0, gm);
    if 0 < hp {
        let t = GUN_PART[g];
        if t == SELF_SLOT {
            set_falling(w, SELF_SLOT, 1);
        } else {
            let r = rec_index(w, id, t);
            set_falling(w, t, r);
            set_rec_index(w, id, t, -1);
        }
        fling(w, id, t);
        if g == 4 {
            for k in 0..PARTS {
                let r = rec_index(w, id, k);
                if r == -1 { continue; }
                if crate::moby_update::story::link(w, r).is_some_and(|m| w.m(m).o_class == PLATE) {
                    set_falling(w, k, r);
                    set_rec_index(w, id, k, -1);
                    fling(w, id, k);
                }
            }
        }
    }
    for _ in 0..0x82 {
        let base = w.m(gm).position;
        let z = w.rng.randf(f32::from_bits(0x3dcc_cccd), 0.5);
        let a = w.rng.rand_angle();
        let rr = w.rng.randf(f32::from_bits(0x3dcc_cccd), f32::from_bits(0x3f4c_cccd));
        let pos = [base[0] + a.cos() * rr, base[1] + a.sin() * rr, base[2] + z, base[3]];
        let (t90, t180) = (w.ticks(0x5a), w.ticks(0xb4));
        let life = w.rng.rand_range(t90, t180);
        let s = w.rng.randf(DT * 0.7, DT * 1.5);
        let vz = w.rng.randf(DT, DT * 5.5);
        part22(w, &crate::particles::type22::Spawn { size: 420_000.0, pos, vel: [a.cos() * s, a.sin() * s, vz, 0.0], c1: 0x500a_1414, c2: 0x000a_0a0a, life });
    }
    let m = w.mm(gm);
    m.has_collision = false;
    m.mode |= mode::HIDDEN | mode::NO_ANIM;
}

/// The damaged gun's smoke (module doc).
fn smoke(w: &mut World, gm: MobyId, h: f32) {
    let k = (9 - h as i32) / 2;
    let n = k + 3;
    for _ in 0..n.max(0) {
        let base = w.m(gm).position;
        let a = w.rng.rand_angle();
        let rr = w.rng.randf(f32::from_bits(0x3dcc_cccd), f32::from_bits(0x3ecc_cccd));
        let pos = [base[0] + a.cos() * rr, base[1] + a.sin() * rr, base[2] + 0.7, base[3]];
        let (t25, t55) = (w.ticks(0x19), w.ticks(0x37));
        let life = w.rng.rand_range(t25, t55);
        let s = w.rng.randf(DT * 0.5, DT * 3.0);
        let vz = w.rng.randf(DT * 7.0, DT * 17.0);
        let vz = (vz * k as f32 * 1.5) / 5.0;
        let g = w.rng.rand_range(0x46, 0xcd) as u32;
        part22(w, &crate::particles::type22::Spawn { size: 420_000.0, pos, vel: [a.cos() * s, a.sin() * s, vz, 0.0], c1: g << 8 | 0x460a_00aa, c2: 0x000a_0a46, life });
    }
    for _ in 0..3 {
        let base = w.m(gm).position;
        let a = w.rng.rand_angle();
        let rr = w.rng.randf(f32::from_bits(0x3dcc_cccd), f32::from_bits(0x3f4c_cccd));
        let pos = [base[0] + a.cos() * rr, base[1] + a.sin() * rr, base[2], base[3]];
        let (t40, t70) = (w.ticks(0x28), w.ticks(0x46));
        let life = w.rng.rand_range(t40, t70);
        let s = w.rng.randf(DT * 0.1, DT * 0.4);
        let vz = w.rng.randf(DT * 2.4, DT * 5.5);
        part22(w, &crate::particles::type22::Spawn { size: f32::from_bits(0x48f6_1801), pos, vel: [a.cos() * s, a.sin() * s, vz, 0.0], c1: 0x460a_1414, c2: 0x000a_0a0a, life });
    }
}
