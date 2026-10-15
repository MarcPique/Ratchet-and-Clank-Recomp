//! **The type-6 camera** (level01: activation `0x317f50` = never by itself, init `0x317f58` → `0x317808`, update
//! `0x318008` → `0x3178d8`, release `0x318028` = empty; the camera table 0x20c480, entry 4): a camera on five springs
//! about a centre point — yaw, elevation and distance of a spherical offset from the centre, plus a pitch and a yaw
//! offset of its view — with the centre following a target. Its entry points: the switch `0x317d88(moby)`, the
//! tracking `0x317aa0(camera, moby)` and the hand-back `0x317e70(camera)`; the placement `0x317778`.
//!
//! **Who uses it.** Only the Visibomb's missile (class 172, `crate::moby_update::classes::visibomb`): on every
//! level the switch is called by the missile's launch `0x2cb540` alone, the tracking by the missile's update and by
//! the switch, the hand-back by the flight's end `0x2cb788` alone (a scan of the 19 overlays for calls to the four
//! functions and to `0x20cdf8(6)`, hero_gameplay.md §17). It is still a camera mode of the camera system (a type of
//! the table, updated by `UpdateAllCameras`), so it lives here and knows nothing of missiles: its input is a
//! [`Target`] (the centre, the Euler, the flight timer and the orbit's steps).
//!
//! * **Switch** (`0x317d88`): no blend (0x167370 = 0), `FUN_0020d110` (a cut [M: the camera record's blend byte is
//!   0, as for the script camera]), the tracking once, the centre snapped to the target, the springs' targets and
//!   current values the defaults 0x208820 (yaw offset π, elevation 15°, distance 0.01, pitch offset 5°, yaw offset
//!   0), the init `0x317808` (velocities 0, roll 0, the placement `0x3176d0`), the centre snapping every update
//!   (D+0x9c = 1), the spring constants 0x208838 (k 0.07 / 1 / 0.07 / 1 / 0.07) and 0x208850 (damping 0.04 / 0.04 /
//!   0.04 / 0.99 / 0.04), no maximum.
//! * **Tracking** (`0x317aa0`), in flight (timer ≥ 1): roll = the target's roll / 2, the pitch-offset target eased 5 %
//!   toward the target's pitch, the centre target = the target's centre, the yaw base (D+0xc8) its yaw. After the
//!   flight (timer < 1): on the first call (timer −1) the camera is re-placed about the centre (`0x317778`: yaw kept,
//!   elevation = the pitch offset, distance 6, pitch offset 0, yaw base 0, velocities 0); then every call the roll
//!   turns by the roll step and the distance target grows by the orbit step (both computed by the caller from its
//!   own decaying values, [`Orbit`]), and the centre target follows.
//! * **Update** (`0x3178d8`): yaw, elevation, pitch and yaw offsets on the angle spring `0x20cf28`, the distance on
//!   `Cam_InterpValues`; the centre snapped; position = centre + the spherical offset (`0x20f180`), Euler = (roll,
//!   elevation + pitch offset, π + yaw + yaw offset), rows = `EulerToMatrix`.
//! * **Hand-back** (`0x317e70`): when Ratchet is more than 32° off the view (`FastDiffRots(camera yaw, atan(Ratchet −
//!   camera))` ≥ 0.5585) or 8 or more from the camera, a cut (+0x7e = 4); else a blend at 0.018 (+0x7e = 2, 0x167373
//!   = 0, rates 0x167388 / 0x167394). The next camera update switches the follow camera in (its activation yields to a
//!   camera with +0x7e set) and snaps it behind Ratchet.
//!
//! Not modelled: `Camera_handleCollWithHero` (as for the script camera), the type's own activation (never).
//! Native `f32`.

use super::script::{angle_spring, euler_rows, sph_point, spring, wrap};
use super::{rows_pf, CamInput, Camera, R3, V4};
use crate::hero::physics::{from_f32x3, to_f32x3};
use crate::ps2v::Pf;
use std::f32::consts::PI;

/// 0x208820: the springs' default targets (yaw offset, elevation, distance, pitch offset, yaw offset).
pub const DEFAULTS: [f32; 5] = [PI, 0.261_799_4, 0.01, 0.087_266_46, 0.0];
/// 0x208838 / 0x208850: the springs' constants and damping.
pub const K: [f32; 5] = [0.07, 1.0, 0.07, 1.0, 0.07];
pub const D: [f32; 5] = [0.04, 0.04, 0.04, 0.99, 0.04];
/// `0x317808`: the centre spring's constant and damping (the switch snaps the centre, so unused while snapping).
const CENTRE_K: f32 = 0.3;
const CENTRE_D: f32 = 0.3;
/// `0x317aa0`: the orbit's distance after the flight.
pub const ORBIT_DIST: f32 = 6.0;
/// `0x317e70`: the hand-back's cut limits (32°, 8 units) and the blend rate.
pub const CUT_ANGLE: f32 = 0.558_505_4;
pub const CUT_DIST: f32 = 8.0;
pub const BLEND_RATE: f32 = 0.018;

/// What `0x317aa0` reads of the tracked moby: its centre (position + 0.12 along its up row), its Euler (roll, pitch,
/// yaw) and its flight timer (+0x10 of its pvars), and after the flight the orbit's steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    pub centre: [f32; 3],
    pub euler: [f32; 3],
    pub timer: i32,
    pub orbit: Orbit,
}

/// The orbit's steps of one tracking call after the flight (the caller keeps and decays the values; the game keeps
/// them in the missile's pvars +0x18 / +0x04).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orbit {
    /// Added to the roll (D+0xac).
    pub roll: f32,
    /// Added to the distance target (D+0x1c).
    pub dist: f32,
}

/// A call into the type-6 camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// `0x317d88`.
    Switch(Target),
    /// `0x317aa0`.
    Track(Target),
    /// `0x317e70`.
    Release,
}

/// The type-6 camera's state (its UpdateCam +0x00.. / +0x30 / +0x40 / +0x50..+0x60 / +0x7e and the data block D).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Type6 {
    /// It is the active camera (0x167280).
    pub active: bool,
    pub pos: [f32; 3],
    pub euler: [f32; 3],
    pub rows: R3,
    /// +0x50..+0x60: yaw, elevation, distance, pitch offset, yaw offset.
    pub cur: [f32; 5],
    /// D+0x14..+0x24 targets, D+0x00.. velocities, D+0x28.. constants, D+0x3c.. damping, D+0x50.. maxima.
    pub target: [f32; 5],
    pub vel: [f32; 5],
    pub k: [f32; 5],
    pub d: [f32; 5],
    pub max: [f32; 5],
    /// D+0xb0 the centre, D+0x80 its target, D+0x90 its velocity; D+0x9c the centre snaps.
    pub centre: [f32; 3],
    pub centre_target: [f32; 3],
    pub centre_vel: [f32; 3],
    pub snap: bool,
    /// D+0xac the roll, D+0xc8 the yaw base.
    pub roll: f32,
    pub yaw_base: f32,
    /// +0x7e: the hand-back asked for (Some(true) a blend, Some(false) a cut), made by the next camera update.
    pub release: Option<bool>,
    /// Updates run since the switch (reports).
    pub ticks: u32,
}

impl Type6 {
    /// `0x3176d0`: the position from the current springs (before the yaw base is added), the yaw base added, the
    /// Euler (no roll) and the rows.
    fn place(&mut self) {
        self.pos = sph_point([self.cur[0], self.cur[1], self.cur[2]], self.centre);
        self.cur[0] = wrap(self.cur[0] + self.yaw_base);
        self.euler = [0.0, wrap(self.cur[1] + self.cur[3]), wrap(wrap(PI + self.cur[0]) + self.cur[4])];
        self.rows = euler_rows(self.euler);
    }

    /// `0x317808`: velocities 0, the default constants, roll 0, the centre spring, the placement, +0x7e = 0.
    fn init(&mut self) {
        self.snap = false;
        self.roll = 0.0;
        self.k = [0.07, 1.0, 0.07, 1.0, 0.07];
        self.d = [0.04, 0.04, 0.04, 0.99, 0.04];
        self.max = [0.0; 5];
        self.vel = [0.0; 5];
        self.centre_vel = [0.0; 3];
        self.place();
        self.release = None;
    }

    /// `0x317aa0` (module docs). Returns the game's result (1 in flight, 0 after).
    pub fn track(&mut self, t: &Target) -> bool {
        if t.timer == -1 {
            // 0x317778(0, camera, (yaw, pitch offset, 6, 0, yaw offset), centre).
            let c = self.cur;
            let s = [c[0], c[3], ORBIT_DIST, 0.0, c[4]];
            self.centre = t.centre;
            self.centre_target = t.centre;
            self.yaw_base = 0.0;
            self.target = s;
            self.cur = s;
            self.vel = [0.0; 5];
            self.centre_vel = [0.0; 3];
            self.place();
        }
        if t.timer < 1 {
            self.roll = wrap(self.roll + t.orbit.roll);
            self.target[2] += t.orbit.dist;
            self.centre_target = t.centre;
            return false;
        }
        self.roll = t.euler[0] * 0.5;
        self.target[3] += (t.euler[1] - self.target[3]) * 0.05;
        self.centre_target = t.centre;
        self.yaw_base = t.euler[2];
        true
    }

    /// `0x3178d8` (module docs).
    pub fn step(&mut self) {
        self.ticks += 1;
        let tgt0 = wrap(self.yaw_base + self.target[0]);
        self.cur[0] = angle_spring(self.cur[0], tgt0, self.k[0], self.d[0], self.max[0], &mut self.vel[0]);
        self.cur[1] = angle_spring(self.cur[1], self.target[1], self.k[1], self.d[1], self.max[1], &mut self.vel[1]);
        self.cur[2] = spring(self.cur[2], self.target[2], self.k[2], self.d[2], self.max[2], &mut self.vel[2]);
        self.cur[3] = angle_spring(self.cur[3], self.target[3], self.k[3], self.d[3], self.max[3], &mut self.vel[3]);
        self.cur[4] = angle_spring(self.cur[4], self.target[4], self.k[4], self.d[4], self.max[4], &mut self.vel[4]);
        if self.snap {
            self.centre = self.centre_target;
        } else {
            for i in 0..3 { self.centre[i] = spring(self.centre[i], self.centre_target[i], CENTRE_K, CENTRE_D, 0.0, &mut self.centre_vel[i]); }
        }
        self.pos = sph_point([self.cur[0], self.cur[1], self.cur[2]], self.centre);
        self.euler = [self.roll, wrap(self.cur[1] + self.cur[3]), wrap(wrap(PI + self.cur[0]) + self.cur[4])];
        self.rows = euler_rows(self.euler);
    }
}

/// `0x317e70`'s choice from the camera 0x167240 / its yaw 0x167258 and Ratchet's position 0x13f3d0: true = the blend.
pub fn release_blends(cam: [f32; 3], cam_yaw: f32, hero: [f32; 3]) -> bool {
    use crate::moby_update::creature::{atan, diff_rots};
    let a = atan(hero[0] - cam[0], hero[1] - cam[1]);
    if CUT_ANGLE <= diff_rots(cam_yaw, a) { return false; }
    let d = ((hero[0] - cam[0]).powi(2) + (hero[1] - cam[1]).powi(2) + (hero[2] - cam[2]).powi(2)).sqrt();
    d < CUT_DIST
}

impl Camera {
    /// A call into the type-6 camera (applied by the tick in the game's order: the switch right after the hand
    /// items' updates, the tracking and the hand-back right after the moby loop).
    pub fn type6_call(&mut self, call: &Call, inp: &CamInput) {
        match call {
            Call::Switch(t) => self.type6_switch(t),
            Call::Track(t) => {
                if self.type6.active { self.type6.track(t); }
            }
            Call::Release => {
                if self.type6.active {
                    let cam = to_f32x3(self.out.pos);
                    self.type6.release = Some(release_blends(cam, self.out.euler[2].to_f32(), to_f32x3(inp.hero.pos)));
                }
            }
        }
    }

    /// `0x317d88` (module docs).
    fn type6_switch(&mut self, t: &Target) {
        self.blend.mode = 0;
        let c = &mut self.type6;
        c.active = true;
        c.ticks = 0;
        c.track(t);
        c.centre = c.centre_target;
        c.target = DEFAULTS;
        c.cur = DEFAULTS;
        c.init();
        c.snap = true;
        c.k = K;
        c.d = D;
        c.max = [0.0; 5];
        self.script.active = false;
        self.first_person.active = false;
        self.g.since_switch = 0;
    }

    /// The type-6 camera is the active camera.
    pub fn type6_active(&self) -> bool { self.type6.active }

    /// `UpdateAllCameras` with the type-6 camera up: a pending hand-back switches the follow camera in (returns the
    /// view switched away from when a blend follows it); else the type-6 update.
    pub(super) fn type6_frame(&mut self, inp: &CamInput) -> Option<([V4; 3], V4)> {
        let Some(blend) = self.type6.release.take() else {
            self.type6.step();
            return None;
        };
        let prev = self.type6_view();
        self.type6.active = false;
        if blend {
            self.blend.next_rot_rate = BLEND_RATE;
            self.blend.next_pos_rate = BLEND_RATE;
            self.blend.kind = 0;
            self.blend.mode = if self.blend.mode == 0 { 1 } else { 2 };
        }
        // FUN_0020d110(follow) and the type-0 init: the snap behind Ratchet.
        self.init(inp);
        self.d0 = self.cam;
        self.g.since_switch = 0;
        blend.then_some(prev)
    }

    /// The type-6 camera's rows and position.
    pub(super) fn type6_view(&self) -> ([V4; 3], V4) {
        let mut p = from_f32x3(self.type6.pos);
        p[3] = Pf::ONE;
        (rows_pf(self.type6.rows), p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flight(centre: [f32; 3], yaw: f32, pitch: f32) -> Target { Target { centre, euler: [0.0, pitch, yaw], timer: 100, orbit: Orbit::default() } }

    /// The switch places the camera on the missile (0.01 behind the centre, 15° up) looking along its yaw, pitched
    /// down 20° (elevation 15° + pitch offset 5°).
    #[test]
    fn switch_places_the_camera_behind_the_target() {
        let mut c = Type6::default();
        c.track(&flight([10.0, 20.0, 5.0], 1.0, 0.0));
        c.centre = c.centre_target;
        c.target = DEFAULTS;
        c.cur = DEFAULTS;
        c.init();
        c.snap = true;
        c.k = K;
        c.d = D;
        assert!((c.cur[0] - wrap(PI + 1.0)).abs() < 1e-6, "{:?}", c.cur);
        assert!((c.euler[2] - 1.0).abs() < 1e-5 && (c.euler[1] - (DEFAULTS[1] + DEFAULTS[3])).abs() < 1e-6, "{:?}", c.euler);
        c.step();
        let d = ((c.pos[0] - 10.0).powi(2) + (c.pos[1] - 20.0).powi(2) + (c.pos[2] - 5.0).powi(2)).sqrt();
        assert!((d - 0.01).abs() < 1e-4, "distance {d}");
        // Behind: the camera is on the side opposite its view direction.
        let f = c.rows[0];
        assert!(f[0] * (c.pos[0] - 10.0) + f[1] * (c.pos[1] - 20.0) < 0.0);
    }

    /// In flight the yaw follows the target's yaw on the 0.07 spring and the pitch offset eases 5 % a tick toward its
    /// pitch; the roll is half the target's.
    #[test]
    fn tracking_follows_a_turning_target() {
        let mut c = Type6 { snap: true, k: K, d: D, target: DEFAULTS, cur: DEFAULTS, ..Default::default() };
        c.track(&flight([0.0; 3], 0.0, 0.0));
        c.place();
        for _ in 0..200 {
            c.track(&Target { euler: [0.4, 0.3, 0.8], ..flight([0.0; 3], 0.8, 0.3) });
            c.step();
        }
        assert!((c.euler[2] - 0.8).abs() < 1e-3, "yaw {:?}", c.euler);
        assert!((c.target[3] - 0.3).abs() < 1e-3 && (c.euler[1] - (DEFAULTS[1] + 0.3)).abs() < 1e-3, "{:?}", c.euler);
        assert!((c.euler[0] - 0.2).abs() < 1e-6);
    }

    /// The first call after the flight re-places the camera 6 from the centre (elevation = the pitch offset, no pitch
    /// offset), looking at the centre; the orbit steps then turn the roll and push the distance out.
    #[test]
    fn orbit_after_the_flight() {
        let mut c = Type6 { snap: true, k: K, d: D, target: DEFAULTS, cur: DEFAULTS, ..Default::default() };
        c.track(&flight([0.0; 3], 0.5, 0.2));
        c.place();
        for _ in 0..50 { c.track(&flight([1.0, 2.0, 3.0], 0.5, 0.2)); c.step(); }
        let po = c.cur[3];
        c.track(&Target { centre: [1.0, 2.0, 3.0], euler: [0.0; 3], timer: -1, orbit: Orbit { roll: 0.01, dist: 0.02 } });
        assert_eq!((c.cur[1], c.cur[2], c.cur[3], c.yaw_base), (po, ORBIT_DIST, 0.0, 0.0));
        c.step();
        let d = ((c.pos[0] - 1.0).powi(2) + (c.pos[1] - 2.0).powi(2) + (c.pos[2] - 3.0).powi(2)).sqrt();
        assert!((d - ORBIT_DIST).abs() < 0.01, "distance {d}");
        // It looks at the centre.
        let to = [(1.0 - c.pos[0]) / d, (2.0 - c.pos[1]) / d, (3.0 - c.pos[2]) / d];
        let f = c.rows[0];
        assert!(f[0] * to[0] + f[1] * to[1] + f[2] * to[2] > 0.999, "{f:?} {to:?}");
        assert!((c.target[2] - (ORBIT_DIST + 0.02)).abs() < 1e-6 && (c.roll - 0.01).abs() < 1e-6);
    }

    #[test]
    fn hand_back_cuts_when_ratchet_is_far_or_off_the_view() {
        // Camera at the origin looking along +x.
        assert!(release_blends([0.0; 3], 0.0, [5.0, 0.0, 0.0]));
        assert!(!release_blends([0.0; 3], 0.0, [9.0, 0.0, 0.0]), "8 or more away: cut");
        assert!(!release_blends([0.0; 3], 0.0, [3.0, 3.0, 0.0]), "45° off: cut");
        assert!(release_blends([0.0; 3], 0.0, [5.0, 2.5, 0.0]), "26.6° off: blend");
    }
}
