//! The gameplay follow camera, type 0 (level01 `CameraUpdate` 0x20eca8 → `UpdateAllCameras` 0x20d620 →
//! type-0 update 0x314e00; init 0x311f38 + snap 0x311890), producing the `Camera` record the renderer uses
//! (position 0x167240, rows forward/left/up 0x167450/60/70, Euler 0x167250 with the yaw 0x167258 the hero
//! reads). Spec: `docs/plan/player_controller.md` §7 and "In the port".
//!
//! **Exactness.** Every step follows the game's op order on the PS2 FPU/VU model ([`Pf`]); the trig is the
//! game's (VU0 sine polynomial, `FastArcTan`, the FPU `asin` polynomial 0x221728, the quaternion rotation
//! 0x274ac8).
//!
//! **Moby collision.** Every camera line / sphere tests the mobys after the world mesh ([`CamInput::mobys`]; the
//! flags 0x12 / 0x14 / 0x34 / 0x94 / 0x96 / 0xb4 / 4 have no bit 0x1, so the moby pass runs; 0x2 only skips the
//! primitives), with the game's `a2`: Ratchet for GetTarget 0x30f498, the snap lines 0x311890, the ceiling lines
//! 0x310a08, the sphere chain 0x312c08 / re-aim 0x3127f0 and the avoidance line; none for the snap sphere
//! (flags 4) and the raise sphere 0x3101c0; the camera moby 0x167354 for the end sphere 0x3124f0 (that moby,
//! class 0x3ef from `Camera_handleCollWithHero`, is not created by the port: none). The avoidance 0x312ef8 first
//! lists the mobys around the pivot–camera midpoint (`coll_sphere_mobys`) and switches the mesh-less ones off for
//! its queries (or nudges the offset ±1° away from the classes of `NUDGE_CLASSES`); a crate blocking its line gets
//! a hit ([`Camera::hit`]).
//!
//! **The level camera system** ([`level`]): the level's camera records as slots, the per-tick choice of the camera
//! (`UpdateAllCameras` 0x20d620 / `Camera_ActivationCheckPriority` 0x20d410) and the class-17 regions that retune this
//! camera through its setters (`0x313560`..`0x313af0`, [`Camera::set_distance`] …). The camera-collision grid
//! (`0x20fdb0`: pass-through volumes `0x30f468`, the push-out `0x30f358`) is empty on all 19 levels, so its users
//! never find a primitive (the port has no grid). The avoidance's level branches are ported (level 15 with body 2,
//! level 13 in state 0x7b).
//!
//! The level's other camera classes: the Swingshot camera ([`swing`], class 7), the rail / slide camera ([`rail`],
//! class 3), the fixed and side views ([`cuboid`], classes 1 / 14; the plumbing of a current level-class camera and
//! the switch's blends: [`class_cam`]), the placed view (class 23) and the moby focus ([`focus`], class 18); the
//! Swingshot targets' look-up hint ([`swing::LookHint`]), the scripted focus moby 0x16735c ([`Camera::set_focus_moby`])
//! and the focus scan of 0x3111d8 (+0x230 = 1) feed the follow camera; the camera moby 1007 ([`camera_moby`]) stands
//! at the follow camera. The hero-state tweaks `0x3111d8` and the target modes `0x30fb08` / `0x3101c0` are ported
//! whole (player_controller.md §15 "The hero-state tweaks", "Target modes"). Not modelled: the Euler pitch/roll
//! (0x2721f0; only the yaw feeds gameplay — pitch and roll are derived here from the rows with the same FastArcTan).
//!
//! **Camera shake** ([`Shake`], [`ShakeRequest`]): the two shake records 0x167260 (along the camera's up row) and
//! 0x167270 (along its forward row) that `CameraUpdate` applies to the published position 0x167240 after the Euler
//! (`0x20e560`, boot `fun_001ed360`). Any gameplay code requests one by storing an amplitude and a tick count
//! (the Thruster stomp 0.2 for 40 ticks, the collapsing platform 701 0.4 / 30 and 0.1 / 20, explosions, …): in the
//! port through [`Camera::request_shake`], fed by the tick from the hero's and the moby loop's requests
//! (`crate::tick`).
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern, clippy::needless_range_loop)] // FPU compare semantics and op order are spelled out on purpose.

use crate::hero::physics::{
    dot3 as dot, fast_add_rotations as add_rot, fast_cos, fast_sin, fast_subtract_rotations as sub_rot, len3 as len,
    set_len3 as norm, to_f32x3, vadd, vscale, vsub, V0, V4,
};
use crate::collision_query::{coll_line_m, coll_sphere_m, coll_sphere_mobys, CollMoby, CollOutput, MobyScene, MobySource, QueryFlags};
use crate::hero::Hero;
use crate::pad::{fast_arctan, PadState};
use crate::ps2v::Pf;
use rc_formats::collision::Collision;

pub mod camera_moby;
pub mod board;
pub mod class_cam;
pub mod cuboid;
pub mod flyby;
pub mod giant;
pub mod race;
pub mod focus;
pub mod level;
pub mod rail;
pub mod script;
pub mod swing;
pub mod type6;

const K_PI: Pf = Pf::b(0x4049_0fdb);
const HALF_PI: Pf = Pf::b(0x3fc9_0fdb);
/// Yaw rates by option 0x15ede4 (0x162228): 1.0°, 1.3°, 1.6° per tick.
pub const YAW_RATES: [Pf; 3] = [Pf::b(0x3c8e_fa35), Pf::b(0x3cb9_dede), Pf::b(0x3ce4_c388)];
const DEG40: Pf = Pf::b(0x3f32_b8c2);
const DEG15: Pf = Pf::b(0x3e86_0a92);
const DEG70: Pf = Pf::b(0x3f9c_61aa);
const DEG1_75: Pf = Pf::b(0x3cfa_35dd);

/// Camera options (boot static values; a memory-card load may change them).
#[derive(Clone, Copy, Debug)]
pub struct CameraOptions {
    /// 0x15eddc: 1 → pitch input = −ry (stick up raises the camera).
    pub pitch_normal: bool,
    /// 0x15ede0: 1 → yaw input = +rx.
    pub yaw_normal: bool,
    /// 0x15ede4: yaw-rate index into [`YAW_RATES`] (default 1 = 1.3°/tick).
    pub yaw_rate: usize,
    /// 0x15edb4: mirror (left row negated after the update).
    pub mirror: bool,
}

impl Default for CameraOptions {
    fn default() -> Self { CameraOptions { pitch_normal: true, yaw_normal: true, yaw_rate: 1, mirror: false } }
}

/// The `Camera` record the renderer and the hero read (level01 0x167240.., boot 0x187080..).
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraView {
    /// 0x167240: position.
    pub pos: V4,
    /// 0x167250: Euler (x roll, y pitch, z yaw); **yaw 0x167258 = atan2(fwd.y, fwd.x)**.
    pub euler: V4,
    /// 0x167450 / 0x167460 / 0x167470: forward, left, up.
    pub rows: [V4; 3],
    /// The current camera's class (`[0x167280]+0x86`: [`Camera::current_class`]), as this update left it.
    pub class: i32,
}

impl CameraView {
    pub fn yaw(&self) -> Pf { self.euler[2] }
    pub fn pos_f32(&self) -> [f32; 3] { to_f32x3(self.pos) }
    pub fn rows_f32(&self) -> [[f32; 3]; 3] { self.rows.map(to_f32x3) }
}

/// Camera globals G (0x167290..) written by the pre-steps.
#[derive(Clone, Copy, Debug, Default)]
pub struct CamGlobals {
    /// G+0x00: hero position with z springed (k 0.0075, d 0.175; vel G+0x10). Not read by type 0.
    pub hero_s: V4,
    pub hero_s_vel: Pf,
    /// G+0x20: smoothed up; G+0x30 / 0x40: up2 = −gravity, previous; G+0x50: up_s spring velocities.
    pub up_s: V4,
    pub up2: V4,
    pub up2_prev: V4,
    pub up_vel: [Pf; 3],
    /// G+0x60 previous hero position, G+0x70 delta, G+0x80 horizontal move direction (0x167310), G+0x90
    /// vertical part, G+0xa0 / 0xa4 (0x167334) / 0xa8: |delta|, horizontal speed, vertical speed.
    pub prev_hero: V4,
    pub delta: V4,
    pub move_dir: V4,
    pub delta_v: V4,
    pub delta_len: Pf,
    pub h_speed: Pf,
    pub v_speed: Pf,
    /// G+0xac..0xbc: hero yaw history (newest last).
    pub yaw_hist: [Pf; 5],
    /// G+0xd8 / 0xdc: ground-moby z / platform vertical delta (no moby platforms: 0).
    pub ground_z: Pf,
    pub plat_dz: Pf,
    /// 0x167498: ticks since the last camera switch.
    pub since_switch: i32,
}

/// The data block D (0x169590) plus the UpdateCam fields type 0 uses.
#[derive(Clone, Copy, Debug, Default)]
pub struct FollowCamera {
    /// UpdateCam +0x00/0x10/0x20 rows (forward, left, up), +0x30 position, +0x40 saved forward, +0x64 previous position.
    pub rows: [V4; 3],
    pub pos: V4,
    pub saved_fwd: V4,
    pub prev_pos: V4,
    /// D+0x00: desired point pivot + off (next tick's leash).
    pub desired: V4,
    /// D+0x10 (s16): leash enable.
    pub leash: i16,
    /// D+0x20 (s16): row blend timer.
    pub row_blend: i16,
    /// D+0x22 (s16).
    pub pitch_neg: i16,
    /// D+0x24/0x2c smoothed look height / vel; D+0x28/0x30 smoothed pivot height / vel; D+0x34/0x38 bias / vel.
    pub look_h_s: Pf,
    pub look_h_v: Pf,
    pub pivot_h_s: Pf,
    pub pivot_h_v: Pf,
    pub bias: Pf,
    pub bias_v: Pf,
    /// D+0x40 target T, D+0x50 vertical target, D+0x60/0x70 raise endpoints, D+0x80 look, D+0x90 pivot,
    /// D+0xa0 smoothed target S, D+0xb0/0xc0 spring velocities, D+0xd0/0xe0 smoothed look (+vel).
    pub target: V4,
    pub vtarget: V4,
    pub raise_a: V4,
    pub raise_b: V4,
    pub look: V4,
    pub pivot: V4,
    pub smooth: V4,
    pub vel_h: [Pf; 3],
    pub vel_v: [Pf; 3],
    pub look_s: V4,
    pub look_s_vel: [Pf; 3],
    /// D+0xf0 look height (1.5), 0xf4 override target, 0xf8 rate, 0xfc vel.
    pub look_h: Pf,
    pub look_h_tgt: Pf,
    pub look_h_rate: Pf,
    pub look_h_vel: Pf,
    /// D+0x104 / 0x105 target mode / previous; 0x106 (s16) / 0x108 blend timer / 1/length.
    pub mode: u8,
    pub mode_prev: u8,
    pub mode_t: i16,
    pub mode_inv: Pf,
    /// D+0x10c / 0x110 / 0x114 raise lerp 1/length, height, timer; 0x116 / 0x117 flags; 0x118 look blend; 0x11a.
    pub raise_inv: Pf,
    pub raise_h: Pf,
    pub raise_t: i16,
    pub look_from_s: u8,
    pub raise_frozen: u8,
    pub look_blend: i16,
    pub look_h_ovr: i16,
    /// D+0x11c/0x120 horizontal k, d; 0x124/0x128 vertical k, d.
    pub kh: Pf,
    pub dh: Pf,
    pub kv: Pf,
    pub dv: Pf,
    /// D+0x130 desired offset, D+0x140 placed offset.
    pub off: V4,
    pub placed: V4,
    /// D+0x150 / 0x154 / 0x158 placement velocities (yaw, elevation, distance).
    pub yaw_v: Pf,
    pub el_v: Pf,
    pub dist_v: Pf,
    /// D+0x15c distance (4.64), 0x160 pivot height (2.0), 0x164 / 0x168 yaw / pitch step.
    pub dist: Pf,
    pub pivot_h: Pf,
    pub yaw_step: Pf,
    pub pitch_step: Pf,
    /// D+0x16c/0x170/0x174/0x178 distance override flag/target/vel/rate; 0x16e/0x17c/0x180/0x184 pivot height.
    pub dist_ovr: i16,
    pub dist_tgt: Pf,
    pub dist_ovr_v: Pf,
    pub dist_rate: Pf,
    pub ph_ovr: i16,
    pub ph_tgt: Pf,
    pub ph_v: Pf,
    pub ph_rate: Pf,
    /// D+0x188 base distance.
    pub base_dist: Pf,
    /// D+0x1a8/0x1ac smoothed yaw input; 0x1b0/0x1b4 normalised pitch; 0x1b8 script flags; 0x1bc yaw rate;
    /// 0x1c4 scripted yaw; 0x1c8 platform yaw; 0x1cc scripted pitch.
    pub yaw_in: Pf,
    pub yaw_in_v: Pf,
    pub pitch_n: Pf,
    pub pitch_n_v: Pf,
    pub script: u32,
    pub yaw_rate: Pf,
    pub script_yaw: Pf,
    pub plat_yaw: Pf,
    pub script_pitch: Pf,
    /// D+0x1f0 end-sphere candidate; 0x200 distance reduction; 0x204 (s16) recovery timer; 0x208 (s16) end
    /// hit class; 0x20a (s16) fast recovery; 0x20c / 0x210 / 0x214 sphere scale / chain count+1 / radius step;
    /// 0x218 end flags; 0x21c blocked ticks.
    pub end_cand: V4,
    pub red: Pf,
    pub rec_t: i16,
    pub end_class: i16,
    pub fast_rec: i16,
    pub sph_scale: Pf,
    pub sph_count: Pf,
    pub sph_step: Pf,
    pub end_flags: u32,
    pub blocked: i32,
    /// D+0x224 (s16) run-toward timer, 0x226 lock, 0x228 pull-back distance, 0x22c pull-back max (6.0).
    pub toward_t: i16,
    pub toward_lock: i16,
    pub pull: Pf,
    pub pull_max: Pf,
}

/// The whole camera state: globals, the type-0 camera and the defaults snapshot D0 (0x169a90).
#[derive(Clone, Debug, Default)]
pub struct Camera {
    pub g: CamGlobals,
    pub cam: FollowCamera,
    /// D0: copy of D right after init (`BackupCurrentCam`).
    pub d0: FollowCamera,
    pub out: CameraView,
    pub opts: CameraOptions,
    /// Line / avoidance / end-sphere flags (0x15ef58 / 0x15ef5c / 0x15ef60).
    pub line_flags: u32,
    pub sph_flags: u32,
    pub end_flags: u32,
    /// Number of 30-tick blocked resets so far (diagnostics).
    pub resets: u32,
    /// The crate the blocked camera line hit during the last update (0x312ef8): the moby and the hit
    /// template's direction. The tick delivers it (FUN_0026e968).
    pub hit: Option<(usize, V4)>,
    /// The shake records 0x167260 (along up) and 0x167270 (along forward), [`ShakeAxis`] order.
    pub shake: [Shake; 2],
    /// The first-person camera (type 4, [`FirstPerson`]).
    pub first_person: FirstPerson,
    /// The camera switch blend 0x167370.. ([`CamBlend`]).
    pub blend: CamBlend,
    /// The script camera (type 5, [`script::ScriptCamera`]): `CameraScript` / `CameraScript2`.
    pub script: script::ScriptCamera,
    /// The type-6 camera ([`type6::Type6`]: the Visibomb's missile view).
    pub type6: type6::Type6,
    /// The level's camera slots, the class-17 regions and the follow camera's lock words ([`level::LevelCameras`]).
    pub level_cams: level::LevelCameras,
    /// The Swingshot camera (class 7, [`swing::SwingCamera`]).
    pub swing: swing::SwingCamera,
    /// The Swingshot targets' look-up hint for the follow camera ([`swing::LookHint`]).
    pub hint: swing::LookHint,
    /// What the tick feeds the camera from the moby world each tick ([`CamWorld`]).
    pub world: CamWorld,
    /// The current level-class camera (classes 3, 1, 14: [`class_cam::ClassCam`]).
    pub class_cam: class_cam::ClassCam,
    /// 0x16735c: the scripted focus moby the follow camera turns toward (`0x3111d8`; written by the classes that stage
    /// a look, [`Camera::set_focus_moby`]: units 1422, 1470, 1051, not ported) and 0x167360, the ticks since the right
    /// stick last moved (the turn eases in over 400).
    pub focus_moby: Option<usize>,
    pub focus_ticks: i32,
    /// 0x167354: the camera moby (class 1007, [`camera_moby`]) and this tick's request to the moby world for it.
    pub cam_moby: Option<usize>,
    pub cam_moby_call: Option<camera_moby::Call>,
    /// The engine words the level cameras read (the fade 0x15f3fc, the view's tangent 0x16cf70), given by the tick, and
    /// those class 19 writes this tick ([`flyby`]).
    pub engine: flyby::CamEngine,
    pub flyby_out: flyby::FlybyOut,
}

/// The moby-world facts the camera code reads beyond the collision queries, fed by the tick before each camera
/// update (`crate::tick`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CamWorld {
    /// While Ratchet swings (0x2c): the swung-on target's moby group (the Swingshot camera's look, `0x3182c8`).
    pub swing_group: Option<swing::SwingGroup>,
    /// The mobys whose target record (`FUN_002711f8`: mode 0x20, the pvar record) has its byte +0x0d set, Ratchet
    /// excluded: the candidates of the follow camera's focus scan (`0x3111d8`, D+0x230 = 1).
    pub focus: Vec<usize>,
    /// The mobys the class-18 regions name (their moby +0x28, their group's members): state and position.
    pub mobys: std::collections::BTreeMap<usize, focus::CamMoby>,
    /// The class-18 regions' moby groups (`0x1abcc0[g]`, list order).
    pub groups: std::collections::BTreeMap<i32, Vec<usize>>,
    /// The race cameras' rail bots (classes 20 / 21, [`race`]): their rails and marks.
    pub race: std::collections::BTreeMap<usize, race::RaceMoby>,
    /// Class 20's race mobys' groups (list order).
    pub race_groups: std::collections::BTreeMap<usize, Vec<usize>>,
}

/// Which shake record a request writes: 0x167260 moves the camera along its up row, 0x167270 along its forward row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShakeAxis {
    Up = 0,
    Forward = 1,
}

/// A camera shake request: the two stores every writer makes (`record+0 = amplitude`, `record+8 = ticks`); the
/// last request of a tick wins, as the stores do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShakeRequest {
    pub axis: ShakeAxis,
    pub amp: f32,
    pub ticks: i32,
}

/// One shake record (0x167260 / 0x167270): `+0` amplitude, `+4` the offset applied this tick, `+8` the timer, `+0xc`
/// the timer's largest value since it last ran out (the envelope's length).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shake {
    pub amp: Pf,
    pub offset: Pf,
    pub timer: i32,
    pub max: i32,
}

impl Shake {
    /// `0x20e560(record, axis)`: while the timer runs, `max = max(max, timer)`, `FastDecTimer(timer)`, then with
    /// `f = timer / max` the offset is `amp·cos(NormalizeAngle(2·timer))·f·f` (a ±amp buzz with a period of π ticks
    /// fading out quadratically), and `pos += setlen(row, offset)`; at 0 the envelope resets (`max = 0`).
    pub fn step(&mut self, pos: &mut V4, row: V4) {
        if self.timer == 0 {
            self.max = 0;
            return;
        }
        if self.max < self.timer { self.max = self.timer; }
        // FastDecTimer__FRi 0x220e78.
        self.timer = (self.timer.max(1) - 1).max(0);
        let f = i2f(self.timer) / i2f(self.max);
        let t = i2f(self.timer);
        let a = crate::moby_update::services::normalize_angle(t + t);
        self.offset = ((self.amp * fast_cos(a)) * f) * f;
        let d = norm(row, self.offset);
        pos[0] = pos[0] + d[0];
        pos[1] = pos[1] + d[1];
        pos[2] = pos[2] + d[2];
    }
}

// ------------------------------------------------------------------------------------------------
// Math (spec §0).

/// `vlerp(a, b, t)` 0x2211e8: `a + (b − a)·t`.
fn vlerp(a: V4, b: V4, t: Pf) -> V4 { vadd(a, vscale(vsub(b, a), t)) }
/// `FastVecCross(out, a, b)` 0x2212d0 = **b × a** (`o.x = b.y·a.z − a.y·b.z`, …).
fn cross(a: V4, b: V4) -> V4 {
    [b[1] * a[2] - a[1] * b[2], b[2] * a[0] - a[2] * b[0], b[0] * a[1] - a[0] * b[1], Pf::ZERO]
}
/// Standard `a × b` (`vopmula a,b; vopmsub b,a`).
fn cross_std(a: V4, b: V4) -> V4 {
    [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], Pf::ZERO]
}
/// 0x221398: 2-D distance.
fn dist2d(a: V4, b: V4) -> Pf { crate::hero::physics::dist2(a, b) }
fn i2f(i: i32) -> Pf { Pf::from_i32(i) }

/// `Cam_InterpValues(cur, tgt, k, d, max, *vel)` 0x20ce40.
pub fn interp(cur: Pf, tgt: Pf, k: Pf, d: Pf, max: Pf, vel: &mut Pf) -> Pf {
    let e = tgt - cur;
    let a = k * e;
    let b = d * *vel;
    *vel = *vel + (a - b);
    let v = *vel;
    if max != Pf::ZERO {
        if max < v { *vel = max; } else if v < -max { *vel = -max; }
    }
    let ae = e.abs();
    let v = *vel;
    if ae < v { *vel = ae; } else if v < -ae { *vel = -ae; }
    cur + *vel
}

/// `AngInterp` 0x20cf28: the same on wrapped angles.
pub fn ang_interp(cur: Pf, tgt: Pf, k: Pf, d: Pf, max: Pf, vel: &mut Pf) -> Pf {
    let e = sub_rot(tgt, cur);
    let a = k * e;
    let b = d * *vel;
    *vel = *vel + (a - b);
    let v = *vel;
    if max != Pf::ZERO {
        if max < v { *vel = max; } else if v < -max { *vel = -max; }
    }
    let ae = e.abs();
    let v = *vel;
    if ae < v { *vel = ae; } else if v < -ae { *vel = -ae; }
    add_rot(cur, *vel)
}

/// `asin` 0x221728 (FPU polynomial with the VU square root).
pub fn asin(x: Pf) -> Pf {
    const C: [Pf; 4] = [Pf::b(0x3fc9_0da4), Pf::b(0xbe59_3484), Pf::b(0x3d98_1627), Pf::b(0xbc99_6e30)];
    let (mut x, mut sgn) = (x, Pf::ONE);
    if x < Pf::ZERO { x = x.abs(); sgn = Pf::b(0xbf80_0000); }
    let s = Pf::ZERO + (Pf::ONE - x).sqrt();
    let xx = x * x;
    let acc = C[0] + Pf::ZERO;
    let acc = acc + x * C[1];
    let acc = acc + xx * C[2];
    let p = acc + (x * xx) * C[3];
    sgn * (HALF_PI - p * s)
}

/// Quaternion product 0x221d78: xyz = ((b·a.w) + (a·b.w)) + a × b; w = b.w·a.w − ((a.x·b.x + a.y·b.y) + 1·a.z·b.z).
fn qmul(a: V4, b: V4) -> V4 {
    let c = cross_std(a, b);
    let d = (a[0] * b[0] + a[1] * b[1]) + Pf::ONE * (a[2] * b[2]);
    let w = b[3] * a[3];
    [
        (b[0] * a[3] + a[0] * b[3]) + c[0],
        (b[1] * a[3] + a[1] * b[3]) + c[1],
        (b[2] * a[3] + a[2] * b[3]) + c[2],
        w - d,
    ]
}

/// `rot(v, θ, axis)` 0x274ac8: rotate `v` by θ (counter-clockwise) about `axis` with a quaternion.
pub fn rot(v: V4, theta: Pf, axis: V4) -> V4 {
    if theta.abs() < Pf::b(0x3727_c5ac) { return v; }
    let n = norm(axis, Pf::ONE);
    let h = theta * Pf::b(0x3f00_0000);
    let s = fast_sin(h);
    let q = [n[0] * s, n[1] * s, n[2] * s, fast_cos(h)];
    let m = Pf::b(0xbf80_0000);
    let c = [q[0] * m, q[1] * m, q[2] * m, q[3]];
    let p = [v[0], v[1], v[2], Pf::ZERO];
    let r = qmul(qmul(q, p), c);
    [r[0], r[1], r[2], v[3]]
}

/// `CosInterp(a, b, t)` 0x26cc38.
fn cos_interp(a: Pf, b: Pf, t: Pf) -> Pf {
    if t == Pf::ZERO { return a; }
    if t == Pf::ONE { return b; }
    let c = fast_cos(t * K_PI);
    let f1 = (Pf::ONE - c) * Pf::b(0x3f00_0000);
    let f0 = (b - a) * f1;
    a + f0
}

/// `DecTimer` 0x220ea8 (s16): 1 when already 0, 2 when it just reached 0, else 0.
fn dec_timer(t: &mut i16) -> i32 {
    if *t == 0 { return 1; }
    *t = (*t).max(1) - 1;
    if *t > 0 { 0 } else { 2 }
}

fn t(n: i32) -> i32 { n }

/// `CollLine_Fix(a, b, flags, ignore, 0)`: the world mesh, then the mobys of [`CamInput::mobys`] (`ignore` = the
/// `a2` the game passes: Ratchet for every camera line).
fn line_out(inp: &CamInput, a: V4, b: V4, flags: u32, ignore: Option<usize>) -> Option<CollOutput> {
    coll_line_m(inp.coll, inp.mobys, to_f32x3(a), to_f32x3(b), QueryFlags(flags), ignore)
}
/// [`line_out`]'s hit point.
fn line_hit(inp: &CamInput, a: V4, b: V4, flags: u32, ignore: Option<usize>) -> Option<V4> {
    line_out(inp, a, b, flags, ignore).map(|o| crate::hero::physics::from_f32x3(o.point))
}
/// Sphere 0x212960 `(r, c, flags, ignore)`, world mesh then mobys: (closest point +0x20, pushed centre +0x30).
fn sphere(inp: &CamInput, r: Pf, c: V4, flags: u32, ignore: Option<usize>) -> Option<(V4, V4)> {
    use crate::hero::physics::from_f32x3;
    coll_sphere_m(inp.coll, inp.mobys, to_f32x3(c), r.to_f32(), QueryFlags(flags), ignore)
        .map(|o| (from_f32x3(o.point), from_f32x3(o.pushed_centre.unwrap_or(o.point))))
}

/// The avoidance's view of the mobys (0x312ef8): the ones its `coll_sphere_mobys` listed and switched off
/// (`+0x94 = 0` until the end of the avoidance) report no collision.
struct MaskedMobys<'a> {
    inner: &'a dyn MobySource,
    off: &'a [usize],
}

impl MobySource for MaskedMobys<'_> {
    fn moby(&self, id: usize) -> Option<CollMoby> {
        self.inner.moby(id).map(|mut m| {
            if self.off.contains(&id) { m.collision = false; }
            m
        })
    }
    fn joints(&self, id: usize, count: usize) -> Vec<[u32; 4]> { self.inner.joints(id, count) }
}

/// The classes 0x312ef8 turns the offset away from (±1° about up_s) instead of switching them off.
const NUDGE_CLASSES: [i16; 8] = [0x72, 0xb, 0x2c, 0x9a, 0xff, 0x392, 0x452, 0x353];
/// `FUN_00273278(m)`: a moby of classes 500..=540 (the crates); the blocked camera line hits those.
fn crate_class(o_class: i16) -> bool { ((o_class as i32 - 500) as u32 & 0xffff) < 0x29 }

// ------------------------------------------------------------------------------------------------

/// What the camera reads from the hero each tick.
#[derive(Clone, Copy)]
pub struct CamInput<'a> {
    pub hero: &'a Hero,
    pub pad: &'a PadState,
    pub coll: &'a Collision,
    /// The mobys the camera's line / sphere queries test after the world mesh (the moby table as the hero's
    /// write-back 0x229f20 left it, the grid, class blobs, pose cache; None: world only).
    pub mobys: Option<&'a MobyScene<'a>>,
    /// Ratchet's moby `0x1413d0`, the `a2` (ignored moby) of most camera queries.
    pub hero_moby: Option<usize>,
}

impl Camera {
    /// Level start (0x20ef58 + reset 0x20ee80): globals from the hero, type-0 init and snap behind it.
    pub fn new(inp: &CamInput, opts: CameraOptions) -> Camera {
        let mut c = Camera { opts, ..Default::default() };
        let h = inp.hero;
        let up = norm(h.moby_rows[2], Pf::ONE);
        c.g.up_s = up;
        c.g.up2 = up;
        c.g.up2_prev = up;
        c.g.prev_hero = h.pos;
        c.g.ground_z = h.pos[2];
        c.g.yaw_hist = [h.rot[2]; 5];
        c.g.hero_s = h.pos;
        c.cam.yaw_rate = YAW_RATES[opts.yaw_rate.min(2)];
        c.reset(inp);
        c
    }

    /// Reset 0x20ee80: re-run the motion pre-step, init, backup D0, publish the position.
    pub fn reset(&mut self, inp: &CamInput) {
        // The follow camera made current (the level-class and Swingshot cameras dropped, their +0x7e cleared), no
        // blend (0x167370 = 0).
        self.class_cam.active = false;
        self.class_cam.release = 0;
        self.swing.active = false;
        self.swing.release = 0;
        self.level_cams.release = 0;
        self.blend.mode = 0;
        self.pre_motion(inp);
        self.init(inp);
        self.d0 = self.cam;
        self.out.pos = self.cam.pos;
        self.cam.prev_pos = self.cam.pos;
        self.g.hero_s = inp.hero.pos;
    }

    /// Type-0 init 0x311f38 (not switched in: target reset + snap).
    fn init(&mut self, inp: &CamInput) { self.init_from(inp, None); }

    /// Type-0 init 0x311f38; `pose`: switched in with the previous camera's pose copied (+0x7d = 2, `FUN_0020d110`
    /// for the release kinds 3 / 5 or the blend kinds 3 / 6) and whether that camera was the Swingshot's with
    /// Ratchet on the ground: `0x311dd0` places the target, pivot and look about Ratchet under the copied camera
    /// instead of the snap, and the row blend D+0x20 runs 90 ticks (0 after the Swingshot camera on the ground).
    fn init_from(&mut self, inp: &CamInput, pose: Option<bool>) {
        let h = inp.hero;
        let d = &mut self.cam;
        d.target = h.pos;
        d.look_h = Pf::b(0x3fc0_0000);
        d.mode = 0;
        d.look_from_s = 0;
        d.dh = Pf::b(0x3e4c_cccd);
        d.kv = Pf::b(0x3c75_c28f);
        d.kh = Pf::b(0x3c75_c28f);
        d.dv = Pf::b(0x3e4c_cccd);
        d.look_h_rate = Pf::b(0x3ba3_d70a);
        d.mode_t = 0;
        d.look_blend = 0;
        d.look_h_ovr = 0;
        d.look_h_tgt = Pf::ZERO;
        d.look_h_vel = Pf::ZERO;
        d.ph_rate = Pf::b(0x3b44_9ba6);
        d.pivot_h = Pf::b(0x4000_0000);
        d.base_dist = Pf::b(0x4094_7ae1);
        d.dist = Pf::b(0x4094_7ae1);
        d.dist_rate = Pf::b(0x3b44_9ba6);
        d.yaw_step = Pf::ZERO;
        d.pitch_step = Pf::ZERO;
        d.dist_v = Pf::ZERO;
        d.yaw_v = Pf::ZERO;
        d.el_v = Pf::ZERO;
        d.dist_ovr = 0;
        d.ph_ovr = 0;
        d.dist_ovr_v = Pf::ZERO;
        d.ph_v = Pf::ZERO;
        d.leash = 1;
        d.row_blend = 0;
        d.pivot_h_v = Pf::ZERO;
        d.look_h_v = Pf::ZERO;
        d.pitch_neg = 0;
        d.bias = Pf::ZERO;
        d.bias_v = Pf::ZERO;
        d.yaw_in = Pf::ZERO;
        d.yaw_rate = YAW_RATES[self.opts.yaw_rate.min(2)];
        d.script_pitch = Pf::ZERO;
        d.pitch_n = Pf::ZERO;
        d.yaw_in_v = Pf::ZERO;
        d.pitch_n_v = Pf::ZERO;
        d.script = 0;
        d.script_yaw = Pf::ZERO;
        d.plat_yaw = Pf::ZERO;
        d.toward_t = 0;
        d.toward_lock = 0;
        d.pull = d.dist;
        d.pull_max = Pf::b(0x40c0_0000);
        d.red = Pf::ZERO;
        d.fast_rec = 0;
        d.end_class = 0;
        d.blocked = 0;
        d.end_flags = self.end_flags;
        d.rec_t = t(2000) as i16;
        d.sph_step = Pf::b(0x3dcc_cccd);
        d.sph_scale = Pf::ONE;
        d.sph_count = Pf::b(0x4100_0000);
        // D+0x220 (the class-17 region lock) and D+0x230 cleared.
        self.level_cams.owner = None;
        self.level_cams.focus = 0;
        match pose {
            None => {
                self.target_reset(inp);
                self.snap(inp);
            }
            Some(swing_on_ground) => {
                self.place_under_pose(inp);
                let d = &mut self.cam;
                d.saved_fwd = d.rows[0];
                d.row_blend = if swing_on_ground { 0 } else { t(90) as i16 };
            }
        }
        let d = &mut self.cam;
        d.off = vsub(d.pos, d.pivot);
        d.placed = d.off;
        d.desired = d.pos;
        d.end_cand = d.pos;
        let grav = h.gravity_dir;
        let k = dot(d.off, grav);
        let hz = vsub(d.off, vscale(grav, k));
        d.red = d.dist - len(hz);
        if d.red < Pf::b(0xbdcc_cccd) { d.fast_rec = 1; }
        d.pivot_h_s = d.pivot_h;
        d.look_h_s = d.look_h;
    }

    /// `0x311dd0` (the follow camera switched in with a copied pose): target T = S = Ratchet, the vertical target
    /// along −gravity, the springs' velocities cleared, the pivot and look at their heights above T along −gravity,
    /// the offsets from the copied position, D+0x00 = the position, D+0x1f0 = pivot + offset, the region lock D+0x220
    /// cleared.
    fn place_under_pose(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let up = vscale(h.gravity_dir, Pf::b(0xbf80_0000));
        let d = &mut self.cam;
        d.target = h.pos;
        d.smooth = d.target;
        d.vtarget = vscale(up, dot(d.target, up));
        d.vel_h = [Pf::ZERO; 3];
        d.vel_v = [Pf::ZERO; 3];
        d.look_s_vel = [Pf::ZERO; 3];
        d.pivot = vadd(vscale(up, d.pivot_h), d.target);
        d.look_s = vadd(vscale(up, d.look_h), d.target);
        d.look = d.look_s;
        d.off = vsub(d.pos, d.pivot);
        d.placed = d.off;
        d.desired = d.pos;
        d.end_cand = vadd(d.pivot, d.off);
        self.level_cams.owner = None;
    }

    /// GetTarget 0x30f498 (flag 0 / 1): the hero, or the ground under it while airborne.
    fn get_target(&self, inp: &CamInput, flag: bool) -> (V4, i32) {
        let h = inp.hero;
        if h.f65c == 0 { return (h.pos, 1); }
        let ht = h.height;
        let up2 = self.g.up2;
        if !(Pf::b(0x41f0_0000) <= ht) && flag { return (vadd(h.pos, vscale(up2, -ht)), 1); }
        let p0 = vadd(h.pos, vscale(up2, Pf::b(0x3e4c_cccd)));
        let p1 = vadd(h.pos, vscale(up2, Pf::b(0xc1f0_0000)));
        match line_hit(inp, p0, p1, 0x12, inp.hero_moby) {
            Some(p) => (p, 0),
            None => (h.pos, -1),
        }
    }

    /// 0x30f608: target, look and pivot reset.
    fn target_reset(&mut self, inp: &CamInput) {
        let (tg, _) = self.get_target(inp, false);
        let up2 = self.g.up2;
        let up_s = self.g.up_s;
        let d = &mut self.cam;
        let v = vscale(up2, dot(tg, up2));
        d.target = vsub(tg, v);
        d.vtarget = v;
        d.target = vadd(d.target, d.vtarget);
        let a = vscale(up_s, d.look_h);
        d.look = vadd(a, d.target);
        d.look_s = d.look;
        d.look_s_vel = [Pf::ZERO; 3];
        d.mode = 0;
        d.mode_inv = Pf::ONE;
        d.mode_prev = 0;
        d.mode_t = 0;
        d.smooth = d.target;
        d.vel_h = [Pf::ZERO; 3];
        d.vel_v = [Pf::ZERO; 3];
        d.pivot = vadd(vscale(up_s, d.pivot_h), d.target);
    }

    /// Init snap 0x311890(1): behind the hero at the nominal distance and pivot height, pulled in by walls.
    fn snap(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let d = &mut self.cam;
        let lo = [-d.dist, Pf::ZERO, d.pivot_h, Pf::ZERO];
        let r = [h.moby_rows[0], h.moby_rows[1], h.moby_rows[2]];
        let mut pos = vadd(d.target, crate::hero::physics::mul_rows3(&r, lo));
        let look = vadd(d.target, vscale(r[2], d.look_h));
        let mut fl = 0x94;
        if let Some(p) = line_hit(inp, look, pos, 0x94, inp.hero_moby) {
            if dist2d(p, look) < Pf::b(0x3a83_126f) { fl = 0x96; }
        }
        if let Some(hit) = line_hit(inp, look, pos, fl, inp.hero_moby) {
            let w = vsub(hit, look);
            let l = len(w);
            if l == Pf::ZERO {
                pos = d.target;
            } else if l < Pf::b(0x3e4c_cccd) {
                let look2 = vadd(d.target, vscale(r[2], Pf::b(0x3f00_0000)));
                let mut p = look2;
                let mut rad = Pf::b(0x3f00_0000);
                let mut i = 0;
                while i < 3 {
                    if let Some((_, pc)) = sphere(inp, rad, p, 4, None) {
                        p = pc;
                        i += 1;
                        continue;
                    }
                    if i != 0 { break; }
                    rad = rad + Pf::b(0x3e80_0000);
                    i += 1;
                }
                let w = vsub(p, look2);
                let l = len(w);
                if l == Pf::ZERO {
                    pos = d.target;
                } else {
                    pos = vadd(look2, vscale(w, d.dist / l));
                    if let Some(hit) = line_hit(inp, look2, pos, fl, inp.hero_moby) {
                        let w = vsub(hit, p);
                        let l = len(w);
                        pos = if l == Pf::ZERO { d.target } else { vadd(p, vscale(w, (l - Pf::b(0x3f00_0000)) / l)) };
                    }
                }
            } else {
                pos = vadd(look, vscale(w, (l - Pf::b(0x3f00_0000)) / l));
            }
        }
        let up2 = self.g.up2;
        let hz = vsub(pos, d.target);
        let hz = vsub(hz, vscale(up2, dot(hz, up2)));
        if len(hz) < Pf::b(0x3d4c_cccd) {
            // The game picks the signs with a random bit (0x26c930(2)); the port always takes +.
            pos[0] = pos[0] + Pf::b(0x3f00_0000);
            pos[1] = pos[1] + Pf::b(0x3f00_0000);
            pos[2] = pos[2] + Pf::b(0x3f00_0000);
        }
        d.pos = pos;
        d.prev_pos = pos;
        let a = norm(up2, d.look_h);
        let look = vadd(a, d.target);
        let v = vsub(look, pos);
        let fwd = norm(v, Pf::ONE);
        let left = norm(cross(fwd, up2), Pf::ONE);
        let upr = cross(left, fwd);
        d.rows = [fwd, left, upr];
        d.saved_fwd = fwd;
    }

    /// A shake request (the writer's stores into 0x167260 / 0x167270): amplitude and timer; the envelope's length
    /// (+0xc) is taken by the next update.
    pub fn request_shake(&mut self, r: ShakeRequest) {
        let s = &mut self.shake[r.axis as usize];
        s.amp = Pf::f(r.amp);
        s.timer = r.ticks;
    }

    /// One `CameraUpdate` (0x20eca8) after the hero update. Returns the published view.
    pub fn update(&mut self, inp: &CamInput) -> CameraView {
        self.hit = None;
        self.g.since_switch += 1;
        self.pre_flags(inp);
        self.pre_motion(inp);
        // UpdateAllCameras 0x20d620: the active camera's own check (the first-person camera's release 0x316c08), the
        // other camera's activation (the first-person check 0x316880; the follow camera takes over from a released
        // one), the switch `0x20d110` with the new type's init, then the active type's update.
        // With another camera current the loop's checks still run (class 17's regions see it and leave). The script
        // camera (type 5) is released by `CameraScript2` (script.rs), or switched away from by an armed fly-by (class
        // 19: its check answers 1 against every other class whatever the priorities, and `0x20d620` switches any winner;
        // Qwark's crash camera over Gemlik's ship). [L] Other winners do not take it: their checks weigh the current
        // camera's priority, which the port does not keep for the script camera (taking them stalled Gemlik's fight).
        // The type-6 camera (the missile view) holds until its hand-back `0x317e70`.
        let mut switched = None;
        if !self.follow_is_current() && !self.swing.active && !self.class_cam.active {
            let won = self.activation_loop(inp);
            if self.script.active && !self.type6.active {
                if let (Some(c), Some(slot)) = (won, self.level_cams.won_slot) {
                    let armed_flyby = c == flyby::CLASS_FLYBY && self.flyby_rec(slot).is_some_and(|r| r.armed != 0 && r.done == 0);
                    if armed_flyby && self.level_cams.ports.runs_class_cam(c) {
                        let p = self.script_view();
                        self.script.active = false;
                        self.script.release = None;
                        self.class_switch_in(inp, c, slot, p);
                        switched = Some(p);
                    }
                }
            }
        }
        let prev = if switched.is_some() {
            switched
        } else if self.type6.active {
            self.type6_frame(inp)
        } else if self.script.active {
            self.script_frame(inp)
        } else if self.swing.active {
            self.swing_frame(inp)
        } else if self.class_cam.active {
            self.class_frame(inp)
        } else {
            self.switch_cameras(inp)
        };
        if self.type6.active || self.script.active || self.swing.active || self.class_cam.active {
            // Its update ran in type6_frame / script_frame / swing_frame / class_frame.
        } else if self.first_person.active {
            self.first_person_update(inp);
        } else {
            self.update_type0(inp);
            self.cam.prev_pos = self.cam.pos;
        }
        // `Camera_handleCollWithHero` 0x20d068 for the camera now current (the camera moby; in the game before the
        // current camera's update: the request only reads the class and the last published position).
        self.handle_coll_with_hero();
        // `ExecuteCamPostUpdFuncs` 0x20cd88 (the end of `UpdateAllCameras`): the hint's post-update.
        self.look_hint_post();
        let (rows, pos) = self.active_view();
        // CameraUpdate: the blend's capture of the previous camera (`fun_001ec8a0`, 0x167370 = 1 / 2) and the blend
        // (`fun_001ed2b0`, 0x167370 = 3); else the active camera as it is.
        let (rows, pos) = self.blend.step(prev, rows, pos, crate::hero::physics::to_f32x3(inp.hero.plat_applied), &BlendHero::of(inp.hero));
        self.out.pos = pos;
        self.out.rows = rows;
        self.out.class = self.current_class();
        let f = self.out.rows[0];
        // 0x2721f0: yaw = atan2(fwd.y, fwd.x); pitch / roll from the rows (see the module doc).
        let yaw = fast_arctan(f[0], f[1]);
        let fh = Pf::ZERO + (f[0] * f[0] + f[1] * f[1]).sqrt();
        let pitch = fast_arctan(fh, -f[2]);
        let l = self.out.rows[1];
        let roll = fast_arctan(Pf::ZERO + (l[0] * l[0] + l[1] * l[1]).sqrt(), l[2]);
        self.out.euler = [roll, pitch, yaw, Pf::ZERO];
        // The shakes (0x167260 along up, 0x167270 along forward) move the published position only, after the Euler.
        let (up, fwd) = (self.out.rows[2], self.out.rows[0]);
        self.shake[0].step(&mut self.out.pos, up);
        self.shake[1].step(&mut self.out.pos, fwd);
        if self.opts.mirror { self.out.rows[1] = cross(self.out.rows[2], self.out.rows[0]); }
        self.out
    }

    /// 0x20eb40: collision flags (no hero-side surface flags on land: lines 0xb4, spheres 0x94, end 0x14).
    fn pre_flags(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let mut f = 0x14;
        if h.group == 0x11 || h.group == 0x12 || h.state == 0x73 { f = 0x34; }
        if h.group != 0x11 && h.water_level < self.out.pos[2] { f = 0x14; }
        self.end_flags = f;
        self.sph_flags = f | 0x80;
        self.line_flags = 0xb4;
    }

    /// 0x20e670: up vectors and hero motion.
    fn pre_motion(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let g = &mut self.g;
        let mut u = norm(h.gravity_dir, Pf::b(0xbf80_0000));
        g.up2_prev = g.up2;
        g.up2 = u;
        if dot(g.up_s, u) < Pf::b(0xbf7a_e148) {
            let k = Pf::b(0x3e4c_cccd);
            u[0] = u[0] + k;
            u[1] = u[1] + k;
            u[2] = u[2] + k;
        }
        let (k, dd) = (Pf::b(0x3c75_c28f), Pf::b(0x3e4c_cccd));
        for c in 0..3 { g.up_s[c] = interp(g.up_s[c], u[c], k, dd, Pf::ZERO, &mut g.up_vel[c]); }
        g.up_s = norm(g.up_s, Pf::ONE);
        g.delta = vsub(h.pos, g.prev_hero);
        g.delta_len = len(g.delta);
        g.v_speed = dot(g.delta, u);
        let v = norm(u, g.v_speed);
        g.delta_v = v;
        g.move_dir = vsub(g.delta, v);
        g.h_speed = len(g.move_dir);
        g.move_dir = vscale(g.move_dir, Pf::ONE / g.h_speed);
        g.prev_hero = h.pos;
        g.hero_s[0] = h.pos[0];
        g.hero_s[1] = h.pos[1];
        if !(h.f15d4 == 0x50 && h.state != 0x11) {
            g.hero_s[2] = interp(g.hero_s[2], h.pos[2], Pf::b(0x3bf5_c28f), Pf::b(0x3e33_3333), Pf::ZERO, &mut g.hero_s_vel);
            g.hero_s[3] = h.pos[2];
        }
        g.yaw_hist.rotate_left(1);
        g.yaw_hist[4] = h.rot[2];
        // Moby platforms: none (ground moby always 0).
        g.plat_dz = Pf::ZERO;
        g.ground_z = h.pos[2];
    }

    /// The type-0 update 0x314e00.
    fn update_type0(&mut self, inp: &CamInput) {
        self.platform_carry(inp);
        // 0x3111d8 state tweaks (the camera's reset 0x311010 at the end of the update undoes them each tick).
        // The ledge states (0x1415d4 = 0xd): the auto-yaw behind the hanging hero, yaw rate D+0x1bc = 12°/tick and
        // the scripted yaw input D+0x1c4 (`0x313af0(0.2094, 0, dir(ledge yaw + π))` → `0x313888`,
        // `Hero::ledge_camera_yaw`), and the look flag D+0x116 = 1 (`0x313820`). Not modelled: the camera data's
        // +0x230 = 0x14d exception and the script lock +0x86 (the camera has no script).
        // Not while a class-17 region of mode 10 holds +0x230 = 0x14d.
        let ledge = if self.level_cams.focus != 0x14d { inp.hero.ledge_camera_yaw(to_f32x3(self.cam.off), to_f32x3(self.g.up_s)) } else { None };
        if let Some((rate, yaw_in)) = ledge {
            let d = &mut self.cam;
            d.yaw_rate = Pf::f(rate);
            d.script_yaw = Pf::f(yaw_in);
            d.look_from_s = 1;
        }
        // Under water (group 0x11) the look height is 0.25 (written each tick; the spring-back pulls it toward 1.5
        // once) and the pivot height aims at 0.5 at rate 0.003 (`0x313690`: +0x17c, +0x184, +0x16e = 1); gliding
        // (group 5) the same with the pivot at 2.5 (1.5 for Clank, body 1); on the sinking floor (group 0x10) the
        // look height only.
        let ph = match inp.hero.group {
            0x11 => Some(Pf::b(0x3f00_0000)),
            5 if inp.hero.mode == 1 => Some(Pf::b(0x3fc0_0000)),
            5 => Some(Pf::b(0x4020_0000)),
            _ => None,
        };
        if let Some(tgt) = ph {
            self.cam.look_h = Pf::b(0x3e80_0000);
            self.set_pivot_height(tgt.to_f32(), f32::from_bits(0x3b44_9ba6));
        }
        if inp.hero.group == 0x10 { self.cam.look_h = Pf::b(0x3e80_0000); }
        // Clank (body 1): distance 3 (base too) at 0.003 while no region holds the settings, look and pivot heights 1
        // unless gliding; the sphere chain's scale D+0x20c = 0.6.
        let h = inp.hero;
        if h.mode == 1 {
            if self.level_cams.owner.is_none() {
                self.set_distance(3.0, f32::from_bits(0x3b44_9ba6), true);
                if h.group != 5 {
                    self.cam.look_h = Pf::ONE;
                    self.set_pivot_height(1.0, f32::from_bits(0x3b44_9ba6));
                }
            }
            self.cam.sph_scale = Pf::b(0x3f19_999a);
        }
        // Giant Clank (body 2, G-HERO-005): look 9 / distance 12 (base) / pivot 15 unless a mode-11 region holds
        // D+0x230 = 2, the run-toward lock, the sphere chain's scale 2.0 and step 0.14 (level01 gp 0x162210..0x162220).
        if h.mode == 2 {
            if self.level_cams.focus < 2 {
                self.set_look_height(9.0, f32::from_bits(0x3ba3_d70a), false);
                self.set_distance(12.0, f32::from_bits(0x3b44_9ba6), true);
                self.set_pivot_height(15.0, f32::from_bits(0x3b44_9ba6));
            }
            self.lock_toward();
            self.cam.sph_scale = Pf::b(0x4000_0000);
            self.cam.sph_step = Pf::b(0x3e0f_5c29);
        }
        // A weapon held up (0x1413fa) off the rails (group ≠ 0xf): the leash off, the look from the smoothed target, the
        // horizontal spring 0.04 / 0.2.
        if h.f13fa != 0 && h.group != 0xf {
            self.set_leash(0);
            self.look_from_smoothed();
            self.set_h_spring(f32::from_bits(0x3d23_d70a), f32::from_bits(0x3e4c_cccd));
        }
        // The Swingshot targets' look-up hint's callback (record 0x167490: `0x2eb408`, swing.rs).
        self.look_hint_callback();
        // The scripted focus moby (0x16735c): gone (state ≥ 0x80) → cleared; else, with no mode-11 region (D+0x230 < 2),
        // the right stick moving (|x| or |y| ≥ 0.3) restarts 0x167360, which counts up to 400 ticks, and the camera
        // turns toward the moby at (count / 400)·12° a tick (`0x313b48`). +0x230 = 0 either way.
        if let Some(id) = self.focus_moby {
            if self.level_cams.focus < 2 {
                match self.world.mobys.get(&id).copied() {
                    Some(m) if (m.state as i8) < 0 => self.focus_moby = None,
                    m => {
                        if 0.3 <= inp.pad.rx.to_f32().abs() || 0.3 <= inp.pad.ry.to_f32().abs() { self.focus_ticks = 0; }
                        self.focus_ticks += 1;
                        if t(400) < self.focus_ticks { self.focus_ticks = t(400); }
                        let rate = (self.focus_ticks as f32 / t(400) as f32) * 0.209_439_52;
                        if let Some(m) = m { self.turn_toward_point(rate, 0.0, m.pos); }
                    }
                }
            }
        }
        // The focus scan (`coll_sphere_mobys(15, Ratchet's feet, 1, Ratchet)`): a listed moby whose target record's byte
        // +0x0d is set → +0x230 = 1 (class 17's modes 6 / 8 read it next tick).
        self.level_cams.focus = 0;
        if let Some(sc) = inp.mobys {
            let feet = to_f32x3(inp.hero.pos);
            let near = coll_sphere_mobys(sc, feet, 15.0, QueryFlags(1), inp.hero_moby);
            if near.iter().any(|id| self.world.focus.contains(id)) { self.level_cams.focus = 1; }
        }
        // State 0x81 with L2 / R2 held (0x13cae0 & 3): the leash off, the look from the smoothed target, the spring
        // 0.04 / 0.2 (and D+0x12..0x16 = 0: read only by the yaw-stabiliser branch below). The yaw stabiliser (gp
        // 0x16220c ≠ 0: D+0x12..0x16, 0x15ef40 from the yaw history 0x167340) is never on: its switch has no writer.
        if h.state == 0x81 && inp.pad.held & 3 != 0 {
            self.set_leash(0);
            self.look_from_smoothed();
            self.set_h_spring(f32::from_bits(0x3d23_d70a), f32::from_bits(0x3e4c_cccd));
        }
        self.targets(inp);
        self.leash();
        let (yi, pi) = self.stick_read(inp);
        self.stick_apply(yi, pi);
        if self.avoidance(inp) { self.resets += 1; }
        self.placement(inp);
        self.rows(inp.hero.pos);
        self.spring_back();
    }

    /// 0x30f890: platform carry (0x13f490) and the rigid follow of states 0xb / 0xc.
    fn platform_carry(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let c = h.plat_applied;
        let d = &mut self.cam;
        d.pos = vadd(d.pos, c);
        d.smooth = vadd(d.smooth, c);
        d.target = vadd(d.target, c);
        d.look_s = vadd(d.look_s, c);
        d.desired = vadd(d.desired, c);
        if h.f658 == 0 && (h.ground_moby.is_none() || self.g.since_switch > t(30)) { d.plat_yaw = h.platform[3]; }
        if h.state == 0xb || h.state == 0xc {
            d.leash = 0;
            let s = vscale(self.g.move_dir, self.g.h_speed);
            let up2 = self.g.up2;
            let ch = vsub(c, vscale(up2, dot(up2, c)));
            let s = vsub(s, ch);
            d.pos = vadd(d.pos, s);
            d.smooth = vadd(d.smooth, s);
            d.target = vadd(d.target, s);
            d.look_s = vadd(d.look_s, s);
            d.desired = vadd(d.desired, s);
        }
    }

    /// Target mode state machine 0x30fb08 (D+0x104; D+0x105 the previous mode, D+0x106 / +0x108 the look blend's timer
    /// and 1 / length, D+0x114 / +0x10c the raise's timer and 1 / length, D+0x110 its height, D+0x117 frozen, D+0x60 /
    /// +0x70 its end points; gp constants of level 01 0x16219c..0x1621f8). From mode 0: group 2 → 1 (10 ticks); state
    /// 0xf → 3 (the raise 3.0 over 85 ticks, look blend 30); state 0xc → 8 (30); state 0xb → 6 (the raise 1.0 over 35,
    /// 30); states 0xd / 0xe → 5 (the raise 3.5 over 45, 30); group 4 → 2 (30); the raise modes only with no platform
    /// motion (0x16736c = 0). From the others: state 0x14 → 9 (the raise 1.5 over 30, no blend) unless in 1 or 9; 1 / 2
    /// out of groups 2 and 4 → 0 (no blend, D+0x105 kept); 7: not descending (0x13f76e = 0) → 6 again, group 2 → 1
    /// (20); 4 out of group 4 → 1 in group 2 else 0 (20); 3 in state 0xb → 6; 5 in state 0xc → 8 (30); 8 out of state
    /// 0xc → 0 (20); 6 out of state 0xb → 0 (20); 3 / 5 out of group 4 → 0 (20); 9 / 10 in group 2 or 55 ticks in the
    /// air → 1 (20; the row blend D+0x20 = 90 ticks, the saved forward = the forward); 9 out of state 0x14 → 10 (30);
    /// 10 when the raise timer runs out → 0 (20); 5 in state 0x10 → 0 (20). Docs: player_controller.md §15
    /// "Target modes".
    fn target_mode(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let pdz = self.g.plat_dz;
        let up2 = self.g.up2;
        let up_s = self.g.up_s;
        let air = h.f65c as i16 as i32;
        let d = &mut self.cam;
        let set = |d: &mut FollowCamera, prev: u8, m: u8, k: i32| {
            d.mode_prev = prev;
            d.mode = m;
            d.mode_t = k as i16;
            d.mode_inv = Pf::ONE / i2f(k);
        };
        // The raise (`0x30fa78`): from the vertical target to height `ht` above Ratchet along up_s, over `ticks`.
        let raise = |d: &mut FollowCamera, ticks: i32, ht: Pf| {
            d.raise_t = ticks as i16;
            d.raise_inv = Pf::ONE / i2f(ticks);
            d.raise_frozen = 0;
            d.raise_h = ht;
            d.raise_b = vscale(up2, dot(up2, vadd(vscale(up_s, ht), h.pos)));
            d.raise_a = d.vtarget;
        };
        let plat = pdz == Pf::ZERO;
        if d.mode == 0 {
            if h.group == 2 {
                set(d, 0, 1, 10);
            } else if h.state == 0xf && plat {
                set(d, 0, 3, 30);
                raise(d, t(0x55), Pf::b(0x4040_0000));
            } else if h.state == 0xc && plat {
                set(d, 0, 8, 30);
            } else if h.state == 0xb && plat {
                set(d, 0, 6, 30);
                raise(d, t(0x23), Pf::ONE);
            } else if (h.state == 0xd || h.state == 0xe) && plat {
                set(d, 0, 5, 30);
                raise(d, t(0x2d), Pf::b(0x4060_0000));
            } else if h.group == 4 && plat {
                set(d, 0, 2, 30);
            }
            return;
        }
        let m = d.mode;
        if m != 9 && m != 1 && h.state == 0x14 {
            d.mode_prev = m;
            d.mode = 9;
            d.mode_t = 0;
            d.mode_inv = Pf::ONE;
            raise(d, t(0x1e), Pf::b(0x3fc0_0000));
            return;
        }
        if (m == 1 || m == 2) && h.group != 4 && h.group != 2 {
            d.mode = 0;
            d.mode_inv = Pf::ONE;
            d.mode_t = 0;
            return;
        }
        // Back to 6 (LAB_0030fed4).
        let to_six = |d: &mut FollowCamera, prev: u8| {
            set(d, prev, 6, 30);
            raise(d, t(0x23), Pf::ONE);
        };
        if m == 7 {
            if h.jump.descending == 0 {
                to_six(d, 7);
                return;
            }
            if h.group == 2 {
                set(d, 7, 1, t(0x14));
                return;
            }
        }
        if m == 4 && h.group != 4 {
            set(d, 4, if h.group == 2 { 1 } else { 0 }, t(0x14));
            return;
        }
        if m == 3 && h.state == 0xb {
            to_six(d, 3);
            return;
        }
        if m == 5 && h.state == 0xc {
            set(d, 5, 8, 30);
            return;
        }
        if m == 8 && h.state != 0xc {
            set(d, 8, 0, t(0x14));
            return;
        }
        if m == 6 && h.state != 0xb {
            set(d, 6, 0, t(0x14));
            return;
        }
        if (m == 3 || m == 5) && h.group != 4 {
            set(d, m, 0, t(0x14));
            return;
        }
        // 9 / 10 → 1 (LAB_00310090): in group 2, or 55 ticks in the air.
        if (m == 9 || m == 10) && (h.group == 2 || t(0x37) <= air) {
            set(d, m, 1, t(0x14));
            d.row_blend = t(0x5a) as i16;
            d.saved_fwd = d.rows[0];
            return;
        }
        if m == 9 && h.state != 0x14 {
            set(d, 9, 10, 0x1e);
            return;
        }
        if m == 10 && dec_timer(&mut d.raise_t) != 0 {
            set(d, 10, 0, t(0x14));
            return;
        }
        if m == 5 && h.state == 0x10 { set(d, 5, 0, t(0x14)); }
    }

    /// VerticalTarget 0x3101c0: returns the vertical target V and writes the horizontal part to D+0x40.
    fn vertical_target(&mut self, inp: &CamInput) -> V4 {
        let h = inp.hero;
        let up2 = self.g.up2;
        let hero = h.pos;
        let plain = |d: &mut FollowCamera| {
            let v = vscale(up2, dot(hero, up2));
            d.target = vsub(hero, v);
            v
        };
        // State 0x77: the target flattened, the vertical target from the smoothed one; mode 11 (the row blend 90
        // ticks, the saved forward = the forward) on the first tick.
        if h.state == 0x77 {
            let d = &mut self.cam;
            d.target = vsub(d.target, vscale(up2, dot(d.target, up2)));
            let v = vscale(up2, dot(up2, d.smooth));
            d.vtarget = v;
            if d.mode != 11 {
                d.mode = 11;
                d.saved_fwd = d.rows[0];
                d.row_blend = t(0x5a) as i16;
            }
            return v;
        }
        match self.cam.mode {
            2 => {
                let to_one = |d: &mut FollowCamera| {
                    d.mode = 1;
                    d.mode_t = 10;
                    d.mode_inv = Pf::ONE / i2f(10);
                };
                if h.state == 0x11 {
                    to_one(&mut self.cam);
                } else if h.jump.descending != 0 && Pf::b(0x3e4c_cccd) < h.height {
                    let a = self.view_angle(h);
                    if a < Pf::b(0xbc23_d70a) || HALF_PI < a { to_one(&mut self.cam); }
                }
                let mut hold = false;
                if h.f65c != 0 && self.g.plat_dz == Pf::ZERO && (h.jump.descending == 0 || h.jump.land_eta >= 16) {
                    hold = true;
                } else {
                    let (g, r) = self.get_target(inp, true);
                    if r != 1 || dot(g, up2) < len(self.cam.vtarget) { hold = true; }
                }
                if !hold {
                    let (g, _) = self.get_target(inp, true);
                    let v = vscale(up2, dot(g, up2));
                    self.cam.target = vsub(g, v);
                    return v;
                }
                let p = self.g.up2_prev;
                if (0..3).any(|c| Pf::b(0x3a83_126f) < (up2[c] - p[c]).abs()) { return plain(&mut self.cam); }
                let v = self.cam.vtarget;
                self.cam.target = vsub(hero, vscale(up2, dot(hero, up2)));
                v
            }
            3 | 5 | 6 | 7 => {
                let d = &mut self.cam;
                d.vtarget = vlerp(d.raise_b, d.raise_a, i2f(d.raise_t as i32) * d.raise_inv);
                if d.raise_frozen == 0 && sphere(inp, Pf::b(0x3e4c_cccd), d.pos, 0x12, None).is_some() {
                    d.raise_b = d.vtarget;
                    d.raise_frozen = 1;
                }
                let v = d.vtarget;
                d.target = vsub(hero, vscale(up2, dot(hero, up2)));
                if dec_timer(&mut d.raise_t) != 0 {
                    if d.mode == 6 {
                        d.mode = 7;
                        d.raise_t = t(35) as i16;
                        d.raise_b = d.raise_a;
                        d.raise_a = d.vtarget;
                    } else {
                        d.mode = 4;
                        d.mode_t = 20;
                        d.mode_inv = Pf::ONE / i2f(20);
                    }
                }
                v
            }
            8 => {
                let v = self.cam.vtarget;
                self.cam.target = vsub(hero, vscale(up2, dot(hero, up2)));
                v
            }
            // Modes 9 / 10 (state 0x14 and after): halfway between the plain vertical target and the raise's start.
            9 | 10 => {
                let v = plain(&mut self.cam);
                vlerp(v, self.cam.raise_a, Pf::b(0x3f00_0000))
            }
            _ => plain(&mut self.cam),
        }
    }

    /// 0x30f758: signed angle between the view and the hero's look point (left component removed).
    fn view_angle(&self, h: &Hero) -> Pf {
        let d = &self.cam;
        let p = vadd(h.pos, vscale(self.g.up_s, d.look_h));
        let v = vsub(p, d.pos);
        let left = d.rows[1];
        let v = vsub(v, vscale(left, dot(v, left)));
        let l = len(v);
        if l == Pf::ZERO { return Pf::ZERO; }
        let c = dot(v, d.rows[0]) / l;
        let a = HALF_PI - asin(c);
        if dot(v, d.rows[2]) < Pf::ZERO { -a } else { a }
    }

    /// 0x310a08: targets, look point, pivot and the ceiling lines.
    fn targets(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let hp = h.pos;
        self.target_mode(inp);
        let v = self.vertical_target(inp);
        self.cam.vtarget = v;
        let up2 = self.g.up2;
        let up_s = self.g.up_s;
        let d = &mut self.cam;
        let mut sv = vscale(up2, dot(up2, d.smooth));
        let mut sh = vsub(d.smooth, sv);
        for c in 0..3 { sv[c] = interp(sv[c], d.vtarget[c], d.kv, d.dv, Pf::ZERO, &mut d.vel_v[c]); }
        for c in 0..3 { sh[c] = interp(sh[c], d.target[c], d.kh, d.dh, Pf::ZERO, &mut d.vel_h[c]); }
        d.target = vadd(d.target, d.vtarget);
        d.smooth = vadd(sh, sv);
        let lo = vscale(up_s, d.look_h);
        if d.look_from_s == 0 {
            dec_timer(&mut d.look_blend);
            if d.look_blend < 0 { d.look_blend = 0; }
        } else {
            d.look_blend += 1;
            if d.look_blend > t(90) as i16 { d.look_blend = t(90) as i16; }
        }
        let tt = i2f(d.look_blend as i32) / i2f(t(90));
        d.look = vadd(lo, vlerp(d.target, d.smooth, tt));
        // LookSmooth 0x310908 (D+0xd0; not read by the rows).
        if dec_timer(&mut d.mode_t) != 0 { d.mode_prev = d.mode; }
        let tm = i2f(d.mode_t as i32) * d.mode_inv;
        let tgt = vadd(lo, if d.mode == 1 { hp } else { d.target });
        let (k, dd) = if matches!(d.mode, 1 | 3 | 5 | 6 | 7) {
            (Pf::b(0x3d75_c28f) + (Pf::b(0x3c23_d70a) - Pf::b(0x3d75_c28f)) * tm, Pf::b(0x3ecc_cccd) + (Pf::b(0x3e4c_cccd) - Pf::b(0x3ecc_cccd)) * tm)
        } else {
            (Pf::b(0x3c23_d70a), Pf::b(0x3e4c_cccd))
        };
        for c in 0..3 { d.look_s[c] = interp(d.look_s[c], tgt[c], k, dd, Pf::ZERO, &mut d.look_s_vel[c]); }
        // Pivot and ceiling.
        let b = match h.mode { 2 => Pf::b(0x4040_0000), 1 => Pf::b(0x3ecc_cccd), _ => Pf::b(0x3f00_0000) };
        let base = vadd(hp, vscale(up_s, b));
        d.pivot = vadd(vscale(up_s, d.pivot_h), d.target);
        let hh = Pf::b(0x3e80_0000) + (d.sph_step * d.sph_scale) * (d.sph_count - Pf::ONE);
        let twenty = Pf::b(0x41a0_0000);
        let mut cc = vadd(base, vscale(up_s, twenty));
        if let Some(p) = line_hit(inp, base, cc, self.line_flags, inp.hero_moby) { cc = p; }
        let mut ff = vsub(base, vscale(up_s, twenty));
        if let Some(p) = line_hit(inp, base, ff, self.line_flags, inp.hero_moby) { ff = p; }
        let p = if len(vsub(ff, cc)) < hh + hh {
            vscale(vadd(cc, ff), Pf::b(0x3f00_0000))
        } else if len(vsub(cc, d.target)) <= d.pivot_h + hh {
            vsub(cc, vscale(up_s, hh))
        } else {
            if dot(ff, up2) < dot(d.target, up2) { ff = d.target; }
            d.pivot = vadd(vscale(up_s, d.pivot_h), ff);
            return;
        };
        d.pivot = p;
        d.ph_tgt = dot(vsub(p, d.target), up_s);
        d.ph_rate = Pf::b(0x3b44_9ba6);
        d.ph_ovr = 1;
    }

    /// Leash 0x314118.
    fn leash(&mut self) {
        let d = &mut self.cam;
        if d.leash <= 0 { return; }
        let w = vsub(d.desired, d.pivot);
        d.off = vlerp(d.off, w, Pf::b(0x3f40_0000));
        let mut l = len(d.off);
        if d.dist < l {
            l = d.dist;
            d.off = norm(d.off, l);
        } else {
            let e = d.dist - d.red;
            if l < e {
                l = e;
                d.off = norm(d.off, l);
            }
        }
        d.red = d.dist - l;
    }

    /// Stick read 0x313b88 → (yawIn, pitchIn).
    fn stick_read(&mut self, inp: &CamInput) -> (Pf, Pf) {
        let (rx, ry) = (inp.pad.rx, inp.pad.ry);
        let d = &mut self.cam;
        let mut yi = if d.script & 1 != 0 { Pf::ZERO } else if self.opts.yaw_normal { rx } else { -rx };
        if yi == Pf::ZERO {
            if d.script_yaw != Pf::ZERO { yi = d.script_yaw; }
        } else {
            d.yaw_rate = YAW_RATES[self.opts.yaw_rate.min(2)];
        }
        let pi = if d.script & 2 != 0 {
            d.script_pitch / DEG40
        } else {
            let p = if self.opts.pitch_normal { -ry } else { ry };
            if p == Pf::ZERO { d.script_pitch / DEG40 } else { p }
        };
        (yi, pi)
    }

    /// Stick apply 0x313d90.
    fn stick_apply(&mut self, yi: Pf, pi: Pf) {
        let up_s = self.g.up_s;
        let d = &mut self.cam;
        d.yaw_in = interp(d.yaw_in, yi, Pf::ONE, Pf::ONE, Pf::ZERO, &mut d.yaw_in_v);
        d.yaw_step = d.yaw_rate * d.yaw_in;
        let k = Pf::b(0x3fb6_db6e);
        let p = if pi < Pf::b(0xbe99_999a) { (pi + Pf::b(0x3e99_999a)) * k } else if Pf::b(0x3e99_999a) < pi { (pi - Pf::b(0x3e99_999a)) * k } else { Pf::ZERO };
        let cur = d.pitch_n;
        let same_side_inner = p.abs() < cur.abs() && ((p < Pf::ZERO && cur < Pf::ZERO) || (Pf::ZERO < p && Pf::ZERO < cur));
        let rate = if p == Pf::ZERO || same_side_inner { Pf::b(0x3c23_d70a) } else { Pf::b(0x3ca3_d70a) };
        d.pitch_n = interp(cur, p, Pf::ONE, Pf::ONE, rate, &mut d.pitch_n_v);
        let tgt = d.pitch_n * DEG40;
        d.off = rot(d.off, if yi == Pf::ZERO { d.plat_yaw } else { d.yaw_step }, up_s);
        let e = dot(d.off, up_s);
        let l = len(d.off);
        let ax = norm(cross(up_s, d.off), Pf::ONE);
        let el = fast_arctan(l, e);
        if el.abs() < tgt.abs() && rate == Pf::b(0x3c23_d70a) { d.pitch_n = el / DEG40; }
        if el < tgt {
            let s = add_rot(el, DEG1_75);
            d.pitch_step = if tgt < s { sub_rot(tgt, el) } else { DEG1_75 };
        } else if tgt < el {
            if d.end_class < 0 {
                d.pitch_step = Pf::ZERO;
            } else {
                let s = add_rot(el, -DEG1_75);
                d.pitch_step = if s < tgt { sub_rot(tgt, el) } else { -DEG1_75 };
            }
        } else {
            d.pitch_step = Pf::ZERO;
        }
        if tgt < Pf::ZERO { d.pitch_neg = 1; }
        d.off = rot(d.off, d.pitch_step, ax);
    }

    /// Avoidance 0x312ef8. Returns true when the 30-tick blocked reset fired.
    fn avoidance(&mut self, inp: &CamInput) -> bool {
        let h = inp.hero;
        // coll_sphere_mobys(((dist − red) + 1.5)·0.5, pivot + (0x167240 − pivot)·0.5, 1, Ratchet): the listed mobys
        // with collision but no triangle mesh (FUN_0026e7b0: blob +8 = 0) are switched off (+0x94 = 0) for the rest
        // of the avoidance, reset included, and restored at its end; the NUDGE_CLASSES instead turn the offset.
        // On level 15 (0x15ed84 = 0xf) with body 2 (0x1413f4, Giant Clank) the meshed ones are listed too.
        // Level 13 (0xd) in state 0x7b (sinking, no health) skips the sphere chain and the line, as state 0x7f does.
        let level = self.level_cams.level;
        let all_listed = level == 0xf && h.mode == 2;
        let no_line = h.state == 0x7f || (level == 0xd && h.state == 0x7b);
        let mut off_ids: Vec<usize> = Vec::new();
        if let Some(sc) = inp.mobys {
            let d = &self.cam;
            let r = ((d.dist - d.red) + Pf::b(0x3fc0_0000)) * Pf::b(0x3f00_0000);
            let c = vlerp(d.pivot, self.out.pos, Pf::b(0x3f00_0000));
            for id in coll_sphere_mobys(sc, to_f32x3(c), r.to_f32(), QueryFlags(1), inp.hero_moby) {
                let Some(m) = sc.mobys.moby(id) else { continue };
                if !m.collision || (!all_listed && sc.classes.get(&m.o_class).is_some_and(|b| !b.faces.is_empty())) { continue; }
                if NUDGE_CLASSES.contains(&m.o_class) { self.nudge(m.position.map(Pf)) } else { off_ids.push(id) }
            }
        }
        let masked = inp.mobys.map(|sc| MaskedMobys { inner: sc.mobys, off: &off_ids });
        let masked_scene = inp.mobys.zip(masked.as_ref()).map(|(sc, m)| MobyScene { mobys: m, grid: sc.grid, classes: sc.classes, cache: sc.cache });
        let inp = &CamInput { mobys: masked_scene.as_ref(), ..*inp };
        let mut blocked = false;
        if !no_line && self.sphere_chain(inp) { blocked = true; }
        if self.end_sphere(inp) { blocked = true; }
        let d = &mut self.cam;
        if d.dist - d.red < Pf::b(0x3fc0_0000) {
            d.red = d.dist - Pf::b(0x3fc0_0000);
            d.rec_t = t(2000) as i16;
        }
        let t2 = t(2000) as i16;
        let (rx, ry) = (inp.pad.rx, inp.pad.ry);
        let rec = if d.fast_rec != 0 {
            true
        } else if blocked {
            false
        } else if 0 < d.rec_t && d.rec_t < t2 {
            true
        } else {
            rx < Pf::b(0x3e99_999a) && ry < Pf::b(0x3e99_999a)
        };
        if rec {
            if dec_timer(&mut d.rec_t) != 0 { d.fast_rec = 0; }
            if d.fast_rec != 0 && dec_timer(&mut d.rec_t) != 0 { d.fast_rec = 0; }
            d.red = cos_interp(Pf::ZERO, d.red, i2f(d.rec_t as i32) / i2f(t2 as i32));
        } else if t2 < d.rec_t {
            dec_timer(&mut d.rec_t);
        } else {
            d.rec_t = t2;
        }
        d.off = norm(d.off, d.dist - d.red);
        let e = vadd(d.off, d.pivot);
        if !no_line {
            if let Some(o) = line_out(inp, d.pivot, e, self.line_flags, inp.hero_moby) {
                // InVolume `0x30f468` (the camera-collision grid's pass-through volumes): the grid is empty on all 19
                // levels (docs/plan/player_controller.md §15), so never.
                // A crate in the way (FUN_00273278: classes 500..=540) is hit: FUN_0026e808(20, tmpl, Ratchet,
                // 0x800000, dir) → 0x26e968, dir = the horizontal unit vector camera 0x167240 → crate, w 5627.9
                // (delivered by the tick, [`Camera::hit`]).
                if let Some(m) = o.moby.and_then(|id| inp.mobys?.mobys.moby(id).map(|m| (id, m))).filter(|(_, m)| crate_class(m.o_class)) {
                    let p = m.1.position.map(Pf);
                    let mut dir = vsub([p[0], p[1], p[2], Pf::ZERO], self.out.pos);
                    dir[2] = Pf::ZERO;
                    dir = norm(dir, Pf::ONE);
                    dir[3] = Pf::b(0x45af_df66);
                    self.hit = Some((m.0, dir));
                }
                d.blocked += 1;
                if d.blocked >= t(30) {
                    self.reset(inp);
                    return true;
                }
            } else {
                d.blocked = 0;
            }
        } else {
            d.blocked = 0;
        }
        false
    }

    /// 0x312ef8's turn away from a listed moby of a NUDGE_CLASSES class at `mpos`: when the offset is within
    /// 25° of the moby's direction from the pivot (in the plane of up2), ±1° about up_s, away from it.
    fn nudge(&mut self, mpos: [Pf; 3]) {
        let (up2, up_s) = (self.g.up2, self.g.up_s);
        let d = &mut self.cam;
        let a = vsub([mpos[0], mpos[1], mpos[2], Pf::ZERO], d.pivot);
        let b = vscale(up2, dot(up2, a));
        let c = norm(vsub(a, b), Pf::ONE);
        let x = asin(dot(c, d.off) / (d.dist - d.red));
        let x = (HALF_PI - x).abs();
        if x < Pf::b(0x3edf_66f3) {
            let n = norm(cross(d.off, up2), Pf::ONE);
            d.off = rot(d.off, if Pf::ZERO < dot(n, a) { Pf::b(0xbc8e_fa35) } else { Pf::b(0x3c8e_fa35) }, up_s);
        }
    }

    /// SphereChain 0x312c08: the 6 spheres along pivot → offset.
    fn sphere_chain(&mut self, inp: &CamInput) -> bool {
        let rx = inp.pad.rx;
        let up2 = self.g.up2;
        let mode = inp.hero.mode;
        let flags = self.sph_flags;
        let mut p = self.cam.pivot;
        let mut dir = norm(self.cam.off, Pf::ONE);
        let mut r = Pf::b(0x3e80_0000);
        let mut acc = Pf::ZERO;
        let mut blocked = false;
        let n = (self.cam.sph_count - Pf::ONE).to_i32();
        for i in 0..n {
            if i > 0 {
                if let Some((hit, _pc)) = sphere(inp, r, p, flags, inp.hero_moby) {
                    let behind = i == 1 && dot(vsub(hit, p), self.cam.off) < Pf::ZERO;
                    if !behind {
                        let q = p;
                        if self.reaim(inp, acc, r, &mut p) {
                            blocked = true;
                            let d = &mut self.cam;
                            if mode != 2 && Pf::b(0x3d4c_cccd) < rx.abs() {
                                let u = norm(vsub(hit, q), Pf::ONE);
                                if dot(up2, u).abs() < Pf::b(0x3f40_0000) {
                                    d.rec_t = 0x884;
                                    d.red = d.red + Pf::b(0x3d99_999a) * Pf::ONE;
                                    if d.dist - d.red < Pf::b(0x3e4c_cccd) { d.red = d.dist - Pf::b(0x3e4c_cccd); }
                                }
                            }
                            d.off = norm(vsub(p, d.pivot), d.dist - d.red);
                            dir = norm(d.off, Pf::ONE);
                        }
                    }
                }
            }
            let d = &self.cam;
            let s = (d.dist - d.red) / d.dist;
            r = r + d.sph_step * d.sph_scale;
            acc = acc + r;
            p = vadd(p, vscale(dir, r * s));
        }
        blocked
    }

    /// Reaim 0x3127f0: push the sphere centre so the pivot → camera segment clears the hit point.
    fn reaim(&mut self, inp: &CamInput, acc: Pf, r: Pf, p: &mut V4) -> bool {
        let d = &self.cam;
        let hh = Pf::b(0x3e80_0000) + (d.sph_step * d.sph_scale) * (d.sph_count - Pf::ONE);
        let p0 = *p;
        let a = d.pivot;
        let bp = vadd(a, d.off);
        let eff = d.dist - d.red;
        let Some((mut hit, mut pc)) = sphere(inp, r, p0, self.sph_flags, inp.hero_moby) else { return false };
        let mut disp = V0;
        let mut ret = false;
        for pass in 0..3 {
            if pass > 0 {
                match sphere(inp, r, pc, self.sph_flags, inp.hero_moby) {
                    Some((h2, pc2)) => {
                        hit = vadd(h2, vsub(p0, pc));
                        pc = pc2;
                    }
                    None => break,
                }
            }
            let (dd, c) = seg_dist(hit, a, bp);
            let tt = len(vsub(a, c));
            let allow = (Pf::b(0x3e80_0000) + (hh - Pf::b(0x3e80_0000)) * (tt / eff)) * Pf::b(0x3f40_0000);
            if !(dd < allow) { break; }
            ret = true;
            let c2 = vadd(c, norm(vsub(c, hit), allow - dd));
            let x = vadd(a, norm(vsub(c2, a), acc));
            let push = vsub(x, p0);
            disp = if pass == 0 { push } else { vadd(disp, push) };
        }
        *p = vadd(p0, disp);
        ret
    }

    /// EndSphere 0x3124f0 + 0x312310.
    fn end_sphere(&mut self, inp: &CamInput) -> bool {
        let up2 = self.g.up2;
        let group = inp.hero.group;
        let mode = inp.hero.mode;
        let rx = inp.pad.rx;
        let d = &mut self.cam;
        let mut e = vadd(d.pivot, d.off);
        d.end_class = 2;
        let hh = Pf::b(0x3e80_0000) + (d.sph_step * d.sph_scale) * (d.sph_count - Pf::ONE);
        let mut hit = false;
        let vol = false;
        for _ in 0..3 {
            // a2 = the camera moby 0x167354 (`Camera_handleCollWithHero`), which the port does not create: none.
            let Some((pt, pc)) = sphere(inp, hh, e, d.end_flags, None) else { break };
            hit = true;
            if d.end_class != 0 {
                let n = norm(vsub(pt, e), Pf::ONE);
                let c = dot(up2, n);
                d.end_class = if Pf::b(0x3f40_0000) < c { 1 } else if c < Pf::b(0xbf40_0000) { -1 } else { 0 };
            }
            e = pc;
        }
        let l0 = len(d.off);
        d.off = vsub(e, d.pivot);
        let ret;
        if d.end_class != 0 {
            d.off = norm(d.off, d.dist - d.red);
            ret = hit;
        } else {
            let mut l = len(d.off);
            if mode != 2 && Pf::b(0x3d4c_cccd) < rx.abs() && hit {
                l = l - Pf::b(0x3d99_999a) * Pf::ONE;
                d.rec_t = 0x884;
            }
            if !vol && group != 2 && l0 < l {
                d.off = vscale(d.off, l0 / l);
                d.red = d.dist - l0;
                ret = hit;
            } else {
                if l < Pf::b(0x3e4c_cccd) {
                    d.off = if l <= Pf::b(0x38d1_b717) { norm(inp.hero.moby_rows[0], Pf::b(0xbe4c_cccd)) } else { vscale(d.off, Pf::b(0x3e4c_cccd) / l) };
                    l = Pf::b(0x3e4c_cccd);
                }
                d.red = d.dist - l;
                ret = true;
            }
        }
        d.end_cand = vadd(d.pivot, d.off);
        if d.end_class == 2 { d.end_class = 0; }
        ret
    }

    /// Placement springs and the final position 0x3148f8.
    fn placement(&mut self, inp: &CamInput) {
        let up_s = self.g.up_s;
        let up2 = self.g.up2;
        let (k, dd) = self.dist_rate(inp);
        let d = &mut self.cam;
        d.desired = vadd(d.pivot, d.off);
        let ddist = d.dist - d.red;
        let l = len(d.placed);
        let ln = interp(l, ddist, k, dd, Pf::ZERO, &mut d.dist_v);
        let e1 = dot(d.off, up_s);
        let el_d = asin(e1 / ddist);
        let e2 = dot(d.placed, up_s);
        let el_p = asin(e2 / ln);
        let el = ang_interp(el_p, el_d, Pf::b(0x3c75_c28f), Pf::b(0x3e4c_cccd), Pf::ZERO, &mut d.el_v);
        let oh = vsub(d.off, vscale(up_s, e1));
        let ph = vsub(d.placed, vscale(up_s, e2));
        let m = len(oh) * len(ph);
        let a = HALF_PI - asin(dot(oh, ph) / m);
        let mut ys = ang_interp(Pf::ZERO, a, Pf::b(0x3c75_c28f), Pf::b(0x3e4c_cccd), Pf::ZERO, &mut d.yaw_v);
        let s = dot(cross(ph, up_s), oh);
        ys = ys * if Pf::ZERO <= s { Pf::ONE } else { Pf::b(0xbf80_0000) };
        let v = rot(ph, ys, up_s);
        let v = rot(v, el, cross(up_s, v));
        d.placed = norm(v, ln);
        let base = vadd(vscale(up_s, d.pivot_h), d.smooth);
        let c = dot(d.placed, up2);
        let l2 = len(d.placed);
        if l2 != Pf::ZERO {
            let ang = HALF_PI - asin(c / l2);
            if ang.abs() < DEG15 {
                d.placed = rot(d.placed, sub_rot(DEG15, ang.abs()), cross(d.placed, up2));
                d.el_v = Pf::ZERO;
                d.dist_v = Pf::ZERO;
                d.yaw_v = Pf::ZERO;
            }
        }
        d.pos = vadd(base, d.placed);
    }

    /// DistRate 0x3146f0: the distance spring constants (faster, and pulled back, when the hero runs toward
    /// the camera).
    fn dist_rate(&mut self, inp: &CamInput) -> (Pf, Pf) {
        let g = &self.g;
        let d = &mut self.cam;
        let k14 = Pf::b(0x4164_923a);
        let mut f: Option<Pf> = None;
        if d.toward_lock == 0 {
            let lim = Pf::b(0xbe99_999a);
            if d.toward_t != 0 && g.h_speed == Pf::ZERO && dot(inp.hero.moby_rows[0], d.rows[0]) <= lim {
                f = Some((g.h_speed * k14).min(Pf::ONE));
            } else if Pf::b(0x38d1_b717) < g.h_speed.abs() && dot(g.move_dir, d.rows[0]) <= lim {
                let x = g.h_speed * k14;
                f = Some(if Pf::ONE < x { Pf::ONE } else { x });
            }
        }
        match f {
            Some(f) => {
                d.toward_t = t(120) as i16;
                let far = if inp.hero.mode == 1 { Pf::b(0x4080_0000) } else { d.pull_max };
                let x = d.base_dist + (far - d.base_dist) * f;
                if d.pull < x { d.pull = x; }
                d.dist_tgt = d.pull;
                d.dist_rate = Pf::b(0x3b44_9ba6);
                d.dist_ovr = 1;
                (Pf::b(0x3ca3_d70a) + (Pf::b(0x3d23_d70a) - Pf::b(0x3ca3_d70a)) * f, Pf::b(0x3e4c_cccd) + (Pf::b(0x3e99_999a) - Pf::b(0x3e4c_cccd)) * f)
            }
            None => {
                if d.toward_t != 0 {
                    d.toward_t = 0;
                    d.pull = d.base_dist;
                }
                (Pf::b(0x3ca3_d70a), Pf::b(0x3e4c_cccd))
            }
        }
    }

    /// Rows 0x3141e8. Mode 11 (the fall 0x77, [`FollowCamera::vertical_target`]): the forward straight at Ratchet
    /// (`hero` 0x13f3d0 − the position), the blend timer counted down here and again below (twice a tick).
    fn rows(&mut self, hero: V4) {
        let up_s = self.g.up_s;
        let d = &mut self.cam;
        let mut fwd = d.rows[0];
        if d.mode == 11 {
            fwd = norm(vsub(hero, d.pos), Pf::ONE);
            dec_timer(&mut d.row_blend);
        } else {
            let lh = vsub(d.look, vscale(up_s, dot(d.look, up_s)));
            let ph = vsub(d.pos, vscale(up_s, dot(d.pos, up_s)));
            let hv = vsub(lh, ph);
            let l = len(hv);
            if Pf::b(0x3d4c_cccd) <= l { fwd = vscale(hv, Pf::ONE / l); }
            let left = norm(cross(fwd, up_s), Pf::ONE);
            d.pivot_h_s = interp(d.pivot_h_s, d.pivot_h, Pf::b(0x3b83_126f), Pf::b(0x3e4c_cccd), Pf::ZERO, &mut d.pivot_h_v);
            d.look_h_s = interp(d.look_h_s, d.look_h, Pf::b(0x3b83_126f), Pf::b(0x3e4c_cccd), Pf::ZERO, &mut d.look_h_v);
            let dh = d.pivot_h_s - d.look_h_s;
            let e = dot(d.placed, up_s);
            let a = vsub(vscale(up_s, -e), hv);
            let w = vsub(d.pos, a);
            let w = vsub(w, vscale(up_s, -dh));
            let v = vsub(w, d.pos);
            let lv = len(v);
            if lv != Pf::ZERO {
                let mut pit = HALF_PI - asin(dot(fwd, v) / lv);
                let up = cross(left, fwd);
                d.rows[2] = up;
                if dot(up, v) < Pf::ZERO { pit = -pit; }
                let el = fast_arctan(len(d.off), dot(d.off, up_s));
                let mut q = el / DEG40;
                let mut tb = Pf::ZERO;
                if q < Pf::b(0xbdcc_cccd) {
                    q = -q;
                    if Pf::b(0x3f00_0000) < q { q = Pf::ONE - q; }
                    tb = (q + q) * Pf::b(0x3e86_0a92) + Pf::ZERO;
                }
                d.bias = ang_interp(d.bias, tb, Pf::b(0x3ba3_d70a), Pf::b(0x3e4c_cccd), Pf::ZERO, &mut d.bias_v);
                let mut p = sub_rot(pit, d.bias);
                if DEG70 < p { p = DEG70; } else if p < -DEG70 { p = -DEG70; }
                fwd = norm(rot(fwd, p, left), Pf::ONE);
            }
        }
        if d.row_blend != 0 {
            dec_timer(&mut d.row_blend);
            fwd = norm(vlerp(fwd, d.saved_fwd, i2f(d.row_blend as i32) / i2f(t(90))), Pf::ONE);
        }
        let left = norm(cross(fwd, up_s), Pf::ONE);
        let up = cross(left, fwd);
        d.rows = [fwd, left, up];
    }

    /// Parameter spring-back and resets 0x311010.
    fn spring_back(&mut self) {
        let d0 = self.d0;
        let end_flags = self.end_flags;
        let d = &mut self.cam;
        d.yaw_rate = YAW_RATES[self.opts.yaw_rate.min(2)];
        d.script_yaw = Pf::ZERO;
        d.plat_yaw = Pf::ZERO;
        d.script = 0;
        d.script_pitch = Pf::ZERO;
        let tgt = if d.dist_ovr != 0 { d.dist_tgt } else { d0.dist };
        d.dist = interp(d.dist, tgt, d.dist_rate, Pf::b(0x3e4c_cccd), d.script_yaw, &mut d.dist_ovr_v);
        let tgt = if d.ph_ovr != 0 { d.ph_tgt } else { d0.pivot_h };
        d.pivot_h = interp(d.pivot_h, tgt, d.ph_rate, Pf::b(0x3e4c_cccd), Pf::ZERO, &mut d.ph_v);
        let tgt = if d.look_h_ovr != 0 { d.look_h_tgt } else { d0.look_h };
        d.look_h = interp(d.look_h, tgt, d.look_h_rate, Pf::b(0x3e4c_cccd), Pf::ZERO, &mut d.look_h_vel);
        d.dist_ovr = 0;
        d.ph_ovr = 0;
        d.dist_tgt = Pf::ZERO;
        d.ph_tgt = Pf::ZERO;
        d.base_dist = d0.dist;
        d.look_h_ovr = 0;
        d.look_h_tgt = Pf::ZERO;
        d.leash = 1;
        d.pitch_neg = 0;
        d.look_from_s = 0;
        d.kv = Pf::b(0x3c75_c28f);
        d.dv = Pf::b(0x3e4c_cccd);
        d.kh = Pf::b(0x3c75_c28f);
        d.dh = Pf::b(0x3e4c_cccd);
        d.sph_step = Pf::b(0x3dcc_cccd);
        d.end_flags = end_flags;
        d.sph_scale = Pf::ONE;
        d.sph_count = Pf::b(0x4100_0000);
        d.pull_max = Pf::b(0x40c0_0000);
        d.toward_lock = 0;
    }

    // --------------------------------------------------------------------------------------------
    // The follow camera's setters (level01 0x313560..0x313b48): what other code writes into the follow camera's data
    // for one tick (the spring-back 0x311010 undoes it), each a no-op unless the follow camera is current
    // (0x167280 +0x86 = 0). Callers: the class-17 regions (`level`), the hero-state tweaks of `update_type0`
    // (`0x3111d8`), the first-person camera's entry turn. Native `f32` arguments.

    /// `0x313628(d, rate, base)`: the distance target (D+0x16c = 1, +0x170, rate +0x178; `base`: also the base
    /// distance +0x188).
    pub fn set_distance(&mut self, dist: f32, rate: f32, base: bool) {
        if !self.follow_is_current() { return; }
        let d = &mut self.cam;
        d.dist_ovr = 1;
        d.dist_rate = Pf::f(rate);
        d.dist_tgt = Pf::f(dist);
        if base { d.base_dist = Pf::f(dist); }
    }

    /// `0x313668(v)`: the pull-back maximum D+0x22c.
    pub fn set_pull_max(&mut self, v: f32) {
        if self.follow_is_current() { self.cam.pull_max = Pf::f(v); }
    }

    /// `0x313690(h, rate)`: the pivot-height target (D+0x16e = 1, +0x17c, rate +0x184).
    pub fn set_pivot_height(&mut self, h: f32, rate: f32) {
        if !self.follow_is_current() { return; }
        let d = &mut self.cam;
        d.ph_rate = Pf::f(rate);
        d.ph_ovr = 1;
        d.ph_tgt = Pf::f(h);
    }

    /// `0x3136c8(h, rate, add)`: the look-height target (D+0x11a = 1, rate +0xf8, +0xf4 = `h`, or D0's look height
    /// + `h` when `add`).
    pub fn set_look_height(&mut self, h: f32, rate: f32, add: bool) {
        if !self.follow_is_current() { return; }
        let base = self.d0.look_h;
        let d = &mut self.cam;
        d.look_h_ovr = 1;
        d.look_h_rate = Pf::f(rate);
        d.look_h_tgt = if add { base + Pf::f(h) } else { Pf::f(h) };
    }

    /// `0x313718(p)`: the scripted pitch D+0x1cc (radians).
    pub fn set_script_pitch(&mut self, p: f32) {
        if self.follow_is_current() { self.cam.script_pitch = Pf::f(p); }
    }

    /// `0x313740(v)`: the leash D+0x10.
    pub fn set_leash(&mut self, v: i16) {
        if self.follow_is_current() { self.cam.leash = v; }
    }

    /// `0x313768(k, d)`: the horizontal spring D+0x11c / +0x120 (a zero argument leaves its field).
    pub fn set_h_spring(&mut self, k: f32, dd: f32) {
        if !self.follow_is_current() { return; }
        if k != 0.0 { self.cam.kh = Pf::f(k); }
        if dd != 0.0 { self.cam.dh = Pf::f(dd); }
    }

    /// `0x3137b0(k, d)`: the vertical spring D+0x124 / +0x128 (a zero argument leaves its field).
    pub fn set_v_spring(&mut self, k: f32, dd: f32) {
        if !self.follow_is_current() { return; }
        if k != 0.0 { self.cam.kv = Pf::f(k); }
        if dd != 0.0 { self.cam.dv = Pf::f(dd); }
    }

    /// `0x3137f8()`: the run-toward lock D+0x226 = 1 (DistRate's pull-back off).
    pub fn lock_toward(&mut self) {
        if self.follow_is_current() { self.cam.toward_lock = 1; }
    }

    /// `0x313820()`: D+0x116 = 1 (the look point from the smoothed target).
    pub fn look_from_smoothed(&mut self) {
        if self.follow_is_current() { self.cam.look_from_s = 1; }
    }

    /// `0x313858()`: the stick off (D+0x1b8 |= 3).
    pub fn stick_off(&mut self) {
        if self.follow_is_current() { self.cam.script |= 3; }
    }

    /// A class's stores into the follow camera when it is current (Orxon's teleport pads 351 / 1301, level10
    /// `0x2be858`): D+0x20 = `ticks(90)` and the camera's +0x40 = +0x00 (the saved forward from the forward row: the
    /// row blend out of the old view).
    pub fn row_blend(&mut self) {
        if !self.follow_is_current() { return; }
        let d = &mut self.cam;
        d.row_blend = t(90) as i16;
        d.saved_fwd = d.rows[0];
    }

    /// `0x313af0(rate, tolerance, dir)`: turn toward `dir` at `rate` per tick (D+0x1bc) through the scripted yaw input
    /// D+0x1c4 (`0x313888`, [`yaw_input_toward`]); nothing when `rate` is 0.
    pub fn turn_toward(&mut self, rate: f32, tolerance: f32, dir: [f32; 3]) {
        if !self.follow_is_current() || rate == 0.0 { return; }
        self.cam.yaw_rate = Pf::f(rate);
        if let Some(y) = yaw_input_toward(dir, to_f32x3(self.cam.off), to_f32x3(self.g.up_s), tolerance) { self.cam.script_yaw = Pf::f(y); }
    }

    /// `0x313b48(rate, tolerance, point)`: [`Camera::turn_toward`] toward `point` from the follow camera's target T
    /// (`0x313aa0`: `point − D+0x40`), without the zero-rate check.
    pub fn turn_toward_point(&mut self, rate: f32, tolerance: f32, point: [f32; 3]) {
        if !self.follow_is_current() { return; }
        self.cam.yaw_rate = Pf::f(rate);
        let t = to_f32x3(self.cam.target);
        let dir = [point[0] - t[0], point[1] - t[1], point[2] - t[2]];
        if let Some(y) = yaw_input_toward(dir, to_f32x3(self.cam.off), to_f32x3(self.g.up_s), tolerance) { self.cam.script_yaw = Pf::f(y); }
    }

    /// The store a class makes into 0x16735c (the scripted focus moby: `None` = 0). The follow camera's tweaks
    /// (`0x3111d8`) turn toward it while it lives and clear it when its state goes ≥ 0x80; class 18's region test takes a
    /// region whose moby it is as entered. Writers in the game: units 1422 (L18 0x2f2bf0), 1470 (L17 0x2f26d0), 1051
    /// (L06 0x2f9a28), none ported; a port of one calls this (through the moby → camera channel,
    /// `crate::cinematic::CinematicCall::FocusMoby`). The tick feeds the moby's state and position (`CamWorld::mobys`).
    pub fn set_focus_moby(&mut self, id: Option<usize>) { self.focus_moby = id; }

    /// The end-sphere flags D+0x218 (level02 `0x2f6570`; level 01 has no copy: no caller there).
    pub fn set_end_flags(&mut self, v: u32) {
        if self.follow_is_current() { self.cam.end_flags = v; }
    }

    /// The sphere chain's scale D+0x20c, count D+0x210 and step D+0x214, each only while it is not 0 (level02
    /// `0x2f6598`; no level-01 copy).
    pub fn set_sphere_chain(&mut self, scale: f32, count: f32, step: f32) {
        if !self.follow_is_current() { return; }
        let d = &mut self.cam;
        if d.sph_scale != Pf::ZERO { d.sph_scale = Pf::f(scale); }
        if d.sph_count != Pf::ZERO { d.sph_count = Pf::f(count); }
        if d.sph_step != Pf::ZERO { d.sph_step = Pf::f(step); }
    }
}

/// `0x313888(tolerance, camera, dir)`: the scripted yaw input toward `dir` from the camera offset `off` about `up`: the
/// angle `a` between the flattened offset and the flattened reverse of `dir` (the camera behind), `t = min(a / 90°, 1)`,
/// the input `±(2t − t²)` (`0x26cc00(−1, 0, 1, 0, t)`), signed by the side; None (D+0x1c4 kept) when a vector is flat
/// to nothing or when `tolerance` is not 0 and `|a|` is more than it. The hero's ledge turn calls it with tolerance 0
/// (`Hero::ledge_camera_yaw`).
pub fn yaw_input_toward(d: [f32; 3], off: [f32; 3], up: [f32; 3], tolerance: f32) -> Option<f32> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let dotf = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let flat = |v: [f32; 3]| {
        let k = dotf(v, up);
        [v[0] - up[0] * k, v[1] - up[1] * k, v[2] - up[2] * k]
    };
    let (dc, cc) = (flat(d), flat(off));
    let (ld, lc) = (dotf(dc, dc).sqrt(), dotf(cc, cc).sqrt());
    if ld == 0.0 || ld * lc == 0.0 { return None; }
    let s = (dotf(dc, cc) / (ld * lc)).clamp(-1.0, 1.0).asin();
    let cross = [up[1] * dc[2] - up[2] * dc[1], up[2] * dc[0] - up[0] * dc[2], up[0] * dc[1] - up[1] * dc[0]];
    let sign = if 0.0 <= dotf(cross, cc) { 1.0 } else { -1.0 };
    let mut a = PI - (FRAC_PI_2 - s);
    if PI <= a { a -= 2.0 * PI; } else if a < -PI { a += 2.0 * PI; }
    if tolerance != 0.0 && tolerance < a.abs() { return None; }
    let t = (a / FRAC_PI_2).min(1.0);
    Some(sign * (2.0 * t - t * t))
}

/// SegDist 0x272b98: distance from `q` to the segment a–b and the closest point.
fn seg_dist(q: V4, a: V4, b: V4) -> (Pf, V4) {
    let u = norm(vsub(b, a), Pf::ONE);
    let tt = -((-dot(q, u)) + dot(a, u)) / dot(u, u);
    let mut c = vadd(a, vscale(u, tt));
    let outside = (0..3).any(|k| (a[k] < c[k] && b[k] < c[k]) || (c[k] < a[k] && c[k] < b[k]));
    if outside { c = if len(vsub(q, a)) <= len(vsub(q, b)) { a } else { b }; }
    (len(vsub(q, c)), c)
}

// ------------------------------------------------------------------------------------------------
// The first-person camera (type 4) and the camera switch blend

/// **The first-person camera, type 4** (level01: activation `0x316880`, init `0x316b98` → `0x3162e8` / `0x3161c8`,
/// update `0x316330` with the eye `0x3160b8`, release `0x316c08`; the camera table 0x20c480, entry 2). It is the
/// look stances' camera: the hero's camera mode 0x1415d4 = 4 (state 1, L1 / L2 held) or 0x51 (0x1e). While the
/// follow camera is up and the stance holds, it turns the follow camera toward Ratchet's facing (4° per tick,
/// `0x313af0`) for 7 ticks, then takes over, blending in over 20 ticks (rate 0.05). Its eye is 1.6 above the feet
/// (0.9 for Clank, 9.5 for Giant Clank) plus the platform displacement; the sticks (left first, then right, then
/// the d-pad) turn the view: yaw up to 1.5° per tick about Ratchet's up, pitch the same about the view's left
/// (stick up looks down with the default option 0x15eddc = 1), eased within 20° of the 84° limit; looking down
/// past 64° pushes the eye 0.5 forward (springed, pulled back when a 0.3 sphere there hits). Once its blend-in is
/// over it sets 0x1413f5 every tick (the hero then faces the view and is hidden: `crate::hero`). When the stance
/// ends it hands back to the follow camera, which snaps behind Ratchet and is blended in over ~56 ticks (0.018),
/// or cut when the views are 80° or more apart during the blend-in.
///
/// Native `f32` (the follow camera is the PS2-exact one). Not modelled: the camera collision grid's "no first
/// person" cells (`0x20fdb0` bit 1: the port has no camera grid), the camera moby's entry sound (class 0x3ef is not
/// created by the port), the 30-tick turn behind Ratchet after an aborted entry uses the same scripted yaw as the
/// entry turn (8° per tick).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FirstPerson {
    /// The first-person camera is the active one (0x167280).
    pub active: bool,
    /// UpdateCam +0x00 / +0x10 / +0x20 rows (forward, left, up) and +0x30 position.
    pub rows: [[f32; 3]; 3],
    pub pos: [f32; 3],
    /// Its data (the D block's first words): yaw input and its spring velocity, pitch input and velocity, the
    /// pitch so far (positive down), the eye push and its velocity.
    pub d: [f32; 7],
    /// The instance's +0x20 (ticks of the look stance before the switch), +0x22 (counting), +0x24 (the turn-back
    /// timer).
    pub count: i16,
    pub counting: bool,
    pub turn_back: i32,
    /// This tick's update set 0x1413f5 (it was up with no blend running: 0x167370 = 0).
    pub flag: bool,
}

/// The camera switch blend (`0x167370`: 0 none, 1 / 2 capture next, 3 blending; the record 0x167380: rotation t and
/// rate, position t and rate, the start position and rotation). Native `f32`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CamBlend {
    pub mode: u8,
    pub t_rot: f32,
    pub rot_rate: f32,
    pub t_pos: f32,
    pub pos_rate: f32,
    pub start_pos: [f32; 3],
    pub start_q: [f32; 4],
    /// 0x167388 / 0x167394: the rates of the next blend.
    pub next_rot_rate: f32,
    pub next_pos_rate: f32,
    /// 0x1673c0 / 0x1673d0: the captured previous camera.
    pub cap_pos: [f32; 3],
    pub cap_q: [f32; 4],
    /// 0x167373: the kind of the next blend (0 the rates blend above, 2 the orbit about Ratchet; 1: no ported writer),
    /// set by the camera that asks for the blend; 0x167372: the running blend's.
    pub kind: u8,
    pub running: u8,
    /// 0x1673f4: the orbit blend's length in ticks (the capture adds 1).
    pub orbit_len: i32,
    /// The orbit blend's record 0x1673e0.
    pub orbit: OrbitBlend,
}

/// The orbit blend (`0x167373` = 2; record 0x1673e0, capture `fun_001ec8a0` / `fun_001ec710`, step `fun_001eccd8`): the
/// blended camera is kept as (yaw, pitch, distance) about Ratchet in his frame at the capture and eased each tick
/// toward the new camera's by `1 / CosInterp(1, n, left / n)` of the rest (the last tick lands), its rotation turned by
/// the same share of the yaw and pitch between them. Only the Swingshot camera asks for it. Native `f32`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OrbitBlend {
    /// +0x00: (yaw, pitch, distance) of the blended position.
    pub sph: [f32; 3],
    /// +0x0c ticks left, +0x10 1 / length.
    pub left: i32,
    pub inv: f32,
    /// +0x20 / +0x30: Ratchet's forward and up rows at the capture (normalised).
    pub fwd: [f32; 3],
    pub up: [f32; 3],
    /// +0x40: the blended rotation; +0x50 the blended position; +0x60 the rotation published last (a quaternion).
    pub q: [f32; 4],
    pub pos: [f32; 3],
    pub q_out: [f32; 4],
}

/// What the orbit blend reads of Ratchet: his position 0x13f3d0, his moby's rows (+0xc0 / +0xd0 / +0xe0) and the
/// gravity 0x13f5e0.
#[derive(Clone, Copy, Debug)]
pub struct BlendHero {
    pub pos: [f32; 3],
    pub rows: R3,
    pub grav: [f32; 3],
}

impl BlendHero {
    pub fn of(h: &Hero) -> BlendHero {
        BlendHero { pos: to_f32x3(h.pos), rows: [to_f32x3(h.moby_rows[0]), to_f32x3(h.moby_rows[1]), to_f32x3(h.moby_rows[2])], grav: to_f32x3(h.gravity_dir) }
    }
}

/// `fun_001ec530(out, p, c, f, l, u)`: (yaw, pitch, distance) of `p` about `c` in the frame (f, l, u).
fn orbit_coords(p: [f32; 3], c: [f32; 3], f: [f32; 3], l: [f32; 3], u: [f32; 3]) -> [f32; 3] {
    use std::f32::consts::FRAC_PI_2;
    let d = fsub(p, c);
    let flat = fsub(d, fnorm(u, fdot(d, u)));
    let mut lf = fdot(flat, flat).sqrt();
    if lf == 0.0 { lf = 0.0001; }
    let mut yaw = FRAC_PI_2 - (fdot(f, flat) / lf).clamp(-1.0, 1.0).asin();
    if fdot(l, fnorm(flat, 1.0)) < 0.0 { yaw = -yaw; }
    let r = frot(f, yaw, u);
    let ld = fdot(d, d).sqrt();
    let ldz = if ld == 0.0 { 0.0001 } else { ld };
    let mut pitch = FRAC_PI_2 - (fdot(r, d) / ldz).clamp(-1.0, 1.0).asin();
    if 0.0 <= fdot(u, fnorm(d, 1.0)) { pitch = -pitch; }
    [yaw, pitch, ld]
}

/// `FastVecCross(out, a, b)` = b × a, native.
fn vcross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { fcross(b, a) }

type R3 = [[f32; 3]; 3];

fn fdot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn fcross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn fnorm(a: [f32; 3], l: f32) -> [f32; 3] {
    let n = fdot(a, a).sqrt();
    if n == 0.0 { a } else { [a[0] * l / n, a[1] * l / n, a[2] * l / n] }
}
fn fadd(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn fsub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn fscale(a: [f32; 3], k: f32) -> [f32; 3] { [a[0] * k, a[1] * k, a[2] * k] }
/// `rot(v, θ, axis)` 0x274ac8 (counter-clockwise), native.
fn frot(v: [f32; 3], t: f32, axis: [f32; 3]) -> [f32; 3] {
    if t.abs() < 1e-5 { return v; }
    let a = fnorm(axis, 1.0);
    let (s, c) = t.sin_cos();
    let k = fcross(a, v);
    let d = fdot(a, v) * (1.0 - c);
    [v[0] * c + k[0] * s + a[0] * d, v[1] * c + k[1] * s + a[1] * d, v[2] * c + k[2] * s + a[2] * d]
}
/// `Cam_InterpValues` 0x20ce40, native.
fn finterp(cur: f32, tgt: f32, k: f32, d: f32, max: f32, vel: &mut f32) -> f32 {
    let e = tgt - cur;
    *vel += k * e - d * *vel;
    if max != 0.0 { *vel = vel.clamp(-max, max); }
    let ae = e.abs();
    *vel = vel.clamp(-ae, ae);
    cur + *vel
}
fn fwrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let mut x = a % t;
    if x > std::f32::consts::PI { x -= t; }
    if x < -std::f32::consts::PI { x += t; }
    x
}
/// The angle between the horizontal parts (about `up`) of `a` and `b` (`90° − asin(dot / lengths)`).
fn flat_angle(a: [f32; 3], b: [f32; 3], up: [f32; 3]) -> Option<f32> {
    let fa = fsub(a, fscale(up, fdot(up, a)));
    let fb = fsub(b, fscale(up, fdot(up, b)));
    let (la, lb) = (fdot(fa, fa).sqrt(), fdot(fb, fb).sqrt());
    if lb == 0.0 || la * lb == 0.0 { return None; }
    Some(std::f32::consts::FRAC_PI_2 - (fdot(fb, fa) / (la * lb)).clamp(-1.0, 1.0).asin())
}
/// Rows (forward, left, up) → quaternion (x, y, z, w) of the matrix with those rows.
fn rows_quat(r: R3) -> [f32; 4] {
    let m = |i: usize, j: usize| r[i][j];
    let tr = m(0, 0) + m(1, 1) + m(2, 2);
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [(m(1, 2) - m(2, 1)) / s, (m(2, 0) - m(0, 2)) / s, (m(0, 1) - m(1, 0)) / s, 0.25 * s]
    } else if m(0, 0) > m(1, 1) && m(0, 0) > m(2, 2) {
        let s = (1.0 + m(0, 0) - m(1, 1) - m(2, 2)).sqrt() * 2.0;
        [0.25 * s, (m(0, 1) + m(1, 0)) / s, (m(2, 0) + m(0, 2)) / s, (m(1, 2) - m(2, 1)) / s]
    } else if m(1, 1) > m(2, 2) {
        let s = (1.0 + m(1, 1) - m(0, 0) - m(2, 2)).sqrt() * 2.0;
        [(m(0, 1) + m(1, 0)) / s, 0.25 * s, (m(1, 2) + m(2, 1)) / s, (m(2, 0) - m(0, 2)) / s]
    } else {
        let s = (1.0 + m(2, 2) - m(0, 0) - m(1, 1)).sqrt() * 2.0;
        [(m(2, 0) + m(0, 2)) / s, (m(1, 2) + m(2, 1)) / s, 0.25 * s, (m(0, 1) - m(1, 0)) / s]
    };
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    q.map(|c| c / n)
}
fn quat_rows(q: [f32; 4]) -> R3 {
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ]
}
/// Quaternion slerp (`fun_001fa400`).
fn slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let mut b = b;
    if d < 0.0 {
        d = -d;
        b = b.map(|c| -c);
    }
    let (ka, kb) = if d > 0.9995 {
        (1.0 - t, t)
    } else {
        let th = d.acos();
        let s = th.sin();
        (((1.0 - t) * th).sin() / s, (t * th).sin() / s)
    };
    let q = [a[0] * ka + b[0] * kb, a[1] * ka + b[1] * kb, a[2] * ka + b[2] * kb, a[3] * ka + b[3] * kb];
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    q.map(|c| c / n)
}
/// `CosInterp(0, 1, t)` 0x26cc38, native.
fn fcos_interp(t: f32) -> f32 {
    if t == 0.0 { return 0.0; }
    if t == 1.0 { return 1.0; }
    (1.0 - (t * std::f32::consts::PI).cos()) * 0.5
}
fn rows_f(r: [V4; 3]) -> R3 { r.map(to_f32x3) }
fn rows_pf(r: R3) -> [V4; 3] { r.map(crate::hero::physics::from_f32x3) }

impl CamBlend {
    /// The capture (`fun_001ec8a0`) and the blend step (`fun_001ed2b0` → `fun_001ecaf8`, or `fun_001eccd8` for the
    /// orbit); `prev` = the camera the last switch left (rows, position), `plat` = the platform displacement 0x13f490
    /// (the start moves with it), `hero` Ratchet for the orbit.
    fn step(&mut self, prev: Option<([V4; 3], V4)>, rows: [V4; 3], pos: V4, plat: [f32; 3], hero: &BlendHero) -> ([V4; 3], V4) {
        if self.mode == 1 || self.mode == 2 {
            if self.mode == 1 {
                if let Some((r, p)) = prev {
                    if self.kind == 2 {
                        self.orbit.pos = to_f32x3(p);
                        self.orbit.q_out = rows_quat(rows_f(r));
                        self.orbit_capture(hero);
                    } else {
                        // Kind 0 (kind 1, the camera's own pose about Ratchet: no ported writer).
                        self.cap_pos = to_f32x3(p);
                        self.cap_q = rows_quat(rows_f(r));
                    }
                }
            } else if self.kind == 2 {
                // `fun_001ec7f0`: a running rates blend's capture becomes the orbit's start (moved with the platform).
                if self.running == 0 {
                    self.orbit.pos = fadd(self.cap_pos, plat);
                    self.orbit.q_out = self.cap_q;
                }
                self.orbit_capture(hero);
            } else if self.kind == 0 && self.running != 0 {
                // `fun_001ec868`: a running orbit's pose becomes the rates blend's capture.
                self.cap_pos = self.orbit.pos;
                self.cap_q = self.orbit.q_out;
            }
            self.mode = 3;
            self.running = self.kind;
            if self.kind == 0 {
                self.t_pos = 0.0;
                self.pos_rate = self.next_pos_rate;
                self.start_pos = self.cap_pos;
                self.t_rot = 0.0;
                self.rot_rate = self.next_rot_rate;
                self.start_q = self.cap_q;
            } else {
                self.orbit_len += 1;
                self.orbit.left = self.orbit_len;
                self.orbit.inv = 1.0 / self.orbit_len as f32;
            }
        }
        if self.mode != 3 { return (rows, pos); }
        if self.running != 0 {
            return match self.orbit_step(rows_f(rows), to_f32x3(pos), hero) {
                Some((r, p)) => {
                    let mut pos_out = crate::hero::physics::from_f32x3(p);
                    pos_out[3] = pos[3];
                    (rows_pf(r), pos_out)
                }
                None => {
                    self.running = 0;
                    self.mode = 0;
                    (rows, pos)
                }
            };
        }
        if self.t_pos == 1.0 && self.t_rot == 1.0 {
            self.mode = 0;
            return (rows, pos);
        }
        let s = fcos_interp(self.t_pos);
        self.start_pos = fadd(self.start_pos, plat);
        let cp = to_f32x3(pos);
        let p = fadd(self.start_pos, fscale(fsub(cp, self.start_pos), s));
        let q = slerp(self.start_q, rows_quat(rows_f(rows)), fcos_interp(self.t_rot));
        let r = quat_rows(q);
        self.t_pos = (self.t_pos + self.pos_rate).min(1.0);
        self.t_rot = (self.t_rot + self.rot_rate).min(1.0);
        let mut pos_out = crate::hero::physics::from_f32x3(p);
        pos_out[3] = pos[3];
        (rows_pf(r), pos_out)
    }
}

impl CamBlend {
    /// `fun_001ec710`: Ratchet's frame now, the captured position's (yaw, pitch, distance) in it; +0x40 = +0x60.
    fn orbit_capture(&mut self, h: &BlendHero) {
        let o = &mut self.orbit;
        o.fwd = fnorm(h.rows[0], 1.0);
        let left = fnorm(h.rows[1], 1.0);
        o.up = fnorm(h.rows[2], 1.0);
        o.sph = orbit_coords(o.pos, h.pos, o.fwd, left, o.up);
        o.q = o.q_out;
    }

    /// `fun_001eccd8(camera, 0x1673e0)`: one orbit step toward the camera (`rows`, `pos`); None when no tick is left
    /// (the camera's own view is published and the blend ends).
    fn orbit_step(&mut self, rows: R3, pos: [f32; 3], h: &BlendHero) -> Option<(R3, [f32; 3])> {
        use std::f32::consts::{FRAC_PI_2, TAU};
        let o = &mut self.orbit;
        if o.left < 1 { return None; }
        let f = 1.0 / fcos_interp_ab(1.0, self.orbit_len as f32, o.left as f32 * o.inv);
        let (fwd, up) = (o.fwd, o.up);
        let left = vcross(fwd, up);
        let t = orbit_coords(pos, h.pos, fwd, left, up);
        let dyaw = fwrap1(t[0] - o.sph[0]);
        o.sph[0] = fwrap1(o.sph[0] + dyaw * f);
        o.sph[1] = fwrap1(o.sph[1] + fwrap1(t[1] - o.sph[1]) * f);
        o.sph[2] += (t[2] - o.sph[2]) * f;
        let v = frot(fnorm(fwd, o.sph[2]), o.sph[0], up);
        let l2 = fnorm(vcross(v, up), 1.0);
        let v = frot(v, o.sph[1], l2);
        o.pos = fadd(h.pos, v);
        // The rotation: the blended frame turned about its up by the share of the flat angle to the camera's forward
        // (the long way round when that disagrees with the yaw step's side), then pitched toward it by the same share.
        let [sf, sl, su] = quat_rows(o.q);
        let cf = rows[0];
        let proj = fsub(cf, fscale(su, fdot(su, cf)));
        let lp = fdot(proj, proj).sqrt();
        let mut a = FRAC_PI_2 - (fdot(sf, proj) / lp).clamp(-1.0, 1.0).asin();
        let side = if 0.0 <= fdot(proj, sl) { 1.0 } else { -1.0 };
        a *= side;
        if FRAC_PI_2 < dyaw.abs() && ((0.0 <= dyaw) != (0.0 <= side)) {
            a = if a < 0.0 { a + TAU } else { a - TAU };
        }
        let st = a * f;
        let (bf, bl) = if st.abs() < 1e-5 { (sf, sl) } else { (frot(sf, st, su), frot(sl, st, su)) };
        let full = if a.abs() < 1e-5 { sf } else { frot(sf, a, su) };
        let mut p = FRAC_PI_2 - fdot(full, cf).clamp(-1.0, 1.0).asin();
        if fdot(full, rows[2]) < 0.0 { p = -p; }
        let bf = frot(bf, p * f, bl);
        let fwd_o = fnorm(bf, 1.0);
        let left_o = fnorm(vcross(fwd_o, h.grav), -1.0);
        let up_o = vcross(left_o, fwd_o);
        let out = [fwd_o, left_o, up_o];
        o.q = rows_quat(out);
        o.q_out = o.q;
        o.left -= 1;
        Some((out, o.pos))
    }
}

/// `CosInterp(a, b, t)` 0x26cc38, native.
fn fcos_interp_ab(a: f32, b: f32, t: f32) -> f32 {
    if t == 0.0 { return a; }
    if t == 1.0 { return b; }
    a + (b - a) * ((1.0 - (t * std::f32::consts::PI).cos()) * 0.5)
}

/// `fast_add_rotations` / `fast_subtract_rotations`: wrapped once into [−π, π).
fn fwrap1(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if a >= PI { a - TAU } else if a < -PI { a + TAU } else { a }
}

/// The release of the first-person camera (`0x316c08`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Release {
    None,
    Blend,
    Cut,
}

impl Camera {
    /// The current camera's own position (0x167280 +0x30: before the blend and the shakes), for the moby loop's
    /// readers (the camera moby's update, `moby_update::services::LoopGlobals::cam_pos`).
    pub fn current_pos(&self) -> [f32; 3] { to_f32x3(self.active_view().1) }

    /// The active camera's rows and position (UpdateCam +0x00.. and +0x30).
    fn active_view(&self) -> ([V4; 3], V4) {
        if self.type6.active { return self.type6_view(); }
        if self.script.active { return self.script_view(); }
        if self.swing.active {
            let (r, p) = self.swing_view();
            return (rows_pf(r), level::pos4(p));
        }
        if self.class_cam.active { return self.class_view(); }
        if self.first_person.active {
            let mut p = crate::hero::physics::from_f32x3(self.first_person.pos);
            p[3] = Pf::ONE;
            (rows_pf(self.first_person.rows), p)
        } else {
            (self.cam.rows, self.cam.pos)
        }
    }

    /// The part of `UpdateAllCameras` before the active update: returns the camera switched away from (its rows and
    /// position), if any.
    fn switch_cameras(&mut self, inp: &CamInput) -> Option<([V4; 3], V4)> {
        if self.first_person.active {
            let r = self.first_person_release(inp);
            if r == Release::None { return None; }
            let prev = self.active_view();
            // `0x20d110(follow)` after a release: the blend (0x167370 = 1 / 2) unless cut; the type-0 init
            // (+0x7d = 0: target reset and the snap behind Ratchet) and `BackupCurrentCam`.
            if r == Release::Blend { self.blend.mode = if self.blend.mode == 0 { 1 } else { 2 }; }
            self.first_person.active = false;
            self.init(inp);
            self.d0 = self.cam;
            self.g.since_switch = 0;
            return Some(prev);
        }
        // The follow camera is current: every slot's check (class 17's regions, the first-person camera's own), then
        // the switch to the winner the port runs (the first-person and the Swingshot cameras); another is recorded.
        match self.activation_loop(inp) {
            Some(level::CLASS_FIRST_PERSON) => {}
            Some(c) if c == swing::CLASS_SWING && self.level_cams.ports.swing => {
                let prev = self.active_view();
                self.level_cams.release = 0;
                self.swing_switch_in(inp, prev);
                return Some(prev);
            }
            Some(c) if self.level_cams.ports.runs_class_cam(c) && self.level_cams.won_slot.is_some() => {
                let prev = self.active_view();
                let slot = self.level_cams.won_slot.unwrap();
                self.class_switch_in(inp, c, slot, prev);
                return Some(prev);
            }
            Some(c) => {
                self.level_cams.wanted = Some(c);
                return None;
            }
            None => return None,
        }
        let prev = self.active_view();
        // The switch clears the camera it leaves' +0x7e (class 1's hook may have set the follow camera's).
        self.level_cams.release = 0;
        self.first_person.active = true;
        self.g.since_switch = 0;
        self.first_person_init(inp, rows_f(prev.0)[0]);
        Some(prev)
    }

    /// The first-person camera's activation check `0x316880` (with the follow camera up): true = switch.
    fn first_person_activation(&mut self, inp: &CamInput) -> bool {
        let h = inp.hero;
        let fp = &mut self.first_person;
        let facing = [h.rot[2].to_f32().cos(), h.rot[2].to_f32().sin(), 0.0];
        if h.f15d4 == 4 || h.f15d4 == 0x51 {
            // The follow camera is up (its mode 0): count 7 ticks, turning it toward Ratchet's facing.
            fp.count += 1;
            fp.counting = true;
            fp.turn_back = 0;
            if 7 <= fp.count {
                fp.count = 0;
                fp.counting = false;
                return true;
            }
            self.scripted_turn(inp, f32::from_bits(0x3d8e_fa35), facing);
            return false;
        }
        fp.count = 0;
        if fp.counting {
            fp.counting = false;
            let up = to_f32x3(self.g.up2);
            let hero_fwd = to_f32x3(h.moby_rows[0]);
            if flat_angle(to_f32x3(self.cam.rows[0]), hero_fwd, up).is_some_and(|a| f32::from_bits(0x3fb2_b8c2) <= a) {
                // CameraResetBehindHero 0x20ee80.
                self.reset(inp);
                self.resets += 1;
                return false;
            }
            self.first_person.turn_back = 30;
        }
        if self.first_person.turn_back != 0 {
            self.first_person.turn_back -= 1;
            self.scripted_turn(inp, f32::from_bits(0x3e0e_fa35), facing);
        }
        false
    }

    /// `0x313af0(rate, 0, dir)` on the follow camera: the scripted yaw input toward `dir` at `rate` per tick (the
    /// ledge's turn uses the same, `Hero::ledge_camera_yaw`).
    fn scripted_turn(&mut self, _inp: &CamInput, rate: f32, dir: [f32; 3]) { self.turn_toward(rate, 0.0, dir); }

    /// The init `0x316b98` (→ `0x3162e8`, `0x3161c8`): the blend in (0x167370 = 1, or 2 during a blend; rates 0.05),
    /// the data cleared, the eye, the rows from the previous camera's forward about Ratchet's up.
    fn first_person_init(&mut self, inp: &CamInput, prev_fwd: [f32; 3]) {
        self.blend.mode = if self.blend.mode == 0 { 1 } else { 2 };
        self.blend.kind = 0;
        self.blend.next_rot_rate = f32::from_bits(0x3d4c_cccd);
        self.blend.next_pos_rate = f32::from_bits(0x3d4c_cccd);
        let h = inp.hero;
        let up = fnorm(to_f32x3(h.moby_rows[2]), 1.0);
        let fp = &mut self.first_person;
        fp.d = [0.0; 7];
        fp.pos = fadd(to_f32x3(h.pos), fscale(up, eye_height(h)));
        let left = fnorm(fcross(up, prev_fwd), 1.0);
        fp.rows = [fcross(left, up), left, up];
    }

    /// The release `0x316c08` (the active first-person camera's own check at the start of `UpdateAllCameras`).
    fn first_person_release(&mut self, inp: &CamInput) -> Release {
        let h = inp.hero;
        if h.f15d4 == 0x51 || h.f15d4 == 4 { return Release::None; }
        if self.blend.mode != 0 {
            let up = to_f32x3(self.g.up2);
            if flat_angle(self.first_person.rows[0], to_f32x3(h.moby_rows[0]), up).is_some_and(|a| f32::from_bits(0x3fb2_b8c2) <= a) {
                self.blend.mode = 0;
                return Release::Cut;
            }
        }
        self.blend.kind = 0;
        self.blend.next_rot_rate = f32::from_bits(0x3c93_74bc);
        self.blend.next_pos_rate = f32::from_bits(0x3c93_74bc);
        Release::Blend
    }

    /// The update `0x316330`.
    fn first_person_update(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let pad = inp.pad;
        let up = fnorm(to_f32x3(h.moby_rows[2]), 1.0);
        let blending = self.blend.mode != 0;
        let fp = &mut self.first_person;
        fp.flag = !blending;
        // The eye (`0x3160b8`): the feet + up·height + the platform displacement 0x13f490.
        fp.pos = fadd(fadd(to_f32x3(h.pos), fscale(up, eye_height(h))), to_f32x3(h.plat_applied));
        let (k, dd) = (f32::from_bits(0x3cf5_c28f), f32::from_bits(0x3ecc_cccd));
        let (lo, hi) = (0.0f32, f32::from_bits(0x3cd6_7750));
        // Yaw: −left x, else −right x, else the d-pad (left +1, right −1).
        let mut v = -pad.lx.to_f32();
        if v == 0.0 { v = -pad.rx.to_f32(); }
        if v == 0.0 { v = ((pad.held >> 15) & 1) as f32; }
        if v == 0.0 { v = -(((pad.held >> 13) & 1) as f32); }
        let rate = lo + (hi - lo) * fp.d[0].abs();
        fp.d[0] = finterp(fp.d[0], v, k, dd, 0.0, &mut fp.d[1]);
        let mut fwd = fp.rows[0];
        if fp.d[0] != 0.0 { fwd = frot(fwd, rate * fp.d[0], up); }
        let plat_yaw = h.plat_applied[3].to_f32();
        if plat_yaw != 0.0 && h.f658 == 0 { fwd = frot(fwd, plat_yaw, up); }
        // Pitch: −left y, else −right y, else the d-pad (up +1, down −1); none while blending in from a walk.
        let mut v = -pad.ly.to_f32();
        if v == 0.0 { v = -pad.ry.to_f32(); }
        if v == 0.0 { v = ((pad.held >> 12) & 1) as f32; }
        if v == 0.0 { v = -(((pad.held >> 14) & 1) as f32); }
        if blending && h.prev_state == 2 { v = 0.0; }
        if !self.opts.pitch_normal { v = -v; }
        let rate = lo + (hi - lo) * fp.d[2].abs();
        fp.d[2] = finterp(fp.d[2], v, k, dd, 0.0, &mut fp.d[3]);
        if fp.d[2] != 0.0 {
            let left = fcross(up, fwd);
            let mut step = rate * fp.d[2];
            let a = fwrap(fp.d[4] + step).abs();
            if f32::from_bits(0x3f8e_fa35) < a && fp.d[4].abs() < a {
                let over = fwrap(a - f32::from_bits(0x3f8e_fa35));
                step += (0.0 - step) * (over / f32::from_bits(0x3eb2_b8c4));
            }
            let push_to = if f32::from_bits(0x3f8e_c104) <= a && 0.0 < fp.d[4] { 0.5 } else { 0.0 };
            fp.d[5] = finterp(fp.d[5], push_to, f32::from_bits(0x3ba3_d70a), f32::from_bits(0x3e4c_cccd), 0.0, &mut fp.d[6]);
            let hf = fnorm([fwd[0], fwd[1], 0.0], fp.d[5]);
            fp.pos = fadd(fp.pos, hf);
            let c = crate::hero::physics::from_f32x3(fp.pos);
            if sphere(inp, Pf::b(0x3e99_999a), c, 0x12, inp.hero_moby).is_some() {
                let fp = &mut self.first_person;
                fp.d[5] = finterp(fp.d[5], 0.0, f32::from_bits(0x3ba3_d70a), f32::from_bits(0x3e4c_cccd), 0.0, &mut fp.d[6]);
            }
            let fp = &mut self.first_person;
            let a2 = fwrap(fp.d[4] + step);
            if f32::from_bits(0x3fbb_a866) < a2.abs() {
                let lim = if a2 <= 0.0 { -f32::from_bits(0x3fbb_a866) } else { f32::from_bits(0x3fbb_a866) };
                step = fwrap(lim - fp.d[4]);
            }
            fwd = frot(fwd, step, left);
            fp.d[4] = fwrap(fp.d[4] + step);
        }
        let fp = &mut self.first_person;
        let fwd = fnorm(fwd, 1.0);
        let left = fnorm(fcross(up, fwd), 1.0);
        fp.rows = [fwd, left, fcross(fwd, left)];
    }

    /// 0x1413f5: the first-person camera sets it while it is up and its blend-in is over (`0x316330`: when
    /// 0x167370 = 0); the tick stores it into the hero (`Hero::f13f5`).
    pub fn first_person_flag(&self) -> bool { self.first_person.active && self.first_person.flag }
}

/// `0x3160b8`'s eye height by the hero body 0x1413f4: 1.6 (Ratchet), 0.9 (Clank; 1.2 with 0x15edb3), 9.5 (Giant Clank).
fn eye_height(h: &Hero) -> f32 {
    match h.mode {
        2 => f32::from_bits(0x4118_0000),
        1 if h.cheats.on(crate::cheats::slot::CLANK) => f32::from_bits(0x3f99_999a),
        1 => f32::from_bits(0x3f66_6666),
        _ => f32::from_bits(0x3fcc_cccd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asin_and_rot() {
        for &x in &[0.0f32, 0.3, -0.5, 0.9, 1.0] {
            assert!((asin(Pf::f(x)).to_f32() - x.asin()).abs() < 1e-4, "asin {x}");
        }
        let v = [Pf::ONE, Pf::ZERO, Pf::ZERO, Pf::ZERO];
        let z = [Pf::ZERO, Pf::ZERO, Pf::ONE, Pf::ZERO];
        let r = rot(v, HALF_PI, z);
        assert!(r[0].to_f32().abs() < 1e-5 && (r[1].to_f32() - 1.0).abs() < 1e-5, "{r:?}");
        let c = cross(v, z); // z × x = y
        assert_eq!(c[1], Pf::ONE);
    }

    /// The look stance (camera mode 4): 7 ticks turning the follow camera, then the first-person camera takes over
    /// with a 20-tick blend (rate 0.05); once it is over the flag 0x1413f5 is set; the eye 1.6 above the feet; the
    /// left stick turns the view 1.5° per tick at full deflection (springed); back to the follow camera with the
    /// 0.018 blend when the stance ends.
    #[test]
    fn first_person_enter_turn_exit() {
        let (coll, mut hero, mut pad) = setup();
        let neutral = crate::pad::PadInput::neutral();
        let mut cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }, CameraOptions::default());
        let step = |cam: &mut Camera, hero: &crate::hero::Hero, pad: &mut PadState, inp: crate::pad::PadInput| {
            pad.update(Some(&inp.bytes()), false);
            cam.update(&CamInput { hero, pad, coll: &coll, mobys: None, hero_moby: None })
        };
        for _ in 0..5 { step(&mut cam, &hero, &mut pad, neutral); }
        hero.f15d4 = 4;
        let mut on = None;
        for t in 1..=10 {
            step(&mut cam, &hero, &mut pad, neutral);
            if cam.first_person.active && on.is_none() { on = Some(t); }
        }
        assert_eq!(on, Some(7));
        let mut flag_at = None;
        for t in 11..=40 {
            let v = step(&mut cam, &hero, &mut pad, neutral);
            if flag_at.is_none() && cam.first_person_flag() {
                flag_at = Some(t);
                assert!((v.pos_f32()[2] - 101.6).abs() < 1e-3, "eye {:?}", v.pos_f32());
            }
        }
        assert_eq!(flag_at, Some(7 + 21), "the flag the tick after the 20-tick blend");
        let y0 = cam.out.yaw().to_f32();
        for _ in 0..30 { step(&mut cam, &hero, &mut pad, neutral.stick(1.0, 0.0)); }
        let y1 = cam.out.yaw().to_f32();
        assert!(y1 < y0 - 0.3 && y1 > y0 - 0.8, "turned right {y0} → {y1}");
        hero.f15d4 = 0;
        step(&mut cam, &hero, &mut pad, neutral);
        assert!(!cam.first_person.active && cam.blend.mode == 3);
        let mut n = 1;
        while cam.blend.mode != 0 && n < 200 {
            step(&mut cam, &hero, &mut pad, neutral);
            n += 1;
        }
        assert!((55..=58).contains(&n), "the 0.018 blend back took {n}");
    }

    /// Leaving within the blend-in with the views 80° apart cuts back without a blend.
    #[test]
    fn first_person_cut_when_far_off() {
        let (coll, mut hero, mut pad) = setup();
        let neutral = crate::pad::PadInput::neutral();
        let mut cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }, CameraOptions::default());
        hero.f15d4 = 4;
        for _ in 0..8 {
            pad.update(Some(&neutral.bytes()), false);
            cam.update(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None });
        }
        assert!(cam.first_person.active && cam.blend.mode == 3);
        // Ratchet turned away 90°.
        hero.moby_rows = crate::hero::physics::euler_rows([Pf::ZERO, Pf::ZERO, HALF_PI, Pf::ZERO]);
        hero.f15d4 = 0;
        pad.update(Some(&neutral.bytes()), false);
        cam.update(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None });
        assert!(!cam.first_person.active);
        assert_eq!(cam.blend.mode, 0, "cut");
    }

    fn setup() -> (Collision, crate::hero::Hero, PadState) {
        let coll = crate::hero::testkit::floor(100.0, 98, 108, 98, 108);
        let hero = crate::hero::Hero::spawn([410.0, 410.0, 100.0], 0.0);
        (coll, hero, PadState::default())
    }

    /// Init snap 0x311890 behind a hero facing +x: pos = hero + (−4.64, 0, 2.0), looking at hero + 1.5 up.
    #[test]
    fn init_snap_behind_the_hero() {
        let (coll, hero, pad) = setup();
        let cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }, CameraOptions::default());
        let p = cam.cam.pos.map(Pf::to_f32);
        assert!((p[0] - (410.0 - 4.64)).abs() < 1e-4 && p[1] == 410.0 && p[2] == 102.0, "{p:?}");
        let f = cam.cam.rows[0].map(Pf::to_f32);
        let e = [4.64f32, 0.0, -0.5];
        let l = (e[0] * e[0] + e[2] * e[2]).sqrt();
        assert!((f[0] - e[0] / l).abs() < 1e-5 && (f[2] - e[2] / l).abs() < 1e-5, "{f:?}");
    }

    /// Standing still, the camera stays put; full right stick turns the desired offset by 1.3° per tick
    /// (13° over 10 ticks) counter-clockwise, and the placed offset lags behind it.
    #[test]
    fn right_stick_yaw() {
        let (coll, hero, mut pad) = setup();
        let mut cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }, CameraOptions::default());
        for _ in 0..30 { pad.update(Some(&crate::pad::PadInput::neutral().bytes()), false); cam.update(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }); }
        let p0 = cam.out.pos_f32();
        assert!((p0[0] - (410.0 - 4.64)).abs() < 1e-3 && (p0[1] - 410.0).abs() < 1e-3, "{p0:?}");
        let a0 = cam.cam.off[1].to_f32().atan2(cam.cam.off[0].to_f32());
        let r = crate::pad::PadInput::neutral().rstick(1.0, 0.0);
        for _ in 0..10 { pad.update(Some(&r.bytes()), false); cam.update(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }); }
        let a1 = cam.cam.off[1].to_f32().atan2(cam.cam.off[0].to_f32());
        let wrap = |d: f32| { let d = d.to_degrees().rem_euclid(360.0); if d > 180.0 { d - 360.0 } else { d } };
        let turned = wrap(a1 - a0);
        let ap = cam.cam.placed[1].to_f32().atan2(cam.cam.placed[0].to_f32());
        let placed = wrap(ap - a0);
        let yaw = cam.out.yaw().to_f32().to_degrees();
        eprintln!("desired offset turned {turned}°, placed {placed}°, view yaw {yaw}°");
        assert!((turned - 13.0).abs() < 0.05, "{turned}");
        assert!(0.0 < placed && placed < turned);
        assert!((yaw - placed).abs() < 0.5, "the view follows the placed offset");
    }

    /// The stomp's shake (0.2 along up for 40 ticks): the first update applies 0.2·cos(78)·(39/40)² ≈ −0.163, the
    /// offset buzzes with a period of π ticks inside a quadratically shrinking envelope, reaches 0 on the 40th update
    /// and resets the envelope after it; only the position moves.
    #[test]
    fn shake_envelope() {
        let mut s = Shake { amp: Pf::f(0.2), timer: 40, ..Shake::default() };
        let up = [Pf::ZERO, Pf::ZERO, Pf::ONE, Pf::ZERO];
        let mut offs = Vec::new();
        for _ in 0..41 {
            let mut p = [Pf::f(10.0), Pf::f(20.0), Pf::f(30.0), Pf::ZERO];
            s.step(&mut p, up);
            offs.push(p[2].to_f32() - 30.0);
            assert_eq!((p[0].to_f32(), p[1].to_f32()), (10.0, 20.0));
        }
        assert!((offs[0] + 0.1631).abs() < 2e-4, "{}", offs[0]);
        assert!(offs.iter().all(|o| o.abs() <= 0.2));
        assert!(offs[1] > 0.1, "{}", offs[1]);
        assert_eq!((offs[39], offs[40]), (0.0, 0.0));
        assert_eq!((s.timer, s.max), (0, 0));
        // The tick's request path: the camera's record takes amplitude and timer.
        let mut c = Camera::default();
        c.request_shake(ShakeRequest { axis: ShakeAxis::Forward, amp: 0.4, ticks: 30 });
        assert_eq!((c.shake[1].amp, c.shake[1].timer, c.shake[0].timer), (Pf::f(0.4), 30, 0));
    }
}
