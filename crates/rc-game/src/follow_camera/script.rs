//! **The script camera, type 5** (level01: activation `0x317668` = never by itself, init `0x3171b8`, update
//! `0x317670` → `0x317278`, release `0x3176c8` = empty; the camera table 0x20c480, entry 3) and its two entry points
//! `CameraScript(pos, euler, mode, ticks, collide)` 0x316ef8 / `CameraScript2(kind)` 0x317070, plus the target
//! setters `0x316dd0` (position) / `0x316e28` (Euler) / `0x316e88` (mode 3's distance curve). It is the camera of
//! every in-engine cinematic that is not a mode-2 scene: the camera trigger 737's cutaways, the gunship 688's fly-by,
//! the bolt crank 280's door views, the troopers' and the gold bolt's cameras, the vendor. Spec: docs/plan/cutscenes.md §3.
//!
//! * **Switch in** (`CameraScript`): `FUN_0020d110(cam5)` makes it the active camera (the follow camera's springs
//!   stop where they are) and runs its init: position spring `Cam_InterpValues` k 0.3, damping 0.3, max 1.0 per
//!   axis (D+0x10..0x1c), Euler spring `0x20cf28` k 0.05, damping 0.3, max 1.0 (D+0x50..0x5c), velocities 0,
//!   +0x7e = 0. Then the camera and its targets (D+0x80 / D+0x90) are both set to `(pos, euler)`, 0x167240 = pos,
//!   the mode (+0x88) and the timer (D+0xe0 = D+0xe4 = `ticks`) stored. No blend: the switch is a cut [M: the
//!   record byte that could ask for a blend is 0 for every caller seen].
//! * **Update** by mode: 0 springs toward the targets (a caller may move them every tick: 737 does, tracking the
//!   mission NPC); 1 snaps to them; 2 moves toward them over the timer with `t = (total − left)/total` and the
//!   cosine ease `(1 − cos πt)/2` applied to the *current* pose each tick (`0x26cd50`, `0x26ce30`); 3 swings from
//!   the view it was switched in from (`CameraScript` keeps the current camera, not `pos`, for mode 3, and records
//!   its yaw / elevation / distance about Ratchet, D+0xfc, and his position, D+0x110) to the targets over the timer:
//!   the anchor eases to Ratchet, the yaw eases to the target's (taking the long way round when the short way and
//!   the target view's yaw disagree by more than 20°: then the difference is added as eased steps), the elevation
//!   eases, the distance follows the Hermite curve `0x26cc00(D+0x124, 0, 1, D+0x128, t)` (defaults 1 / 0 =
//!   smoothstep; `0x316e88` sets them), the Euler eases to the target's; once the timer is out it holds the targets
//!   (the bolt crank 280 uses it). Then rows = `EulerToMatrix(euler)` (forward, left, up). The collision push
//!   `0x20f2a8` (radius 0.5) when `collide`: no Novalis caller passes it (not ported).
//! * **Release** (`CameraScript2(kind)`, then the follow camera takes over in the next `UpdateAllCameras`): kind 0
//!   first moves the script camera 1 behind and 1.2 above Ratchet along his rows, then the follow camera is switched
//!   in with a **cut** (`+0x7e = 1` → `+0x8e = 1`: no blend) and its init snaps it behind him; kind 2 **blends**
//!   (`0x167370`, rates 0.018 both); kind 4 blends from the copied pose at 0.018 (0.01 on Novalis, `FUN_0020d110`'s
//!   level-1 rate); kind 1 copies the pose without a blend (treated as a cut) [kind 1 / 4: M].
//!
//! Native `f32`.

use super::{rows_pf, Camera, CamInput, R3};
use crate::hero::physics::{from_f32x3, to_f32x3, V4};
use crate::ps2v::Pf;
use std::f32::consts::PI;

/// A `CameraScript` mode (+0x88).
pub const MODE_SPRING: u8 = 0;
pub const MODE_SNAP: u8 = 1;
pub const MODE_LERP: u8 = 2;
pub const MODE_ORBIT: u8 = 3;

/// The script camera's state (its UpdateCam +0x30 / +0x40 / +0x88 and its data block D).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScriptCamera {
    /// It is the active camera (0x167280).
    pub active: bool,
    /// +0x30 position, +0x40 Euler (x roll, y pitch, z yaw).
    pub pos: [f32; 3],
    pub euler: [f32; 3],
    /// +0x00.. rows (forward, left, up) from the Euler.
    pub rows: R3,
    /// +0x88.
    pub mode: u8,
    /// D+0x80 / D+0x90: the targets.
    pub target_pos: [f32; 3],
    pub target_euler: [f32; 3],
    /// D+0x00.. / D+0x30..: the spring velocities.
    pub vel: [f32; 3],
    pub avel: [f32; 3],
    /// D+0xe0 (left) / D+0xe4 (total).
    pub timer: i32,
    pub total: i32,
    /// D+0x120: the collision push (not ported).
    pub collide: bool,
    /// Mode 3: D+0xe8 the target's (yaw, elevation, distance) about Ratchet, D+0xfc the camera's, D+0x110 the
    /// anchor (Ratchet's position, eased), D+0x124 / D+0x128 the distance curve's end slopes.
    pub orbit_target: [f32; 3],
    pub orbit: [f32; 3],
    pub anchor: [f32; 3],
    pub curve: [f32; 2],
    /// `CameraScript2(kind)` waiting for the next camera update, with the level (kind 4's rate).
    pub release: Option<(u8, u32)>,
    /// `0x312b40`'s springs (position, Euler: k, damping, max at D+0x10..0x18 / D+0x50..0x58) after the init's
    /// (0.3, 0.3, 1) / (0.05, 0.3, 1); None: the init's. The next `CameraScript` restores the init's.
    pub springs: Option<[[f32; 3]; 2]>,
    /// Updates run since the switch (reports).
    pub ticks: u32,
}

/// Rows (forward, left, up) of `EulerToMatrix(e)` (0x221980, R = Rz·Ry·Rx, rows = its columns).
pub fn euler_rows(e: [f32; 3]) -> R3 {
    let (sx, cx) = e[0].sin_cos();
    let (sy, cy) = e[1].sin_cos();
    let (sz, cz) = e[2].sin_cos();
    [
        [cy * cz, cy * sz, -sy],
        [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy],
        [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy],
    ]
}

/// `fast_add_rotations` / `fast_subtract_rotations`: wrapped once into [−π, π).
pub(super) fn wrap(a: f32) -> f32 {
    if a >= PI { a - 2.0 * PI } else if a < -PI { a + 2.0 * PI } else { a }
}

/// `Cam_InterpValues` 0x20ce40.
pub(super) fn spring(cur: f32, tgt: f32, k: f32, d: f32, max: f32, vel: &mut f32) -> f32 {
    let e = tgt - cur;
    *vel += k * e - d * *vel;
    if max != 0.0 { *vel = vel.clamp(-max, max); }
    *vel = vel.clamp(-e.abs(), e.abs());
    cur + *vel
}

/// `0x20cf28`: the same on an angle (wrapped difference and sum).
pub(super) fn angle_spring(cur: f32, tgt: f32, k: f32, d: f32, max: f32, vel: &mut f32) -> f32 {
    let e = wrap(tgt - cur);
    *vel += k * e - d * *vel;
    if max != 0.0 { *vel = vel.clamp(-max, max); }
    *vel = vel.clamp(-e.abs(), e.abs());
    wrap(cur + *vel)
}

/// `0x20f230(out, p, c)`: (yaw, elevation, distance) of `p − c`.
fn spherical(p: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
    let xy = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let yaw = if d[0] == 0.0 && d[1] == 0.0 { 0.0 } else { d[1].atan2(d[0]) };
    let elev = if xy == 0.0 && d[2] == 0.0 { 0.0 } else { d[2].atan2(xy) };
    [yaw, elev, (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()]
}

/// `0x20f180(out, s, c)`: the point at (yaw, elevation, distance) `s` about `c`.
pub(super) fn sph_point(s: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let [yaw, elev, dist] = s;
    let (se, ce) = elev.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    [dist * ce * cy + c[0], dist * ce * sy + c[1], dist * se + c[2]]
}

/// `(1 − cos πt)/2`.
fn ease(t: f32) -> f32 { (1.0 - (t * PI).cos()) * 0.5 }

/// `0x26ccd0(a, b, t)`: `a + wrap(b − a)·ease(t)`, wrapped.
fn rot_ease(a: f32, b: f32, t: f32) -> f32 { wrap(a + wrap(b - a) * ease(t)) }

/// `0x26cc38(a, b, t)`: `a` at 0, `b` at 1, else `a + (b − a)·ease(t)`.
pub(crate) fn cos_interp(a: f32, b: f32, t: f32) -> f32 {
    if t == 0.0 { a } else if t == 1.0 { b } else { a + (b - a) * ease(t) }
}

/// `0x26cc00(p1, p2, p3, p4, t)`.
pub(super) fn hermite(p1: f32, p2: f32, p3: f32, p4: f32, t: f32) -> f32 {
    let f = (p4 - p3) - (p1 - p2);
    f * t * t * t + ((p1 - p2) - f) * t * t + (p3 - p1) * t + p2
}

/// 20° (0.34906584).
const SWING_LIMIT: f32 = 0.349_065_84;

impl ScriptCamera {
    /// The update `0x317278` (module docs) with Ratchet's position 0x13f3d0 and the camera yaw 0x167258 of the last
    /// camera update.
    pub fn step(&mut self, hero: [f32; 3], cam_yaw: f32) {
        self.ticks += 1;
        match self.mode {
            MODE_SNAP => self.snap(),
            MODE_LERP => {
                // FastDecTimer(D+0xe0), then t = (total − left)/total.
                if self.timer > 0 { self.timer -= 1; }
                let t = if self.total != 0 { (self.total - self.timer) as f32 / self.total as f32 } else { 1.0 };
                let s = if t == 0.0 { 0.0 } else if t == 1.0 { 1.0 } else { ease(t) };
                for k in 0..3 { self.pos[k] += (self.target_pos[k] - self.pos[k]) * s; }
                for k in 0..3 { self.euler[k] = rot_ease(self.euler[k], self.target_euler[k], t); }
            }
            MODE_ORBIT => {
                // FastDecTimer(D+0xe0) ≠ 0 (it was out, or just ran out): hold the targets.
                if self.timer == 0 { return self.snap_rows(); }
                self.timer -= 1;
                if self.timer < 1 { return self.snap_rows(); }
                let t = (self.total - self.timer) as f32 / self.total as f32;
                self.orbit_target = spherical(self.target_pos, hero);
                let dyaw = wrap(self.orbit_target[0] - self.orbit[0]);
                let dcy = wrap(self.target_euler[2] - cam_yaw);
                let long_way = if SWING_LIMIT < dyaw && dcy < -SWING_LIMIT {
                    true
                } else {
                    dyaw < -SWING_LIMIT && SWING_LIMIT < dcy
                };
                if long_way {
                    let mut d = self.orbit_target[0] - self.orbit[0];
                    if (d < 0.0 && 0.0 < dcy) || (0.0 < d && dcy < 0.0) { d = -d; }
                    self.orbit[0] += cos_interp(0.0, d, t);
                } else {
                    self.orbit[0] = rot_ease(self.orbit[0], self.orbit_target[0], t);
                }
                let k = hermite(self.curve[0], 0.0, 1.0, self.curve[1], t);
                self.orbit[2] += (self.orbit_target[2] - self.orbit[2]) * k;
                self.orbit[1] = rot_ease(self.orbit[1], self.orbit_target[1], t);
                // 0x26cd50(t, anchor, anchor, hero) and 0x20f180(pos, orbit, anchor).
                let s = if t == 0.0 { 0.0 } else if t == 1.0 { 1.0 } else { ease(t) };
                for k in 0..3 { self.anchor[k] += (hero[k] - self.anchor[k]) * s; }
                self.pos = sph_point(self.orbit, self.anchor);
                for k in 0..3 { self.euler[k] = rot_ease(self.euler[k], self.target_euler[k], t); }
            }
            _ => {
                let [p, e] = self.springs.unwrap_or([[0.3, 0.3, 1.0], [0.05, 0.3, 1.0]]);
                for k in 0..3 { self.pos[k] = spring(self.pos[k], self.target_pos[k], p[0], p[1], p[2], &mut self.vel[k]); }
                for k in 0..3 { self.euler[k] = angle_spring(self.euler[k], self.target_euler[k], e[0], e[1], e[2], &mut self.avel[k]); }
            }
        }
        self.rows = euler_rows(self.euler);
    }

    fn snap(&mut self) {
        self.pos = self.target_pos;
        self.euler = self.target_euler;
    }

    fn snap_rows(&mut self) {
        self.snap();
        self.rows = euler_rows(self.euler);
    }
}

impl Camera {
    /// `CameraScript(pos, euler, mode, ticks, collide)` 0x316ef8: the script camera is switched in and placed.
    /// `hero`: Ratchet's position 0x13f3d0 (mode 3 swings about it).
    pub fn camera_script(&mut self, pos: [f32; 3], euler: [f32; 3], mode: u8, ticks: i32, collide: bool, hero: [f32; 3]) {
        // The view before the switch (0x167240 / 0x167250), kept by mode 3.
        let (cur_pos, e) = (to_f32x3(self.out.pos), self.out.euler);
        let cur_euler = [e[0].to_f32(), e[1].to_f32(), e[2].to_f32()];
        let s = &mut self.script;
        // FUN_0020d110 + the init 0x3171b8 (velocities 0, rows from its old Euler; +0x7e = 0).
        s.active = true;
        s.release = None;
        s.springs = None;
        s.vel = [0.0; 3];
        s.avel = [0.0; 3];
        s.ticks = 0;
        // CameraScript's stores.
        s.pos = pos;
        s.euler = euler;
        s.target_pos = pos;
        s.target_euler = euler;
        s.rows = euler_rows(euler);
        s.mode = mode;
        s.timer = ticks;
        s.total = ticks;
        s.collide = collide;
        if mode == MODE_ORBIT {
            s.curve = [1.0, 0.0];
            s.pos = cur_pos;
            s.euler = cur_euler;
            s.rows = euler_rows(cur_euler);
            s.anchor = hero;
            s.orbit = spherical(cur_pos, hero);
        }
        self.first_person.active = false;
        self.g.since_switch = 0;
        self.out.pos = from_f32x3(self.script.pos);
        self.out.pos[3] = Pf::ONE;
    }

    /// `0x316dd0` / `0x316e28`: new targets for the script camera (it need not be active: the setters address the
    /// type-5 camera wherever it is).
    pub fn camera_script_targets(&mut self, pos: Option<[f32; 3]>, euler: Option<[f32; 3]>) {
        if let Some(p) = pos { self.script.target_pos = p; }
        if let Some(e) = euler { self.script.target_euler = e; }
    }

    /// `0x312b40(d_p, k_p, max_p, d_e, k_e, max_e)` (level06; the script camera's data block D+0x10..0x18 /
    /// D+0x50..0x58): its position and Euler springs, each (k, damping, max).
    pub fn camera_script_springs(&mut self, pos: [f32; 3], euler: [f32; 3]) { self.script.springs = Some([pos, euler]); }

    /// Level02 `0x2f8a18(mode, ticks)` (the launch tube 713's ride): the script camera's mode (+0x88); modes 2 and 3
    /// restart its timer (D+0xe0 = D+0xe4 = `ticks`), mode 3 also takes Ratchet's position as its anchor (D+0x110) and
    /// the current view's (yaw, elevation, distance) about it (D+0xfc, from 0x1673c0); other modes keep the timer.
    pub fn camera_script_mode(&mut self, mode: u8, ticks: i32, hero: [f32; 3]) {
        let cur = to_f32x3(self.out.pos);
        let s = &mut self.script;
        s.mode = mode;
        if mode != MODE_LERP && mode != MODE_ORBIT { return; }
        if mode == MODE_ORBIT {
            s.anchor = hero;
            s.orbit = spherical(cur, hero);
        }
        s.timer = ticks;
        s.total = ticks;
    }

    /// `0x316e88(a, b)`: mode 3's distance curve end slopes (D+0x124 = a, D+0x128 = b).
    pub fn camera_script_curve(&mut self, a: f32, b: f32) { self.script.curve = [a, b]; }

    /// `CameraScript2(kind)` 0x317070 on level `level`: the script camera asks to be released (applied by the next
    /// update).
    pub fn camera_script2(&mut self, kind: u8, level: u32) {
        if self.script.active { self.script.release = Some((kind, level)); }
    }

    /// The script camera is the active camera.
    pub fn script_active(&self) -> bool { self.script.active }

    /// `UpdateAllCameras` with the script camera up: a pending release switches the follow camera back in (returns
    /// the view switched away from when a blend follows it); else the script camera's update.
    pub(super) fn script_frame(&mut self, inp: &CamInput) -> Option<([V4; 3], V4)> {
        let Some((kind, level)) = self.script.release.take() else {
            let cam_yaw = self.out.euler[2].to_f32();
            self.script.step(to_f32x3(inp.hero.pos), cam_yaw);
            return None;
        };
        if kind == 0 {
            // CameraScript2(0): 1 behind and 1.2 above Ratchet, looking along his rows.
            let h = inp.hero;
            let r = [to_f32x3(h.rows[0]), to_f32x3(h.rows[1]), to_f32x3(h.rows[2])];
            let n = |v: [f32; 3], l: f32| {
                let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if m == 0.0 { v } else { v.map(|c| c * l / m) }
            };
            let (b, u, p) = (n(r[0], -1.0), n(r[2], 1.2), to_f32x3(h.pos));
            self.script.pos = [p[0] + u[0] + b[0], p[1] + u[1] + b[1], p[2] + u[2] + b[2]];
            self.script.rows = r;
        }
        let prev = self.script_view();
        self.script.active = false;
        // FUN_0020d110(follow) and the type-0 init: the snap behind Ratchet; a blend for kinds 2 / 4.
        let rate = match kind {
            2 => Some(0.018),
            4 => Some(if level == 1 { 0.01 } else { 0.018 }),
            _ => None,
        };
        if let Some(r) = rate {
            self.blend.next_rot_rate = r;
            self.blend.next_pos_rate = r;
            self.blend.kind = 0;
            self.blend.mode = if self.blend.mode == 0 { 1 } else { 2 };
        }
        self.init(inp);
        self.d0 = self.cam;
        self.g.since_switch = 0;
        rate.map(|_| prev)
    }

    /// The script camera's rows and position (UpdateCam +0x00.. / +0x30).
    pub(super) fn script_view(&self) -> ([V4; 3], V4) {
        let mut p = from_f32x3(self.script.pos);
        p[3] = Pf::ONE;
        (rows_pf(self.script.rows), p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_rows_forward_is_yaw_pitch() {
        // Yaw 90°, pitch 0: forward +y, left −x, up +z.
        let r = euler_rows([0.0, 0.0, PI / 2.0]);
        assert!((r[0][1] - 1.0).abs() < 1e-6 && (r[1][0] + 1.0).abs() < 1e-6 && (r[2][2] - 1.0).abs() < 1e-6);
        // Pitch +0.3 looks down (forward z = −sin 0.3).
        let r = euler_rows([0.0, 0.3, 0.0]);
        assert!((r[0][2] + 0.3f32.sin()).abs() < 1e-6);
    }

    #[test]
    fn spring_mode_converges_and_tracks_a_moving_target() {
        let mut s = ScriptCamera { active: true, mode: MODE_SPRING, ..Default::default() };
        s.target_euler = [0.0, 0.2, 1.0];
        for _ in 0..200 { s.step([0.0; 3], 0.0); }
        assert!((s.euler[2] - 1.0).abs() < 1e-3 && (s.euler[1] - 0.2).abs() < 1e-3, "{:?}", s.euler);
        // The angle spring takes the short way across ±π.
        s.euler = [0.0, 0.0, 3.0];
        s.avel = [0.0; 3];
        s.target_euler = [0.0, 0.0, -3.0];
        s.step([0.0; 3], 0.0);
        assert!(s.euler[2] > 3.0 || s.euler[2] < -3.0, "{}", s.euler[2]);
        // The step never overshoots (|vel| ≤ |error|).
        let mut s = ScriptCamera { mode: MODE_SPRING, ..Default::default() };
        s.target_pos = [10.0, 0.0, 0.0];
        let mut last = 0.0;
        for _ in 0..100 { s.step([0.0; 3], 0.0); assert!(s.pos[0] <= 10.0 && s.pos[0] >= last); last = s.pos[0]; }
    }

    #[test]
    fn orbit_mode_swings_about_the_hero_and_holds_the_targets() {
        let hero = [10.0, 0.0, 0.0];
        // From 5 behind (−x) to 5 to the side (+y), yaw target 0 → π/2 view.
        let mut s = ScriptCamera { mode: MODE_ORBIT, timer: 60, total: 60, curve: [1.0, 0.0], anchor: hero, ..Default::default() };
        s.pos = [5.0, 0.0, 0.0];
        s.orbit = spherical(s.pos, hero);
        s.target_pos = [10.0, 5.0, 0.0];
        s.target_euler = [0.0, 0.0, -PI / 2.0];
        let mut dmin = f32::MAX;
        for _ in 0..59 {
            s.step(hero, 0.0);
            let d = ((s.pos[0] - 10.0).powi(2) + s.pos[1].powi(2)).sqrt();
            dmin = dmin.min(d);
        }
        // It swings on a circle (distance 5 kept), not through Ratchet.
        assert!((dmin - 5.0).abs() < 1e-3, "{dmin}");
        s.step(hero, 0.0);
        assert_eq!(s.pos, s.target_pos);
        assert_eq!(s.euler, s.target_euler);
        // The distance curve: smoothstep by default.
        assert!((hermite(1.0, 0.0, 1.0, 0.0, 0.5) - 0.5).abs() < 1e-6 && hermite(1.0, 0.0, 1.0, 0.0, 1.0) == 1.0);
    }

    #[test]
    fn lerp_mode_lands_on_the_target_when_the_timer_runs_out() {
        let mut s = ScriptCamera { mode: MODE_LERP, timer: 30, total: 30, ..Default::default() };
        s.target_pos = [3.0, -6.0, 9.0];
        s.target_euler = [0.0, 0.5, -2.0];
        // The ease is applied to the current pose each tick, so it closes in fast and never overshoots.
        let mut last = 0.0;
        for k in 0..30 {
            s.step([0.0; 3], 0.0);
            assert!(last <= s.pos[0] && s.pos[0] <= 3.0);
            if k == 0 { assert!(s.pos[0] < 0.1); }
            last = s.pos[0];
        }
        assert_eq!(s.pos, [3.0, -6.0, 9.0]);
        assert!((s.euler[2] + 2.0).abs() < 1e-6);
    }
}
