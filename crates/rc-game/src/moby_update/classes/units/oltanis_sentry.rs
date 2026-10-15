//! **Oltanis's searchlight sentries, class 8** (level14 `0x2ac618`; census U488, 11 placed). A hovering sentry that
//! sweeps a searchlight along a path while it drifts along its own; the beam finding Ratchet (or a decoy 203 or a
//! chicken 270) raises its turret group (the pop-up turrets 30, [`super::popup_turret`]). Read from the level14 decomp
//! and disassembly; native `f32`.
//!
//! * **Every tick**: the record's +0xe at 2 (written elsewhere): gone. The animation (sequence 2 wrapped → 3 over 3
//!   ticks, 1 wrapped → 2 over 10); the hits (`0x2ad8b8`, mask 0x330000, the resolver column 4, health +0x20: dead →
//!   7, not targetable, its death bits; red 200 / 100; the flash; a burning hit (out 3) burns while key B is 1);
//!   in states 1..6 within 40 of the camera the shadow probe.
//! * **0**: with +0x1b6 = 1 it is the group's draw master (state 8, no collision, always updated, hidden). Else (its
//!   paths +0x74 / +0x78 / +0x204 and turret group +0x70 checked, else gone) health 4, at its path's start, the
//!   light at the light path's start, the chords (`0x2ae448`; +0x210 ≠ 0 starts both that far along), the
//!   manipulators (lists 0 / 1) → 1.
//! * **1** (grey glow 0xaaaaaa): a raised turret in its group (or none alive) → 3 for `ticks(120)`. The light runs
//!   its path at +0x168 a tick (back and forth unless closed, +0x8a), the sentry follows its own path in step
//!   (`0x2ae7d8`), bobs and wobbles and turns ±120° every `ticks(260)` (`0x2aea08`). The beam in view (`0x2aeba0`:
//!   the lamp (joint list 1) turned to the light, the spot drawn): spheres of 0.48·r at its middle or 0.9·r at the
//!   spot (r = +0x214) touching Ratchet or a decoy / chicken first → 2 at once. A hit → 2.
//! * **2**: the hum released, the turret group raised (`0x2b44a8`) → 3 for `ticks(120)`.
//! * **3** (yellow glow, red beam): the light springs onto the target (the ground under it within 4), the beam
//!   drawn; the target gone or the time up → 4.
//! * **4**: out along the exit path +0x204 at 4 a second, the lamp straightened; once the turrets are down
//!   (`0x2b4500`) → 5 back along it at 5 a second, → 6 at its start: back onto the patrol (the light at the
//!   nearest point of its path) → 1 within 0.5.
//! * **7** dead: the turrets raised, the explosion (`SpawnBeamExplosion`, 5 debris), flung off Ratchet (0.075..0.2
//!   a tick turned 40..70°, up 0.1..0.2, spinning), the body at a tenth with the lamp ten times (it alone shows)
//!   → 9: flying with drag and gravity, smoking; after `ticks(180)` or on touching something the explosion
//!   (damage 1 within 2) and gone; out of the world: gone.
//! * **Sounds**: the hum (sound 0, loop) in 1; the alarm (sound 2, loop) in 3 / 4.
//! * **Draws**: the master (`0x2aee28`) draws its group farthest first: a sentry flagged 8 (in view in 3 / 4) its
//!   lamp glow (16 layers of two camera-facing quads turning about joint list 2, `0x2add48`), flagged 2 (its beam in
//!   view) the beam's cone (`0x2ada18`: an 82-point half mesh drawn twice, its alphas cycling (`0x2aed50`), red
//!   when alert) and the lamp's flare (`0x2ae0a8`). Each sentry with its beam in view draws the spot (`0x2ae260`).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ac618` | 8 | [`update`] |
//! | `0x2ad8b8` | hits | [`hits`] |
//! | `0x2ae448` / `0x2ae6d8` / `0x2ae7d8` / `0x2ae900` | chords, the light, the follow, the exit path | [`chords`], [`light`], [`follow`], [`exit`] |
//! | `0x2aea08` / `0x2aead8` / `0x2aeba0` | bob and turn, the falling flicker, the aim | [`hover`], [`flicker`], [`aim`] |
//! | `0x2b44a8` / `0x2b4500` / `0x2b4670` | the turret group | [`super::popup_turret`] |
//! | `0x2aed50` / `0x2aee28` / `0x2aef28` / `0x2af048` | the master's tick, draw, order, count | [`master_tick`], [`fx_quad_groups`] |
//! | `0x2add48` / `0x2ada18` / `0x2ae0a8` / `0x2ae260` | glow, cone, flare, spot | [`fx_quad_groups`], [`spot_quads`] |
//!
//! [L] The draws are built from the camera the frame part saw. The record's +0xe (set 1 here, 2 deletes) has no
//! other reader found. A spotted target is kept as its index + 1 (the game: its pointer).

use super::{popup_turret as turret, FxQuad, FxQuads};
use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, damage, flash, fx, DT, SPEED};
use crate::moby_update::services::{euler_rows, pv, pvar as p, Services, World};
use crate::moby_update::{manip, scheduler, story};
use crate::ps2v::Pf;
use crate::spline::{self, Cursor};

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2a_c618;
/// The master's group draw and each sentry's spot draw.
pub const MASTER_DRAW_FN: u32 = 0x2a_ee28;
pub const SPOT_FN: u32 = 0x2a_e260;
pub const CLASSES: [i16; 1] = [8];

mod o {
    pub const HEALTH: usize = 0x20;
    pub const KILL: usize = 0x2e;
    pub const FLASH: usize = 0x60;
    pub const GROUP: usize = 0x70;
    pub const PATH: usize = 0x74;
    pub const LIGHT_PATH: usize = 0x78;
    pub const EXIT_SEG: usize = 0x80;
    pub const LIGHT_SEG: usize = 0x84;
    pub const ARMED: usize = 0x88;
    pub const CLOSED: usize = 0x8a;
    pub const EXIT_T: usize = 0x8c;
    pub const SPOT: usize = 0x90;
    pub const ROWS: usize = 0xa0;
    pub const LAMP: usize = 0xd0;
    pub const YAW_NODE: usize = 0xe0;
    pub const PITCH_NODE: usize = 0x120;
    pub const SPIN: usize = 0x160;
    pub const FRAC: usize = 0x164;
    pub const LIGHT_SPEED: usize = 0x168;
    pub const LIGHT_LEN: usize = 0x16c;
    pub const LIGHT: usize = 0x170;
    pub const LIGHT_V: usize = 0x180;
    pub const TARGET: usize = 0x18c;
    pub const PATH_LEN: usize = 0x190;
    pub const POS_V: usize = 0x194;
    pub const BOB: usize = 0x1a0;
    pub const WOBBLE: usize = 0x1a8;
    pub const YAW_GOAL: usize = 0x1b0;
    pub const YAW_T: usize = 0x1b4;
    pub const MASTER: usize = 0x1b6;
    pub const YAW_V: usize = 0x1b8;
    pub const HEAD_YAW: usize = 0x1bc;
    pub const HEAD_PITCH: usize = 0x1c0;
    pub const HEAD_YAW_V: usize = 0x1c4;
    pub const HEAD_PITCH_V: usize = 0x1c8;
    pub const LIGHT_T: usize = 0x1cc;
    pub const VOICE: usize = 0x1d0;
    pub const GLOW: usize = 0x1d4;
    pub const FLY: usize = 0x1e0;
    pub const CRASH_T: usize = 0x1f0;
    pub const CHASE_T: usize = 0x1f2;
    pub const SPINS: usize = 0x1f4;
    pub const SHADOW: usize = 0x200;
    pub const ALERT: usize = 0x202;
    pub const EXIT_PATH: usize = 0x204;
    pub const EXIT_END: usize = 0x208;
    pub const COUNT: usize = 0x20a;
    pub const FLAGS: usize = 0x20c;
    pub const START: usize = 0x210;
    pub const RADIUS: usize = 0x214;
    pub const PULL: usize = 0x218;
    pub const F21C: usize = 0x21c;
    /// The port's: the glow's angle drawn this tick (the game turns it inside the draw).
    pub const GLOW_ANGLE: usize = 0x220;
    pub const SIZE: usize = 0x224;
}

/// Level14 gp words 0x16139c.. (gp−0x5864..).
mod k {
    pub const VIEW_R: f32 = 4.0;
    pub const K: f32 = 0.02;
    pub const D: f32 = 0.2;
    pub const BOB: (f32, f32) = (0.25, 0.06);
    pub const WOBBLE: (f32, f32, f32) = (0.15, 0.05, 0.025);
    pub const TURN_TICKS: i32 = 260;
    pub const TURN: f32 = f32::from_bits(0x4006_0a92);
    pub const TURN_SPRING: (f32, f32, f32) = (0.003, 0.2, 0.02);
    pub const CHASE_TICKS: i32 = 120;
    pub const CRASH_TICKS: i32 = 180;
    pub const FLING: (f32, f32) = (0.075, 0.2);
    pub const FLING_UP: (f32, f32) = (0.1, 0.2);
    pub const GRAVITY: f32 = 0.003;
    pub const DRAG: f32 = 0.97;
    pub const DRAG_MIN: f32 = 0.05;
    pub const EXIT_SPEED: f32 = 4.0;
    pub const EXIT_K: f32 = 0.005;
    pub const EXIT_D: f32 = 0.2;
    pub const FLARE: f32 = 0.75;
    pub const FLARE_OUT: f32 = 0.5;
    pub const SPOT_GAP: f32 = 2.0;
    pub const SPOT_RGBA: u32 = 0x4050_7f7f;
    pub const ALERT_RGBA: u32 = 0x7f40_407f;
}

const DECOY: i16 = 0xcb;
const CHICKEN: i16 = 0x10e;
/// Level14 0x1d8600 / 0x1d8640: the camera-facing and the flat quads' corners.
const FACING: [[f32; 3]; 4] = [[0.0, -1.0, 1.0], [0.0, -1.0, -1.0], [0.0, 1.0, 1.0], [0.0, 1.0, -1.0]];
const FLAT: [[f32; 3]; 4] = [[-1.0, 1.0, 0.0], [-1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [1.0, -1.0, 0.0]];
const ST: [[f32; 2]; 4] = [[1.0, 1.0], [0.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
const DEATH: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 5, sound: 1, shake: true };
const CRASH: fx::Beam = fx::Beam { damage_r: 2.0, damage: 1.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 5, sound: 1, shake: true };

/// The cone's shared colours (level14 0x1d80c8, their alphas cycled by the masters, `0x2aed50`) and the camera the
/// draws' frame parts saw.
#[derive(Clone, Debug, Default)]
pub struct Cone {
    rgba: Option<[u32; 82]>,
    phase: Vec<i32>,
    cam: [f32; 3],
}

impl Cone {
    fn colours(&self) -> [u32; 82] { self.rgba.unwrap_or(CONE_RGBA) }
}

fn cone<'a>(w: &'a mut World<'_>) -> &'a mut Cone {
    let g = &mut w.svc.units.oltanis_sentry;
    if g.rgba.is_none() {
        g.rgba = Some(CONE_RGBA);
        g.phase = CONE_PHASES.to_vec();
    }
    g
}

fn path_pts(w: &World, i: i32) -> Option<Vec<[f32; 4]>> {
    let s = w.svc.splines.get(usize::try_from(i).ok()?)?;
    Some(s.iter().map(|q| q.map(f32::from_bits)).collect())
}
fn path_ok(w: &World, i: i32) -> bool { i != -1 && path_pts(w, i).is_some_and(|p| !p.is_empty()) }
fn target(w: &World, id: MobyId) -> Option<MobyId> {
    let v = c::pi32(w, id, o::TARGET);
    usize::try_from(v - 1).ok().filter(|&m| v > 0 && m < w.table.mobys.len())
}
fn v3(w: &World, id: MobyId, at: usize) -> [f32; 3] { [c::pf(w, id, at), c::pf(w, id, at + 4), c::pf(w, id, at + 8)] }

/// `Spring(t, k, d, max, &x, &v)` 0x263c40 (= level01 0x270780).
fn spring(t: f32, kk: f32, d: f32, max: f32, x: &mut f32, v: &mut f32) {
    let (mut px, mut pv_) = (Pf::f(*x), Pf::f(*v));
    crate::hero::physics::spring(Pf::f(t), Pf::f(kk), Pf::f(d), Pf::f(max), &mut px, &mut pv_);
    (*x, *v) = (px.to_f32(), pv_.to_f32());
}
/// Three springs on a pvar vector (velocities at `vo`).
fn spring_pvar(w: &mut World, id: MobyId, t: [f32; 3], kk: f32, d: f32, xo: usize, vo: usize) {
    for (j, &tj) in t.iter().enumerate() {
        let (mut x, mut v) = (c::pf(w, id, xo + 4 * j), c::pf(w, id, vo + 4 * j));
        spring(tj, kk, d, 0.0, &mut x, &mut v);
        c::set_pf(w, id, xo + 4 * j, x);
        c::set_pf(w, id, vo + 4 * j, v);
    }
}
/// Three springs on the position (velocities at `vo`).
fn spring_pos(w: &mut World, id: MobyId, t: [f32; 3], kk: f32, d: f32) {
    for (j, &tj) in t.iter().enumerate() {
        let (mut x, mut v) = (w.m(id).position[j], c::pf(w, id, o::POS_V + 4 * j));
        spring(tj, kk, d, 0.0, &mut x, &mut v);
        w.mm(id).position[j] = x;
        c::set_pf(w, id, o::POS_V + 4 * j, v);
    }
}
fn release_voice(w: &mut World, id: MobyId) {
    let s = c::pi32(w, id, o::VOICE);
    if s != -1 && w.sound_alive(s, id) { w.release_sound(s, id); }
    c::set_pi32(w, id, o::VOICE, -1);
}
/// Segment lengths before `seg` plus `t`.
fn along(pts: &[[f32; 4]], seg: i32, t: f32) -> f32 { pts.iter().take(seg.max(0) as usize).map(|q| q[3]).sum::<f32>() + t }

/// `0x2ae448`: the three paths' chords (point i to (i + 1) mod n, the last one not counted in the lengths +0x16c /
/// +0x190); the cursors at 0, or with a start fraction +0x210 both paths that far along (the position written by
/// each).
pub fn chords(w: &mut World, id: MobyId) {
    let len = |w: &mut World, i: i32| -> f32 {
        let Some(pts) = path_pts(w, i) else { return 0.0 };
        let n = pts.len();
        let mut sum = 0.0;
        for k in 0..n {
            let d = c::dist3(pts[k], pts[(k + 1) % n]);
            w.svc.splines[i as usize][k][3] = d.to_bits();
            if k != n - 1 { sum += d; }
        }
        sum
    };
    let l = len(w, c::pi32(w, id, o::LIGHT_PATH));
    c::set_pf(w, id, o::LIGHT_LEN, l);
    let l = len(w, c::pi32(w, id, o::PATH));
    c::set_pf(w, id, o::PATH_LEN, l);
    len(w, c::pi32(w, id, o::EXIT_PATH));
    for at in [o::LIGHT_T, o::EXIT_SEG, o::LIGHT_SEG, o::EXIT_T] { c::set_pi32(w, id, at, 0); }
    let f = c::pf(w, id, o::START);
    if f == 0.0 { return; }
    let closed = c::pi16(w, id, o::CLOSED) != 0;
    for (path, total, seg, t) in [(o::LIGHT_PATH, o::LIGHT_LEN, o::LIGHT_SEG, o::LIGHT_T), (o::PATH, o::PATH_LEN, o::EXIT_SEG, o::EXIT_T)] {
        let pts = path_pts(w, c::pi32(w, id, path)).unwrap_or_default();
        let mut cur = Cursor::default();
        let (q, _) = spline::advance(&pts, closed, f * c::pf(w, id, total), &mut cur);
        c::set_pi32(w, id, seg, cur.seg);
        c::set_pf(w, id, t, cur.t);
        let m = w.mm(id);
        m.position = [q[0], q[1], q[2], m.position[3]];
    }
}

/// `0x2ae6d8`: the light along its path at +0x168 a tick (an open path's end turns it back), the light point
/// springing after it.
pub fn light(w: &mut World, id: MobyId) {
    let pts = path_pts(w, c::pi32(w, id, o::LIGHT_PATH)).unwrap_or_default();
    let closed = c::pi16(w, id, o::CLOSED) != 0;
    let old = Cursor { seg: c::pi32(w, id, o::LIGHT_SEG), t: c::pf(w, id, o::LIGHT_T) };
    let mut cur = old;
    let (q, ended) = spline::advance(&pts, closed, c::pf(w, id, o::LIGHT_SPEED), &mut cur);
    c::set_pi32(w, id, o::LIGHT_SEG, cur.seg);
    c::set_pf(w, id, o::LIGHT_T, cur.t);
    spring_pvar(w, id, q, k::K, k::D, o::LIGHT, o::LIGHT_V);
    if !closed && ended {
        c::set_pf(w, id, o::LIGHT_T, old.t);
        c::set_pi32(w, id, o::LIGHT_SEG, old.seg);
        let s = c::pf(w, id, o::LIGHT_SPEED);
        c::set_pf(w, id, o::LIGHT_SPEED, -s);
    }
}

/// Its own path's point as far along it as the light is along its path.
fn follow_point(w: &World, id: MobyId) -> [f32; 3] {
    let lp = path_pts(w, c::pi32(w, id, o::LIGHT_PATH)).unwrap_or_default();
    let d = along(&lp, c::pi32(w, id, o::LIGHT_SEG), c::pf(w, id, o::LIGHT_T)) / c::pf(w, id, o::LIGHT_LEN) * c::pf(w, id, o::PATH_LEN);
    let pts = path_pts(w, c::pi32(w, id, o::PATH)).unwrap_or_default();
    spline::advance(&pts, c::pi16(w, id, o::CLOSED) != 0, d, &mut Cursor::default()).0
}

/// `0x2ae7d8`: the sentry springs to [`follow_point`].
pub fn follow(w: &mut World, id: MobyId) {
    let q = follow_point(w, id);
    spring_pos(w, id, q, k::K, k::D);
}

/// `0x2ae900(k, speed, dir, m)`: along the exit path at `speed·dt·dir` (its end reached: +0x208, then its last
/// point), the sentry springing after it.
pub fn exit(w: &mut World, id: MobyId, kk: f32, speed: f32, dir: f32) {
    let pts = path_pts(w, c::pi32(w, id, o::EXIT_PATH)).unwrap_or_default();
    let q = if c::pi16(w, id, o::EXIT_END) == 0 {
        let mut cur = Cursor { seg: c::pi32(w, id, o::EXIT_SEG), t: c::pf(w, id, o::EXIT_T) };
        let (q, ended) = spline::advance(&pts, false, speed * DT * dir, &mut cur);
        c::set_pi32(w, id, o::EXIT_SEG, cur.seg);
        c::set_pf(w, id, o::EXIT_T, cur.t);
        if ended { c::set_pi16(w, id, o::EXIT_END, 1); }
        q
    } else {
        pts.last().map_or([0.0; 3], |p| [p[0], p[1], p[2]])
    };
    spring_pos(w, id, q, kk, k::EXIT_D);
}

/// `0x2aea08`: the bob and wobble; in state 1 a turn of ±120° every `ticks(260)` (sprung).
pub fn hover(w: &mut World, id: MobyId) {
    super::bob(w, id, k::BOB.0, k::BOB.1, o::BOB, o::BOB + 4);
    super::wobble(w, id, k::WOBBLE.0, k::WOBBLE.1, k::WOBBLE.2, o::WOBBLE, o::WOBBLE + 4);
    if w.m(id).state != 1 { return; }
    if c::dec_timer_pvar_s16(w, id, o::YAW_T) != 0 {
        let t = w.ticks(k::TURN_TICKS);
        c::set_pi16(w, id, o::YAW_T, t as i16);
        let a = if w.rng.randi(2) != 0 { -k::TURN } else { k::TURN };
        let g = c::add_rot(w.m(id).rotation[2], a);
        c::set_pf(w, id, o::YAW_GOAL, g);
    }
    let (mut x, mut v) = (w.m(id).rotation[2], c::pf(w, id, o::YAW_V));
    spring(c::pf(w, id, o::YAW_GOAL), k::TURN_SPRING.0, k::TURN_SPRING.1, k::TURN_SPRING.2, &mut x, &mut v);
    w.mm(id).rotation[2] = x;
    c::set_pf(w, id, o::YAW_V, v);
}

/// `0x20cca8(m, 1, +0xa0)`: the lamp's world rows (normalised) and point.
fn lamp_matrix(w: &mut World, id: MobyId) {
    let m = w.joint_matrix(id, 1);
    for (j, r) in m.iter().take(3).enumerate() {
        let n = c::set_len3([r[0], r[1], r[2], 0.0], 1.0);
        c::set_pv4(w, id, o::ROWS + 0x10 * j, [n[0], n[1], n[2], r[3]]);
    }
    c::set_pv4(w, id, o::LAMP, m[3]);
}

fn draw_dist(w: &World, id: MobyId) -> f32 { w.m(id).draw_dist as f32 }

/// `0x2aead8`: four ticks in five, in view: the lamp's matrix and the beam flagged (+0x20c = 2).
pub fn flicker(w: &mut World, id: MobyId) {
    if w.rng.randi(5) == 0 { return; }
    let p = c::pos(w, id);
    if fx::in_view(w, draw_dist(w, id), p, k::VIEW_R) {
        lamp_matrix(w, id);
        c::set_pi32(w, id, o::FLAGS, 2);
    }
}

/// `0x2aeba0(m, draw)`: the beam (lamp to light) in view: the lamp's matrix, the head turned to the light (yaw on
/// list 0, pitch on list 1), the spot at the light; with `draw` the spot drawn and the beam flagged. Whether it was.
pub fn aim(w: &mut World, id: MobyId, draw: bool) -> bool {
    let (l, lamp) = (v3(w, id, o::LIGHT), v3(w, id, o::LAMP));
    let half: [f32; 4] = std::array::from_fn(|j| if j < 3 { (l[j] - lamp[j]) * 0.5 } else { 0.0 });
    let mid = [lamp[0] + half[0], lamp[1] + half[1], lamp[2] + half[2], 0.0];
    if !fx::in_view(w, draw_dist(w, id), mid, c::len3(half) + 2.0) { return false; }
    lamp_matrix(w, id);
    let yaw = c::sub_rot(c::atan(half[0], half[1]), w.m(id).rotation[2]);
    manip::set_axis(w, id, id, o::YAW_NODE, yaw, 2);
    c::set_pf(w, id, o::HEAD_YAW, yaw);
    let pitch = -c::atan(c::len2(half), half[2]);
    manip::set_axis(w, id, id, o::PITCH_NODE, pitch, 1);
    c::set_pf(w, id, o::HEAD_PITCH, pitch);
    let lv = c::pv4(w, id, o::LIGHT);
    c::set_pv4(w, id, o::SPOT, lv);
    if draw {
        register(w, id, SPOT_FN);
        let f = c::pi32(w, id, o::FLAGS);
        c::set_pi32(w, id, o::FLAGS, f | 2);
    }
    true
}

fn register(w: &mut World, id: MobyId, f: u32) {
    if let Some(row) = super::row(REFERENCE_LEVEL, f) {
        w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
        w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
    }
}

/// `0x2ad8b8` (module doc).
pub fn hits(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x33_0000, false);
    let r = damage::resolve(w, id, hit, o::HEALTH, 0, 4);
    if r.out5 != 1 && w.m(id).state <= 6 {
        let h = c::pf(w, id, o::HEALTH) - r.damage;
        c::set_pf(w, id, o::HEALTH, h);
        let reaction = if h <= 0.0 { 1 } else { r.reaction };
        if w.m(id).state == 1 { w.mm(id).state = 2; }
        if r.out5 == 3 { w.mm(id).cmd = 1; }
        match reaction {
            1 | 2 => {
                let m = w.mm(id);
                m.state = 7;
                m.mode &= !mode::TARGETABLE;
                crate::moby_update::classes::crate_::set_death_bits(w, id, 0, -1);
                flash::start(w, id, o::FLASH);
            }
            3..=8 => {
                c::set_pu8(w, id, o::FLASH + 7, 200);
                w.anim_blend(id, 1, 0, 5);
                flash::start(w, id, o::FLASH);
            }
            9 | 10 => {
                c::set_pu8(w, id, o::FLASH + 7, 100);
                w.anim_blend(id, 1, 0, 5);
                flash::start(w, id, o::FLASH);
            }
            _ => flash::start(w, id, o::FLASH),
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, o::FLASH);
}

/// `0x2aed50`: the master's tick of the cone's alphas (a step every other tick; phase p → alpha p, or 0xa0 − p past
/// 0x50; past 0x90 back by 0x80).
pub fn master_tick(w: &mut World, id: MobyId) {
    let f = c::pf(w, id, o::FRAC) + 0.5;
    c::set_pf(w, id, o::FRAC, f);
    let step = f as i32;
    if 1.0 <= f { c::set_pf(w, id, o::FRAC, 0.0); }
    let g = cone(w);
    let mut rgba = g.colours();
    for (ph, c_) in g.phase.iter_mut().zip(rgba.iter_mut()) {
        if *ph == 0 { continue; }
        let mut n = *ph + step;
        if 0x90 < n { n -= 0x80; }
        *ph = n;
        let a = if 0x50 < n { 0xa0 - n } else { n };
        *c_ = (*c_ & 0xff_ffff) | (a as u32) << 24;
    }
    g.rgba = Some(rgba);
}

/// Level14 `0x2ac618` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    if c::pu8(w, id, o::KILL) == 2 {
        c::set_pi32(w, id, o::FLAGS, 0);
        w.delete_moby(id);
        return;
    }
    if w.m(id).state == 8 {
        master_tick(w, id);
        register(w, id, MASTER_DRAW_FN);
        return;
    }
    c::set_pi32(w, id, o::FLAGS, 0);
    let a = w.m(id).anim;
    if a.flags & 2 != 0 && a.seq_a == a.seq_b {
        if a.seq_a == 2 { w.anim_blend(id, 3, 0, 3); }
        if w.m(id).anim.seq_a == 1 { w.anim_blend(id, 2, 0, 10); }
    }
    hits(w, id);
    if (1..=6).contains(&w.m(id).state) {
        let cam = crate::hero::physics::to_f32x3(w.camera);
        if c::dist3(c::pos(w, id), [cam[0], cam[1], cam[2], 0.0]) < 40.0 { crate::shadows::probe_down(w, id); }
        c::set_pi16(w, id, o::SHADOW, 0);
    }
    let mut st = w.m(id).state;
    if st == 0 {
        if let Some(r) = crate::moby_update::triggers::pvar_record(w.m(id)) { c::set_pu8(w, id, r + 0xe, 1); }
        if c::pi16(w, id, o::MASTER) == 1 {
            let n = scheduler::group_ids(w, w.m(id).group).len();
            let m = w.mm(id);
            m.has_collision = false;
            m.update_dist = 0xff;
            m.state = 8;
            m.mode = m.mode & 0xefff | 0x41;
            c::set_pu8(w, id, 0x58, 17);
            c::set_pu8(w, id, 0x5a, 6);
            c::set_pi16(w, id, o::COUNT, n as i16);
            return;
        }
        if !path_ok(w, c::pi32(w, id, o::PATH)) || !path_ok(w, c::pi32(w, id, o::LIGHT_PATH)) || c::pi32(w, id, o::GROUP) < 0 || !path_ok(w, c::pi32(w, id, o::EXIT_PATH)) {
            w.delete_moby(id);
            return;
        }
        w.mm(id).b7f = 0x20;
        c::set_pf(w, id, o::HEALTH, 4.0);
        c::set_pi16(w, id, o::HEALTH + 4, 4);
        c::set_pu8(w, id, o::HEALTH + 8, 3);
        let p0 = path_pts(w, c::pi32(w, id, o::PATH)).unwrap_or_default()[0];
        w.mm(id).position = p0;
        let l0 = path_pts(w, c::pi32(w, id, o::LIGHT_PATH)).unwrap_or_default()[0];
        c::set_pv4(w, id, o::LIGHT, l0);
        chords(w, id);
        c::set_pi16(w, id, o::ARMED, 1);
        c::set_pf(w, id, o::F21C, -1.0);
        for at in [o::LIGHT_V, o::LIGHT_V + 4, o::LIGHT_V + 8, o::POS_V, o::POS_V + 4, o::POS_V + 8, o::SPIN, o::FLAGS, o::FRAC, o::BOB, o::BOB + 4, o::WOBBLE, o::WOBBLE + 4, o::YAW_V] {
            c::set_pi32(w, id, at, 0);
        }
        c::set_pi16(w, id, o::YAW_T, 0);
        let yaw = w.m(id).rotation[2];
        c::set_pf(w, id, o::YAW_GOAL, yaw);
        w.mm(id).pvars[o::YAW_NODE..o::YAW_NODE + 0x80].fill(0);
        manip::attach(w, id, 0, id, o::YAW_NODE);
        manip::attach(w, id, 1, id, o::PITCH_NODE);
        for at in [o::HEAD_PITCH_V, o::HEAD_YAW_V, o::TARGET] { c::set_pi32(w, id, at, 0); }
        let m = w.mm(id);
        m.state = 1;
        m.cmd = 0;
        st = 1;
    } else {
        st = states(w, id, st);
        if st == 0xff { return; }
    }
    tail(w, id, st);
}

/// States 1..9; 0xff: returned without the tail.
fn states(w: &mut World, id: MobyId, st: u8) -> u8 {
    match st {
        1 => {
            w.mm(id).glow = 0xffaa_aaaa;
            if turret::any_raised(w, c::pi32(w, id, o::GROUP)) {
                w.mm(id).state = 3;
                let t = w.ticks(k::CHASE_TICKS);
                c::set_pi16(w, id, o::CHASE_T, t as i16);
            }
            c::set_pi16(w, id, o::ALERT, 0);
            c::set_pi16(w, id, o::SHADOW, 1);
            let seen = aim(w, id, true);
            light(w, id);
            follow(w, id);
            hover(w, id);
            if seen && spotted(w, id) { return alarm(w, id); }
            w.m(id).state
        }
        2 => alarm(w, id),
        3 => {
            w.mm(id).glow = 0xff00_ffff;
            let Some(t) = target(w, id).filter(|_| c::pi32(w, id, o::TARGET) != 0) else {
                c::set_pi32(w, id, o::EXIT_SEG, 0);
                c::set_pi32(w, id, o::EXIT_T, 0);
                return to_exit(w, id);
            };
            c::set_pi16(w, id, o::ALERT, 1);
            c::set_pi16(w, id, o::SHADOW, 1);
            aim(w, id, true);
            let tp = c::pos(w, t);
            let top = [tp[0], tp[1], tp[2] + 0.5, tp[3]];
            let z = match w.coll_line(pv(top), pv([top[0], top[1], top[2] - 4.0, top[3]]), 0x12, None) {
                Some(h) => h.point[2],
                None => top[2] - 0.5,
            };
            spring_pvar(w, id, [tp[0], tp[1], z], f32::from_bits(0x3b44_9ba6), 0.2, o::LIGHT, o::LIGHT_V);
            follow(w, id);
            if (w.m(t).state as i8) < 0 || c::dec_timer_pvar_s16(w, id, o::CHASE_T) != 0 {
                c::set_pi32(w, id, o::EXIT_T, 0);
                c::set_pi32(w, id, o::EXIT_SEG, 0);
                return to_exit(w, id);
            }
            out_state(w, id)
        }
        4 => out_state(w, id),
        5 => {
            hover(w, id);
            exit(w, id, 0.01, 5.0, -1.0);
            if c::pi16(w, id, o::EXIT_END) != 0 {
                let pts = path_pts(w, c::pi32(w, id, o::LIGHT_PATH)).unwrap_or_default();
                let p = c::pos(w, id);
                let cur = spline::nearest(&pts, false, 40.0, 5.0, 0.0, [p[0], p[1], p[2]]).map(|(_, k)| k).unwrap_or_default();
                c::set_pi32(w, id, o::LIGHT_SEG, cur.seg);
                c::set_pf(w, id, o::LIGHT_T, cur.t);
                c::set_pi16(w, id, o::SHADOW, 0);
                w.mm(id).state = 6;
            }
            w.m(id).state
        }
        6 => {
            hover(w, id);
            let q = follow_point(w, id);
            spring_pos(w, id, q, f32::from_bits(0x3ba3_d70a), k::D);
            aim(w, id, false);
            if c::dist3(c::pos(w, id), [q[0], q[1], q[2], 0.0]) < 0.5 { w.mm(id).state = 1; }
            let pts = path_pts(w, c::pi32(w, id, o::LIGHT_PATH)).unwrap_or_default();
            let mut cur = Cursor { seg: c::pi32(w, id, o::LIGHT_SEG), t: c::pf(w, id, o::LIGHT_T) };
            let (l, _) = spline::advance(&pts, c::pi16(w, id, o::CLOSED) != 0, 0.0, &mut cur);
            c::set_pi32(w, id, o::LIGHT_SEG, cur.seg);
            c::set_pf(w, id, o::LIGHT_T, cur.t);
            spring_pvar(w, id, l, k::K, k::D, o::LIGHT, o::LIGHT_V);
            w.m(id).state
        }
        7 => {
            die(w, id);
            flicker(w, id);
            0xff
        }
        9 => {
            if crash(w, id) { flicker(w, id); }
            0xff
        }
        s => s,
    }
}

/// Spheres along the beam (module doc): Ratchet, a decoy or a chicken listed first → kept as the target.
fn spotted(w: &mut World, id: MobyId) -> bool {
    let r = c::pf(w, id, o::RADIUS);
    let (spot, lamp) = (c::pv4(w, id, o::SPOT), c::pv4(w, id, o::LAMP));
    let mid: [f32; 4] = std::array::from_fn(|j| spot[j] + (lamp[j] - spot[j]) * 0.5);
    let mut first = w.sphere_mobys_list(Pf::f(r * 0.48), pv(mid), 0, None, None).first().copied();
    if first.is_none() { first = w.sphere_mobys_list(Pf::f(r * 0.9), pv(spot), 0, None, None).first().copied(); }
    let Some(m) = first else { return false };
    let ok = Some(m) == w.hero_moby || matches!(w.m(m).o_class, DECOY | CHICKEN);
    if ok { c::set_pi32(w, id, o::TARGET, m as i32 + 1); }
    ok
}

/// State 2: the hum released, the turrets raised → 3.
fn alarm(w: &mut World, id: MobyId) -> u8 {
    release_voice(w, id);
    turret::raise_group(w, c::pi32(w, id, o::GROUP));
    w.mm(id).state = 3;
    let t = w.ticks(k::CHASE_TICKS);
    c::set_pi16(w, id, o::CHASE_T, t as i16);
    3
}

fn to_exit(w: &mut World, id: MobyId) -> u8 {
    c::set_pi16(w, id, o::EXIT_END, 0);
    w.mm(id).state = 4;
    out_state(w, id)
}

/// State 4's body (state 3 runs its first part: the glow, the lamp glow flagged in view).
fn out_state(w: &mut World, id: MobyId) -> u8 {
    w.mm(id).glow = 0xff00_ffff;
    let p = c::pos(w, id);
    if fx::in_view(w, draw_dist(w, id), p, k::VIEW_R) {
        let g = w.joint_point(id, 2);
        c::set_pv4(w, id, o::GLOW, [g[0], g[1], g[2], c::pf(w, id, o::GLOW + 12)]);
        let f = c::pi32(w, id, o::FLAGS);
        c::set_pi32(w, id, o::FLAGS, f | 8);
        hover(w, id);
    }
    if w.m(id).state != 4 { return w.m(id).state; }
    if w.m(id).anim.seq_b == 0 { w.anim_blend(id, 2, 0, 5); }
    exit(w, id, k::EXIT_K, k::EXIT_SPEED, 1.0);
    if c::pf(w, id, o::HEAD_PITCH) != 0.0 || c::pf(w, id, o::HEAD_YAW) != 0.0 {
        for (x, v) in [(o::HEAD_PITCH, o::HEAD_PITCH_V), (o::HEAD_YAW, o::HEAD_YAW_V)] {
            let (mut a, mut b) = (c::pf(w, id, x), c::pf(w, id, v));
            spring(0.0, f32::from_bits(0x3b83_126f), 0.2, 0.0, &mut a, &mut b);
            c::set_pf(w, id, x, a);
            c::set_pf(w, id, v, b);
        }
        for x in [o::HEAD_PITCH, o::HEAD_YAW] {
            if c::pf(w, id, x).abs() < 0.0001 { c::set_pf(w, id, x, 0.0); }
        }
        let (y, pt) = (c::pf(w, id, o::HEAD_YAW), c::pf(w, id, o::HEAD_PITCH));
        manip::set_axis(w, id, id, o::YAW_NODE, y, 2);
        manip::set_axis(w, id, id, o::PITCH_NODE, pt, 1);
    }
    if !turret::lower_group(w, c::pi32(w, id, o::GROUP)) { return 4; }
    let pts = path_pts(w, c::pi32(w, id, o::EXIT_PATH)).unwrap_or_default();
    let n = pts.len() as i32 - 2;
    w.mm(id).state = 5;
    c::set_pi32(w, id, o::EXIT_SEG, n);
    let t = usize::try_from(n).ok().and_then(|n| pts.get(n)).map_or(0.0, |q| q[3]);
    c::set_pf(w, id, o::EXIT_T, t);
    c::set_pi16(w, id, o::EXIT_END, 0);
    exit(w, id, 0.01, 5.0, -1.0);
    let t = w.ticks(10);
    w.anim_blend(id, 0, 0, t);
    5
}

/// State 7: dead (module doc).
fn die(w: &mut World, id: MobyId) {
    let t = w.ticks(k::CRASH_TICKS);
    c::set_pi16(w, id, o::CRASH_T, t as i16);
    turret::raise_group(w, c::pi32(w, id, o::GROUP));
    release_voice(w, id);
    let p = c::pos(w, id);
    fx::beam_explosion(w, &DEATH, Some(id), p);
    let h = super::hero_pos(w);
    let away = [p[0] - h[0], p[1] - h[1], 0.0, 0.0];
    let l = w.rng.randf(k::FLING.0 * SPEED, k::FLING.1 * SPEED);
    let away = c::set_len3(away, l);
    let a = w.rng.randf_sym(f32::from_bits(0x3f32_b8c2), f32::from_bits(0x3f9c_61aa));
    let v = crate::follow_camera::rot(pv(away), Pf::f(a), w.hero.gravity_dir).map(|x| x.to_f32());
    c::set_pf(w, id, o::FLY, v[0]);
    c::set_pf(w, id, o::FLY + 4, v[1]);
    let up = w.rng.randf(k::FLING_UP.0 * SPEED, k::FLING_UP.1 * SPEED);
    c::set_pf(w, id, o::FLY + 8, up);
    w.mm(id).scale *= 0.1;
    lamp_scale(w, id);
    for j in 0..3 {
        let s = w.rng.randf_sym(0.0, f32::from_bits(0x3db2_b8c2));
        c::set_pf(w, id, o::SPINS + 4 * j, s);
    }
    for j in 0..2 {
        let s = w.rng.randf_sym(0.0, f32::from_bits(0x3f49_0fdb));
        let r = c::add_rot(w.m(id).rotation[j], s);
        w.mm(id).rotation[j] = r;
    }
    w.mm(id).state = 9;
}

/// The lamp's node (list 1) at ten times.
fn lamp_scale(w: &mut World, id: MobyId) {
    for j in 0..3 { c::set_pf(w, id, o::PITCH_NODE + manip::rec::SCALE + 4 * j, 10.0); }
    manip::sync(w, id, id, o::PITCH_NODE);
}

/// State 9: the flight (module doc); false: gone.
fn crash(w: &mut World, id: MobyId) -> bool {
    let mut v = c::pv4(w, id, o::FLY);
    let p = c::add(c::pos(w, id), [v[0], v[1], v[2], 0.0]);
    w.mm(id).position = [p[0], p[1], p[2], w.m(id).position[3]];
    let f = (k::DRAG - 1.0) * SPEED + 1.0;
    for x in v.iter_mut().take(2) {
        if k::DRAG_MIN * SPEED < x.abs() { *x *= f; }
    }
    if 0.0 < v[2] { v[2] *= f; }
    v[2] -= k::GRAVITY * SPEED * SPEED;
    c::set_pv4(w, id, o::FLY, v);
    let r = c::add_rot(w.m(id).rotation[2], c::pf(w, id, o::SPINS + 8));
    w.mm(id).rotation[2] = r;
    lamp_scale(w, id);
    let p = c::pos(w, id);
    if !super::oltanis_zapper::in_box(p) {
        w.delete_moby(id);
        return false;
    }
    if c::dec_timer_pvar_s16(w, id, o::CRASH_T) == 0 && w.coll_sphere(pv(p), Pf::f(0.5), 0, Some(id)).is_none() {
        let life = { let (a, b) = (w.ticks(10), w.ticks(0x14)); w.rng.rand_range(a, b) };
        fx::part21(w, 60000.0, p, [0.0; 4], 0x4f00_7fff, 0x1fff_ffff, life, 1);
        super::oltanis_zapper::puffs(w, p, [0.0; 4], 2, 40000.0, 100_000.0);
        return true;
    }
    fx::beam_explosion(w, &CRASH, Some(id), p);
    w.delete_moby(id);
    false
}

/// The tail: a burning hit's sparks while key B is 1; the hum in 1, the alarm in 3 / 4.
fn tail(w: &mut World, id: MobyId, st: u8) {
    if (st as i8) < 0 || w.m(id).anim.seq_b != 1 {
        w.mm(id).cmd = 0;
    } else if w.m(id).cmd == 1 {
        crate::moby_update::creature::knock::burn_sparks(w, id, [0.0; 4], 0xffff);
    }
    let st = w.m(id).state;
    for (s, sound) in [(1, 0), (3, 2), (4, 2)] {
        if st == s && !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
            let v = w.play_sound(sound, 4, id);
            c::set_pi32(w, id, o::VOICE, v);
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The draws.

/// The draws' frame part: the camera kept; the master also turns its flagged sentries' glows (−4° a draw).
pub fn frame(w: &mut World, id: MobyId) {
    let cam = crate::hero::physics::to_f32x3(w.camera);
    cone(w).cam = cam;
    if w.m(id).state != 8 { return; }
    for m in scheduler::group_ids(w, w.m(id).group) {
        if w.m(m).o_class != CLASSES[0] || w.m(m).pvars.len() < o::SIZE || w.m(m).state >= 0xfd { continue; }
        if c::pi32(w, m, o::FLAGS) & 8 == 0 { continue; }
        let a = c::pf(w, m, o::SPIN);
        c::set_pf(w, m, o::GLOW_ANGLE, a);
        c::set_pf(w, m, o::SPIN, c::add_rot(a, f32::from_bits(0xbd8e_fa35)));
    }
}

/// `fun_001fa030` of the Euler angles (0, −pitch, yaw) facing `cam` from `p`.
fn facing(cam: [f32; 3], p: [f32; 3]) -> [[f32; 3]; 3] {
    let d = [cam[0] - p[0], cam[1] - p[1], cam[2] - p[2], 0.0];
    let yaw = c::atan(d[0], d[1]);
    let pitch = -c::atan(c::len2(d), d[2]);
    let r = euler_rows(pv([0.0, pitch, yaw, 0.0])).map(|row| row.map(|x| f32::from_bits(x.0)));
    [[r[0][0], r[0][1], r[0][2]], [r[1][0], r[1][1], r[1][2]], [r[2][0], r[2][1], r[2][2]]]
}
fn xf(m: &[[f32; 3]; 3], v: [f32; 3], t: [f32; 3]) -> [f32; 3] { std::array::from_fn(|j| v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j] + t[j]) }
fn quad(corners: [[f32; 3]; 4], rgba: u32) -> FxQuad { FxQuad { corners, st: ST, rgba: [rgba; 4] } }
fn ff3(pvars: &[u8], at: usize) -> [f32; 3] { [p::ff(pvars, at), p::ff(pvars, at + 4), p::ff(pvars, at + 8)] }

/// Level14 `0x2aee28` (module doc): FX 0x16 (the cones) and FX 0xb (the flares and glows), additive.
pub fn fx_quad_groups(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<FxQuads> {
    let Some(master) = table.mobys.get(id) else { return Vec::new() };
    let g = &svc.units.oltanis_sentry;
    let cam = g.cam;
    let mut list: Vec<(f32, MobyId)> = svc.groups.lists.get(master.group.max(0) as usize).and_then(|l| l.clone()).unwrap_or_default().into_iter()
        .map(|m| m as MobyId)
        .filter(|&m| table.mobys.get(m).is_some_and(|x| x.o_class == CLASSES[0] && x.pvars.len() >= o::SIZE && x.state < 0xfd))
        .map(|m| { let q = table.mobys[m].position; (c::dist3(q, [cam[0], cam[1], cam[2], 0.0]), m) })
        .collect();
    if master.group < 0 { list.clear(); }
    list.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (mut cones, mut glows) = (Vec::new(), Vec::new());
    let colours = g.colours();
    for (_, m) in list {
        let pvs = &table.mobys[m].pvars;
        let flags = p::i32(pvs, o::FLAGS);
        let alert = p::i16(pvs, o::ALERT) != 0;
        if flags & 8 != 0 { glow_quads(pvs, cam, &mut glows); }
        if flags & 2 != 0 { cone_quads(pvs, cam, alert, &colours, &mut cones, &mut glows); }
    }
    vec![FxQuads { fx: 0x16, additive: true, subtract: false, quads: cones }, FxQuads { fx: 0xb, additive: true, subtract: false, quads: glows }]
}

/// `0x2add48`: 16 layers (sizes 0.1 + 0.2·i, offsets 0.025 + 0.25·i along the glow's angle, both ways), colours
/// (0x7f, 0x64, 0x46, 0x5a) less (0, 0x10, 0x10, 8) a layer.
fn glow_quads(pvs: &[u8], cam: [f32; 3], out: &mut Vec<FxQuad>) {
    let g = ff3(pvs, o::GLOW);
    let at = [g[0], g[1], g[2] + 0.125];
    let a = p::ff(pvs, o::GLOW_ANGLE);
    let (cs, sn) = (a.cos(), a.sin());
    let m = facing(cam, at);
    let (mut r, mut gg, mut b, mut al) = (0x7f_i32, 0x64_i32, 0x46_i32, 0x5a_i32);
    let (mut off, mut size) = (0.025_f32, 0.1_f32);
    for _ in 0..16 {
        let rgba = (al as u32) << 24 | (b as u32) << 16 | (gg as u32) << 8 | r as u32;
        let d = [cs * off, sn * off, 0.0];
        let base = FACING.map(|v| xf(&m, v.map(|x| x * size), at));
        out.push(quad(base.map(|q| [q[0] + d[0], q[1] + d[1], q[2]]), rgba));
        out.push(quad(base.map(|q| [q[0] - d[0], q[1] - d[1], q[2]]), rgba));
        gg = (gg - 0x10).max(0);
        al = (al - 8).max(0);
        r = r.max(0);
        b = (b - 0x10).max(0);
        size += 0.2;
        off += 0.25;
    }
}

/// `0x2ada18` with `0x2ae0a8`: the lamp's flare (0.5 out along the lamp's row 0, 0.75, 0x7f7f7f7f or red), then the
/// cone's half mesh in the frame (row 0, side, row 0 × side) at the lamp, drawn again mirrored.
fn cone_quads(pvs: &[u8], cam: [f32; 3], alert: bool, colours: &[u32; 82], cones: &mut Vec<FxQuad>, glows: &mut Vec<FxQuad>) {
    let r0 = ff3(pvs, o::ROWS);
    let lamp = ff3(pvs, o::LAMP);
    let fp: [f32; 3] = std::array::from_fn(|j| lamp[j] + r0[j] * k::FLARE_OUT);
    let m = facing(cam, fp);
    let rgba = if alert { k::ALERT_RGBA } else { 0x7f7f_7f7f };
    glows.push(quad(FACING.map(|v| xf(&m, v.map(|x| x * k::FLARE), fp)), rgba));
    let d = [lamp[0] - cam[0], lamp[1] - cam[1], lamp[2] - cam[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let side = cross(d, r0);
    let n = (side[0] * side[0] + side[1] * side[1] + side[2] * side[2]).sqrt();
    let side = if n == 0.0 { side } else { side.map(|x| x / n) };
    let up = cross(r0, side);
    let rgba: [u32; 82] = std::array::from_fn(|i| if alert { colours[i] & 0xff00_00ff } else { colours[i] });
    for sgn in [1.0f32, -1.0] {
        let f = [r0, side.map(|x| x * sgn), up.map(|x| x * sgn)];
        let pts: Vec<[f32; 3]> = CONE_POINTS.iter().map(|v| xf(&f, v.map(f32::from_bits), lamp)).collect();
        // The strip as quads: triangles 2i, 2i + 1 are the quad's (corners 2i..2i + 3).
        for i in (0..pts.len() - 3).step_by(2) {
            let st = [0, 1, 2, 3].map(|j| CONE_ST[i + j].map(f32::from_bits));
            cones.push(FxQuad { corners: [pts[i], pts[i + 1], pts[i + 2], pts[i + 3]], st, rgba: [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]] });
        }
    }
}

/// Level14 `0x2ae260`: the spot (FX 0xb, additive): a flat quad of half-size +0x214 at the light just above the
/// ground, drawn up to +0x218 toward the camera but not within 2 of it; 0x40507f7f, red when alert.
pub fn spot_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= o::SIZE)?;
    let cam = svc.units.oltanis_sentry.cam;
    let s = ff3(&m.pvars, o::SPOT);
    let mut at = [s[0], s[1], s[2] + 0.01];
    let d = [cam[0] - at[0], cam[1] - at[1], cam[2] - at[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if 0.0 < l {
        let mut pull = p::ff(&m.pvars, o::PULL);
        let room = l - k::SPOT_GAP;
        if room < pull { pull = if room < 0.0 { 0.0 } else { room }; }
        for j in 0..3 { at[j] += d[j] / l * pull; }
    }
    let r = p::ff(&m.pvars, o::RADIUS);
    let rgba = if p::i16(&m.pvars, o::ALERT) != 0 { k::ALERT_RGBA } else { k::SPOT_RGBA };
    let corners = FLAT.map(|v| [v[0] * r + at[0], v[1] * r + at[1], v[2] * r + at[2]]);
    Some(FxQuads { fx: 0xb, additive: true, subtract: false, quads: vec![quad(corners, rgba)] })
}

/// Level14 0x1d7a60: the cone's half mesh, one strip of 82 points (bits).
const CONE_POINTS: [[u32; 3]; 82] = [
    [0x406dbbae, 0xc06be996, 0x80000000],
    [0x4067399f, 0xbf95167f, 0x80000000],
    [0x3f3e43bb, 0xbf1e005c, 0x00000000],
    [0x3f56b789, 0xbedd6c51, 0x80000000],
    [0x3b775537, 0xbe98fa7b, 0x00000000],
    [0x3f56b789, 0xbedd6c51, 0x80000000],
    [0xb928d21c, 0xbe581733, 0x00000000],
    [0x3f56b789, 0xbedd6c51, 0x80000000],
    [0xb928d21c, 0xbe4d8384, 0x3d854de8],
    [0x3f56b789, 0xbedd6c51, 0x80000000],
    [0x3f77ae36, 0xbee322f2, 0x3e13547e],
    [0x4067399f, 0xbf95167f, 0x80000000],
    [0x3f77ae36, 0xbee322f2, 0x3e13547e],
    [0x4078cf29, 0xbf969eb2, 0x3ec36567],
    [0x3f77ae36, 0xbee322f2, 0x3e13547e],
    [0x407a66f1, 0xbf80ce25, 0x3f3ad1bf],
    [0x3f77ae36, 0xbee322f2, 0x3e13547e],
    [0x3f79a22f, 0xbec20c4a, 0x3e8cb945],
    [0xb928d21c, 0xbe4d8384, 0x3d854de8],
    [0x3f79a22f, 0xbec20c4a, 0x3e8cb945],
    [0xb928d21c, 0xbe2ed203, 0x3dfd8f93],
    [0x3f79a22f, 0xbec20c4a, 0x3e8cb945],
    [0xb928d21c, 0xbdfe07a3, 0x3e2e7f6f],
    [0x3f79a22f, 0xbec20c4a, 0x3e8cb945],
    [0x3f7a046c, 0xbe8d1a65, 0x3ec1da7b],
    [0x407a66f1, 0xbf80ce25, 0x3f3ad1bf],
    [0x3f7a046c, 0xbe8d1a65, 0x3ec1da7b],
    [0x4074fb16, 0xbf37cce2, 0x3f7c8334],
    [0x3f7a046c, 0xbe8d1a65, 0x3ec1da7b],
    [0x40653719, 0xbeb6f8ad, 0x3f8c8598],
    [0x3f7a046c, 0xbe8d1a65, 0x3ec1da7b],
    [0x3f45af3a, 0xbe034a45, 0x3ec9a933],
    [0xb928d21c, 0xbdfe07a3, 0x3e2e7f6f],
    [0x3f45af3a, 0xbe034a45, 0x3ec9a933],
    [0xb928d21c, 0xbd858d58, 0x3e4d2285],
    [0x3f45af3a, 0xbe034a45, 0x3ec9a933],
    [0xb928d21c, 0x00000000, 0x3e57b0f7],
    [0x3f45af3a, 0xbe034a45, 0x3ec9a933],
    [0x3f227547, 0x00000000, 0x3ec179e1],
    [0x40653719, 0xbeb6f8ad, 0x3f8c8598],
    [0x3f227547, 0x00000000, 0x3ec179e1],
    [0x4051fa01, 0x00000000, 0x3f899d91],
    [0x3f227547, 0x00000000, 0x3ec179e1],
    [0x4052e350, 0x3eab02f7, 0x3f83563b],
    [0x3f227547, 0x00000000, 0x3ec179e1],
    [0x3f2a69be, 0x3df4c87a, 0x3ebbfe3b],
    [0xb928d21c, 0x00000000, 0x3e57b0f7],
    [0x3f2a69be, 0x3df4c87a, 0x3ebbfe3b],
    [0xb928d21c, 0x3d858d58, 0x3e4d2285],
    [0x3f2a69be, 0x3df4c87a, 0x3ebbfe3b],
    [0xb928d21c, 0x3dfe07a3, 0x3e2e7f6f],
    [0x3f2a69be, 0x3df4c87a, 0x3ebbfe3b],
    [0x3f4fdc16, 0x3e800581, 0x3eafe176],
    [0x4052e350, 0x3eab02f7, 0x3f83563b],
    [0x3f4fdc16, 0x3e800581, 0x3eafe176],
    [0x405edb1f, 0x3f2a11ba, 0x3f69a5ec],
    [0x3f4fdc16, 0x3e800581, 0x3eafe176],
    [0x406a8a1a, 0x3f740f77, 0x3f30fe37],
    [0x3f4fdc16, 0x3e800581, 0x3eafe176],
    [0x3f5bff47, 0x3eb563ed, 0x3e838b69],
    [0xb928d21c, 0x3dfe07a3, 0x3e2e7f6f],
    [0x3f5bff47, 0x3eb563ed, 0x3e838b69],
    [0xb928d21c, 0x3e2ed203, 0x3dfd8f93],
    [0x3f5bff47, 0x3eb563ed, 0x3e838b69],
    [0xb928d21c, 0x3e4d8384, 0x3d854de8],
    [0x3f5bff47, 0x3eb563ed, 0x3e838b69],
    [0x3f4cca3e, 0x3ecd99ed, 0x3e055c96],
    [0x406a8a1a, 0x3f740f77, 0x3f30fe37],
    [0x3f4cca3e, 0x3ecd99ed, 0x3e055c96],
    [0x406eb1e2, 0x3f918aa0, 0x3ebccec4],
    [0x3f3a4b77, 0x3ece6b1e, 0x00000000],
    [0x4070c9a3, 0x3f9a2318, 0x00000000],
    [0x3f3a4b77, 0x3ece6b1e, 0x00000000],
    [0x4070c9a3, 0x3f9a2318, 0x00000000],
    [0x3f3a4b77, 0x3ece6b1e, 0x00000000],
    [0x3f3a4b77, 0x3ece6b1e, 0x00000000],
    [0x3f3a4b77, 0x3ece6b1e, 0x00000000],
    [0xb928d21c, 0x3e581733, 0x00000000],
    [0x3f3a4b77, 0x3ece6b1e, 0x00000000],
    [0xb928d21c, 0x3e581733, 0x00000000],
    [0x3f4cca3e, 0x3ecd99ed, 0x3e055c96],
    [0xb928d21c, 0x3e4d8384, 0x3d854de8],
];
/// Level14 0x1d7e38: its ST (bits).
const CONE_ST: [[u32; 2]; 82] = [
    [0x3f000000, 0x00000000], [0x3f000000, 0x00000000], [0x3f000000, 0x00000000], [0x3f000000, 0x3e99999a],
    [0x3f000000, 0x00000000], [0x3f000000, 0x3e99999a], [0x3f000000, 0x3f800000], [0x3f000000, 0x3e99999a],
    [0x3f000000, 0x3f800000], [0x3f000000, 0x3e99999a], [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x00000000], [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x3dcccccd], [0x3f000000, 0x3f800000], [0x3f000000, 0x3dcccccd],
    [0x3f000000, 0x3f800000], [0x3f000000, 0x3dcccccd], [0x3f000000, 0x3f800000], [0x3f000000, 0x3dcccccd],
    [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x00000000], [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x00000000], [0x3f000000, 0x3e4ccccd], [0x3f000000, 0x3e99999a],
    [0x3f000000, 0x3f800000], [0x3f000000, 0x3e99999a], [0x3f000000, 0x3f800000], [0x3f000000, 0x3e99999a],
    [0x3f000000, 0x3f800000], [0x3f000000, 0x3e99999a], [0x3f000000, 0x3ecccccd], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3ecccccd], [0x3f000000, 0x00000000], [0x3f000000, 0x3ecccccd], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3ecccccd], [0x3f000000, 0x3f000000], [0x3f000000, 0x3f800000], [0x3f000000, 0x3f000000],
    [0x3f000000, 0x3f800000], [0x3f000000, 0x3f000000], [0x3f000000, 0x3f800000], [0x3f000000, 0x3f000000],
    [0x3f000000, 0x3f19999a], [0x3f000000, 0x00000000], [0x3f000000, 0x3f19999a], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3f19999a], [0x3f000000, 0x00000000], [0x3f000000, 0x3f19999a], [0x3f000000, 0x3f333333],
    [0x3f000000, 0x3f800000], [0x3f000000, 0x3f333333], [0x3f000000, 0x3f800000], [0x3f000000, 0x3f333333],
    [0x3f000000, 0x3f800000], [0x3f000000, 0x3f333333], [0x3f000000, 0x3f4ccccd], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3f4ccccd], [0x3f000000, 0x00000000], [0x3f000000, 0x3f666666], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3f666666], [0x3f000000, 0x00000000], [0x3f000000, 0x3f666666], [0x3f000000, 0x00000000],
    [0x3f000000, 0x3f666666], [0x3f000000, 0x00000000], [0x3f000000, 0x3f666666], [0x3f000000, 0x3f800000],
    [0x3f000000, 0x3f4ccccd], [0x3f000000, 0x3f800000],
];
/// Level14 0x1d80c8: its colours as loaded (GS RGBA).
const CONE_RGBA: [u32; 82] = [
    0x00507f7f, 0x00507f7f, 0x00507f7f, 0x10507f7f, 0x00507f7f, 0x16507f7f, 0x7f507f7f, 0x20507f7f,
    0x7f507f7f, 0x26507f7f, 0x30507f7f, 0x00507f7f, 0x36507f7f, 0x00507f7f, 0x40507f7f, 0x00507f7f,
    0x46507f7f, 0x50507f7f, 0x7f507f7f, 0x46507f7f, 0x7f507f7f, 0x40507f7f, 0x7f507f7f, 0x36507f7f,
    0x30507f7f, 0x00507f7f, 0x26507f7f, 0x00507f7f, 0x20507f7f, 0x00507f7f, 0x16507f7f, 0x10507f7f,
    0x7f507f7f, 0x16507f7f, 0x7f507f7f, 0x20507f7f, 0x7f507f7f, 0x26507f7f, 0x30507f7f, 0x00507f7f,
    0x36507f7f, 0x00507f7f, 0x40507f7f, 0x00507f7f, 0x46507f7f, 0x50507f7f, 0x7f507f7f, 0x46507f7f,
    0x7f507f7f, 0x40507f7f, 0x7f507f7f, 0x36507f7f, 0x30507f7f, 0x00507f7f, 0x26507f7f, 0x00507f7f,
    0x20507f7f, 0x00507f7f, 0x16507f7f, 0x10507f7f, 0x7f507f7f, 0x16507f7f, 0x7f507f7f, 0x20507f7f,
    0x7f507f7f, 0x26507f7f, 0x30507f7f, 0x00507f7f, 0x36507f7f, 0x00507f7f, 0x40507f7f, 0x00507f7f,
    0x46507f7f, 0x00507f7f, 0x50507f7f, 0x00507f7f, 0x46507f7f, 0x00507f7f, 0x40507f7f, 0x7f507f7f,
    0x36507f7f, 0x7f507f7f,
];
/// Level14 0x1d8210: the colours' alpha phases as loaded (0: the alpha stays).
const CONE_PHASES: [i32; 82] = [
    0, 0, 0, 48, 0, 48, 0, 48, 0, 48, 32, 0, 32, 0, 32, 0,
    32, 16, 0, 16, 0, 16, 0, 16, 32, 0, 32, 0, 32, 0, 32, 48,
    0, 48, 0, 48, 0, 48, 64, 0, 64, 0, 64, 0, 64, 80, 0, 80,
    0, 80, 0, 80, 64, 0, 64, 0, 64, 0, 64, 48, 0, 48, 0, 48,
    0, 48, 32, 0, 32, 0, 16, 0, 16, 0, 16, 16, 16, 0, 16, 0,
    32, 0,
];
