//! **The level-class cameras that become current** (classes 3, 1, 14, 8, 19, 20 and 21: [`super::rail`], [`super::cuboid`],
//! [`super::board`], [`super::flyby`], [`super::race`]) and the
//! part of the camera switch `FUN_0020d110` they share with every other camera: the blend by the new camera's record
//! (pvar +0x1d) and the old camera's +0x7e. Spec: docs/plan/player_controller.md §15.
//!
//! **System or not.** The switch, the choice (`UpdateAllCameras` 0x20d620 / `Camera_ActivationCheckPriority` 0x20d410)
//! and the shared data block D (0x169590: every current camera's UpdateCam +0x70; the switch copies it to 0x169810 for
//! the camera it leaves) are one system on all 19 levels ([`super::level`]). The classes are per class: each has its own
//! four `camvtbl` functions, one code on the levels that list it ([`super::level::CameraPorts`]). This module is the
//! plumbing a class needs to be current (its UpdateCam words, the switch in and out, the pre hook → choice → update
//! order); a new class adds its state to [`ClassCam`], its four functions, a row in [`CLASS_CAMS`] and its
//! `CameraPorts` check.
//!
//! **The shared D block.** The block is not cleared on a switch: a camera reads what the one before it left in the
//! words its init does not write. Read before written by the ported classes: class 3's angle-spring velocity D+0x64
//! (after the follow camera: its raise end point A's y, D+0x64) and class 1's D+0x00 (after the follow camera: the
//! desired point's x); [`Camera::d_word_00`] / [`Camera::d_word_64`] give them from the camera being left. The words
//! the classes write into the follow camera's block (class 3: D+0x00..0x5c, 0x80.., 0x100..0x160; class 14: 0x00..0x30,
//! 0xb0..0x124) are all rewritten by its init `0x311f38` / `0x311dd0` before it reads them, except D+0x105 / 0x108 /
//! 0x10c / 0x110 / 0x114 / 0x117 (the target mode's previous mode and blend, the raise), which the follow camera reads
//! only after a target-mode change that writes them first, and D+0x64 (the raise's start, written at the raise's
//! entry): n/a.
//!
//! | address | call / branch | port |
//! |---|---|---|
//! | 0x20d110 | old +0x7e = 4 → new +0x8e = 1, no blend | [`Camera::switch_blend`] (+0x8e: no reader in the camera code, n/a) |
//! | | old +0x7e = 2: new blend 1 → the rates blend at +0x78 (0.018); 5 → the orbit blend (0x167373 = 2) over +0x78 ticks (40); any → 0x167370 = 1 (2 while one runs) | `switch_blend` |
//! | | new blend 1 → the rates blend (0.018), kind 0; 5 → the orbit (40 ticks) | `switch_blend` |
//! | | old +0x7e = 3 / 5 or new blend 3 / 6 → the old pose copied, +0x7d = 2; old 5 or new 6 → the rates blend at +0x78 (0.018; 0.01 on level 1), kind 0 | `switch_blend` (the copy: the caller) |
//! | | else → new +0x8e = 1 | n/a (no reader) |
//! | | UpdateCam +0x78: −1.0 on every slot (the slot init 0x20ef58; no other writer) → 0.018 / 40 | the constants |
//! | | old +0x7e / +0x7d / +0x8e = 0; 0x167284 = old; D → 0x169810; new +0x70 = D; 0x167498 = 0 | [`Camera::class_switch_in`], [`Camera::follow_switch_in`] |
//! | | the new camera's init (+0x8); `BackupCurrentCam`; 0x167240 = the new position (unless 0x16c4ec); +0x64 = +0x30 | the class inits; `d0` for the follow camera; 0x167240: the port publishes after the update (n/a) |
//! | 0x20d620 | the current camera's pre hook (+0x10) before the choice; after a switch the new camera's update (+0xc) runs this tick; +0x64 = +0x30 | [`Camera::class_frame`] |
//! | 0x20ee80 | `CameraResetBehindHero`: the current camera's +0x8e / +0x7d / +0x7e = 0, the follow camera current, 0x167370 = 0 (no blend) | `Camera::reset` (the level-class and Swingshot cameras dropped, the blend stopped) |

use super::{level::pos4, rows_f, rows_pf, CamInput, Camera, R3, V4};
use super::cuboid::{CLASS_FIXED, CLASS_SIDE};
use super::level::CLASS_FOLLOW;
use super::rail::CLASS_RAIL;
use super::board::CLASS_BOARD;
use super::race::{CLASS_INTRO, CLASS_RACE};

/// The level classes the port runs as a current camera through this module.
pub const CLASS_CAMS: [i32; 7] = [CLASS_RAIL, CLASS_FIXED, CLASS_SIDE, CLASS_BOARD, super::flyby::CLASS_FLYBY, CLASS_INTRO, CLASS_RACE];

/// `0x20d110`'s rates blend rate with UpdateCam +0x78 = −1 (`0.018`; the level-1 copy path's 0.01 is separate).
const BLEND_RATE: f32 = 0.018;
/// `0x20d110`'s orbit blend length with +0x78 = −1 (`0x28`).
const ORBIT_TICKS: i32 = 0x28;

/// The current level-class camera: its UpdateCam words and the D-block words each class reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassCam {
    /// A level-class camera is current (0x167280 +0x86 = `class`).
    pub active: bool,
    pub class: i32,
    /// Its slot (UpdateCam index = record index).
    pub slot: usize,
    /// UpdateCam +0x00 / +0x10 / +0x20 rows (forward, left, up), +0x30 position, +0x40 saved forward, +0x64 the
    /// position after the last update.
    pub rows: R3,
    pub pos: [f32; 3],
    pub saved_fwd: [f32; 3],
    pub prev_pos: [f32; 3],
    /// +0x7e: set by its pre hook when it lets go (3; class 3's mode 3: 5), or 2 by class 1's hook.
    pub release: u8,
    /// Class 3's D words.
    pub rail: super::rail::RailState,
    /// Class 1's D words.
    pub fixed: super::cuboid::FixedState,
    /// Class 14's D words.
    pub side: super::cuboid::SideState,
    /// Class 8's D words.
    pub board: super::board::BoardCamState,
    /// Class 19's D words (class 20's too).
    pub flyby: super::flyby::FlybyState,
    /// Class 21's D words.
    pub race: super::race::RaceState,
}

impl Camera {
    /// `FUN_0020d110`'s blend for a switch to a camera whose record has blend kind `blend` (pvar +0x1d) from one whose
    /// +0x7e is `release` (module doc). Returns whether the old pose is copied (+0x7d = 2).
    pub(super) fn switch_blend(&mut self, blend: u8, release: u8) -> bool {
        let level = self.level_cams.level;
        let b = &mut self.blend;
        let start = |b: &mut super::CamBlend| b.mode = if b.mode == 0 { 1 } else { 2 };
        let rates = |b: &mut super::CamBlend, r: f32| {
            b.next_rot_rate = r;
            b.next_pos_rate = r;
            b.kind = 0;
        };
        let orbit = |b: &mut super::CamBlend| {
            b.kind = 2;
            b.orbit_len = ORBIT_TICKS;
        };
        if release == 4 { return false; }
        if release == 2 {
            match blend {
                1 => rates(b, BLEND_RATE),
                5 => orbit(b),
                _ => {}
            }
            start(b);
            return false;
        }
        match blend {
            1 => {
                rates(b, BLEND_RATE);
                start(b);
                false
            }
            5 => {
                orbit(b);
                start(b);
                false
            }
            _ if release == 3 || release == 5 || blend == 3 || blend == 6 => {
                if release == 5 || blend == 6 {
                    rates(b, if level == 1 { 0.01 } else { BLEND_RATE });
                    start(b);
                }
                true
            }
            _ => false,
        }
    }

    /// The shared D block's word +0x00 as the current camera left it (class 1 reads it as its spring velocity before
    /// writing it): the follow camera's desired point x, class 3's position velocity x, class 1's own, class 14's
    /// smoothed x.
    pub(super) fn d_word_00(&self) -> f32 {
        if self.class_cam.active {
            match self.class_cam.class {
                CLASS_RAIL => return self.class_cam.rail.vel[0],
                CLASS_FIXED => return self.class_cam.fixed.vel,
                CLASS_SIDE => return self.class_cam.side.at[0],
                CLASS_BOARD => return self.class_cam.board.vel[0],
                _ => {}
            }
        }
        self.cam.desired[0].to_f32()
    }

    /// The shared D block's word +0x64 as the current camera left it (class 3 reads it as its angle-spring velocity
    /// before writing it): class 3's own, else the follow camera's raise end point A's y (classes 1 and 14 do not
    /// write it).
    pub(super) fn d_word_64(&self) -> f32 {
        if self.class_cam.active && self.class_cam.class == CLASS_RAIL { return self.class_cam.rail.ang_vel; }
        self.cam.raise_a[1].to_f32()
    }

    /// The class's record slot of the follow camera's blend kind (pvar +0x1d; 0 on every level).
    fn follow_blend(&self) -> u8 { self.level_cams.slots.iter().find(|s| s.record.class == CLASS_FOLLOW).map_or(0, |s| s.header.blend) }

    /// `FUN_0020d110` to the level-class camera `class` of slot `slot` from the current camera (`prev`: its rows and
    /// position; its +0x7e read here), then this tick's update (`0x20d620`).
    pub(super) fn class_switch_in(&mut self, inp: &CamInput, class: i32, slot: usize, prev: ([V4; 3], V4)) {
        let release = self.current_release();
        let blend = self.level_cams.slots[slot].header.blend;
        // The words the new camera reads from the shared block before writing them (module doc).
        let (d00, d64) = (self.d_word_00(), self.d_word_64());
        self.switch_blend(blend, release);
        // The old camera's +0x7e / +0x7d / +0x8e = 0.
        self.set_current_release(0);
        self.swing.active = false;
        self.g.since_switch = 0;
        let prev = (rows_f(prev.0), crate::hero::physics::to_f32x3(prev.1));
        let c = &mut self.class_cam;
        c.active = true;
        c.class = class;
        c.slot = slot;
        c.release = 0;
        match class {
            CLASS_RAIL => {
                self.class_cam.rail.ang_vel = d64;
                self.rail_init(inp, prev);
            }
            CLASS_FIXED => {
                self.class_cam.fixed.vel = d00;
                self.fixed_init();
            }
            CLASS_SIDE => self.side_init(inp),
            CLASS_BOARD => self.board_init(inp, prev),
            super::flyby::CLASS_FLYBY => self.flyby_init(prev),
            CLASS_INTRO => self.intro_init(prev),
            CLASS_RACE => self.race_init(inp),
            _ => {}
        }
        self.class_cam.prev_pos = self.class_cam.pos;
        self.class_update(inp);
        self.class_cam.prev_pos = self.class_cam.pos;
    }

    /// `FUN_0020d110` back to the follow camera from the current level-class camera (`prev`: its view): the follow
    /// record's blend against the class camera's +0x7e (3 / 5: the pose copied, 5 with the rates blend), then the
    /// follow camera's init (`0x311dd0` under the copied pose, the row blend D+0x20 = 90 ticks; or the snap) and
    /// `BackupCurrentCam`. The follow camera's update runs after this in the tick.
    pub(super) fn follow_switch_in(&mut self, inp: &CamInput, prev: ([V4; 3], V4)) {
        let release = self.current_release();
        let copied = self.switch_blend(self.follow_blend(), release);
        self.class_cam.active = false;
        self.class_cam.release = 0;
        self.level_cams.release = 0;
        self.g.since_switch = 0;
        if copied {
            self.cam.rows = prev.0;
            self.cam.pos = prev.1;
            // 0x311f38: D+0x20 = 0 only after the Swingshot camera on the ground; a level-class camera → 90 ticks.
            self.init_from(inp, Some(false));
        } else {
            self.init_from(inp, None);
        }
        self.d0 = self.cam;
    }

    /// `UpdateAllCameras` with a level-class camera current: its pre hook, the choice, the switch (to the follow
    /// camera, another level-class camera (class 3's next rail), or the Swingshot camera), or its update. A class the
    /// port does not run winning is recorded in [`super::level::LevelCameras::wanted`]; over a released camera the
    /// follow camera comes back instead (G-HERO-027). Returns the camera a switch left (the blend's capture).
    pub(super) fn class_frame(&mut self, inp: &CamInput) -> Option<([V4; 3], V4)> {
        self.class_pre(inp);
        let won = self.activation_loop(inp);
        let slot = self.level_cams.won_slot;
        match won {
            Some(CLASS_FOLLOW) => {
                let prev = self.active_view();
                self.follow_switch_in(inp, prev);
                return Some(prev);
            }
            Some(c) if self.level_cams.ports.runs_class_cam(c) && slot.is_some() => {
                let prev = self.active_view();
                self.class_switch_in(inp, c, slot.unwrap(), prev);
                return Some(prev);
            }
            Some(c) if c == super::swing::CLASS_SWING && self.level_cams.ports.swing => {
                let prev = self.active_view();
                let release = self.current_release();
                self.switch_blend(self.level_cams.slots.get(slot.unwrap_or(usize::MAX)).map_or(3, |s| s.header.blend), release);
                self.class_cam.active = false;
                self.class_cam.release = 0;
                self.swing_switch_in(inp, prev);
                return Some(prev);
            }
            Some(c) => {
                self.level_cams.wanted = Some(c);
                if self.class_cam.release != 0 {
                    let prev = self.active_view();
                    self.follow_switch_in(inp, prev);
                    return Some(prev);
                }
            }
            None => {}
        }
        self.class_update(inp);
        self.class_cam.prev_pos = self.class_cam.pos;
        None
    }

    /// The current level-class camera's pre hook (`camvtbl` +0x10).
    fn class_pre(&mut self, inp: &CamInput) {
        match self.class_cam.class {
            CLASS_RAIL => self.rail_pre(inp),
            CLASS_FIXED => self.fixed_pre(inp),
            CLASS_SIDE => self.side_pre(inp),
            CLASS_RACE => self.race_pre(inp),
            _ => {}
        }
    }

    /// The current level-class camera's update (`camvtbl` +0xc).
    fn class_update(&mut self, inp: &CamInput) {
        match self.class_cam.class {
            CLASS_RAIL => self.rail_update(inp),
            CLASS_FIXED => self.fixed_update(inp),
            CLASS_SIDE => self.side_update(inp),
            CLASS_BOARD => self.board_update(inp),
            super::flyby::CLASS_FLYBY => self.flyby_update(inp),
            CLASS_INTRO => self.intro_update(inp),
            CLASS_RACE => self.race_update(inp),
            _ => {}
        }
    }

    /// The current level-class camera's view (rows, position).
    pub(super) fn class_view(&self) -> ([V4; 3], V4) { (rows_pf(self.class_cam.rows), pos4(self.class_cam.pos)) }
}

/// `FastVecNormalize(len, v)` as the class cameras use it: `v` scaled to `len`, a zero vector stays zero.
pub(super) fn set_len(v: [f32; 3], len: f32) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { [0.0; 3] } else { [v[0] * len / l, v[1] * len / l, v[2] * len / l] }
}

/// The rows (forward, left, up) from a forward and the up vector `up` (`left = up × fwd` normalised, `up' = fwd × left`),
/// as the class cameras finish their updates.
pub(super) fn rows_about(fwd: [f32; 3], up: [f32; 3]) -> R3 {
    let left = set_len(super::fcross(up, fwd), 1.0);
    let up2 = super::fcross(fwd, left);
    [fwd, left, up2]
}
