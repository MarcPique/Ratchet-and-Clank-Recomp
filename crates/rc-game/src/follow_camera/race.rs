//! **Oltanis's grind race** (level 14): camera class 20, the race's intro fly-by (hook `0x315920`, init `0x314f00`,
//! update `0x315290`, pre `0x3159d0` (empty), the region test `0x314e20`, which is class 19's), and class 21, the race
//! camera (hook `0x316748`, init `0x3159d8`, update `0x315d48`, pre `0x3167e0`), with the race start `0x2d6570` the
//! intro calls at its end. Ratchet grinds into the intro's region: the camera takes over and holds him in its cuboid
//! (state 0x72) while it flies its path; at the end it puts him on his rail at the start (state 0x28), sends the rail
//! bots 211 after him (their state 6, `crate::moby_update::classes::units::oltanis_rail_bot`) and lets go with a cut to
//! the race camera, which orbits him by a keyed table along the race (the share of his rail behind him) and tells the
//! bots when to attack. It lets go when he leaves the rail. Spec: docs/plan/player_controller.md §15.
//!
//! **System or not.** Two classes (`camvtbl` rows) found on level 14 only ([`super::level::CameraPorts`] compares the
//! functions with level 14's). Class 20 is class 19's code ([`super::flyby`]) without the fades, with its FOV eased and
//! its own end. Their stores into the moby world and the hero block are [`RaceOut`], made before the next moby loop
//! (the camera's `rand` draw with them, ahead of the frame's draw callbacks, as in the game).
//!
//! **Class 20's record** = class 19's ([`super::flyby::FlybyRec`]; +0x38 = 3 at its init: never again) and +0x50 the
//! race's moby (its group: the rail bots), +0x54 the cuboid Ratchet waits in. **Class 21's**: +0x20 its state (0 →
//! 1 once class 20 is current → 2 at its init), +0x24 / +0x28 / +0x2c the three rail bots A / B / C, +0x30 (s16) the
//! attack calls made, +0x32 (s16) the stage, +0x34 / +0x38 / +0x3c the three race rails.
//!
//! | address | call / branch | port |
//! |---|---|---|
//! | 0x315920 | +0x38 set → −1; the best camera not releasing and Ratchet not grinding (group 0xf) → 0; the region test, group +0x48 / state +0x44 → 1 | [`Camera::intro_hook`] |
//! | 0x314f00 | class 19's init with +0x38 = 3 and the FOV keys as they are (no 180° substitution); `HeroTeleport` to the cuboid +0x54's centre, Euler 0, state 0x72 | [`Camera::intro_init`] |
//! | 0x315290 phase 0 | the letterbox on, phase 1, the camera at the path's first point facing the look path's first point (or the moby + offset) | [`Camera::intro_update`] |
//! | phase 1 | along the path at +0x3c·dt; the timer, +0x39 → the end; else the FOV between its keys eased (`0x260358` = `CosInterp`), the look as class 19's | `intro_update` |
//! | the end | the FOV back, the fade-out rate 0x1675d8 = 0.25, +0x7e = 4, the letterbox off, the race start; the camera 10 along Ratchet's rail from his cursor and 2 up, looking at his rail point 1.5 up | `intro_update` |
//! | 0x2d6570 | over the race moby's group: a member on Ratchet's rail: his cursor = the rail's nearest segment (20, 5) to its marks path's first point, t 0, `HeroTeleport` to that point facing along the rail (3 points on) − 90°, state 0x28; the first three members: state 6, facing from the race moby to Ratchet, their cursor his, 9 back along their rails, +0x90 cleared, +0xf8 = 2k, +0xfc = 2k + 1, their light freed | [`Camera::race_start`] (Ratchet's part) and `oltanis_rail_bot::race_start` |
//! | 0x316748 | +0x20 = 0: class 20 current → +0x20 = 1; 0. +0x20 = 1: class 20 not current, or the best releasing → 1; else 0 | [`Camera::race_hook`] |
//! | 0x3167e0 | Ratchet not grinding → +0x7e = 3 | [`Camera::race_pre`] |
//! | 0x3159d8 | +0x20 = 2, +0x30 = +0x32 = 0; the three rails' running chord lengths (0x1f6c80 / 0x1f7c20 / 0x1f8bc0, 1000 each); D+0x88 = 1, D+0x8c..0x98 = 0, row 0's angles and distance, the springs' velocities 0; the camera at row 0's pitch and distance from Ratchet along his moby's row 1, looking 1.5 above him | [`Camera::race_init`] |
//! | 0x315d48 | the share of Ratchet's rail behind him (his rail among the three, else the third); the stage (`STAGES`) and in stage 1 his head turned back (records 3's targets y 20°, z −120°); the attack calls (`CALLS`): the bot on his rail attacks (command 1), with the second and third calls one or two others wait (command 5, +0x1c8 = 80 / 45 and 110 ticks; with B on his rail a coin, `0x260050(2)`, picks which); the row (`ROWS`, one step a tick, to its last), the angles and distance sprung to the rows lerped (0.005 / 0.01, 0.2), the facing override eased in and out (0.05, smoothstep); the camera at (his row-1 yaw + yaw, pitch, distance) from him looking 1.5 above him, tilted about left and up | [`Camera::race_update`] |
//!
//! Native `f32`. The gp words 0x162490..0x1624c0 (the rates; the progress and row copies, which nothing reads) are the
//! constants below.

use super::class_cam::{rows_about, set_len};
use super::level::Best;
use super::script::{angle_spring, cos_interp, hermite, sph_point, spring, wrap};
use super::{fadd, fcross, frot, fscale, fsub, CamInput, Camera};
use crate::hero::physics::to_f32x3;
use crate::spline::{advance, nearest, Cursor};

/// The camera classes.
pub const CLASS_INTRO: i32 = 20;
pub const CLASS_RACE: i32 = 21;
/// Level 14's functions ([`super::level::CameraPorts::from_overlays`]).
pub const INTRO_FNS: [u32; 4] = [0x31_5920, 0x31_4f00, 0x31_5290, 0x31_59d0];
pub const INTRO_HELPERS: [u32; 2] = [0x31_4e20, 0x2d_6570];
pub const RACE_FNS: [u32; 4] = [0x31_6748, 0x31_59d8, 0x31_5d48, 0x31_67e0];

const fn f(b: u32) -> f32 { f32::from_bits(b) }
const DEG: f32 = 0.017_453_292;
/// gp−0x4770 / −0x476c: the yaw and pitch springs' stiffness; −0x4768 / −0x4764 / −0x4760 the distance's and tilts'.
const ANGLE_K: f32 = f(0x3ba3_d70a);
const DIST_K: f32 = f(0x3c23_d70a);
/// The springs' damping.
const DAMP: f32 = f(0x3e4c_cccd);
/// gp−0x475c: the facing override's rate.
const FACE_RATE: f32 = f(0x3d4c_cccd);
/// gp−0x4750 / −0x4754: stage 1's head turn (degrees, y and z).
const LOOK_Y: f32 = 20.0;
const LOOK_Z: f32 = -120.0;
/// The intro's fade-out rate at its end (0x1675d8).
const FADE_OUT: f32 = 0.25;

/// One row of the race camera's table (0x1f6488, stride 0x30): yaw, pitch (degrees), distance, the two tilts
/// (degrees), a word (rows from 28 on: 10000, the end), the facing override by rail (radians, 0: none) and the share
/// of the rail where the row starts, by rail.
#[derive(Clone, Copy, Debug)]
struct Row {
    yaw: f32,
    pitch: f32,
    dist: f32,
    tilt: [f32; 2],
    word: i32,
    face: [f32; 3],
    at: [f32; 3],
}

#[rustfmt::skip]
const ROWS: [Row; 31] = [
    Row { yaw: 0.0, pitch: f(0x41200000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 0, face: [0.0, 0.0, 0.0], at: [0.0, 0.0, 0.0] },
    Row { yaw: 0.0, pitch: f(0x41700000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 60, face: [0.0, 0.0, 0.0], at: [f(0x3dd69e3d), f(0x3dead03e), f(0x3df7a030)] },
    Row { yaw: 0.0, pitch: f(0x41c80000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 120, face: [0.0, 0.0, 0.0], at: [f(0x3e0ebbfd), f(0x3e19624a), f(0x3e1f073a)] },
    Row { yaw: 0.0, pitch: f(0x420c0000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 180, face: [0.0, 0.0, 0.0], at: [f(0x3e323e9f), f(0x3e3d45e9), f(0x3e41cc10)] },
    Row { yaw: 0.0, pitch: f(0x420c0000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 240, face: [0.0, 0.0, 0.0], at: [f(0x3e57aede), f(0x3e626677), f(0x3e651d26)] },
    Row { yaw: 0.0, pitch: f(0x42200000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 315, face: [0.0, 0.0, 0.0], at: [f(0x3e803e42), f(0x3e858eeb), f(0x3e86748e)] },
    Row { yaw: 0.0, pitch: f(0x428c0000), dist: f(0x41100000), tilt: [f(0xc1200000), 0.0], word: 460, face: [0.0, 0.0, 0.0], at: [f(0x3e9e5626), f(0x3ea3a5e3), f(0x3ea3647c)] },
    Row { yaw: 0.0, pitch: f(0x428c0000), dist: f(0x41200000), tilt: [f(0xc1200000), 0.0], word: 490, face: [0.0, 0.0, 0.0], at: [f(0x3ea4a1ad), f(0x3ea9ea14), f(0x3ea97aee)] },
    Row { yaw: 0.0, pitch: f(0x41a00000), dist: f(0x41300000), tilt: [f(0xc1200000), 0.0], word: 550, face: [0.0, 0.0, 0.0], at: [f(0x3eb4cf8d), f(0x3eba5bfb), f(0x3eb9b131)] },
    Row { yaw: 0.0, pitch: f(0x41a00000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 700, face: [0.0, 0.0, 0.0], at: [f(0x3ee78940), f(0x3eed1c9f), f(0x3eeaa23c)] },
    Row { yaw: f(0xc2340000), pitch: f(0x41a00000), dist: f(0x40c00000), tilt: [0.0, 0.0], word: 745, face: [0.0, 0.0, 0.0], at: [f(0x3ef49c6f), f(0x3ef9a6b5), f(0x3ef6596d)] },
    Row { yaw: f(0xc2b40000), pitch: f(0x41a00000), dist: f(0x41000000), tilt: [0.0, 0.0], word: 790, face: [0.0, 0.0, 0.0], at: [f(0x3f0024b3), f(0x3f027558), f(0x3f00382a)] },
    Row { yaw: f(0xc3070000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 835, face: [0.0, 0.0, 0.0], at: [f(0x3f05ab9f), f(0x3f079757), f(0x3f04d2c4)] },
    Row { yaw: f(0xc3070000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 880, face: [0.0, 0.0, 0.0], at: [f(0x3f0af178), f(0x3f0cafb4), f(0x3f098ce3)] },
    Row { yaw: f(0xc3070000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 960, face: [0.0, 0.0, 0.0], at: [f(0x3f143276), f(0x3f15be6e), f(0x3f12071c)] },
    Row { yaw: f(0xc3340000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 1034, face: [0.0, 0.0, 0.0], at: [f(0x3f1758e2), f(0x3f187fcc), f(0x3f1460aa)] },
    Row { yaw: f(0xc3340000), pitch: f(0x42480000), dist: f(0x41900000), tilt: [0.0, 0.0], word: 1064, face: [f(0xbfa66666), 0.0, f(0xbffc28f6)], at: [f(0x3f1b4396), f(0x3f1c154d), f(0x3f176c8b)] },
    Row { yaw: f(0xc3340000), pitch: f(0x42480000), dist: f(0x41900000), tilt: [0.0, 0.0], word: 1200, face: [f(0xbfa66666), 0.0, f(0xbffc28f6)], at: [f(0x3f2f06f7), f(0x3f2c5d64), f(0x3f27ced9)] },
    Row { yaw: f(0xc3340000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 1230, face: [0.0, 0.0, 0.0], at: [f(0x3f333333), f(0x3f30a3d7), f(0x3f2bfb16)] },
    Row { yaw: f(0xc3340000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 1320, face: [0.0, 0.0, 0.0], at: [f(0x3f425461), f(0x3f404189), f(0x3f3b851f)] },
    Row { yaw: f(0xc3340000), pitch: f(0x42200000), dist: f(0x41800000), tilt: [0.0, 0.0], word: 1350, face: [0.0, 0.0, f(0xbfe66666)], at: [f(0x3f45119d), f(0x3f432618), f(0x3f3e703b)] },
    Row { yaw: f(0x43200000), pitch: f(0x42200000), dist: f(0x41800000), tilt: [0.0, 0.0], word: 1490, face: [0.0, 0.0, f(0xbfe66666)], at: [f(0x3f4d844d), f(0x3f4c0ebf), f(0x3f49e1b1)] },
    Row { yaw: f(0x43200000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 1520, face: [0.0, 0.0, 0.0], at: [f(0x3f51a9fc), f(0x3f5075f7), f(0x3f4e76c9)] },
    Row { yaw: f(0x43340000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 1600, face: [0.0, 0.0, 0.0], at: [f(0x3f5e86f9), f(0x3f5fc600), f(0x3f5fcd79)] },
    Row { yaw: f(0x43340000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 1680, face: [0.0, 0.0, 0.0], at: [f(0x3f62339c), f(0x3f63a29c), f(0x3f63c9ef)] },
    Row { yaw: f(0x43340000), pitch: f(0x42200000), dist: f(0x41800000), tilt: [0.0, 0.0], word: 1710, face: [f(0xc0200000), 0.0, 0.0], at: [f(0x3f648e8a), f(0x3f65cfab), f(0x3f65f6fd)] },
    Row { yaw: f(0x43340000), pitch: f(0x42200000), dist: f(0x41800000), tilt: [0.0, 0.0], word: 1800, face: [f(0xc0200000), 0.0, 0.0], at: [f(0x3f6f34d7), f(0x3f6ebee0), f(0x3f6edfa4)] },
    Row { yaw: f(0x43340000), pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 1830, face: [0.0, 0.0, 0.0], at: [f(0x3f71bda5), f(0x3f715b57), f(0x3f71758e)] },
    Row { yaw: 0.0, pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 10000, face: [0.0, 0.0, 0.0], at: [0.0, 0.0, 0.0] },
    Row { yaw: 0.0, pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 10000, face: [0.0, 0.0, 0.0], at: [0.0, 0.0, 0.0] },
    Row { yaw: 0.0, pitch: f(0x41a00000), dist: f(0x41100000), tilt: [0.0, 0.0], word: 10000, face: [0.0, 0.0, 0.0], at: [0.0, 0.0, 0.0] },
];

/// The share of the rail (by rail) where the attack calls 0..2 are made (0x1f6c08).
#[rustfmt::skip]
const CALLS: [[f32; 3]; 3] = [[f(0x3e0c63f1), f(0x3e170a3d), f(0x3e1cc63f)], [f(0x3e762b6b), f(0x3e8068dc), f(0x3e8154ca)], [f(0x3eb182aa), f(0x3eb70a3d), f(0x3eb66cf4)]];
/// The share of the rail (by rail) where the stages 0 → 1 → 2 begin (0x1f6c38).
#[rustfmt::skip]
const STAGES: [[f32; 3]; 2] = [[f(0x3dcccccd), f(0x3de0ded3), f(0x3dedc5d6)], [f(0x3e0d013b), f(0x3e17a787), f(0x3e1d4952)]];

/// Row `i`: past the data, the zero rows the init marks as the end.
fn row(i: i32) -> Row {
    const END: Row = Row { yaw: 0.0, pitch: 0.0, dist: 0.0, tilt: [0.0; 2], word: 10_000, face: [0.0; 3], at: [0.0; 3] };
    usize::try_from(i).ok().and_then(|i| ROWS.get(i)).copied().unwrap_or(END)
}

/// `0x263ad0(a, b, t)`: `a + wrap(b − a)·t`, wrapped.
fn lerp_rot(a: f32, b: f32, t: f32) -> f32 { wrap(a + wrap(b - a) * t) }

/// Class 20's words past class 19's (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IntroRec {
    pub moby: i32,
    pub cuboid: i32,
}

impl IntroRec {
    pub fn parse(p: &[u8]) -> Option<IntroRec> {
        let i = |o: usize| p.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        Some(IntroRec { moby: i(0x50)?, cuboid: i(0x54)? })
    }
}

/// Class 21's record (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaceRec {
    pub state: i32,
    pub bots: [i32; 3],
    pub calls: i16,
    pub stage: i16,
    pub rails: [i32; 3],
}

impl RaceRec {
    pub fn parse(p: &[u8]) -> Option<RaceRec> {
        let i = |o: usize| p.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let h = |o: usize| p.get(o..o + 2).map(|b| i16::from_le_bytes([b[0], b[1]]));
        Some(RaceRec {
            state: i(0x20)?,
            bots: [i(0x24)?, i(0x28)?, i(0x2c)?],
            calls: h(0x30)?,
            stage: h(0x32)?,
            rails: [i(0x34)?, i(0x38)?, i(0x3c)?],
        })
    }
}

/// What the camera reads of a race moby (the tick gives it, [`super::CamWorld::race`]): a rail bot's rail (pvar +0x60,
/// a grind path) and its marks path's first point (pvar +0x88).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaceMoby {
    pub rail: i32,
    pub mark0: Option<[f32; 3]>,
}

/// Class 21's D words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceState {
    /// D+0x60 / +0x64 / +0x68: yaw (off Ratchet's row 1), pitch, distance; D+0x6c / +0x70 the tilts.
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub tilt: [f32; 2],
    /// D+0x74..0x84: the five springs' velocities.
    pub vel: [f32; 5],
    /// D+0x88: ticks; D+0x8c: the row.
    pub ticks: i32,
    pub row: i32,
    /// D+0x90 / +0x94: the facing override of the row and of the next; D+0x98 its weight.
    pub face: [f32; 2],
    pub face_t: f32,
    /// gp−0x4740: the share of Ratchet's rail behind him.
    pub progress: f32,
    /// 0x1f6c80 / 0x1f7c20 / 0x1f8bc0: each race rail's chord lengths summed up to each point (1000 entries; the last
    /// point's entry is the length).
    pub run: [Vec<f32>; 3],
}

/// Ratchet's `HeroTeleport` by a race camera.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeroPlace {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub state: i32,
}

/// The race start `0x2d6570` on the race moby `moby`, with Ratchet's place on his rail when a member rides it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaceStart {
    pub moby: usize,
    pub hero: Option<(Cursor, HeroPlace)>,
}

/// An attack call of class 21: the calls made before it, which bot (0..2: A, B, C) is on Ratchet's rail, the bots.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BotCall {
    pub calls: i16,
    pub on: u8,
    pub bots: [Option<usize>; 3],
}

/// The race cameras' stores this tick (module doc), applied before the next moby loop
/// (`crate::moby_update::classes::units::oltanis_rail_bot::camera_stores`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaceOut {
    pub teleport: Option<HeroPlace>,
    pub start: Option<RaceStart>,
    pub call: Option<BotCall>,
    /// Ratchet's head record (3) targets y / z (0x17aff4 / 0x17aff8).
    pub head: Option<[f32; 2]>,
}

impl RaceOut {
    pub fn is_empty(&self) -> bool { *self == RaceOut::default() }
}

/// The running chord lengths of grind path `i` (`0x3159d8`'s loops: entry k = the chords before point k, the last
/// point's entry the length; 1000 entries).
fn run_lengths(v: Option<&rc_formats::volumes::Volumes>, i: i32) -> Vec<f32> {
    let pts = usize::try_from(i).ok().and_then(|i| v.and_then(|v| v.grind_paths.get(i))).map(|g| g.points.as_slice()).unwrap_or(&[]);
    let n = pts.len();
    let mut out = vec![0.0; n.max(1000)];
    let mut acc = 0.0;
    let last = n.saturating_sub(1);
    for (k, q) in pts.iter().take(last).enumerate() {
        out[k] = acc;
        acc += q[3];
    }
    out[last] = acc;
    out
}

impl Camera {
    fn intro_rec(&self, slot: usize) -> Option<IntroRec> { self.level_cams.slots.get(slot).and_then(|s| s.intro) }
    fn race_rec(&self, slot: usize) -> Option<RaceRec> { self.level_cams.slots.get(slot).and_then(|s| s.race) }
    fn race_rec_mut(&mut self, slot: usize) -> Option<&mut RaceRec> { self.level_cams.slots.get_mut(slot).and_then(|s| s.race.as_mut()) }

    /// Ratchet's rail (0x13f8b0) as the level loaded it.
    fn hero_rail(&self, inp: &CamInput) -> Option<Vec<[f32; 4]>> {
        let i = inp.hero.boots.rail?;
        self.level_cams.shapes()?.grind_paths.get(i).map(|g| g.points.clone())
    }

    /// `0x315920` for slot `i` (module doc).
    pub(super) fn intro_hook(&self, i: usize, best: Best, inp: &CamInput) -> i32 {
        let Some(r) = self.flyby_rec(i) else { return 0 };
        if r.armed != 0 { return -1; }
        let h = inp.hero;
        if !best.releasing && h.group != 0xf { return 0; }
        if !self.flyby_region(i, inp) { return 0; }
        if 0 <= r.group && h.group != r.group { return 0; }
        if 0 <= r.state && h.state != r.state { return 0; }
        1
    }

    /// `0x316748` for slot `i` (module doc).
    pub(super) fn race_hook(&mut self, i: usize, best: Best) -> i32 {
        let intro = self.current_class() == CLASS_INTRO;
        let Some(r) = self.race_rec_mut(i) else { return 0 };
        match r.state {
            0 => {
                if intro { r.state = 1; }
                0
            }
            1 if !intro || best.releasing => 1,
            _ => 0,
        }
    }

    /// `0x314f00` (module doc).
    pub(super) fn intro_init(&mut self, prev: ([[f32; 3]; 3], [f32; 3])) {
        if self.flyby_begin(prev, 3, false).is_none() { return; }
        let Some(ir) = self.intro_rec(self.class_cam.slot) else { return };
        let centre = usize::try_from(ir.cuboid).ok().and_then(|c| self.level_cams.shapes().and_then(|v| v.cuboids.get(c))).map(|c| c.centre());
        if let Some(pos) = centre { self.flyby_out.race.teleport = Some(HeroPlace { pos, yaw: 0.0, state: 0x72 }); }
    }

    /// `0x315290` (module doc).
    pub(super) fn intro_update(&mut self, inp: &CamInput) {
        let slot = self.class_cam.slot;
        let Some(r) = self.flyby_rec(slot) else { return };
        if self.class_cam.flyby.phase == 0 {
            self.flyby_out.letterbox = Some(true);
            let st = &mut self.class_cam.flyby;
            st.phase = 1;
            let start = st.cam_path.first().map_or(self.class_cam.pos, |q| [q[0], q[1], q[2]]);
            let look0 = st.look_path.first().map_or(start, |q| [q[0], q[1], q[2]]);
            self.class_cam.pos = start;
            let look = self.flyby_look(&r, look0);
            self.flyby_face(look, inp);
            return;
        }
        let old = self.class_cam.pos;
        let dist = r.speed * crate::hero::physics::DT.to_f32();
        let st = &mut self.class_cam.flyby;
        let (p, mut ended) = advance(&st.cam_path, false, dist, &mut st.cursor);
        self.class_cam.pos = p;
        if st.timer != 0 {
            let mut t = st.timer as i32;
            if crate::moby_update::creature::dec_timer_i32(&mut t) != 0 { ended = true; }
            st.timer = t as i16;
        }
        if r.done != 0 { ended = true; }
        if !ended {
            // The FOV between its keys, eased.
            let st = &self.class_cam.flyby;
            let seg = st.cursor.seg;
            for k in 0..7 {
                let (a, b) = (st.keys[k], st.keys[k + 1]);
                if a < 0 || b < 0 { break; }
                let (a, b) = (a as i32, b as i32);
                if !(a <= seg && seg < b) { continue; }
                let chord = |j: i32| st.cam_path.get(j as usize).map_or(0.0, |q| q[3]);
                let before: f32 = (a..seg).map(chord).sum();
                let at = before + st.cursor.t;
                let span = before + (seg..b).map(chord).sum::<f32>();
                let deg = cos_interp(st.degrees[k], st.degrees[k + 1], at / span);
                let half = deg * DEG * 0.5;
                self.flyby_out.tan = Some(half.sin() / half.cos());
            }
            let d = ((p[0] - old[0]).powi(2) + (p[1] - old[1]).powi(2) + (p[2] - old[2]).powi(2)).sqrt();
            let st = &mut self.class_cam.flyby;
            st.flown += d;
            let look = if r.moby < 0 {
                let share = st.flown / st.cam_len * st.look_len;
                let mut c = Cursor::default();
                advance(&st.look_path, false, share, &mut c).0
            } else {
                self.flyby_look(&r, p)
            };
            self.flyby_face(look, inp);
            return;
        }
        // The end: the FOV back, the race started, the cut to the race camera.
        let half = self.class_cam.flyby.base_fov * 0.5;
        self.flyby_out.tan = Some(half.sin() / half.cos());
        self.flyby_out.fade_rate = Some(FADE_OUT);
        self.class_cam.release = 4;
        self.flyby_out.letterbox = Some(false);
        let start = self.intro_rec(slot).and_then(|ir| usize::try_from(ir.moby).ok()).map(|m| self.race_start(m, inp));
        self.flyby_out.race.start = start;
        let cur = start.and_then(|s| s.hero).map_or(inp.hero.boots.cur, |(c, _)| c);
        let Some(rail) = self.hero_rail(inp) else { return };
        let (mut a, mut b) = (cur, cur);
        let (mut pos, _) = advance(&rail, false, 10.0, &mut a);
        pos[2] += 2.0;
        self.class_cam.pos = pos;
        let (mut look, _) = advance(&rail, false, 0.0, &mut b);
        look[2] += 1.5;
        self.flyby_face(look, inp);
    }

    /// `0x2d6570`'s Ratchet part (module doc): his place on his rail by the last member of the race moby's group that
    /// rides it.
    fn race_start(&self, moby: usize, inp: &CamInput) -> RaceStart {
        let mut hero = None;
        let ids = self.world.race_groups.get(&moby).cloned().unwrap_or_default();
        if let (Some(pts), Some(own)) = (self.hero_rail(inp), inp.hero.boots.rail) {
            for id in ids {
                let Some(m) = self.world.race.get(&id) else { continue };
                if usize::try_from(m.rail).ok() != Some(own) { continue; }
                let mark = m.mark0.unwrap_or_default();
                let seg = nearest(&pts, false, 20.0, 5.0, 0.0, mark).map_or(0, |(_, c)| c.seg).max(0) as usize;
                let (p, q) = (pts[seg.min(pts.len() - 1)], pts[(seg + 3).min(pts.len() - 1)]);
                let yaw = wrap(crate::moby_update::creature::atan(q[0] - p[0], q[1] - p[1]) - std::f32::consts::FRAC_PI_2);
                let place = HeroPlace { pos: [p[0], p[1], p[2]], yaw, state: 0x28 };
                hero = Some((Cursor { seg: seg as i32, t: 0.0 }, place));
            }
        }
        RaceStart { moby, hero }
    }

    /// The rows from the camera at `pos` looking 1.5 above Ratchet about −gravity, tilted (`tilt`: about left, then
    /// about up).
    fn race_face(&mut self, hero: [f32; 3], tilt: [f32; 2], inp: &CamInput) {
        let up = fscale(to_f32x3(inp.hero.gravity_dir), -1.5);
        let mut fwd = set_len(fadd(fsub(hero, self.class_cam.pos), up), 1.0);
        if tilt[0] != 0.0 {
            let left = fcross(up, fwd);
            fwd = set_len(frot(fwd, tilt[0], left), 1.0);
        }
        if tilt[1] != 0.0 { fwd = set_len(frot(fwd, tilt[1], up), 1.0); }
        self.class_cam.rows = rows_about(fwd, up);
    }

    /// The yaw of Ratchet's moby row 1 (`FastArcTan(+0xd0, +0xd4)`).
    fn row1_yaw(inp: &CamInput) -> f32 {
        let r = to_f32x3(inp.hero.moby_rows[1]);
        crate::moby_update::creature::atan(r[0], r[1])
    }

    /// `0x3159d8` (module doc).
    pub(super) fn race_init(&mut self, inp: &CamInput) {
        let slot = self.class_cam.slot;
        let Some(r) = self.race_rec_mut(slot) else { return };
        r.state = 2;
        r.calls = 0;
        r.stage = 0;
        let rails = r.rails;
        let vols = self.level_cams.shapes_arc();
        let r0 = row(0);
        self.class_cam.race = RaceState {
            yaw: r0.yaw * DEG,
            pitch: r0.pitch * DEG,
            dist: r0.dist,
            tilt: [r0.tilt[0] * DEG, r0.tilt[1] * DEG],
            ticks: 1,
            run: rails.map(|i| run_lengths(vols.as_deref(), i)),
            ..RaceState::default()
        };
        let hero = to_f32x3(inp.hero.pos);
        let st = &self.class_cam.race;
        self.class_cam.pos = sph_point([Self::row1_yaw(inp), st.pitch, st.dist], hero);
        self.race_face(hero, [0.0; 2], inp);
    }

    /// `0x3167e0`: Ratchet off the rail lets the race camera go (+0x7e = 3).
    pub(super) fn race_pre(&mut self, inp: &CamInput) {
        if inp.hero.group != 0xf { self.class_cam.release = 3; }
    }

    /// `0x315d48` (module doc).
    pub(super) fn race_update(&mut self, inp: &CamInput) {
        let slot = self.class_cam.slot;
        let Some(mut r) = self.race_rec(slot) else { return };
        let h = inp.hero;
        let own = h.boots.rail.and_then(|i| i32::try_from(i).ok());
        let rail = if own == Some(r.rails[0]) { 0 } else if own == Some(r.rails[1]) { 1 } else { 2 };
        let st = &mut self.class_cam.race;
        let run = &st.run[rail];
        let behind = usize::try_from(h.boots.cur.seg).ok().and_then(|s| run.get(s)).copied().unwrap_or(0.0);
        let progress = (behind + h.boots.cur.t) / run.get(999).copied().unwrap_or(0.0);
        st.progress = progress;
        // The stage; in stage 1 Ratchet looks back.
        if r.stage < 2 && STAGES[r.stage as usize][rail] <= progress { r.stage += 1; }
        if r.stage == 1 { self.flyby_out.race.head = Some([LOOK_Y * DEG, LOOK_Z * DEG]); }
        // The attack calls.
        if r.calls < 3 && CALLS[r.calls as usize][rail] <= progress {
            let bots = r.bots.map(|b| usize::try_from(b).ok());
            let on_rail = |k: usize| bots[k].and_then(|b| self.world.race.get(&b)).is_some_and(|m| Some(m.rail) == own);
            let on = if on_rail(0) { 0 } else if !on_rail(1) { 2 } else { 1 };
            self.flyby_out.race.call = Some(BotCall { calls: r.calls, on, bots });
            r.calls += 1;
        }
        if let Some(rm) = self.race_rec_mut(slot) { *rm = r; }
        // The row: one step a tick while the share passes the next row's start.
        let st = &mut self.class_cam.race;
        let next = st.row + 1;
        if !(progress < row(next).at[rail]) { st.row = next; }
        let i = st.row;
        let t = if row(i + 1).word == 10_000 {
            st.row = i - 1;
            1.0
        } else {
            let (a, b) = (row(i).at[rail], row(i + 1).at[rail]);
            (progress - a) / (b - a)
        };
        let (a, b) = (row(st.row), row(st.row + 1));
        let lr = |x: f32, y: f32| lerp_rot(x * DEG, y * DEG, t);
        st.yaw = angle_spring(st.yaw, lr(a.yaw, b.yaw), ANGLE_K, DAMP, 0.0, &mut st.vel[0]);
        st.pitch = angle_spring(st.pitch, lr(a.pitch, b.pitch), ANGLE_K, DAMP, 0.0, &mut st.vel[1]);
        st.dist = spring(st.dist, a.dist + (b.dist - a.dist) * t, DIST_K, DAMP, 0.0, &mut st.vel[2]);
        st.tilt[0] = angle_spring(st.tilt[0], lr(a.tilt[0], b.tilt[0]), DIST_K, DAMP, 0.0, &mut st.vel[3]);
        st.tilt[1] = angle_spring(st.tilt[1], lr(a.tilt[1], b.tilt[1]), DIST_K, DAMP, 0.0, &mut st.vel[4]);
        // The yaw: off Ratchet's row 1, toward a row's facing override while one is keyed.
        let base = wrap(Self::row1_yaw(inp) + st.yaw);
        let (cur, nxt) = (a.face[rail], b.face[rail]);
        let ease = |w: f32| hermite(1.0, 0.0, 1.0, 0.0, w);
        let yaw = if nxt == 0.0 && cur == 0.0 {
            let to = if st.face[1] != 0.0 { st.face[1] } else { st.face[0] };
            st.face_t += (0.0 - st.face_t) * FACE_RATE;
            lerp_rot(base, to, ease(st.face_t))
        } else {
            st.face_t += (if nxt == 0.0 { 0.0 } else { 1.0 } - st.face_t) * FACE_RATE;
            let y = lerp_rot(base, if nxt == 0.0 { cur } else { nxt }, ease(st.face_t));
            st.face = [cur, nxt];
            y
        };
        let hero = to_f32x3(h.pos);
        let (pitch, dist, tilt) = (st.pitch, st.dist, st.tilt);
        st.ticks += 1;
        self.class_cam.pos = sph_point([yaw, pitch, dist], hero);
        self.race_face(hero, tilt, inp);
    }
}
