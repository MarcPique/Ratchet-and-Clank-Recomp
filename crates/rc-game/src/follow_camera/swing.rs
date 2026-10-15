//! **The Swingshot camera** (camera class 7; level01: activation `0x318b10` = 0, init `0x318030`, update `0x318b18` →
//! `0x318ad0` (`0x3182c8` place, `0x318900` rows) → the collision push `0x20f2a8`, pre hook `0x318c08`): the camera
//! while Ratchet pulls himself to a target (states 0x24..0x26) or swings from one (0x2c, then the fall 0x2d). Every
//! level has one class-7 record (priority 6, activation kind 7 = the hero's camera mode 0x1415d4 = 7, blend kind 3);
//! the code is one copy on all 19 levels (masked overlay diff: identical; the level tests inside are data).
//! Spec: docs/plan/player_controller.md §15 "Class 7".
//!
//! * **Switch in** (`FUN_0020d110` with the record's blend kind 3: the follow camera's pose copied, no blend; the init
//!   then starts one): the init `0x318030` takes the previous camera's rows and position, the saved forward = its
//!   forward, and aims at the target: swinging (0x2c / 0x2d) along the hero's yaw to the swung-on target (0x13fcfc),
//!   else flat toward the pull target 0x13fcb4; it keeps that target (D+0xc0). Distance 4.64, height 2.0, look height
//!   1.5 (2.0 on level 14 while swinging), and the **orbit blend** from the previous camera over 60 ticks (0x167370 = 1,
//!   0x167373 = 2, 0x1673f4 = 60: [`super::CamBlend`]).
//! * **Update** (`0x318b18`): in the swing (0x2c) or the pull (0x24..0x26), a new target re-inits the camera from its
//!   own pose with a 45-tick orbit blend. Then `0x3182c8`: the distance → 4.64 and the height → 2.0 at 8 / 4 u/s
//!   (swinging: 3.65 + f and 2f with f = the target's height above Ratchet over the rope length, clamped 0..1), the look
//!   height → height − 0.5 (level 14 swinging: + 0.25) at 8 u/s; the yaw springs toward Ratchet's yaw (0x13f3e8; k
//!   0.00125 swinging, 0.0035 pulling (0x25), d 0.175; not in 0x24 / 0x26 / 0x2d); on levels 14, 7 and 9 while
//!   swinging it looks along the target's moby group instead: the nearest swing / pull target (classes 0x323 / 0x2f6)
//!   ahead of Ratchet by `distance − 10·cos` (below 1000). The camera goes to Ratchet − dir·distance + height; a wall
//!   on the line from where it is (`CollLine_Fix` flags 0x12) stops it 0.5 short, pushed out of 0.75 spheres (at most
//!   5 times), and the yaw then faces Ratchet from there. `0x318900`: the forward turns toward the look point
//!   (Ratchet + look height) by an angle spring (k 0.02, d 0.175; pulling (0x25): a ramp of 0.017 per tick to the
//!   full angle), rows from the smoothed up 0x1672b0. Last the collision push `0x20f2a8(0.5)` and D+0x80 = 0.
//! * **Release** (pre hook `0x318c08`): the hero's camera mode no longer 7 and not falling (state 6) → +0x7e = 3; the
//!   follow camera's slot then wins the choice (a releasing best yields) and is switched in with the pose copied
//!   (+0x7e = 3, its record's blend kind 0: no blend), its init `0x311dd0` (the target, pivot and look placed about
//!   Ratchet under the copied camera, no snap) with the row blend D+0x20 = 0 when Ratchet is on the ground, else 90
//!   ticks.
//!
//! **The collision push** `0x20f2a8(r, pos)` (its one caller on every level is this camera; the script camera's
//! `collide` flag is set by no ported caller): from the previous position (+0x64) to `pos` in `trunc(d / 0.9r) + 1`
//! equal steps, each pushed out of spheres of radius r (`coll_sphere` with the sphere flags 0x15ef5c, ignoring Ratchet,
//! at most 6 times; +0x89 = 1 when pushed); the published position 0x167240 = the result. +0x8a ≠ 0 (set by
//! `CameraScript2(0)` on the script camera only) would skip it once.
//!
//! Native `f32` (the follow camera is the PS2-exact one).

use super::{fadd, fcross, fdot, fnorm, frot, fscale, fsub, line_hit, sphere, CamInput, Camera, R3};
use super::script::angle_spring;
use crate::hero::physics::{from_f32x3, to_f32x3};
use crate::ps2v::Pf;
use std::f32::consts::FRAC_PI_2;

/// The camera class.
pub const CLASS_SWING: i32 = 7;
/// The level-01 functions of class 7 ([`super::level::CameraPorts::from_overlays`] looks for their copies).
pub const SWING_ACTIVATE: u32 = 0x31_8b10;
pub const SWING_INIT: u32 = 0x31_8030;
pub const SWING_UPDATE: u32 = 0x31_8b18;
pub const SWING_PRE: u32 = 0x31_8c08;

/// `0x15ed6c`: the tick (NTSC).
const DT: f32 = 1.0 / 60.0;
/// Hero states: the pull (0x24 fire, 0x25 pull, 0x26 arrive), the swing and the fall after it, the fall.
const ST_PULL: i32 = 0x25;
const ST_SWING: i32 = 0x2c;
const ST_SWING_FALL: i32 = 0x2d;
const ST_FALL: i32 = 6;
/// The Swingshot target classes the level-14 / 7 / 9 look accepts (`+0xa6`): swing 0x323 (803), pull 0x2f6 (758).
const LOOK_CLASSES: [i16; 2] = [0x323, 0x2f6];

/// `FastVecCross(out, a, b)` = b × a.
fn vcross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { fcross(b, a) }
/// `FastArcSin`, native (the argument clamped).
fn asin(x: f32) -> f32 { x.clamp(-1.0, 1.0).asin() }
/// `Approach(t, step, &x)` 0x270728, native.
fn approach(t: f32, step: f32, x: &mut f32) {
    let d = (t - *x).clamp(-step, step);
    *x += d;
}

/// A member of the swung-on target's moby group (`0x1abcc0[target +0x21]`, list order), as the tick feeds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupMember {
    pub id: usize,
    /// `+0xa6`.
    pub class: i16,
    /// `+0x10`.
    pub pos: [f32; 3],
}

/// The swung-on target 0x13fce4 as `0x3182c8` reads it: its group byte `+0x21`, position and its group's members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwingGroup {
    pub group: i8,
    pub on_pos: [f32; 3],
    pub members: Vec<GroupMember>,
}

/// The class-7 camera: its UpdateCam words and the D-block words it uses (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwingCamera {
    /// The Swingshot camera is the current one (0x167280 +0x86 = 7).
    pub active: bool,
    /// UpdateCam +0x00 / +0x10 / +0x20 rows (forward, left, up), +0x30 position, +0x40 saved forward, +0x64 the
    /// position before this tick's update.
    pub rows: R3,
    pub pos: [f32; 3],
    pub saved_fwd: [f32; 3],
    pub prev_pos: [f32; 3],
    /// +0x7e: 3 when the pre hook released it.
    pub release: u8,
    /// +0x89: the collision push moved the camera (set, never cleared: no reader in the camera code [L]).
    pub pushed: bool,
    /// D+0x70: the flat direction toward Ratchet (from the yaw).
    pub dir: [f32; 3],
    /// D+0x8c look height, D+0xa0 distance, D+0xb0 height above Ratchet.
    pub look_h: f32,
    pub dist: f32,
    pub height: f32,
    /// D+0xb8 yaw and D+0xbc its spring velocity.
    pub yaw: f32,
    pub yaw_vel: f32,
    /// D+0xc0: the target the camera was set up for (0x13fce4 swinging, 0x13fcb4 pulling).
    pub target: Option<usize>,
    /// D+0x30: the pitch turn's spring velocity (pulling, 0x25: the ramp 0..1).
    pub pitch_vel: f32,
    /// D+0x80 = 1 with D+0x84's yaw: a class's moby to look along (Oltanis's flying cars 1417, `0x3135b8`); cleared
    /// at the init and after every update.
    pub follow_yaw: Option<f32>,
}

impl Camera {
    /// The pre hook `0x318c08` of the current class-7 camera: released (+0x7e = 3, +0x7d = 0) when the hero's camera
    /// mode is not 7 and he is not falling (state 6).
    pub(super) fn swing_pre(&mut self, inp: &CamInput) {
        let h = inp.hero;
        if h.f15d4 != CLASS_SWING && h.state != ST_FALL { self.swing.release = 3; }
    }

    /// The init `0x318030` from the previous camera's rows and position (`0x167284`).
    pub(super) fn swing_init(&mut self, inp: &CamInput, prev: (R3, [f32; 3])) {
        let h = inp.hero;
        let swinging = h.state == ST_SWING || h.state == ST_SWING_FALL;
        let level = self.level_cams.level;
        let s = &mut self.swing;
        // D+0x80 = 0; D+0x8c; D+0x88 (12°), D+0x98, D+0x9c, D+0x84 (Ratchet's moby), D+0xa4 (0.5), D+0xb4 (0.05), D+0x00,
        // D+0x10..0x1e, D+0x34..0x3c: written, read by no class-7 code (the follow camera's init rewrites its own).
        s.follow_yaw = None;
        s.look_h = if level == 0xe && swinging { 2.0 } else { 1.5 };
        s.dist = 4.64;
        s.height = 2.0;
        s.pitch_vel = 0.0;
        s.yaw_vel = 0.0;
        s.rows = prev.0;
        s.pos = prev.1;
        s.saved_fwd = prev.0[0];
        s.release = 0;
        if swinging {
            let y = h.swing.yaw;
            s.dir = [y.cos(), y.sin(), 0.0];
            s.yaw = y;
            s.target = h.swing.on;
        } else {
            let d = fsub(h.swing.pull_pos, s.pos);
            s.dir = fnorm([d[0], d[1], 0.0], 1.0);
            s.yaw = s.dir[1].atan2(s.dir[0]);
            s.target = h.swing.pull;
        }
        // The orbit blend from the previous camera: 0x167370 = 1, 0x167373 = 2, 0x1673f4 = 60 ticks.
        self.blend.mode = 1;
        self.blend.kind = 2;
        self.blend.orbit_len = 60;
    }

    /// The update `0x318b18`; returns the pose a target change re-initialised from (the blend's capture).
    pub(super) fn swing_update(&mut self, inp: &CamInput) -> Option<(R3, [f32; 3])> {
        let h = inp.hero;
        let now = match h.state {
            ST_SWING => Some(h.swing.on),
            0x24..=0x26 => Some(h.swing.pull),
            _ => None,
        };
        let mut prev = None;
        if let Some(t) = now {
            if t != self.swing.target {
                // The own pose into the previous camera's UpdateCam (0x167284), the init from it, a 45-tick blend.
                let own = (self.swing.rows, self.swing.pos);
                self.swing_init(inp, own);
                self.blend.orbit_len = 45;
                prev = Some(own);
            }
        }
        self.swing_place(inp);
        self.swing_rows(inp);
        let (p, pushed) = self.collision_push(inp, 0.5, self.swing.prev_pos, self.swing.pos);
        self.swing.pos = p;
        if pushed { self.swing.pushed = true; }
        self.swing.follow_yaw = None;
        prev
    }

    /// `0x3182c8`: distance, height, look height, the yaw and the position (module doc).
    fn swing_place(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let hero = to_f32x3(h.pos);
        let level = self.level_cams.level;
        let st = h.state;
        let (dist_t, h_t) = match (st, h.swing.on) {
            (ST_SWING, Some(_)) => {
                let len = h.swing.rope_target;
                let dz = h.swing.on_pos[2] - hero[2];
                let mut f = len;
                if dz <= len {
                    f = dz;
                    if dz < 0.0 { f = 0.0; }
                }
                let f = f / len;
                (f + 3.65, f + f)
            }
            _ => (4.64, 2.0),
        };
        let s = &mut self.swing;
        approach(dist_t, DT * 8.0, &mut s.dist);
        approach(h_t, DT * 4.0, &mut s.height);
        let look_t = if level == 0xe && st == ST_SWING { h_t + 0.25 } else { h_t - 0.5 };
        approach(look_t, DT * 8.0, &mut s.look_h);
        // The yaw: toward Ratchet's yaw (0x13f3e8), or along the target group on levels 14 / 7 / 9; with D+0x80 = 1 (a
        // class's store, [`SwingCamera::follow_yaw`]) the yaw of the moby D+0x84 instead where no group target leads
        // (and while pulling).
        let hero_yaw = h.rot[2].to_f32();
        let follow = s.follow_yaw;
        let spring = match st {
            ST_SWING => {
                let mut tgt = hero_yaw;
                if matches!(level, 0xe | 7 | 9) {
                    match self.world.swing_group.as_ref() {
                        Some(g) if h.swing.on.is_none() || g.group != -1 => match next_target(g, h.swing.on, hero_yaw) {
                            Some(best) => tgt = fsub(best, g.on_pos)[1].atan2(fsub(best, g.on_pos)[0]),
                            None => tgt = follow.unwrap_or(hero_yaw),
                        },
                        Some(_) => {}
                        None => tgt = follow.unwrap_or(hero_yaw),
                    }
                }
                Some((tgt, 0.00125))
            }
            ST_PULL => Some((follow.unwrap_or(hero_yaw), f32::from_bits(0x3b65_6042))),
            _ => None,
        };
        if let Some((tgt, k)) = spring { s.yaw = angle_spring(s.yaw, tgt, k, 0.175, 0.0, &mut s.yaw_vel); }
        s.dir = [s.yaw.cos(), s.yaw.sin(), 0.0];
        let mut p = fnorm(s.dir, -s.dist);
        p[2] += s.height;
        let p = fadd(p, hero);
        let from = s.pos;
        match line_hit(inp, from_f32x3(from), from_f32x3(p), 0x12, None) {
            None => s.pos = p,
            Some(hit) => {
                let mut q = p;
                let w = fsub(to_f32x3(hit), from);
                let l = fdot(w, w).sqrt();
                if 0.5 < l {
                    q = fadd(from, fscale(w, (l - 0.5) / l));
                    for _ in 0..5 {
                        match sphere(inp, Pf::f(0.75), from_f32x3(q), 0, None) {
                            Some((_, c)) => q = to_f32x3(c),
                            None => break,
                        }
                    }
                }
                let s = &mut self.swing;
                s.pos = q;
                let d = fsub(hero, q);
                s.yaw = d[1].atan2(d[0]);
                s.yaw_vel = 0.0;
            }
        }
    }

    /// `0x318900`: the forward turned toward the look point, the rows about the smoothed up 0x1672b0.
    fn swing_rows(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let up_s = to_f32x3(self.g.up_s);
        let s = &mut self.swing;
        let mut look = to_f32x3(h.pos);
        look[2] += s.look_h;
        let a = fnorm(fsub(look, s.pos), 1.0);
        let ang = FRAC_PI_2 - asin(fdot(a, s.rows[0]));
        let turn = if h.state == ST_PULL {
            s.pitch_vel += 0.017;
            if 1.0 < s.pitch_vel { s.pitch_vel = 1.0; }
            ang * s.pitch_vel
        } else {
            angle_spring(0.0, ang, 0.02, 0.175, 0.0, &mut s.pitch_vel)
        };
        let axis = vcross(a, s.saved_fwd);
        let f = frot(s.saved_fwd, turn, axis);
        s.saved_fwd = fnorm(f, 1.0);
        let f = s.saved_fwd;
        let left = fnorm(vcross(f, up_s), 1.0);
        let up = vcross(left, f);
        s.rows = [f, left, up];
    }

    /// The collision push `0x20f2a8(r, pos)` from `prev` (module doc): the pushed position and whether it moved.
    pub(super) fn collision_push(&self, inp: &CamInput, r: f32, prev: [f32; 3], pos: [f32; 3]) -> ([f32; 3], bool) {
        let d = fsub(pos, prev);
        let l = fdot(d, d).sqrt();
        let n = (l / (r * 0.9)) as i32 + 1;
        let step = fscale(d, 1.0 / n as f32);
        let mut p = prev;
        let mut pushed = false;
        for _ in 0..n {
            p = fadd(p, step);
            for _ in 0..6 {
                match sphere(inp, Pf::f(r), from_f32x3(p), self.sph_flags, inp.hero_moby) {
                    Some((_, c)) => {
                        p = to_f32x3(c);
                        pushed = true;
                    }
                    None => break,
                }
            }
        }
        (p, pushed)
    }

    /// The class-7 camera's view (rows, position).
    pub(super) fn swing_view(&self) -> (R3, [f32; 3]) { (self.swing.rows, self.swing.pos) }
}

/// The Swingshot targets' **look-up hint** for the follow camera (level01 0x167480.., level03 0x167100..; the code
/// sits right after the Swingshot camera's on every level: level03 reset `0x2eb3d0`, offer `0x2eb4c0`, callback
/// `0x2eb408`, post-update `0x2eb468`): record +0x00 the camera (written, not read), +0x04 the target offered this
/// tick, +0x08 the best distance, +0x0c the weight, +0x10 the callback.
///
/// * **Reset** (`0x2eb3d0`, by a Swingshot target (classes 758 / 803) turning active, state 0 → 1): camera 0, target
///   0, distance 10000, weight 0, the callback set. The slot init `0x20ef58` clears the camera, the weight and the
///   callback.
/// * **Offer** (`0x2eb4c0(target)`, by an active target while Ratchet is not swinging): with the follow camera current
///   and the right stick's y (0x13ca44) within ±0.05, a target 0 < d < 22 from Ratchet, nearer than the best
///   distance, above him, within 64° of the camera's forward from Ratchet and within 36° of it from the camera:
///   weight += 0.03 (at most 1), the target taken, the best distance = its distance from the camera.
/// * **Callback** (from the follow camera's hero-state tweaks `0x3111d8`, each tick it runs): weight ≠ 0 → the look
///   height `0x3136c8(0.35·weight, 0.005, add)` (above the default 1.5) and the post-update registered
///   (`AddCamPostUpdFunc` 0x1e4a88).
/// * **Post-update** (`ExecuteCamPostUpdFuncs` 0x20cd88 at the end of `UpdateAllCameras`): distance 10000; a target
///   offered → cleared, else the weight −= 0.03 (not below 0); the camera word cleared.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookHint {
    pub target: Option<usize>,
    pub dist: f32,
    pub weight: f32,
    pub callback: bool,
    /// The post-update is registered for this tick's `UpdateAllCameras`.
    pub post: bool,
}

/// A Swingshot target's call into the hint (queued by the moby loop, applied before the camera update:
/// [`crate::cinematic::CinematicCall::LookHint`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HintCall {
    /// `0x2eb3d0`.
    Reset,
    /// `0x2eb4c0(target)`: the target moby and its position, Ratchet's position as the moby loop saw it.
    Offer { target: usize, pos: [f32; 3], hero: [f32; 3] },
}

/// gp−0x4dac: the weight's step per tick.
const HINT_STEP: f32 = 0.03;

impl Camera {
    /// A Swingshot target's hint call (module doc of [`LookHint`]); `ry` = the right stick's y (0x13ca44).
    pub fn look_hint(&mut self, call: HintCall, ry: f32) {
        match call {
            HintCall::Reset => {
                self.hint = LookHint { target: None, dist: 10000.0, weight: 0.0, callback: true, post: self.hint.post };
            }
            HintCall::Offer { target, pos, hero } => {
                if !self.follow_is_current() || 0.05 < ry.abs() { return; }
                let d = fsub(pos, hero);
                let l = fdot(d, d).sqrt();
                if !(l < 22.0) || !(0.0 < l && l < self.hint.dist && 0.0 < d[2]) { return; }
                let fwd = super::rows_f(self.cam.rows)[0];
                if !(FRAC_PI_2 - asin(fdot(fwd, d) / l) < 1.117_010_7) { return; }
                let dc = fsub(pos, to_f32x3(self.cam.pos));
                let mut lc = fdot(dc, dc).sqrt();
                if lc == 0.0 { lc = 0.0001; }
                if !(FRAC_PI_2 - asin(fdot(fwd, dc) / lc) < 0.628_318_55) { return; }
                let h = &mut self.hint;
                h.weight += HINT_STEP;
                h.target = Some(target);
                h.dist = lc;
                if 1.0 < h.weight { h.weight = 1.0; }
            }
        }
    }

    /// The callback `0x2eb408` (from the follow camera's tweaks `0x3111d8`).
    pub(super) fn look_hint_callback(&mut self) {
        if !self.hint.callback || self.hint.weight == 0.0 { return; }
        self.set_look_height(self.hint.weight * 0.35, 0.005, true);
        self.hint.post = true;
    }

    /// The post-update `0x2eb468` when registered (end of `UpdateAllCameras`).
    pub(super) fn look_hint_post(&mut self) {
        if !std::mem::take(&mut self.hint.post) { return; }
        let h = &mut self.hint;
        h.dist = 10000.0;
        if h.target.take().is_none() {
            h.weight -= HINT_STEP;
            if h.weight < 0.0 { h.weight = 0.0; }
        }
    }
}

/// The level-14 / 7 / 9 look (`0x3182c8`): the group member of a Swingshot target class, not the swung-on one, ahead of
/// Ratchet's yaw (`cos > 0` of the flat-yaw direction against the direction from the swung-on target), with the lowest
/// `distance − 10·cos` below 1000 (the first on a tie).
fn next_target(g: &SwingGroup, on: Option<usize>, hero_yaw: f32) -> Option<[f32; 3]> {
    let fwd = [hero_yaw.cos(), hero_yaw.sin(), 0.0];
    let mut best = 1000.0;
    let mut sel = None;
    for m in &g.members {
        if Some(m.id) == on || !LOOK_CLASSES.contains(&m.class) { continue; }
        let d = fsub(m.pos, g.on_pos);
        let c = fdot(fnorm(d, 1.0), fwd);
        if 0.0 < c {
            let score = fdot(d, d).sqrt() - c * 10.0;
            if score < best {
                best = score;
                sel = Some(m.pos);
            }
        }
    }
    sel
}

#[cfg(test)]
mod tests;
