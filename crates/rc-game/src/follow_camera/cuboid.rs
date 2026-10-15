//! **The cuboid cameras of Kerwan and Eudora** (activation kind 4: Ratchet's feet in the record's cuboid; level 03
//! code): class 1, **the fixed view** (level03 hook `0x2e8870`, init `0x2e8940` → `0x2e8628`, update `0x2e8960` →
//! `0x2e8730`, pre hook `0x2e8980`; records: 03 #22 (cuboid 45, blend 1), 04 #0 (cuboid 39, blend 3)): the camera
//! stands at the record with its rotation and turns toward Ratchet; and class 14, **the side view** (hook
//! `0x2ebd70`, init `0x2eb978` → `0x2eb7e0`, update `0x2ebdb0` → `0x2ebb00` / `0x2ebaf0` (empty) / `0x2ebc58`, pre
//! hook `0x2ebde8`; Kerwan's 4 records, cuboids 19, 24, 38, 39, blend 5): the camera keeps the record's facing at
//! pvar +0x20 behind and +0x24 above a smoothed Ratchet, his jumps not followed. Both priority 6. Spec:
//! docs/plan/player_controller.md §15 "Classes 1 and 14".
//!
//! **System or not.** Two classes with their own code (no shared function between them or with class 23's placed
//! view, which is the follow camera retuned); each one copy, level 03's, on the levels that list it (class 1: 03,
//! 04; class 14: 03), [`super::level::CameraPorts`].
//!
//! | address | call / branch | port (`cuboid.rs`) |
//! |---|---|---|
//! | 0x2e8870 | +0x74 = 4 only; a best not releasing with a priority ≥ the record's → 0 | [`Camera::fixed_hook`] |
//! | | the feet (0x13f3d0) in the cuboid (pvar +0x0c): the published camera position (0x167240) in it too → 1; else the current camera's +0x7e = 2, 0x167373 = 0, the rates 0x167388 / 0x167394 = 0.018 → 1; not in it → 0 | `fixed_hook` (the rates blend from the camera outside, `Camera::switch_blend`'s +0x7e = 2 path) |
//! | 0x2e8628 | D+0x5c = pvar +0x18 (the look height; 1.5 first; the record index < 0: a trap, n/a), D+0x50 = 0, D+0x80 = 0, D+0x04 = 0.02, D+0x08 = 0.1, D+0x0c = 0 | [`Camera::fixed_init`] (D+0x50 / +0x80: n/a, no reader) |
//! | | position = the record's, rows = its Euler rotation (`0x1fa050`), +0x40 = the forward, +0x7e = 0 | `fixed_init` |
//! | 0x2e8730 | look = the feet + (0, 0, D+0x5c); dir = unit(look − position); target = 90° − asin(dir · forward) | [`Camera::fixed_update`] |
//! | | turn = `0x1e4c90`(0, target, D+0x04, D+0x08, D+0x0c, D+0x00) (the angle spring; D+0x00 starts as the block's word the camera before left: `Camera::d_word_00`) | `fixed_update` |
//! | | `0x24e6f8`: the saved forward turned toward dir by (90° − asin(saved · dir)) × turn about saved × dir (a quaternion) → forward; saved = it normalised; forward = saved; left = up_s × forward; up = forward × left | `fixed_update` |
//! | 0x2e8980 | +0x74 = 4 and the feet out of the cuboid → +0x7e = 3 | [`Camera::fixed_pre`] |
//! | 0x2ebd70 | camera mode 0x1415d4 = 0x50 (a jump) and group 0x1413dc ≠ 0x11 → −1; mode ≠ 0x50 and group 6 → −1; else 0 (the kind-4 check: the priority, the cuboid) | [`side_hook`] |
//! | 0x2eb978 | D+0xc4 = 1.5; D+0xc0 = Ratchet's moby; D+0xb0 = the feet, D+0xbc = 0; D+0xd0 = D+0xd8 = pvar +0x20; D+0xe0 = pvar +0x24; D+0x28 = 0, D+0x20 = 0.01, D+0x24 = 0.2, D+0x10.. = 0; D+0x00 = the feet; D+0x30 = ticks(120); D+0x110..0x120 = 0, D+0x124 = Ratchet's z; D+0xf0 = row 0 of the record's Euler rotation | [`Camera::side_init`] (D+0xc0, D+0xd8, D+0x30, D+0x110..0x120: n/a, no reader) |
//! | 0x2eb7e0 | position = feet + up·D+0xe0 − D+0xd0·D+0xf0; look = feet + up·D+0xc4; D+0x100 = forward = +0x40 = unit(look − position); left = −unit(gravity × forward); up = forward × left | `side_init` |
//! | | +0x7e = 0; 0x1673f4 = 120 (the orbit blend's length after the switch's 40) | `side_init` |
//! | 0x2ebb00 | not (mode 0x50 and group ≠ 0x11) → D+0x124 = Ratchet's z (a jump keeps the height) | [`Camera::side_update`] |
//! | | D+0x00 / +0x04 / +0x08 spring to (Ratchet x, y, D+0x124) (`Cam_InterpValues` D+0x20, D+0x24, D+0x28, D+0x10..); position = D + up·D+0xe0 − D+0xd0·D+0xf0 | `side_update` |
//! | 0x2ebaf0 | empty | n/a |
//! | 0x2ebc58 | D+0xb8 springs to Ratchet's z (0.005, 0.2, D+0xbc) | `side_update` (`SideState::z_s`; no reader: n/a) |
//! | | look = D + up2 (0x1672c0)·D+0xc4; forward = +0x40 = unit(look − position); left = up_s × forward; up = forward × left | `side_update` |
//! | 0x2ebde8 | cuboid ≥ 0 and the feet out of it → +0x7e = 3 | [`Camera::side_pre`] |
//! | 0x20d110 | in: class 1 blend 1 (03): the rates blend at 0.018; blend 3 (04): the pose copied (a cut) unless the hook set +0x7e = 2 (the rates blend); class 14 blend 5: the orbit blend (120 ticks); out: +0x7e = 3 → the pose copied, no blend | `Camera::class_switch_in` / `follow_switch_in` |
//!
//! Native `f32`.

use super::class_cam::{rows_about, set_len};
use super::level::Best;
use super::script::{angle_spring, euler_rows, spring};
use super::{fadd, fdot, frot, fsub, CamInput, Camera};
use crate::hero::physics::to_f32x3;
use crate::moby_update::triggers::point_in_cuboid;
use std::f32::consts::FRAC_PI_2;

/// The camera classes.
pub const CLASS_FIXED: i32 = 1;
pub const CLASS_SIDE: i32 = 14;
/// Level 03's functions: hook, init, update, pre hook ([`super::level::CameraPorts`] looks for their copies) and the
/// helpers they call.
pub const FIXED_FNS: [u32; 4] = [0x2e_8870, 0x2e_8940, 0x2e_8960, 0x2e_8980];
pub const FIXED_HELPERS: [u32; 2] = [0x2e_8628, 0x2e_8730];
pub const SIDE_FNS: [u32; 4] = [0x2e_bd70, 0x2e_b978, 0x2e_bdb0, 0x2e_bde8];
pub const SIDE_HELPERS: [u32; 4] = [0x2e_b7e0, 0x2e_bb00, 0x2e_baf0, 0x2e_bc58];

/// The hero's jump camera mode (0x1415d4) and the groups class 14 reads.
const MODE_JUMP: i32 = 0x50;
const GROUP_WATER: i32 = 0x11;
const GROUP_6: i32 = 6;

/// Class 1's D words.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FixedState {
    /// D+0x00: the angle spring's velocity.
    pub vel: f32,
    /// D+0x04 / +0x08 / +0x0c: its k, d and limit.
    pub k: f32,
    pub d: f32,
    pub max: f32,
    /// D+0x5c: the look height above the feet.
    pub look_h: f32,
}

/// Class 14's D words.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SideState {
    /// D+0x00..0x08: the smoothed Ratchet; D+0x10..0x18 its velocities; D+0x20 / +0x24 / +0x28 k, d, limit.
    pub at: [f32; 3],
    pub vel: [f32; 3],
    pub k: f32,
    pub d: f32,
    pub max: f32,
    /// D+0x124: the height followed (held during a jump).
    pub held_z: f32,
    /// D+0xb8 / +0xbc: Ratchet's smoothed z and its velocity (written, not read).
    pub z_s: f32,
    pub z_vel: f32,
    /// D+0xc4 look height, D+0xd0 distance, D+0xe0 height, D+0xf0 the facing.
    pub look_h: f32,
    pub dist: f32,
    pub height: f32,
    pub facing: [f32; 3],
}

/// The class-14 hook `0x2ebd70` (module doc).
pub fn side_hook(h: &crate::hero::Hero) -> i32 {
    if h.f15d4 == MODE_JUMP {
        if h.group != GROUP_WATER { -1 } else { 0 }
    } else if h.group == GROUP_6 {
        -1
    } else {
        0
    }
}

impl Camera {
    fn feet_in(&self, inp: &CamInput, cuboid: i32) -> bool {
        self.level_cams.shapes().is_some_and(|v| point_in_cuboid(v, to_f32x3(inp.hero.pos), cuboid))
    }

    /// The class-1 hook `0x2e8870` for slot `i` against the best so far (module doc).
    pub(super) fn fixed_hook(&mut self, i: usize, best: Option<Best>, inp: &CamInput) -> i32 {
        let hd = self.level_cams.slots[i].header;
        if hd.activation != 4 { return 0; }
        if best.is_some_and(|b| !b.releasing && hd.priority <= b.priority) { return 0; }
        if !self.feet_in(inp, hd.cuboid) { return 0; }
        let cam = to_f32x3(self.out.pos);
        if self.level_cams.shapes().is_some_and(|v| point_in_cuboid(v, cam, hd.cuboid)) { return 1; }
        // The camera outside the cuboid: blend from it (0x167373 = 0, the current camera's +0x7e = 2, rates 0.018).
        self.blend.kind = 0;
        self.set_current_release(2);
        self.blend.next_rot_rate = 0.018;
        self.blend.next_pos_rate = 0.018;
        1
    }

    /// The init `0x2e8628` of the current class-1 camera.
    pub(super) fn fixed_init(&mut self) {
        let s = &self.level_cams.slots[self.class_cam.slot];
        let (rec, hd) = (s.record, s.header);
        let c = &mut self.class_cam;
        let f = &mut c.fixed;
        f.look_h = hd.f18;
        f.k = 0.02;
        f.d = 0.1;
        f.max = 0.0;
        c.pos = rec.pos;
        c.rows = euler_rows(rec.rot);
        c.saved_fwd = c.rows[0];
        c.release = 0;
    }

    /// The update `0x2e8730` of the current class-1 camera.
    pub(super) fn fixed_update(&mut self, inp: &CamInput) {
        let up_s = to_f32x3(self.g.up_s);
        let mut look = to_f32x3(inp.hero.pos);
        let c = &mut self.class_cam;
        look[2] += c.fixed.look_h;
        let dir = set_len(fsub(look, c.pos), 1.0);
        let target = FRAC_PI_2 - fdot(dir, c.rows[0]).clamp(-1.0, 1.0).asin();
        let f = &mut c.fixed;
        let turn = angle_spring(0.0, target, f.k, f.d, f.max, &mut f.vel);
        // 0x24e6f8(turn, forward, saved, dir, 1): the saved forward turned toward dir by that share of the angle.
        let a = (FRAC_PI_2 - fdot(c.saved_fwd, dir).clamp(-1.0, 1.0).asin()) * turn;
        let fwd = frot(c.saved_fwd, a, super::fcross(c.saved_fwd, dir));
        c.saved_fwd = set_len(fwd, 1.0);
        c.rows = rows_about(c.saved_fwd, up_s);
    }

    /// The pre hook `0x2e8980` of the current class-1 camera.
    pub(super) fn fixed_pre(&mut self, inp: &CamInput) {
        let hd = self.level_cams.slots[self.class_cam.slot].header;
        if hd.activation == 4 && !self.feet_in(inp, hd.cuboid) { self.class_cam.release = 3; }
    }

    /// The init `0x2eb978` (with `0x2eb7e0`) of the current class-14 camera.
    pub(super) fn side_init(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let feet = to_f32x3(h.pos);
        let g = to_f32x3(h.gravity_dir);
        let s = &self.level_cams.slots[self.class_cam.slot];
        let (rec, sv) = (s.record, s.side.unwrap_or_default());
        let c = &mut self.class_cam;
        let st = &mut c.side;
        st.look_h = 1.5;
        st.z_s = feet[2];
        st.z_vel = 0.0;
        st.dist = sv.distance;
        st.height = sv.height;
        st.max = 0.0;
        st.k = 0.01;
        st.d = 0.2;
        st.vel = [0.0; 3];
        st.at = feet;
        st.held_z = feet[2];
        st.facing = euler_rows(rec.rot)[0];
        // 0x2eb7e0: placed at once.
        let up_h = set_len(g, -st.height);
        c.pos = fadd(fadd(feet, up_h), set_len(st.facing, -st.dist));
        let look = fadd(feet, set_len(g, -st.look_h));
        let fwd = set_len(fsub(look, c.pos), 1.0);
        let left = set_len(super::fcross(g, fwd), -1.0);
        c.rows = [fwd, left, super::fcross(fwd, left)];
        c.saved_fwd = fwd;
        c.release = 0;
        self.blend.orbit_len = 120;
    }

    /// The update `0x2ebdb0` (`0x2ebb00`, `0x2ebaf0`, `0x2ebc58`) of the current class-14 camera.
    pub(super) fn side_update(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let hero = to_f32x3(h.pos);
        let g = to_f32x3(h.gravity_dir);
        let (up_s, up2) = (to_f32x3(self.g.up_s), to_f32x3(self.g.up2));
        let c = &mut self.class_cam;
        let st = &mut c.side;
        if !(h.f15d4 == MODE_JUMP && h.group != GROUP_WATER) { st.held_z = hero[2]; }
        let a = set_len(g, -st.height);
        let b = set_len(st.facing, -st.dist);
        let tgt = [hero[0], hero[1], st.held_z];
        for k in 0..3 { st.at[k] = spring(st.at[k], tgt[k], st.k, st.d, st.max, &mut st.vel[k]); }
        c.pos = fadd(fadd(st.at, a), b);
        st.z_s = spring(st.z_s, hero[2], 0.005, 0.2, 0.0, &mut st.z_vel);
        let look = fadd(st.at, set_len(up2, st.look_h));
        let fwd = set_len(fsub(look, c.pos), 1.0);
        c.saved_fwd = fwd;
        c.rows = rows_about(fwd, up_s);
    }

    /// The pre hook `0x2ebde8` of the current class-14 camera.
    pub(super) fn side_pre(&mut self, inp: &CamInput) {
        let hd = self.level_cams.slots[self.class_cam.slot].header;
        if 0 <= hd.cuboid && !self.feet_in(inp, hd.cuboid) { self.class_cam.release = 3; }
    }
}
