//! **Pokitaru's jet, class 1242** (level11 `0x313290`, census U384, one instance): the flown fighter of the convoy
//! mission. Standing by it Ratchet gets the help line about the Pilot's Helmet until he owns it (item 7), then the
//! "Fly" prompt; △ fades to black, he is hidden and the jet lifts 30 up and flies (hero state 0x32, the ridden-vehicle
//! record [`crate::vehicle`]): the stick steers, ✕ boosts, □ / L1 fire the lasers 1009 ([`super::ship_laser`]),
//! ○ / R1 the missiles 1034 ([`super::ship_missile::spawn_early`]) at the HUD's lock. Near the play area's walls (its
//! cuboid +0x104) the stick is bent back inside. While convoys are on screen the jet calls in ambushing fighters 1319
//! ([`super::ship_fighter::launch`]) every 4 s, up to 5 less the convoys left. Hits wear the record's health;
//! the world wrecks it at once. The HUD ([`super::pokitaru_jet_hud`], the draw callback `0x311d50`) counts the
//! convoys left (+0xe8); none left: 3 s of flight, the fade, the landing: the mission done, Ratchet at the exit. "Quit?"
//! puts him back. Read from the level11 decomp (`0x313290` and its helpers `0x312838` / `0x313f60` / `0x3125a8` /
//! `0x3126f8` / `0x3112b8`, the level's names mapped to level 01 with `rc-trace overlay-diff`) and disassembly; the
//! level data from the overlay (gp 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code: the level's own copy of Gemlik's ship's flight ([`super::gemlik_ship`]), with
//! its own branches (no steering pull, the edge bend instead; no scenery lists; its collision and quit), sharing the
//! code that is the same: the springs, the exhaust (`0x3112b8` = Gemlik's `0x2b8590`, the same descriptors), the
//! screen point. On the shared mechanisms: the vehicle record, the mounted hero state 0x32, the script camera, the
//! view tangent, the occlusion fallback, the help records ([`super::hints`]).
//!
//! **Pvar block** (0x134; the cuboid / group words come with the instance): +0x00 the mount's base velocity, +0x10 the
//! heading (unit), +0x20..+0x28 the camera Euler (sprung), +0x30 / +0x34 / +0x38 roll / pitch / yaw, +0x40 the start
//! position, +0x50 the start rotation, +0x60 (u8) the camera view, +0x61 (u8) the gun side, +0x62 (s16) the wreck
//! ticks (written only), +0x64 the speed, +0x68 (s16) the start-up count, +0x6a (u16) the missile joint, +0x6c /
//! +0x70 the stick's sprung x / y, +0x80 / +0x84 the gun / missile cooldowns, +0x88 the lock, +0x8c the lock timer,
//! +0x90 / +0x94 / +0x98 spring rates, +0xa0..+0xa8 the camera Euler's rates, +0xac / +0xb0 the field of view, +0xb4..
//! +0xbc the camera offset, +0xc0 the FOV rate, +0xc4 the boost fraction, +0xc8..+0xd0 the offset's rates, +0xd4 /
//! +0xd8 the camera yaw / pitch offsets (0), +0xdc the scrape timer, +0xe0 / +0xe4 the crosshair, +0xe8 / +0xea (s16)
//! the convoys left / drawn, +0xec the missile target, +0xf0 / +0xf4 the HUD gauge, +0xf8 the wreck / landing timer,
//! +0xfc the exit cuboid, +0x100 the landing cuboid, +0x104 the play area's cuboid, +0x108 (s16) the convoys' group,
//! +0x10c the ambushers' group, +0x114 the ambush timer, +0x118 −1, +0x11c / +0x120 / +0x128 the HUD's help timers,
//! +0x12c / +0x12e (s16) the engine / second voice, +0x130 the fade.
//!
//! ## Coverage (`0x313290`)
//! | address | what | port |
//! |---|---|---|
//! | entry | `FastDecTimer(+0xdc)` | [`update`] |
//! | entry | state not 7 / 1 / 2: `MobyGetHitMessage(m, 0x30001, 0)`; in state 4: damage below the health → health −= it; the template's +0x30 & 1 → the hit's push added to the position, the velocity polar(speed, yaw, −pitch) reflected off it and yaw / pitch turned toward it by 0.1·damage/32 (pitch the game's +atan) | [`hits`] |
//! | entry | the fatal hit: health 0, → 7, +0x62 = 180, Ratchet 0.2 up (0x13f3d8), his moby +0x98 = 0, `SetState(0, 1)`, the record's class −1 and moby none, `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15, m, v, 0, 10, 3, 16, −1, 1)` | [`hits`] (`fx::beam_explosion`) |
//! | entry | hit slot 0xff; the record names this jet → its bit 2 cleared, 0x140958 = `trunc(health / 256 · 200)` | [`update`] |
//! | 0 | → 1; +0x60 = 0; scale = class scale; +0x20 / +0x24 = +0x30 / +0x34; +0xb0 = +0xac = 2·atan(1, tan_x), +0xc0 = 0; +0x30.., +0x50.. = rotation, +0x40.. = position | [`init`] |
//! | 0 | the gauge's CLUT (FX 0x3c: `0x16df4a` = the FX table's +0x3c CLUT index) saved to 0x1f1088 once (gp−0x4a90) | n/a: the port's gauge draws from the bank's own CLUT ([`super::pokitaru_jet_hud`]) |
//! | 0 | +0xe8 = 1 (s16), +0xf8 = 1, +0x130 = 0; the mission done → 9, no collision, hidden, not drawn | [`init`] |
//! | 1 | the sphere gap to Ratchet ≥ 0 (gp−0x4c30): +0x118 = −1, the fade back | [`wait`] (`pickup::sphere_gap`) |
//! | 1 | touching, no Pilot's Helmet (0x13d4c7): the reminder of help record 0x3c (`Help_Request(0x2afa, 0x3c)`); with it `try_set_help_message(7, 0x2b09)`; △ pressed (0x13cae4 & 0x10) with it → 2, +0x118 = −1 | [`wait`] (`hints::remind`, `interact::try_prompt`) |
//! | 2 | `Approach(1, 4·dt, +0x130)`, the fade (0x15f3fc); at 1: `SetState(0x32, 1)`; z + 30; `MusicRequestTrack(2, 6)`; +0x30.. and +0x20.. zeroed, +0x38 = +0x28 = rot.z; +0x114 = `ticks(1800)`; help record 0x55 unseen → `Help_Request(0x2afc, 0x55)`; → 4 | [`mount`] |
//! | 2 | the record: class, missiles 10 / 20, 0x140948 = 20, 0x14095e = 1, health 255, the moby, +0xf0 = 0, +0xf4 = 0xff, 256, 100, 0x14094b / 49 / 4a = 3, bits 0; `CameraScript(the camera, its Euler, 1, 0, 0)` | [`mount`] ([`crate::vehicle::Record::take`]) |
//! | 2 | +0x68 = 0, speed 0.1, +0x6c / +0x70 / +0x84 / +0x80 / +0xd4 / +0xd8 = 0; distances 0xff; +0xb8 = 0, +0xb4 = −15/4, +0xbc = 1; Ratchet's moby +0x98 = −1; scale = class scale / 4; Ratchet hidden (0x1413f5) | [`mount`] |
//! | 4 | +0x130 ≠ 0 → `Approach(0, 4·dt)`, the fade; 0x15f608 = 2; the flight (`0x312838`) | [`update`], [`fly`] |
//! | 4 | convoys drawn (+0xea) and fewer ambushers on a run (`0x31a758(+0x10c)`) than 5 − +0xe8: `FastDecTimer(+0x114)` out → a launch (`0x31a7d8(+0x10c)`), made → +0x114 = `ticks(240)` | [`update`] (`ship_fighter::{attacking, launch}`) |
//! | 4 | +0xe8 < 1 → +0xf8 = 180, → 8; the record's bit 0 (quit) → 5 | [`update`] |
//! | 5 | Ratchet shown, his moby +0x98 = 0, `SetState(0, 1)`, `CameraScript2(0)`; `FUN_0024b090(0x14095c, 0)`; `MusicRequestTrack(0, 8)`; position / rotation = the start; scale; `fun_0020e098`; `HeroTeleport` to the exit cuboid +0xfc (0, 1); the voices released; → 1; +0x130 = 0.99, the fade | [`quit`] (`FUN_0024b090` on the record's handle: n/a, nothing writes 0x14095c on Pokitaru [L]) |
//! | 7 | 0x15f608 = 2; `FastDecTimer(+0xf8)` out → the death sequence (`0x2319b0`) | [`update`] (`HeroCall::Death`) |
//! | 8 | 0x15f608 = 2; the flight; `FastDecTimer(+0xf8)` out → `Approach(1, 4·dt, +0x130)`, the fade; at 1: Ratchet shown, +0x98 = 0, `SetState(0, 1)`, `CameraScript2(0)`, `MusicRequestTrack(0, 8)`; not drawn, no collision, hidden; at the landing cuboid +0x100 or the start; `fun_0020e098`; `HeroTeleport` to +0xfc; `SetMissionDone(+0xb0)`; → 9 | [`land`] (the `STUB_printf`s n/a) |
//! | 9 | +0x130 ≠ 0 → `Approach(0, 4·dt)`, the fade | [`update`] |
//! | tail | the position clamped to [15, 1008]; in state 4 Ratchet's position / rotation (0x13f3d0 / 0x13f3e0) = the jet's and +0x30..; states 3 / 4: the exhaust (`0x3112b8`), the engine voice (class sound 0, flags 4) restarted when gone; else it is released | [`update`] (`HeroFields::pose`), [`super::gemlik_ship::exhaust`] |
//!
//! ## Coverage (the helpers)
//! | address | what | port |
//! |---|---|---|
//! | 0x312838 | mode &= ~0x100; raw pressed 0x8000 (left) → the view +0x60 + 1, past 1 → 0 | [`fly`] |
//! | 0x312838 | +0x68 > 120: ✕ held (raw 0x40): move record 0x1c bumped (0x141928), speed + 0.035 up to 0.5; else toward 0.4 (+0.025, or 0.1 of the gap above); else +0x68 + 1, speed + 0.003 | [`fly`] (`hints::bump_move`) |
//! | 0x312838 | +0x68 > 5: the stick (0x141070, L3 held: rescaled by 0.0157/0.0123, both springs' max 0.08; else 0.1 / 0.25); the edge bend (`0x313f60`); `Spring(stick, 0.03, 1, max)` on +0x6c / +0x70; the roll `SpringAngle(+0x6c, 0.05, 1, 1)`; pitch −= +0x70·0.01227 clamped to ±80°; yaw −= +0x6c·0.01227 | [`fly`], [`edge_bend`] |
//! | 0x312838 | v = polar(speed, yaw, −pitch); +0x10 = unit(v); position += v | [`fly`] |
//! | 0x312838 | the camera: the boost fraction (+0xc4); the view table (chase: ((−15 − y) − 4f, −3x, 4 + 2y)/4, cockpit (−5.1, 0, 2.6)/4) at +0x60, sprung (`Spring(·, 0.15, 0.8, 1)`); through the Euler (+0x30) rows; `0x316dd0`; the Euler (roll/2 + x/32, pitch + 5.625° − y·0.0123, yaw) sprung (`SpringAngle(·, 0.15, 0.8, 1)`); `0x316e28` with the pitch / yaw less +0xd8 / +0xd4 | [`fly`] (`cinematic::camera_targets`) |
//! | 0x312838 | the FOV `0x270830(✕ held ? 85° : 65°, 2·dt², 2·dt², 2·dt, +0xb0, +0xc0)`; 0x16d4f0 = tan(+0xb0 / 2); `UpdateViewContext` | [`fly`] (`turn::spring`, `Services::view_tan_x`) |
//! | 0x312838 | rotation = (roll + x/2, pitch − y/2, yaw) | [`fly`] |
//! | 0x312838 | `coll_sphere(2, position, 0, m)`: the world → `SpawnBeamExplosion(8, 100, 16, 8, 9, 1, 50, m, 0, 0, 30, 10, 24, −1, 1)`, not drawn, → 7, hidden, +0xf8 = `ticks(90)`; a moby not of class 0, 0x192, 0x4c2, 0x4c3: position = the push; v reflected; yaw / pitch turned 0.1 toward it; speed ·= 0.7; the scrape timer out → health − 5, below 0 the same explosion along the reflected push, → 7; the timer 10 | [`fly`] (the explosion's direction: n/a, the port's explosion takes none) |
//! | 0x312838 | help record 0x55 unseen → `Help_Request(0x2afc, 0x55)`; `RegisterDrawCallback(0x311d50)`; the missiles (`0x3126f8`); the guns (`0x3125a8(cam yaw − x/7, −(cam pitch − y/7 − 0.1))`) | [`fly`] (`Callback::UnitFrame`) |
//! | 0x313f60 | ahead = position + heading·speed·`ticks(60)`; d = ahead − the area cuboid's centre; r = \|d\|xy, R = \|row 0\|xy; z = ahead's own height (`f21`, loaded before the subtraction: the decompiler shows the relative one); beyond R − 10: x moves toward −1 (the area's centre to the left of the yaw) or 1 by clamp((r − R + 10)/10), the record's bit 2; z above row 2's z + the centre's z − 10: y toward −1 by clamp((z − top + 10)/10)·clamp((30° − pitch)/30°), bit 2; else z below 260: y toward 1 by clamp((260 − z)/10)·clamp((pitch + 30°)/30°), bit 2 | [`edge_bend`] |
//! | 0x3125a8 | aim = polar(400·dt, yaw, pitch) + +0x00; `FastDecTimer(+0x80)` out with □ / L1 held (raw & 0x84): the muzzle = joint list (+0x61 & 1) + 3's point + unit(aim)·2.2; `0x308848(200, −1, m, aim, muzzle)` (= the laser's `0x3025f8`); +0x80 = 4; +0x61 ^= 1; `PlayClassSound(3, 0)`; the crosshair: unit(aim)·23 + position through `0x311210` → +0xe0 / +0xe4 | [`guns`] (`ship_laser::spawn`, `gemlik_ship::screen_point`) |
//! | 0x3126f8 | `FastDecTimer(+0x84)` out, ○ / R1 held (raw & 0x28), missiles left: +0x6a = (+0x6a + 1) & 3; joint list +0x6a + 7's point + unit(+0x00)·0.3; the target: the lock (+0x88) when its timer (+0x8c) is below 501; `0x309378(100·dt, m, p, target, rotation, ticks(300))`; made → `PlayClassSound(2, 0)`, one missile spent, +0x84 = 30 | [`missiles`] (`ship_missile::spawn_early`) |
//! | 0x3112b8 | the exhaust: Gemlik's `0x2b8590` (gp−0x4c0c = 0.4 = 24·dt; descriptors gp−0x4a38.. = Gemlik's) | [`super::gemlik_ship::exhaust`] |

use super::gemlik_ship::{spring, spring_angle};
use crate::moby_runtime::MobyId;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::fx::{self, polar};
use crate::moby_update::creature::{self as c, add, add_rot, atan, set_len3, sub_rot, DT, DT2};
use crate::moby_update::services::{fl, pf as to_pf, pv, HeroCall, HeroPose, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x31_3290;
pub const CLASS: i16 = 0x4da;
pub const CLASSES: [i16; 1] = [CLASS];

pub mod pvo {
    pub const BASE_VEL: usize = 0x00;
    pub const HEADING: usize = 0x10;
    pub const CAM_EULER: usize = 0x20;
    pub const ROLL: usize = 0x30;
    pub const PITCH: usize = 0x34;
    pub const YAW: usize = 0x38;
    pub const START_POS: usize = 0x40;
    pub const START_ROT: usize = 0x50;
    pub const VIEW: usize = 0x60;
    pub const GUN_SIDE: usize = 0x61;
    pub const WRECK_T: usize = 0x62;
    pub const SPEED: usize = 0x64;
    pub const STARTUP: usize = 0x68;
    pub const MISSILE_JOINT: usize = 0x6a;
    pub const STICK_X: usize = 0x6c;
    pub const STICK_Y: usize = 0x70;
    pub const GUN_T: usize = 0x80;
    pub const MISSILE_T: usize = 0x84;
    pub const LOCK: usize = 0x88;
    pub const LOCK_T: usize = 0x8c;
    pub const STICK_XV: usize = 0x90;
    pub const STICK_YV: usize = 0x94;
    pub const ROLL_V: usize = 0x98;
    pub const CAM_EULER_V: usize = 0xa0;
    pub const FOV0: usize = 0xac;
    pub const FOV: usize = 0xb0;
    pub const CAM_OFF: usize = 0xb4;
    pub const FOV_V: usize = 0xc0;
    pub const BOOST: usize = 0xc4;
    pub const CAM_OFF_V: usize = 0xc8;
    pub const CAM_YAW_OFF: usize = 0xd4;
    pub const CAM_PITCH_OFF: usize = 0xd8;
    pub const SCRAPE_T: usize = 0xdc;
    pub const CROSS_X: usize = 0xe0;
    pub const CROSS_Y: usize = 0xe4;
    pub const CONVOYS: usize = 0xe8;
    pub const CONVOYS_DRAWN: usize = 0xea;
    pub const MISSILE_TARGET: usize = 0xec;
    pub const GAUGE: usize = 0xf0;
    pub const GAUGE_CUT: usize = 0xf4;
    pub const TIMER: usize = 0xf8;
    pub const EXIT_CUBOID: usize = 0xfc;
    pub const LAND_CUBOID: usize = 0x100;
    pub const AREA_CUBOID: usize = 0x104;
    pub const CONVOY_GROUP: usize = 0x108;
    pub const AMBUSH_GROUP: usize = 0x10c;
    pub const AMBUSH_T: usize = 0x114;
    pub const F118: usize = 0x118;
    pub const HELP_HEALTH_T: usize = 0x11c;
    pub const HELP_MISSILE_T: usize = 0x120;
    pub const HELP_FIGHTER_T: usize = 0x128;
    pub const ENGINE_VOICE: usize = 0x12c;
    pub const VOICE_B: usize = 0x12e;
    pub const FADE: usize = 0x130;
    pub const LEN: usize = 0x134;
}

/// The record's values at the mount (the same as Gemlik's ship's).
const MOUNT: crate::vehicle::Mount = crate::vehicle::Mount { missiles: 10, missiles_max: 20, b48: 20, b49: 3, b4a: 3, b4b: 3, health: 255.0, health_max: 256.0, f54: 100.0, b5e: 1 };

/// The classes a scrape ignores (Ratchet, the level's own pieces, the missile pickup and its parachute).
const NO_SCRAPE: [i16; 4] = [0, 0x192, 0x4c2, 0x4c3];
/// The Pilot's Helmet.
const HELMET: usize = 7;
/// The prompt and the help lines (message, record).
const PROMPT: i32 = 0x2b09;
const HELP_HELMET: (i32, usize) = (0x2afa, 0x3c);
const HELP_FLY: (i32, usize) = (0x2afc, 0x55);
/// Move record 0x1c (0x141928): the boost.
const BOOST_REC: usize = 0x1c;

/// The pad bits (the game's order): △ pressed, ✕, ○ | R1, □ | L1, L3, left (raw pressed).
mod pad {
    pub const TRIANGLE: u32 = 0x10;
    pub const CROSS: u32 = 0x40;
    pub const MISSILE: u32 = 0x28;
    pub const GUN: u32 = 0x84;
    pub const L3: u32 = 0x200;
    pub const LEFT: u32 = 0x8000;
}

const DEG80: f32 = 1.396_263_4;
const DEG80_B: f32 = f32::from_bits(0x3fb2_b8c2);
const DEG30: f32 = std::f32::consts::FRAC_PI_6;
/// gp−0x4bec / −0x4be8: 0.01227 a unit of stick a tick; gp−0x4be4: the L3 rescale.
const STEP: f32 = 0.012_271_847;
const STEP_L3: f32 = 0.015_707_964;
/// gp−0x4c0c / −0x4c08: the cruise and the boost speed; gp−0x4c18 / −0x4c14 / −0x4c04 / −0x4c00: the creep, the
/// approach step, the boost step and the approach share.
const CRUISE: f32 = 0.4;
const BOOST: f32 = 0.5;

fn pf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, o) }
fn set(w: &mut World, id: MobyId, o: usize, v: f32) { c::set_pf(w, id, o, v) }
fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(pi(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }

fn row(name: u32) -> Option<u16> { super::row(REFERENCE_LEVEL, name) }

fn music(w: &mut World, track: i16, stinger: i16) { if let Some(s) = w.sound.as_deref_mut() { s.music_request(track, stinger); } }

fn release_voice(w: &mut World, id: MobyId, o: usize) {
    let s = c::pi16(w, id, o) as i32;
    if s != -1 { w.release_sound(s, id); }
    c::set_pi16(w, id, o, -1);
}

fn release_voices(w: &mut World, id: MobyId) {
    release_voice(w, id, pvo::ENGINE_VOICE);
    release_voice(w, id, pvo::VOICE_B);
}

/// `Approach(to, 4·dt, +0x130)` and the fade 0x15f3fc.
fn fade(w: &mut World, id: MobyId, to: f32) -> f32 {
    let mut f = pf(w, id, pvo::FADE);
    c::turn::approach(to, DT * 4.0, &mut f);
    set(w, id, pvo::FADE, f);
    crate::cinematic::set_fade(w, f);
    f
}

fn ratchet_collision(w: &mut World, bits: u32) { if let Some(h) = w.hero_moby { w.mm(h).coll_disable = bits; } }

fn class_scale(w: &World) -> f32 { fl(w.class_scale(CLASS)) }

fn restore_start(w: &mut World, id: MobyId) {
    let p = c::pv4(w, id, pvo::START_POS);
    let r = c::pv4(w, id, pvo::START_ROT);
    let m = w.mm(id);
    m.position = p;
    m.rotation = r;
}

fn teleport_exit(w: &mut World, id: MobyId) {
    if let Some((pos, rot)) = story::cuboid(w, pi(w, id, pvo::EXIT_CUBOID)) { crate::cinematic::hero_teleport(w, pos, rot, 0, true); }
}

/// Ratchet back on foot: shown, his collision, `SetState(0, 1)`, `CameraScript2(0)`.
fn dismount(w: &mut World) {
    w.hero_fields_mut().hero_hidden = Some(0);
    ratchet_collision(w, 0);
    crate::cinematic::hero_state(w, 0, true);
    crate::cinematic::camera_script2(w, 0);
}

/// Level11 `0x313290` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    c::dec_timer_pvar_i32(w, id, pvo::SCRAPE_T);
    hits(w, id);
    w.mm(id).hit_slot = 0xff;
    if w.svc.vehicle.is(id) {
        w.svc.vehicle.quit &= 0xfd;
        w.svc.vehicle.hud = ((w.svc.vehicle.health / 256.0) * 200.0) as i32;
    }
    match w.m(id).state {
        0 => init(w, id),
        1 => wait(w, id),
        2 => mount(w, id),
        4 => {
            if pf(w, id, pvo::FADE) != 0.0 { fade(w, id, 0.0); }
            w.svc.occlusion_fallback = Some((w.counter, 2));
            fly(w, id);
            let g = pi(w, id, pvo::AMBUSH_GROUP);
            let attacking = super::ship_fighter::attacking(w, g, super::ship_fighter::CLASS);
            let left = c::pi16(w, id, pvo::CONVOYS) as i32;
            if c::pi16(w, id, pvo::CONVOYS_DRAWN) != 0 && attacking < 5 - left && c::dec_timer_pvar_i32(w, id, pvo::AMBUSH_T) != 0 && super::ship_fighter::launch(w, g, super::ship_fighter::CLASS).is_some() {
                let t = w.ticks(0xf0);
                seti(w, id, pvo::AMBUSH_T, t);
            }
            if c::pi16(w, id, pvo::CONVOYS) < 1 {
                seti(w, id, pvo::TIMER, 180);
                w.mm(id).state = 8;
            }
            if w.svc.vehicle.quit & 1 != 0 { w.mm(id).state = 5; }
        }
        5 => quit(w, id),
        7 => {
            w.svc.occlusion_fallback = Some((w.counter, 2));
            if c::dec_timer_pvar_i32(w, id, pvo::TIMER) != 0 { w.hero_fields_mut().call(HeroCall::Death); }
        }
        8 => land(w, id),
        9 if pf(w, id, pvo::FADE) != 0.0 => {
            fade(w, id, 0.0);
        }
        _ => {}
    }
    let p = w.m(id).position;
    let cl = |x: f32| x.clamp(15.0, 1008.0);
    w.mm(id).position = [cl(p[0]), cl(p[1]), cl(p[2]), p[3]];
    let st = w.m(id).state;
    if st == 4 {
        let p = w.m(id).position;
        let yaw = pf(w, id, pvo::YAW);
        w.hero_fields_mut().pose = Some(HeroPose { pos: [p[0], p[1], p[2]], yaw, target_yaw: yaw });
    }
    if st == 3 || st == 4 {
        super::gemlik_ship::exhaust(w, id);
        let v = c::pi16(w, id, pvo::ENGINE_VOICE) as i32;
        if !w.sound_alive(v, id) {
            let s = w.play_sound(0, 4, id);
            c::set_pi16(w, id, pvo::ENGINE_VOICE, s as i16);
        }
    } else {
        release_voice(w, id, pvo::ENGINE_VOICE);
    }
}

/// The hits at the top of `0x313290` (module doc).
fn hits(w: &mut World, id: MobyId) {
    let st = w.m(id).state;
    if matches!(st, 7 | 1 | 2) { return; }
    let Some(h) = w.get_hit(id, 0x3_0001, false) else { return };
    if w.m(id).state != 4 { return; }
    let dmg = fl(h.damage);
    let (yaw, pitch, speed) = (pf(w, id, pvo::YAW), pf(w, id, pvo::PITCH), pf(w, id, pvo::SPEED));
    let a = polar(speed, yaw, -pitch);
    if dmg < w.svc.vehicle.health {
        w.svc.vehicle.health -= dmg;
        if h.w30 & 1 != 0 {
            let k = dmg * 0.031_25;
            let d = crate::moby_update::services::fv(h.dir);
            let p = add(w.m(id).position, [d[0], d[1], d[2], 0.0]);
            w.mm(id).position = p;
            let r = crate::hero::guns::reflect([a[0], a[1], a[2]], [d[0], d[1], d[2]]);
            let ny = add_rot(yaw, sub_rot(atan(r[0], r[1]), yaw) * 0.1 * k);
            set(w, id, pvo::YAW, ny);
            let xy = (r[0] * r[0] + r[1] * r[1]).sqrt();
            let np = add_rot(pitch, sub_rot(atan(xy, r[2]), pitch) * 0.1 * k);
            set(w, id, pvo::PITCH, np);
        }
    } else {
        w.svc.vehicle.health = 0.0;
        w.mm(id).state = 7;
        c::set_pi16(w, id, pvo::WRECK_T, 0xb4);
        let hp = w.hero_point();
        w.hero_fields_mut().pose = Some(HeroPose { pos: [hp[0], hp[1], hp[2] + 0.2], yaw, target_yaw: yaw });
        ratchet_collision(w, 0);
        crate::cinematic::hero_state(w, 0, true);
        w.svc.vehicle.class = -1;
        w.svc.vehicle.moby = None;
        let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 16, debris: 0, sound: -1, shake: true };
        let p = w.m(id).position;
        fx::beam_explosion(w, &b, Some(id), p);
    }
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).state = 1;
    c::set_pu8(w, id, pvo::VIEW, 0);
    let s = class_scale(w);
    w.mm(id).scale = s;
    let (r0, r1) = (pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH));
    set(w, id, pvo::CAM_EULER, r0);
    set(w, id, pvo::CAM_EULER + 4, r1);
    let f = atan(1.0, w.svc.view_tan_x);
    set(w, id, pvo::FOV_V, 0.0);
    set(w, id, pvo::FOV, f + f);
    set(w, id, pvo::FOV0, f + f);
    let r = w.m(id).rotation;
    c::set_pv4(w, id, pvo::ROLL, r);
    c::set_pv4(w, id, pvo::START_ROT, r);
    let p = w.m(id).position;
    c::set_pv4(w, id, pvo::START_POS, p);
    c::set_pi16(w, id, pvo::CONVOYS, 1);
    seti(w, id, pvo::TIMER, 1);
    set(w, id, pvo::FADE, 0.0);
    let mission = w.m(id).mission;
    if mission != 0xff && story::mission_done(w, mission as i32) {
        let m = w.mm(id);
        m.state = 9;
        m.has_collision = false;
        m.mode |= 1;
        m.visible = 0;
    }
}

/// State 1: waiting beside the jet (module doc).
fn wait(w: &mut World, id: MobyId) {
    let gap = w.hero_moby.map_or(f32::MAX, |h| crate::moby_update::classes::pickup::sphere_gap(w, id, h));
    if 0.0 <= gap {
        seti(w, id, pvo::F118, -1);
        if pf(w, id, pvo::FADE) != 0.0 { fade(w, id, 0.0); }
        return;
    }
    let helmet = super::hints::owned(w, HELMET);
    if !helmet {
        super::hints::remind(w, HELP_HELMET.1, HELP_HELMET.0, HELP_HELMET.1 as i32);
    } else {
        w.svc.interact.try_prompt(7, PROMPT);
    }
    if helmet && w.hero.loop_in.pad.pressed & pad::TRIANGLE != 0 {
        w.mm(id).state = 2;
        seti(w, id, pvo::F118, -1);
    }
}

/// State 2: the fade out and the mount (module doc).
fn mount(w: &mut World, id: MobyId) {
    if fade(w, id, 1.0) != 1.0 { return; }
    crate::cinematic::hero_state(w, 0x32, true);
    w.mm(id).position[2] += 30.0;
    music(w, 2, 6);
    c::set_pv4(w, id, pvo::ROLL, [0.0; 4]);
    c::set_pv4(w, id, pvo::CAM_EULER, [0.0; 4]);
    let rz = w.m(id).rotation[2];
    set(w, id, pvo::YAW, rz);
    set(w, id, pvo::CAM_EULER + 8, rz);
    let t = w.ticks(0x708);
    seti(w, id, pvo::AMBUSH_T, t);
    if w.svc.help.records.help[HELP_FLY.1].count == 0 { super::hints::request(w, HELP_FLY.0, HELP_FLY.1 as i32); }
    w.mm(id).state = 4;
    let class = w.m(id).o_class;
    w.svc.vehicle.take(id, class, &MOUNT);
    set(w, id, pvo::GAUGE, 0.0);
    seti(w, id, pvo::GAUGE_CUT, 0xff);
    let (cam, e) = (w.camera_point(), w.hero.loop_in.cam_euler);
    crate::cinematic::camera_script(w, [cam[0], cam[1], cam[2]], e, 1, 0, false);
    c::set_pi16(w, id, pvo::STARTUP, 0);
    set(w, id, pvo::SPEED, 0.1);
    for o in [pvo::STICK_X, pvo::STICK_Y, pvo::MISSILE_T, pvo::GUN_T, pvo::CAM_YAW_OFF, pvo::CAM_PITCH_OFF] { seti(w, id, o, 0); }
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
    }
    set(w, id, pvo::CAM_OFF + 4, 0.0);
    set(w, id, pvo::CAM_OFF, -15.0 * 0.25);
    set(w, id, pvo::CAM_OFF + 8, 4.0 * 0.25);
    ratchet_collision(w, 0xffff_ffff);
    let s = class_scale(w) * 0.25;
    w.mm(id).scale = s;
    w.hero_fields_mut().hero_hidden = Some(1);
}

/// State 5: the quit back to the start (module doc).
fn quit(w: &mut World, id: MobyId) {
    dismount(w);
    music(w, 0, 8);
    restore_start(w, id);
    let s = class_scale(w);
    w.mm(id).scale = s;
    crate::moby_update::creature::react::sphere_lerp(w, id);
    teleport_exit(w, id);
    release_voices(w, id);
    w.mm(id).state = 1;
    set(w, id, pvo::FADE, f32::from_bits(0x3f7d_70a4));
    crate::cinematic::set_fade(w, 0.99);
}

/// State 8: the flight on, then the fade and the landing (module doc).
fn land(w: &mut World, id: MobyId) {
    w.svc.occlusion_fallback = Some((w.counter, 2));
    fly(w, id);
    if c::dec_timer_pvar_i32(w, id, pvo::TIMER) == 0 { return; }
    if fade(w, id, 1.0) < 1.0 { return; }
    dismount(w);
    music(w, 0, 8);
    {
        let m = w.mm(id);
        m.visible = 0;
        m.has_collision = false;
        m.mode |= 1;
    }
    match story::cuboid(w, pi(w, id, pvo::LAND_CUBOID)) {
        None => restore_start(w, id),
        Some((pos, rot)) => {
            let m = w.mm(id);
            m.position = [pos[0], pos[1], pos[2], m.position[3]];
            m.rotation = [rot[0], rot[1], rot[2], m.rotation[3]];
        }
    }
    crate::moby_update::creature::react::sphere_lerp(w, id);
    teleport_exit(w, id);
    let mission = w.m(id).mission;
    if mission != 0xff { crate::cinematic::set_mission_done(w, mission); }
    w.mm(id).state = 9;
}

/// `0x312838`: one tick of flight (module doc).
fn fly(w: &mut World, id: MobyId) {
    w.mm(id).mode &= !0x100;
    let (raw_pressed, raw, held) = { let p = &w.hero.loop_in.pad; (p.raw_pressed, p.raw, p.held) };
    if raw_pressed & pad::LEFT != 0 {
        let v = c::pu8(w, id, pvo::VIEW).wrapping_add(1);
        c::set_pu8(w, id, pvo::VIEW, if 1 < v { 0 } else { v });
    }
    let mut speed = pf(w, id, pvo::SPEED);
    let t = c::pi16(w, id, pvo::STARTUP) as i32;
    if 120 < t {
        if raw & pad::CROSS != 0 {
            super::hints::bump_move(w, BOOST_REC);
            speed += 0.035;
            if BOOST < speed { speed = BOOST; }
        } else if speed < CRUISE - 0.025 {
            speed += 0.025;
        } else if speed <= CRUISE {
            speed = CRUISE;
        } else {
            speed += (CRUISE - speed) * 0.1;
        }
    } else {
        c::set_pi16(w, id, pvo::STARTUP, (t + 1) as i16);
        speed += 0.003;
    }
    set(w, id, pvo::SPEED, speed);
    if 5 < c::pi16(w, id, pvo::STARTUP) as i32 {
        let [sx, sy] = w.hero.stick.map(|x| f32::from_bits(x.0));
        let (mut x, mut y, mut kx, mut ky) = (sx, sy, 0.1f32, 0.25f32);
        if held & pad::L3 != 0 {
            x = (sx / STEP) * STEP_L3;
            y = (sy / STEP) * STEP_L3;
            kx = 0.08;
            ky = 0.08;
        }
        edge_bend(w, id, &mut x, &mut y);
        spring(w, id, x, 0.03, 1.0, kx, pvo::STICK_X, pvo::STICK_XV);
        spring(w, id, y, 0.03, 1.0, ky, pvo::STICK_Y, pvo::STICK_YV);
        let s = pf(w, id, pvo::STICK_X);
        spring_angle(w, id, s, 0.05, 1.0, 1.0, pvo::ROLL, pvo::ROLL_V);
        let p = sub_rot(pf(w, id, pvo::PITCH), pf(w, id, pvo::STICK_Y) * STEP);
        set(w, id, pvo::PITCH, p);
        if DEG80 < p { set(w, id, pvo::PITCH, DEG80_B) } else if p < -DEG80 { set(w, id, pvo::PITCH, -DEG80_B) }
        let yw = sub_rot(pf(w, id, pvo::YAW), pf(w, id, pvo::STICK_X) * STEP);
        set(w, id, pvo::YAW, yw);
    }
    let mut v = polar(speed, pf(w, id, pvo::YAW), -pf(w, id, pvo::PITCH));
    c::set_pv4(w, id, pvo::HEADING, set_len3(v, 1.0));
    let p = add(w.m(id).position, v);
    w.mm(id).position = p;
    // The camera.
    let f = speed - CRUISE;
    let f = if f < 0.0 { 0.0 / (BOOST - CRUISE) } else { f / (BOOST - CRUISE) };
    set(w, id, pvo::BOOST, f);
    let (sx, sy) = (pf(w, id, pvo::STICK_X), pf(w, id, pvo::STICK_Y));
    let chase = [((-15.0 - sy) - f * 4.0) * 0.25, sx * -3.0 * 0.25, (sy + sy + 4.0) * 0.25];
    let cockpit = [-5.1 * 0.25, 0.0, f32::from_bits(0x4026_6666) * 0.25];
    let off = if c::pu8(w, id, pvo::VIEW) == 0 { chase } else { cockpit };
    for (k, &o) in off.iter().enumerate() { spring(w, id, o, 0.15, 0.8, 1.0, pvo::CAM_OFF + 4 * k, pvo::CAM_OFF_V + 4 * k); }
    let o = [pf(w, id, pvo::CAM_OFF), pf(w, id, pvo::CAM_OFF + 4), pf(w, id, pvo::CAM_OFF + 8)];
    let rows = super::ship_fighter::rows_of([pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH), pf(w, id, pvo::YAW)]);
    let pos = w.m(id).position;
    let eye: [f32; 3] = std::array::from_fn(|k| o[0] * rows[0][k] + o[1] * rows[1][k] + o[2] * rows[2][k] + pos[k]);
    crate::cinematic::camera_targets(w, Some(eye), None);
    let ex = add_rot(pf(w, id, pvo::ROLL) * 0.5, sx * 0.031_25);
    let ey = sub_rot(add_rot(pf(w, id, pvo::PITCH), f32::from_bits(0x3dc9_0fdb)), sy * STEP);
    let ez = pf(w, id, pvo::YAW);
    spring_angle(w, id, ex, 0.15, 0.8, 1.0, pvo::CAM_EULER, pvo::CAM_EULER_V);
    spring_angle(w, id, ey, 0.15, 0.8, 1.0, pvo::CAM_EULER + 4, pvo::CAM_EULER_V + 4);
    spring_angle(w, id, ez, 0.15, 0.8, 1.0, pvo::CAM_EULER + 8, pvo::CAM_EULER_V + 8);
    let euler = [pf(w, id, pvo::CAM_EULER), sub_rot(pf(w, id, pvo::CAM_EULER + 4), pf(w, id, pvo::CAM_PITCH_OFF)), sub_rot(pf(w, id, pvo::CAM_EULER + 8), pf(w, id, pvo::CAM_YAW_OFF))];
    crate::cinematic::camera_targets(w, None, Some(euler));
    let target = if raw & pad::CROSS == 0 { f32::from_bits(0x3f91_361e) } else { f32::from_bits(0x3fbd_e44e) };
    let (mut fov, mut fv) = (pf(w, id, pvo::FOV), pf(w, id, pvo::FOV_V));
    c::turn::spring(target, DT2 + DT2, DT2 + DT2, DT + DT, &mut fov, &mut fv);
    set(w, id, pvo::FOV, fov);
    set(w, id, pvo::FOV_V, fv);
    let half = fov * 0.5;
    w.svc.view_tan_x = half.sin() / half.cos();
    {
        let (roll, pitch, yaw) = (pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH), pf(w, id, pvo::YAW));
        let m = w.mm(id);
        m.rotation[0] = add_rot(roll, sx * 0.5);
        m.rotation[1] = add_rot(pitch, -sy * 0.5);
        m.rotation[2] = add_rot(yaw, -sx * 0.0);
    }
    // The jet against the world and the mobys.
    let centre = pv(w.m(id).position);
    if let Some(h) = w.coll_sphere(centre, to_pf(2.0), 0, Some(id)) {
        let blast = fx::Beam { damage_r: 8.0, damage: 100.0, flash: 16.0, flash2: 8.0, flash_dist: 9.0, scale: 1.0, light: 50.0, streaks: 30, sparks: 10, puffs: 24, debris: 0, sound: -1, shake: true };
        match h.moby.map(|m| w.m(m).o_class) {
            None => {
                let p = w.m(id).position;
                fx::beam_explosion(w, &blast, Some(id), p);
                let t = w.ticks(0x5a);
                let m = w.mm(id);
                m.visible = 0;
                m.state = 7;
                m.mode |= 1;
                seti(w, id, pvo::TIMER, t);
            }
            Some(k) if !NO_SCRAPE.contains(&k) => {
                if let Some(q) = h.pushed_centre {
                    let m = w.mm(id);
                    m.position = [q[0], q[1], q[2], m.position[3]];
                }
                let r = crate::hero::guns::reflect([v[0], v[1], v[2]], h.normal);
                v = [r[0], r[1], r[2], 0.0];
                let yw = pf(w, id, pvo::YAW);
                set(w, id, pvo::YAW, add_rot(yw, sub_rot(atan(v[0], v[1]), yw) * 0.1));
                let xy = (v[0] * v[0] + v[1] * v[1]).sqrt();
                let p = pf(w, id, pvo::PITCH);
                set(w, id, pvo::PITCH, add_rot(p, sub_rot(-atan(xy, v[2]), p) * 0.1));
                set(w, id, pvo::SPEED, pf(w, id, pvo::SPEED) * 0.7);
                if pi(w, id, pvo::SCRAPE_T) == 0 {
                    w.svc.vehicle.health -= 5.0;
                    if w.svc.vehicle.health < 0.0 {
                        let p = w.m(id).position;
                        fx::beam_explosion(w, &blast, Some(id), p);
                        w.mm(id).state = 7;
                    }
                    seti(w, id, pvo::SCRAPE_T, 10);
                }
            }
            Some(_) => {}
        }
    }
    if w.svc.help.records.help[HELP_FLY.1].count == 0 { super::hints::request(w, HELP_FLY.0, HELP_FLY.1 as i32); }
    if let Some(i) = row(super::pokitaru_jet_hud::HUD_FN) { w.svc.draw_callbacks.register(Callback::UnitFrame(i), id); }
    missiles(w, id);
    let yaw = add_rot(pf(w, id, pvo::CAM_EULER + 8), -pf(w, id, pvo::STICK_X) / 7.0);
    let pitch = add_rot(pf(w, id, pvo::CAM_EULER + 4), -(pf(w, id, pvo::STICK_Y) / 7.0) - 0.1);
    guns(w, id, yaw, -pitch);
}

/// `0x313f60(m, &x, &y)`: the stick bent back inside the play area (module doc).
fn edge_bend(w: &mut World, id: MobyId, x: &mut f32, y: &mut f32) {
    let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, pi(w, id, pvo::AREA_CUBOID)) else { return };
    let (m, centre) = (s.matrix, s.centre());
    let speed = pf(w, id, pvo::SPEED);
    let k = speed * w.ticks(0x3c) as f32;
    let ahead = add(c::pv4(w, id, pvo::HEADING).map(|q| q * k), w.m(id).position);
    let d = [ahead[0] - centre[0], ahead[1] - centre[1], ahead[2] - centre[2]];
    let r = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let big_r = (m[0][0] * m[0][0] + m[0][1] * m[0][1]).sqrt();
    let top = m[2][2] + centre[2];
    let a = add_rot(atan(d[0], d[1]), f32::from_bits(0x4049_0fd0));
    if big_r - 10.0 < r {
        let k = (((r - big_r) + 10.0) / 10.0).clamp(0.0, 1.0);
        let side = if 0.0 < sub_rot(a, pf(w, id, pvo::YAW)) { -1.0 } else { 1.0 };
        *x += (side - *x) * k;
        w.svc.vehicle.quit |= 2;
    }
    let pitch = pf(w, id, pvo::PITCH);
    let z = ahead[2];
    if top - 10.0 < z {
        let k = (((z - top) + 10.0) / 10.0).clamp(0.0, 1.0);
        let n = ((DEG30 - pitch) / DEG30).clamp(0.0, 1.0);
        *y += (-1.0 - *y) * k * n;
        w.svc.vehicle.quit |= 2;
        return;
    }
    if 260.0 <= z { return; }
    let k = (((250.0 - z) + 10.0) / 10.0).clamp(0.0, 1.0);
    let n = ((pitch + DEG30) / DEG30).clamp(0.0, 1.0);
    *y += (1.0 - *y) * k * n;
    w.svc.vehicle.quit |= 2;
}

/// `0x3125a8(yaw, pitch, m, pvars)` (module doc).
fn guns(w: &mut World, id: MobyId, yaw: f32, pitch: f32) {
    let aim = add(polar(f32::from_bits(0x40d5_5555), yaw, pitch), c::pv4(w, id, pvo::BASE_VEL));
    if c::dec_timer_pvar_i32(w, id, pvo::GUN_T) != 0 && w.hero.loop_in.pad.raw & pad::GUN != 0 {
        let d = set_len3(aim, f32::from_bits(0x400c_cccd));
        let side = c::pu8(w, id, pvo::GUN_SIDE) & 1;
        let muzzle = add(w.joint_point(id, side as usize + 3), d);
        super::ship_laser::spawn(w, 200.0, -1.0, id, aim, muzzle);
        seti(w, id, pvo::GUN_T, 4);
        let s = c::pu8(w, id, pvo::GUN_SIDE) ^ 1;
        c::set_pu8(w, id, pvo::GUN_SIDE, s);
        w.play_sound(3, 0, id);
    }
    let a = set_len3(aim, 23.0);
    let p = w.m(id).position;
    if let Some([x, y]) = super::gemlik_ship::screen_point(w, [a[0] + p[0], a[1] + p[1], a[2] + p[2]], 0) {
        seti(w, id, pvo::CROSS_X, x);
        seti(w, id, pvo::CROSS_Y, y);
    }
}

/// `0x3126f8(m, pvars)` (module doc).
fn missiles(w: &mut World, id: MobyId) {
    if c::dec_timer_pvar_i32(w, id, pvo::MISSILE_T) == 0 { return; }
    if w.hero.loop_in.pad.raw & pad::MISSILE == 0 || w.svc.vehicle.missiles == 0 { return; }
    let j = (c::pi16(w, id, pvo::MISSILE_JOINT) as u16).wrapping_add(1) & 3;
    c::set_pi16(w, id, pvo::MISSILE_JOINT, j as i16);
    let at = w.joint_point(id, j as usize + 7);
    let at = add(at, set_len3(c::pv4(w, id, pvo::BASE_VEL), 0.3));
    let target = link(w, id, pvo::LOCK).filter(|_| pi(w, id, pvo::LOCK_T) < 0x1f5);
    let life = w.ticks(300);
    let rot = w.m(id).rotation;
    if super::ship_missile::spawn_early(w, f32::from_bits(0x3fd5_5555), id, at, target, rot, life).is_some() {
        w.play_sound(2, 0, id);
        w.svc.vehicle.missiles -= 1;
        seti(w, id, pvo::MISSILE_T, 30);
    }
}
