//! **The level camera system**: the level's camera records (`0x15ef50`, [`rc_formats::cameras`]) as the 48 UpdateCam
//! slots, the per-tick choice of the active camera (`UpdateAllCameras` 0x20d620 with `Camera_ActivationCheckPriority`
//! 0x20d410), and the class-17 regions that retune the follow camera (activation `0x319688` → `0x318de0`, region test
//! `0x318c40`). Spec: docs/plan/player_controller.md §15.
//!
//! **Slots** (`FUN_0020ef58`, level start): one UpdateCam (0x1675d0 + i·0xa0, active flag 0x169d10[i]) per record, in
//! record order: +0x84 the record index, +0x86 the class, +0x8c its mode (the class's row of the level's
//! `lvl.camvtbl`), +0x7c the priority (pvar +0x1c), +0x74 the activation kind (pvar +0x1f), +0x78 = −1, +0x7d / +0x7e /
//! +0x8e = 0; the record's pvar +0x04 = the slot. Then `CameraResetBehindHero` 0x20ee80 makes the class-0 slot (the
//! follow camera) current.
//!
//! **The choice** (0x20d620): the current camera's pre hook, then for every active slot but the current one in slot
//! order `Camera_ActivationCheckPriority(slot, best)`; a true answer makes that slot the best (later slots are compared
//! with it). The check ([`activation_check`]): priority 0 never; the class's activation hook first (−1 no, 1 yes, 0 go
//! on); then by the kind: 0 always, 1 / 2 once entered (+0x7d), 4 Ratchet's feet in the cuboid (pvar +0x0c), 7 the
//! hero's camera mode 0x1415d4 equal to the class (class 3 with a rail in pvar +0x24: on that rail, not off its end),
//! any other kind never; and a candidate beats the best only with a **higher** priority, unless the best is releasing
//! (+0x7e ≠ 0). A new best is switched in (`FUN_0020d110`: its blend kind, pvar +0x1d; the port's switches are the
//! first-person, script and type-6 cameras' own, `super`), then its update runs.
//!
//! **What the port runs** ([`CameraPorts`]): class 0 (the follow camera, `super::Camera::update_type0`), 4 (first
//! person), 5 (script) and 6 (the Visibomb's view) — their switches are the existing ones — 7 (the Swingshot camera,
//! [`super::swing`]: switched in and out here), the level-class cameras 3 (the rail / slide camera, [`super::rail`]),
//! 1 (the fixed view) and 14 (the side view, [`super::cuboid`]) through [`super::class_cam`] (the switch's blends
//! `0x20d110`, the shared D block), and the hooks of the never-current classes 17 (below), 23 (the placed view,
//! `placed_*`) and 18 (the moby focus, [`super::focus`]), each on the levels whose code for it is the reference level's.
//! The loop checks every slot but the current camera's own (two class-3 records hand over at a rail change). A choice
//! of any other class (8: the hoverboard, G-HERO-008; 19: the armed fly-bys, no ported arming class; 20 / 21: the
//! level-14 grind race, G-LVL-007; 22: giant Clank, G-HERO-005) is recorded in [`LevelCameras::wanted`] and the
//! current camera stays (G-HERO-027).
//!
//! **Class 17** ([`RegionTweak`]): its hook always answers "no" (it never becomes the camera) and, while the follow
//! camera is current, retunes it through the follow camera's setters (`0x313560`..`0x313b48`, [`super::Camera`]):
//! * Hook `0x319688`: nothing after a "once" region was left (+0x48 = 1, +0x4a ≠ 0); mode 11 only with body 2
//!   (0x1413f4); modes 1 / 10 do not start (+0x2e = 0) while Ratchet is off the ground (0x13f65c ≠ 0) unless on level
//!   14 hanging from a ledge (0x1415d4 = 0xd); mode 2 outside group 0x1a (the cable) only zeroes the counter (the
//!   owner lock stays); else `0x318de0`.
//! * `0x318de0`: mode 8 only while the camera data's +0x230 ≠ 0, mode 6 only while it is 0. Out of the region
//!   (`0x318c40`) → leave (the counter → 0, +0x4a = 1 if it was running, the owner lock released `0x313560`). The lock
//!   (`0x313598`, the camera data's +0x220): free, already this region's, or held by a lower priority → taken; else
//!   the counter → 0 (+0x4a) without a release. Leave when +0x36 is set and the camera yaw 0x167258 is 80° or more off
//!   the record's `rot.z`, or (+0x48, mode 5) L1 / L2 is held (0x13cae0 & 5). On the first tick (+0x2e = 0) the
//!   follow camera's horizontal spring is captured (+0x40 / +0x44); mode 5's first tick places the follow camera at the
//!   region's distance, pivot and look heights at once (the pvar's turn +0x00 zeroed). Then +0x2e += 1; mode 10 → the
//!   stick off (`0x313858`) and +0x230 = 0x14d (the ledge turn of `0x3111d8` then stays off); mode 11 → the stick off,
//!   +0x230 = 2; modes 2 / 4 → the stick off (+0x1b8 |= 3), the counter held at 200, the run-toward lock (`0x3137f8`);
//!   modes 1 / 7 / 9 / 10 → the stick moving sets the counter to 200, at 200 the turn stops (held there while Ratchet
//!   moves), at 400 it starts again from 1; mode 3 → the counter held at 1; others → held at 200. The turn (not while
//!   stopped in modes 1 / 7 / 9): `0x313af0(turn·(counter / 200), tolerance, (cos rot.z, sin rot.z, 0))` (mode 3:
//!   `/ 1`). Distance (+0x24) → `0x313628` at 0.003 and the run-toward lock (modes 6 / 8: 0.002 and the pull-back
//!   maximum `0x313668(d + 1.36)`); pivot height (+0x28) → `0x313690` (0.003 / 0.002); look height (+0x30) →
//!   `0x3136c8` (0.005 / 0.002); pitch (+0x50, degrees) → `0x313718`; +0x54 / +0x56 → +0x1b8 |= 2 / 1; +0x34 = 0 →
//!   the leash off; the spring (+0x38, +0x3c) eased from the captured one over 120 ticks (`0x313768`); modes 1 / 7 /
//!   10 → `0x3137f8`, `0x313820`, the vertical spring 0.01 / 0.2 (`0x3137b0`).
//! * Region test `0x318c40`: false unless the follow camera is current; cuboid +0x0c, cuboid +0x4c, cylinder +0x10,
//!   sphere +0x08 (Ratchet's feet 0x13f3d0), then the path polygon +0x14; any present shape counts as inside
//!   without a test while the region is running (+0x2e > 0) in mode 2 with Ratchet in group 0x1a, or in mode 4 with
//!   the camera mode 0x1415d4 = 3 (the cable's, the rails').
//!
//! Native `f32` for the region's arithmetic; the values go into the PS2-float follow camera through [`Pf::f`].

use super::{Camera, CamInput, V4};
use crate::hero::physics::to_f32x3;
use crate::moby_update::triggers::{point_in_cuboid, point_in_cylinder, point_in_path, point_in_sphere};
use crate::pad::fast_diff_rots;
use crate::ps2v::Pf;
use rc_formats::cameras::{CameraHeader, CameraRecord, LevelCamera, MobyFocus, PlacedView, RailCamera, RegionTweak, SideView};
use rc_formats::volumes::Volumes;
use rc_formats::level_overlay::LevelOverlay;
use std::sync::Arc;

/// The class of the follow camera, the first-person, script and type-6 cameras, and the tweak regions.
pub const CLASS_FOLLOW: i32 = 0;
pub const CLASS_FIRST_PERSON: i32 = 4;
pub const CLASS_SCRIPT: i32 = 5;
pub const CLASS_TYPE6: i32 = 6;
pub const CLASS_REGION: i32 = 17;
pub const CLASS_PLACED: i32 = 23;
pub const CLASS_FOCUS: i32 = 18;
pub use super::swing::CLASS_SWING;

/// The level-01 functions of class 17 ([`CameraPorts::from_overlays`] looks for their copies).
pub const REGION_ACTIVATE: u32 = 0x31_9688;
pub const REGION_UPDATE: u32 = 0x31_8de0;
pub const REGION_TEST: u32 = 0x31_8c40;
/// The level-00 functions of class 23.
pub const PLACED_ACTIVATE: u32 = 0x2e_d8b0;
pub const PLACED_TEST: u32 = 0x2e_d348;
pub const PLACED_UPDATE: u32 = 0x2e_d498;
/// The level-02 functions of class 18 (hook, update, region test, view test).
pub const FOCUS_ACTIVATE: u32 = 0x2f_c298;
pub const FOCUS_UPDATE: u32 = 0x2f_b9c8;
pub const FOCUS_TEST: u32 = 0x2f_b648;
pub const FOCUS_VIEW: u32 = 0x2f_b788;

/// A position as the PS2-typed camera keeps it (w = 1).
pub(super) fn pos4(p: [f32; 3]) -> V4 {
    let mut v = crate::hero::physics::from_f32x3(p);
    v[3] = crate::ps2v::Pf::ONE;
    v
}

/// The priorities the system cameras' records have on every level (class → pvar +0x1c), for a game without records.
fn default_priority(class: i32) -> u8 { if class == CLASS_FOLLOW { 5 } else { 6 } }

/// Which camera classes run the port on this level (the level's `lvl.camvtbl` against level 01's code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraPorts {
    /// Class 17's hook, update and region test are level 01's.
    pub region: bool,
    /// Class 7's init, update and pre hook are level 01's (the Swingshot camera, [`super::swing`]).
    pub swing: bool,
    /// Class 23's hook and its region test / update are level 00's (the placed view, [`PlacedView`]).
    pub placed: bool,
    /// Class 18's hook and helpers are level 02's (the moby focus, [`MobyFocus`]).
    pub focus: bool,
    /// Class 3's hook, init (with `0x314e98`), update and pre hook are level 01's (the rail camera, [`super::rail`]).
    pub rail: bool,
    /// Class 1's four functions and helpers are level 03's (the fixed view, [`super::cuboid`]).
    pub fixed: bool,
    /// Class 14's four functions and helpers are level 03's (the side view, [`super::cuboid`]).
    pub side: bool,
    /// Class 8's four functions and helpers are level 05's (the hoverboard camera, [`super::board`]).
    pub board: bool,
    /// Class 19's four functions and its region test are level 10's (the armed fly-by, [`super::flyby`]).
    pub flyby: bool,
    /// Classes 20 and 21's functions and helpers are level 14's (Oltanis's grind race, [`super::race`]).
    pub race: bool,
    /// Class 22's hook and helpers are level 15's (Giant Clank's view, [`super::giant`]).
    pub giant: bool,
}

impl Default for CameraPorts {
    fn default() -> Self { CameraPorts { region: true, swing: true, placed: true, focus: true, rail: true, fixed: true, side: true, board: true, flyby: true, race: true, giant: true } }
}

impl CameraPorts {
    /// From the level's overlay and level 01's: class 17 runs when the level's table lists it and its activation hook
    /// is a copy of `0x319688` and the level has copies of `0x318de0` and `0x318c40` (true on all 14 levels with the
    /// class; the census in docs/plan/player_controller.md §15).
    /// Classes 7 and 17 against level 01's code, 23 against level 00's, 18 against level 02's (`reference(level)`: that
    /// level's overlay; a class whose reference is missing is not run).
    pub fn from_overlays(target: &LevelOverlay, reference: &dyn Fn(u32) -> Option<Arc<LevelOverlay>>) -> CameraPorts {
        let Some(level01) = reference(1) else {
            return CameraPorts { region: false, swing: false, placed: false, focus: false, rail: false, fixed: false, side: false, board: false, flyby: false, race: false, giant: false };
        };
        let rel = rc_formats::level_overlay::Relocation::new(&level01, target);
        let region = target.camvtbl().iter().find(|e| e.class == CLASS_REGION).is_some_and(|e| {
            rel.same_code(REGION_ACTIVATE, e.activate) && rel.func(REGION_UPDATE).is_some() && rel.func(REGION_TEST).is_some()
        });
        let swing = target.camvtbl().iter().find(|e| e.class == CLASS_SWING).is_some_and(|e| {
            rel.same_code(super::swing::SWING_INIT, e.init)
                && rel.same_code(super::swing::SWING_UPDATE, e.update)
                && rel.same_code(super::swing::SWING_PRE, e.pre)
                && [0x31_82c8, 0x31_8900, 0x20_f2a8].iter().all(|&f| rel.func(f).is_some())
        });
        // A class whose hook (`words` long: the empty init / update / pre follow it, then other code) is a copy of
        // `activate` in `level`'s code, with copies of its helpers.
        let class_of = |class: i32, level: u32, activate: u32, words: usize, helpers: &[u32]| {
            use rc_formats::level_overlay::mask;
            reference(level).is_some_and(|r| {
                let rel = rc_formats::level_overlay::Relocation::new(&r, target);
                target.camvtbl().iter().find(|e| e.class == class).is_some_and(|e| {
                    let hook = r.code(activate, words).zip(target.code(e.activate, words)).is_some_and(|(a, b)| mask(a) == mask(b));
                    hook && helpers.iter().all(|&f| rel.func(f).is_some())
                })
            })
        };
        let placed = class_of(CLASS_PLACED, 0, PLACED_ACTIVATE, 0x88 / 4, &[PLACED_TEST, PLACED_UPDATE]);
        // Class 18's update runs up to its hook (no known function start between them, so its extent would take in the
        // code after the hook, which differs by level): the copy the target's hook calls, compared over its 562 words
        // (level 02 pads 2 more before the hook).
        let focus = class_of(CLASS_FOCUS, 2, FOCUS_ACTIVATE, 0x68 / 4, &[FOCUS_TEST, FOCUS_VIEW])
            && reference(2).is_some_and(|r| {
                use rc_formats::level_overlay::mask;
                let n = ((FOCUS_ACTIVATE - FOCUS_UPDATE) / 4 - 2) as usize;
                let hook = target.camvtbl().iter().find(|e| e.class == CLASS_FOCUS).map(|e| e.activate);
                let called = hook.and_then(|a| {
                    let code = target.code(a, 0x68 / 4)?;
                    code.iter().enumerate().find(|(_, w)| **w >> 26 == 3).map(|(k, w)| ((w & 0x3ff_ffff) << 2) | ((a + 4 * k as u32) & 0xf000_0000))
                });
                called.and_then(|c| target.code(c, n)).zip(r.code(FOCUS_UPDATE, n)).is_some_and(|(t, rf)| mask(t) == mask(rf))
            });
        // Class 3 (level 01) and classes 1 / 14 (level 03): every one of the four `camvtbl` functions a copy of the
        // reference's, with copies of the helpers they call.
        let four = |class: i32, level: u32, f: [u32; 4], helpers: &[u32]| {
            reference(level).is_some_and(|r| {
                let rel = rc_formats::level_overlay::Relocation::new(&r, target);
                target.camvtbl().iter().find(|e| e.class == class).is_some_and(|e| {
                    rel.same_code(f[0], e.activate)
                        && rel.same_code(f[1], e.init)
                        && rel.same_code(f[2], e.update)
                        && rel.same_code(f[3], e.pre)
                        && helpers.iter().all(|&h| rel.func(h).is_some())
                })
            })
        };
        use super::{cuboid, rail};
        let rail = four(rail::CLASS_RAIL, 1, [rail::RAIL_ACTIVATE, rail::RAIL_INIT, rail::RAIL_UPDATE, rail::RAIL_PRE], &[rail::RAIL_DATA_INIT]);
        let fixed = four(cuboid::CLASS_FIXED, 3, cuboid::FIXED_FNS, &cuboid::FIXED_HELPERS);
        let side = four(cuboid::CLASS_SIDE, 3, cuboid::SIDE_FNS, &cuboid::SIDE_HELPERS);
        let board = four(super::board::CLASS_BOARD, 5, super::board::BOARD_FNS, &super::board::BOARD_HELPERS);
        let flyby = four(super::flyby::CLASS_FLYBY, 10, super::flyby::FLYBY_FNS, &super::flyby::FLYBY_HELPERS);
        use super::race as rc;
        let race = four(rc::CLASS_INTRO, 14, rc::INTRO_FNS, &rc::INTRO_HELPERS) && four(rc::CLASS_RACE, 14, rc::RACE_FNS, &[]);
        let giant = class_of(super::giant::CLASS_GIANT, 15, super::giant::GIANT_HOOK, 0x80 / 4, &super::giant::GIANT_HELPERS);
        CameraPorts { region, swing, placed, focus, rail, fixed, side, board, flyby, race, giant }
    }

    /// Whether the port runs `class` as a current camera (the follow, first-person, script and type-6 cameras always).
    pub fn runs(&self, class: i32) -> bool {
        match class {
            CLASS_FOLLOW | CLASS_FIRST_PERSON | CLASS_SCRIPT | CLASS_TYPE6 => true,
            CLASS_SWING => self.swing,
            super::rail::CLASS_RAIL => self.rail,
            super::cuboid::CLASS_FIXED => self.fixed,
            super::cuboid::CLASS_SIDE => self.side,
            super::board::CLASS_BOARD => self.board,
            super::flyby::CLASS_FLYBY => self.flyby,
            super::race::CLASS_INTRO | super::race::CLASS_RACE => self.race,
            _ => false,
        }
    }

    /// Whether `class` is one of the level classes the port runs as a current camera through [`super::class_cam`].
    pub fn runs_class_cam(&self, class: i32) -> bool { super::class_cam::CLASS_CAMS.contains(&class) && self.runs(class) }
}

/// One UpdateCam slot's record, header and run-time words.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub record: CameraRecord,
    pub header: CameraHeader,
    /// Class 17's block (its run-time words live in it: +0x2e, +0x40 / +0x44, +0x4a, the zeroed turn).
    pub region: Option<RegionTweak>,
    /// Class 23's block (run time: +0x20, +0x26, +0x38..+0x40).
    pub placed: Option<PlacedView>,
    /// Class 18's block (run time: +0x20, +0x3e, +0x40, +0x50).
    pub focus: Option<MobyFocus>,
    /// Class 3's rail (pvar +0x24; −1: any).
    pub rail: i32,
    /// Class 3's block with its camera path and mode 2's mappings (run time: +0x34, +0x36; [`super::rail`]).
    pub rail_cam: Option<super::rail::RailSlot>,
    /// Class 14's block ([`super::cuboid`]).
    pub side: Option<SideView>,
    /// Class 8's block with its run-time word ([`super::board`]).
    pub board: Option<super::board::BoardRec>,
    /// Class 19's block with its run-time words +0x38 / +0x39 / +0x3c ([`super::flyby`]); class 20's too.
    pub flyby: Option<super::flyby::FlybyRec>,
    /// Class 20's words past class 19's ([`super::race`]).
    pub intro: Option<super::race::IntroRec>,
    /// Class 21's block with its run-time words +0x20, +0x30, +0x32 ([`super::race`]).
    pub race: Option<super::race::RaceRec>,
    /// Class 22's block with its run-time words +0x20, +0x3c ([`super::giant`]).
    pub giant: Option<super::giant::GiantView>,
}

/// The level's camera slots and the follow camera's lock words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelCameras {
    /// The level (0x15ed84; class 17's ledge exception reads it).
    pub level: u32,
    pub slots: Vec<Slot>,
    pub ports: CameraPorts,
    /// The camera data's +0x220: the region that holds the follow camera's settings (`0x313598` / `0x313560`).
    pub owner: Option<usize>,
    /// The camera data's +0x230: 0x14d (mode 10) / 2 (mode 11) this tick; 1 would be a focus moby near Ratchet
    /// (`0x3111d8`'s `coll_sphere_mobys`, G-HERO-026); reset by the follow camera's update.
    pub focus: i32,
    /// The class the choice picked this tick when the port does not run it (G-HERO-027), for the log and the tests.
    pub wanted: Option<i32>,
    /// The follow camera's UpdateCam +0x7e (releasing; class 1's hook sets it to 2 with its blend, `0x2e8870`); the
    /// switch `0x20d110` clears it on the camera it leaves.
    pub release: u8,
    /// The slot the last activation loop chose (with the class it returns).
    pub won_slot: Option<usize>,
    volumes: Option<Arc<Volumes>>,
    /// The records as loaded (a level restart rebuilds the slots from them: [`LevelCameras::restarted`]).
    source: Arc<[LevelCamera]>,
}

impl LevelCameras {
    /// The slots of `cams` (the slot init 0x20ef58) on `level` with its shapes.
    pub fn new(level: u32, cams: &[LevelCamera], volumes: Option<Arc<Volumes>>, ports: CameraPorts) -> LevelCameras {
        let slots = cams
            .iter()
            .take(48)
            .map(|c| {
                let p = c.pvar.as_deref().unwrap_or(&[]);
                let header = CameraHeader::parse(p).unwrap_or_default();
                let region = if c.record.class == CLASS_REGION { RegionTweak::parse(p) } else { None };
                let placed = if c.record.class == CLASS_PLACED { PlacedView::parse(p) } else { None };
                let focus = if c.record.class == CLASS_FOCUS { MobyFocus::parse(p) } else { None };
                let rail = if c.record.class == 3 && p.len() >= 0x28 { i32::from_le_bytes(p[0x24..0x28].try_into().unwrap()) } else { -1 };
                let rail_cam = if c.record.class == super::rail::CLASS_RAIL { RailCamera::parse(p).map(super::rail::RailSlot::new) } else { None };
                let side = if c.record.class == super::cuboid::CLASS_SIDE { SideView::parse(p) } else { None };
                let board = if c.record.class == super::board::CLASS_BOARD { super::board::BoardRec::parse(p) } else { None };
                let class = c.record.class;
                let flyby = if class == super::flyby::CLASS_FLYBY || class == super::race::CLASS_INTRO { super::flyby::FlybyRec::parse(p) } else { None };
                let intro = if class == super::race::CLASS_INTRO { super::race::IntroRec::parse(p) } else { None };
                let race = if class == super::race::CLASS_RACE { super::race::RaceRec::parse(p) } else { None };
                let giant = if class == super::giant::CLASS_GIANT { super::giant::GiantView::parse(p) } else { None };
                Slot { record: c.record, header, region, placed, focus, rail, rail_cam, side, board, flyby, intro, race, giant }
            })
            .collect();
        LevelCameras {
            level,
            slots,
            ports,
            owner: None,
            focus: 0,
            wanted: None,
            release: 0,
            won_slot: None,
            volumes,
            source: cams.to_vec().into(),
        }
    }

    /// The slots as the level's (re)start makes them (the pvar blocks as loaded).
    pub fn restarted(&self) -> LevelCameras { LevelCameras::new(self.level, &self.source, self.volumes.clone(), self.ports) }

    /// The priority of the current camera of `class` (its record's, or the value every level's record has).
    fn priority_of(&self, class: i32) -> u8 {
        self.slots.iter().find(|s| s.record.class == class).map_or(default_priority(class), |s| s.header.priority)
    }

    pub(super) fn shapes(&self) -> Option<&Volumes> { self.volumes.as_deref() }

    /// The level's shapes and splines, shared.
    pub(super) fn shapes_arc(&self) -> Option<Arc<Volumes>> { self.volumes.clone() }
}

/// A candidate's facts for `Camera_ActivationCheckPriority`.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub class: i32,
    pub priority: u8,
    /// UpdateCam +0x74.
    pub activation: u8,
    /// +0x7d.
    pub entered: bool,
    /// The class's hook: −1 no, 1 yes, 0 go on.
    pub hook: i32,
}

/// The best camera so far (None: none yet, the answer is then only the candidate's own).
#[derive(Clone, Copy, Debug)]
pub struct Best {
    pub priority: u8,
    /// +0x7e ≠ 0: releasing.
    pub releasing: bool,
}

/// `Camera_ActivationCheckPriority(cand, best)` 0x20d410. `in_cuboid` / `on_mode` are the kind-4 and kind-7 tests
/// (Ratchet's feet in the pvar +0x0c cuboid; the hero's camera mode equal to the class, plus class 3's rail).
pub fn activation_check(c: Candidate, best: Option<Best>, in_cuboid: impl FnOnce() -> bool, on_mode: impl FnOnce() -> bool) -> bool {
    if c.priority == 0 { return false; }
    if c.hook == -1 { return false; }
    if c.hook == 1 { return true; }
    let beats = |b: Option<Best>| b.is_none_or(|b| b.releasing || b.priority < c.priority);
    match c.activation {
        0 => beats(best),
        1 | 2 => c.entered && beats(best),
        4 => {
            // The priority first (a releasing best skips it), then the cuboid.
            if best.is_some_and(|b| !b.releasing && c.priority <= b.priority) { return false; }
            in_cuboid()
        }
        7 => on_mode() && beats(best),
        _ => false,
    }
}

impl Camera {
    /// Install the level's cameras (the slot init 0x20ef58; the owner lock and +0x230 start clear).
    pub fn set_level(&mut self, cams: LevelCameras) {
        self.level_cams = cams;
        // 0x20ef58 also clears the camera moby 0x167354, the focus moby 0x16735c and its counter 0x167360, and no
        // level-class camera is current.
        self.cam_moby = None;
        self.cam_moby_call = None;
        self.focus_moby = None;
        self.focus_ticks = 0;
        self.class_cam = Default::default();
        // 0x20ef58 also clears the Swingshot targets' hint (camera, target, weight, callback: 0x167480 / 0x167484 /
        // 0x16748c / 0x167490).
        self.hint.target = None;
        self.hint.weight = 0.0;
        self.hint.callback = false;
    }

    /// Whether the follow camera is the current one (0x167280 +0x86 = 0: the setters' guard).
    pub fn follow_is_current(&self) -> bool {
        !(self.type6.active || self.script.active || self.first_person.active || self.swing.active || self.class_cam.active)
    }

    /// The class of the current camera.
    pub fn current_class(&self) -> i32 {
        if self.type6.active {
            CLASS_TYPE6
        } else if self.script.active {
            CLASS_SCRIPT
        } else if self.first_person.active {
            CLASS_FIRST_PERSON
        } else if self.swing.active {
            CLASS_SWING
        } else if self.class_cam.active {
            self.class_cam.class
        } else {
            CLASS_FOLLOW
        }
    }

    /// The current camera's slot (`0x167280`): a level-class camera's own; for the system cameras the record of their
    /// class (one per level); None without records.
    pub(super) fn current_slot(&self) -> Option<usize> {
        if self.class_cam.active { return Some(self.class_cam.slot); }
        let cur = self.current_class();
        self.level_cams.slots.iter().position(|s| s.record.class == cur)
    }

    /// The current camera's +0x7e (releasing): the Swingshot camera's, a level-class camera's, or the follow camera's
    /// (class 1's hook writes it); 0 for the others (their releases are their own calls).
    pub(super) fn current_release(&self) -> u8 {
        match self.current_class() {
            CLASS_SWING => self.swing.release,
            CLASS_FOLLOW => self.level_cams.release,
            _ if self.class_cam.active => self.class_cam.release,
            _ => 0,
        }
    }

    /// Set the current camera's +0x7e (class 1's hook `0x2e8870`: 2).
    pub(super) fn set_current_release(&mut self, v: u8) {
        match self.current_class() {
            CLASS_SWING => self.swing.release = v,
            CLASS_FOLLOW => self.level_cams.release = v,
            _ if self.class_cam.active => self.class_cam.release = v,
            _ => {}
        }
    }

    /// `UpdateAllCameras`' activation loop 0x20d620: every slot's check in slot order (class 17's hooks retune the
    /// follow camera) against the current camera (releasing when its +0x7e is set: the Swingshot camera's and the
    /// level-class cameras' pre hooks, class 1's hook on the follow camera), then the choice. Every slot but the
    /// current camera's own is checked (two records of one class can hand over: class 3's rails). Returns the class
    /// that won when it is not the current camera (its slot in [`LevelCameras::won_slot`]); the caller switches it in,
    /// or records it in [`LevelCameras::wanted`] when the port does not run it.
    pub(super) fn activation_loop(&mut self, inp: &CamInput) -> Option<i32> {
        let cur = self.current_class();
        let cur_slot = self.current_slot();
        // The best so far: None = the current camera (its +0x7e is read live: a hook may set it), else a slot.
        let cur_priority = cur_slot.map_or(self.level_cams.priority_of(cur), |i| self.level_cams.slots[i].header.priority);
        let mut best: Option<(i32, usize)> = None;
        let mut changed = false;
        let mut fp_checked = false;
        let h = inp.hero;
        let feet = to_f32x3(h.pos);
        self.level_cams.won_slot = None;
        for i in 0..self.level_cams.slots.len() {
            let s = &self.level_cams.slots[i];
            let (class, header, rail) = (s.record.class, s.header, s.rail);
            if Some(i) == cur_slot || (cur_slot.is_none() && class == cur) { continue; }
            let best_now = match best {
                None => Best { priority: cur_priority, releasing: self.current_release() != 0 },
                Some((_, b)) => Best { priority: self.level_cams.slots[b].header.priority, releasing: false },
            };
            let hook = match class {
                CLASS_REGION => {
                    if self.level_cams.ports.region { self.region_hook(i, inp); }
                    0
                }
                CLASS_PLACED => {
                    if self.level_cams.ports.placed { self.placed_hook(i, inp); }
                    0
                }
                CLASS_FOCUS => {
                    if self.level_cams.ports.focus { self.focus_hook(i, inp); }
                    0
                }
                CLASS_FIRST_PERSON => {
                    fp_checked = true;
                    if self.follow_is_current() && self.first_person_activation(inp) { 1 } else { 0 }
                }
                super::cuboid::CLASS_FIXED if self.level_cams.ports.fixed => self.fixed_hook(i, Some(best_now), inp),
                super::cuboid::CLASS_SIDE if self.level_cams.ports.side => super::cuboid::side_hook(h),
                super::flyby::CLASS_FLYBY if self.level_cams.ports.flyby => {
                    let best_class = best.map_or(cur, |(c, _)| c);
                    self.flyby_hook(i, best_class, best_now, inp)
                }
                super::race::CLASS_INTRO if self.level_cams.ports.race => self.intro_hook(i, best_now, inp),
                super::race::CLASS_RACE if self.level_cams.ports.race => self.race_hook(i, best_now),
                super::giant::CLASS_GIANT if self.level_cams.ports.giant => self.giant_hook(i, inp),
                // The script and type-6 cameras' hooks answer 0; they are switched in by their calls (+0x7d is not
                // set by the port's `CameraScript`, which switches directly). Class 3's hook `0x315dd8` answers 0.
                _ => 0,
            };
            // The best as the check reads it (after the hook: class 1's may have set the current camera's +0x7e).
            let best_now = match best {
                None => Best { priority: cur_priority, releasing: self.current_release() != 0 },
                Some(_) => best_now,
            };
            let c = Candidate { class, priority: header.priority, activation: header.activation, entered: false, hook };
            let vols = self.level_cams.volumes.clone();
            let won = activation_check(
                c,
                Some(best_now),
                || vols.as_deref().is_some_and(|v| point_in_cuboid(v, feet, header.cuboid)),
                || h.f15d4 == class && (class != 3 || rail < 0 || (h.boots.rail == Some(rail as usize) && h.boots.off_rail == 0)),
            );
            if won {
                best = Some((class, i));
                changed = true;
            }
        }
        let mut best = best.map(|(c, i)| (c, Some(i)));
        if !fp_checked && self.follow_is_current() && self.first_person_activation(inp) {
            // No records (a game without the level's data): the first-person camera's slot is implicit.
            best = Some((CLASS_FIRST_PERSON, None));
            changed = true;
        }
        self.level_cams.wanted = None;
        match best {
            Some((c, i)) if changed && (c != cur || (i.is_some() && i != cur_slot)) => {
                self.level_cams.won_slot = i;
                Some(c)
            }
            _ => None,
        }
    }

    /// `UpdateAllCameras` with the Swingshot camera current: its pre hook `0x318c08`, the choice, the switch back to
    /// the follow camera (`FUN_0020d110`: +0x7e = 3 → the pose copied, the follow record's blend kind 0 → no blend,
    /// the init `0x311dd0`), or its update `0x318b18`. A class the port does not run winning over the released camera
    /// is recorded in [`LevelCameras::wanted`] and the follow camera is switched in instead (G-HERO-027). Returns the
    /// camera a switch or a re-init left (the blend's capture).
    pub(super) fn swing_frame(&mut self, inp: &CamInput) -> Option<([V4; 3], V4)> {
        self.swing_pre(inp);
        let won = self.activation_loop(inp);
        let back = match won {
            Some(CLASS_FOLLOW) => true,
            Some(c) if self.level_cams.ports.runs_class_cam(c) && self.level_cams.won_slot.is_some() => {
                // A level-class camera (classes 3 / 1 / 14) over the released Swingshot camera.
                let prev = self.active_view();
                let slot = self.level_cams.won_slot.unwrap();
                self.class_switch_in(inp, c, slot, prev);
                return Some(prev);
            }
            Some(c) => {
                self.level_cams.wanted = Some(c);
                self.swing.release != 0
            }
            None => false,
        };
        if back {
            let prev = self.active_view();
            let on_ground = inp.hero.f65c == 0;
            self.swing.active = false;
            self.cam.rows = prev.0;
            self.cam.pos = prev.1;
            self.init_from(inp, Some(on_ground));
            self.d0 = self.cam;
            self.g.since_switch = 0;
            return Some(prev);
        }
        let p = self.swing_update(inp);
        self.swing.prev_pos = self.swing.pos;
        p.map(|(r, p)| (super::rows_pf(r), pos4(p)))
    }

    /// `FUN_0020d110` to the Swingshot camera from the follow camera (`prev`: its rows and position): the record's
    /// blend kind (3 on every level: the pose copied; any kind is overridden by the init's orbit blend), 0x167498 = 0,
    /// the init, +0x64 = the position; then this tick's update.
    pub(super) fn swing_switch_in(&mut self, inp: &CamInput, prev: ([V4; 3], V4)) {
        self.swing.active = true;
        self.g.since_switch = 0;
        self.swing_init(inp, (super::rows_f(prev.0), to_f32x3(prev.1)));
        self.swing.prev_pos = self.swing.pos;
        self.swing_update(inp);
        self.swing.prev_pos = self.swing.pos;
    }

    /// The class-17 hook `0x319688` for slot `i` (module doc).
    fn region_hook(&mut self, i: usize, inp: &CamInput) {
        let h = inp.hero;
        let Some(r) = self.level_cams.slots[i].region else { return };
        if r.once == 1 && r.left != 0 { return; }
        if r.mode == 11 && h.mode != 2 { return; }
        if (r.mode == 1 || r.mode == 10) && r.counter == 0 && h.f65c != 0 && !(self.level_cams.level == 0xe && h.f15d4 == 0xd) {
            return;
        }
        if r.mode == 2 && h.group != 0x1a {
            self.region_mut(i).counter = 0;
            return;
        }
        self.region_update(i, inp);
    }

    fn region_mut(&mut self, i: usize) -> &mut RegionTweak { self.level_cams.slots[i].region.as_mut().unwrap() }

    /// Leave (the counter to 0, "left" if it was running; the lock released when `release`).
    fn region_leave(&mut self, i: usize, release: bool) {
        let r = self.region_mut(i);
        if r.counter != 0 { r.left = 1; }
        r.counter = 0;
        if release { self.release_owner(i); }
    }

    /// `0x318de0` (module doc).
    fn region_update(&mut self, i: usize, inp: &CamInput) {
        const DEG: f32 = 0.017_453_292;
        let r = self.level_cams.slots[i].region.unwrap();
        let rot_z = self.level_cams.slots[i].record.rot[2];
        let focus = self.level_cams.focus;
        if r.mode == 8 && focus == 0 { return; }
        if r.mode == 6 && focus != 0 { return; }
        if !self.region_test(i, inp) {
            self.region_leave(i, true);
            return;
        }
        if self.claim_owner(i) < 1 {
            self.region_leave(i, false);
            return;
        }
        if r.facing != 0 && Pf::f(1.396_263_4) <= fast_diff_rots(Pf::f(rot_z), self.out.euler[2]) {
            self.region_leave(i, true);
            return;
        }
        if r.once != 0 && r.mode == 5 && inp.pad.held & 5 != 0 {
            self.region_leave(i, true);
            return;
        }
        if r.counter == 0 {
            let (k, d) = (self.cam.kh.to_f32(), self.cam.dh.to_f32());
            let m = self.region_mut(i);
            m.from_k = k;
            m.from_d = d;
        }
        if r.mode == 5 && r.counter == 0 { self.region_snap(i, r, inp.hero.pos); }
        let hs = self.g.h_speed.to_f32();
        let (rx, ry) = (inp.pad.rx.to_f32(), inp.pad.ry.to_f32());
        let r = self.region_mut(i);
        let steps = if r.mode == 3 { 1.0 } else { 200.0 };
        r.counter += 1;
        let mode = r.mode;
        let mut stopped = false;
        let mut stick_off = false;
        let mut focus = None;
        match mode {
            10 => { stick_off = true; focus = Some(0x14d); }
            11 => { stick_off = true; focus = Some(2); }
            _ => {}
        }
        let mut lock_toward = false;
        match mode {
            2 | 4 => {
                stick_off = true;
                if 200 < r.counter { r.counter = 200; }
                lock_toward = true;
            }
            1 | 7 | 10 | 9 => {
                if rx != 0.0 || ry != 0.0 { r.counter = 200; }
                if 200 <= r.counter {
                    stopped = true;
                    if 0.0 < hs { r.counter = 200; }
                    if 400 <= r.counter {
                        r.counter = 1;
                        stopped = false;
                    }
                }
            }
            3 => { if 1 < r.counter { r.counter = 1; } }
            _ => { if 200 < r.counter { r.counter = 200; } }
        }
        let r = *r;
        if stick_off { self.cam.script |= 3; }
        if let Some(f) = focus { self.level_cams.focus = f; }
        if lock_toward { self.lock_toward(); }
        let slow = mode == 6 || mode == 8;
        if !(stopped && matches!(mode, 1 | 7 | 9)) {
            let rate = (r.turn * DEG) * (r.counter as f32 / steps);
            let dir = [super::fast_cos(Pf::f(rot_z)).to_f32(), super::fast_sin(Pf::f(rot_z)).to_f32(), 0.0];
            self.turn_toward(rate, r.tolerance * DEG, dir);
        }
        if r.distance != 0.0 {
            if slow {
                self.set_distance(r.distance, f32::from_bits(0x3b03_126f), false);
                self.set_pull_max(r.distance + 1.36);
            } else {
                self.set_distance(r.distance, f32::from_bits(0x3b44_9ba6), false);
                self.lock_toward();
            }
        }
        if r.pivot_height != 0.0 {
            self.set_pivot_height(r.pivot_height, f32::from_bits(if slow { 0x3b03_126f } else { 0x3b44_9ba6 }));
        }
        if r.look_height != 0.0 {
            self.set_look_height(r.look_height, f32::from_bits(if slow { 0x3b03_126f } else { 0x3ba3_d70a }), false);
        }
        if r.pitch != 0.0 { self.set_script_pitch(r.pitch * DEG); }
        if r.no_pitch != 0 { self.cam.script |= 2; }
        if r.no_yaw != 0 { self.cam.script |= 1; }
        if r.leash == 0 { self.set_leash(0); }
        if r.spring_k != 0.0 || r.spring_d != 0.0 {
            if r.counter < 120 {
                let t = r.counter as f32 / 120.0;
                self.set_h_spring(r.from_k + (r.spring_k - r.from_k) * t, r.from_d + (r.spring_d - r.from_d) * t);
            } else {
                self.set_h_spring(r.spring_k, r.spring_d);
            }
        }
        if matches!(mode, 1 | 7 | 10) {
            self.lock_toward();
            self.look_from_smoothed();
            self.set_v_spring(f32::from_bits(0x3c23_d70a), f32::from_bits(0x3e4c_cccd));
        }
    }

    /// Mode 5's first tick (`0x318de0`): the turn zeroed in the block; the follow camera placed at the region's
    /// distance along its offset, pivot and look heights, and its springs' state set to it.
    fn region_snap(&mut self, i: usize, r: RegionTweak, hero_pos: crate::hero::physics::V4) {
        use crate::hero::physics::{set_len3, vadd};
        self.region_mut(i).turn = 0.0;
        let d = &mut self.cam;
        d.off = set_len3(d.off, Pf::f(r.distance));
        d.dist = Pf::f(r.distance);
        d.pivot_h = Pf::f(r.pivot_height);
        d.look_h = Pf::f(r.look_height);
        d.placed = d.off;
        d.pos = vadd(d.pivot, d.off);
        d.look_s = d.look;
        d.target = hero_pos;
        d.smooth = d.target;
        d.desired = d.pos;
        d.end_cand = d.pos;
        d.bias = Pf::ZERO;
        d.look_h_s = d.look_h;
        d.look_h_v = Pf::ZERO;
        d.pivot_h_s = d.pivot_h;
        d.bias_v = Pf::ZERO;
        d.pivot_h_v = Pf::ZERO;
    }

    /// The region test `0x318c40` for slot `i`.
    fn region_test(&self, i: usize, inp: &CamInput) -> bool {
        if !self.follow_is_current() { return false; }
        let h = inp.hero;
        let r = self.level_cams.slots[i].region.unwrap();
        let hd = r.header;
        let forced = r.counter > 0 && ((r.mode == 2 && h.group == 0x1a) || (r.mode == 4 && h.f15d4 == 3));
        let Some(v) = self.level_cams.shapes() else { return false };
        let p = to_f32x3(h.pos);
        type Test = fn(&Volumes, [f32; 3], i32) -> bool;
        let tests: [(i32, Test); 5] = [
            (hd.cuboid, point_in_cuboid),
            (r.cuboid2, point_in_cuboid),
            (hd.cylinder, point_in_cylinder),
            (hd.sphere, point_in_sphere),
            (hd.path, point_in_path),
        ];
        tests.iter().any(|&(idx, f)| 0 <= idx && (forced || f(v, p, idx)))
    }

    /// The class-23 hook `0x2ed8b0` (level 00) for slot `i`: nothing once done (+0x26); else the update `0x2ed498`
    /// (the game mode 0x15f5c4 ≠ 0 branch, which clears +0x20 / +0x26 while inside, is not reached: the port's camera
    /// runs in game mode 0 only). Always answers 0.
    fn placed_hook(&mut self, i: usize, inp: &CamInput) {
        let Some(v) = self.level_cams.slots[i].placed else { return };
        if v.done != 0 { return; }
        self.placed_update(i, inp);
    }

    fn placed_mut(&mut self, i: usize) -> &mut PlacedView { self.level_cams.slots[i].placed.as_mut().unwrap() }

    /// Leave (`0x2ed498`'s tail): done when it was running, the counter 0; the lock released when `release`.
    fn placed_leave(&mut self, i: usize, release: bool) {
        let v = self.placed_mut(i);
        if v.counter != 0 { v.done = 1; }
        v.counter = 0;
        if release { self.release_owner(i); }
    }

    /// The region test `0x2ed348`: false unless the follow camera is current; not as Clank with +0x44, only as Clank
    /// with +0x46; with +0x34 only while the camera is still (|D+0x164|, |D+0x1b0| ≤ 0.01); Ratchet's feet in the
    /// cuboid +0x0c, the cylinder +0x10 or the sphere +0x08; outside all of them it is done (+0x26 = 1).
    fn placed_test(&mut self, i: usize, inp: &CamInput) -> bool {
        if !self.follow_is_current() { return false; }
        let h = inp.hero;
        let v = self.level_cams.slots[i].placed.unwrap();
        if v.clank_only != 0 && h.mode != 1 { return false; }
        if v.not_clank != 0 && h.mode == 1 { return false; }
        if v.still != 0 && (0.01 < self.cam.yaw_step.to_f32().abs() || 0.01 < self.cam.pitch_n.to_f32().abs()) { return false; }
        let p = to_f32x3(h.pos);
        let hd = v.header;
        let inside = self.level_cams.shapes().is_some_and(|vol| {
            (0 <= hd.cuboid && point_in_cuboid(vol, p, hd.cuboid))
                || (0 <= hd.cylinder && point_in_cylinder(vol, p, hd.cylinder))
                || (0 <= hd.sphere && point_in_sphere(vol, p, hd.sphere))
        });
        if !inside { self.placed_mut(i).done = 1; }
        inside
    }

    /// The update `0x2ed498` (module doc of [`PlacedView`]).
    fn placed_update(&mut self, i: usize, inp: &CamInput) {
        const DEG: f32 = 0.017_453_292;
        if !self.placed_test(i, inp) {
            self.placed_leave(i, true);
            return;
        }
        if self.claim_owner(i) < 1 {
            self.placed_leave(i, false);
            return;
        }
        if inp.pad.held & 5 != 0 {
            self.placed_leave(i, true);
            return;
        }
        let h = inp.hero;
        let hero = to_f32x3(h.pos);
        let grav = to_f32x3(h.gravity_dir);
        if self.level_cams.slots[i].placed.unwrap().counter < 2 {
            // The camera placed at the record's position: distance and height from it, the look height from the angle.
            let rp = self.level_cams.slots[i].record.pos;
            let d = [rp[0] - hero[0], rp[1] - hero[1], rp[2] - hero[2]];
            let dz = d[0] * grav[0] + d[1] * grav[1] + d[2] * grav[2];
            let flat = [d[0] - grav[0] * dz, d[1] - grav[1] * dz, d[2] - grav[2] * dz];
            let v = self.placed_mut(i);
            v.distance = (flat[0] * flat[0] + flat[1] * flat[1] + flat[2] * flat[2]).sqrt();
            v.height = -dz;
            v.look_height = if v.look_angle.abs() == std::f32::consts::FRAC_PI_2 {
                0.0
            } else {
                let (s, c) = v.look_angle.sin_cos();
                v.height - v.distance * (s / c)
            };
            let v = *v;
            self.placed_snap(rp, v, h);
        }
        let v = self.placed_mut(i);
        v.counter += 1;
        if 200 < v.counter { v.counter = 200; }
        let v = *v;
        if v.distance != 0.0 {
            self.set_distance(v.distance, f32::from_bits(0x3b44_9ba6), false);
            self.lock_toward();
        }
        if v.height != 0.0 { self.set_pivot_height(v.height, f32::from_bits(0x3b44_9ba6)); }
        if v.look_height != 0.0 { self.set_look_height(v.look_height, f32::from_bits(0x3ba3_d70a), false); }
        if v.pitch != 0.0 { self.set_script_pitch(v.pitch * DEG); }
        if v.leash == 0 { self.set_leash(0); }
    }

    /// `0x2ed498`'s first two ticks: the follow camera at `at` with the region's distance and heights, pivot and look
    /// above Ratchet along −gravity, the offsets from there, the springs' state reset to them.
    fn placed_snap(&mut self, at: [f32; 3], v: PlacedView, h: &crate::hero::Hero) {
        use crate::hero::physics::{vadd, vscale, vsub};
        let d = &mut self.cam;
        let w = d.pos[3];
        d.pos = crate::hero::physics::from_f32x3(at);
        d.pos[3] = w;
        d.dist = Pf::f(v.distance);
        d.pivot_h = Pf::f(v.height);
        d.look_h = Pf::f(v.look_height);
        d.pivot = vadd(h.pos, vscale(h.gravity_dir, -d.pivot_h));
        d.off = vsub(d.pos, d.pivot);
        d.placed = d.off;
        d.look = vadd(h.pos, vscale(h.gravity_dir, -d.look_h));
        d.look_s = d.look;
        d.target = h.pos;
        d.smooth = d.target;
        d.desired = d.pos;
        d.end_cand = d.pos;
        d.red = Pf::ZERO;
        d.bias = Pf::ZERO;
        d.look_h_s = d.look_h;
        d.look_h_v = Pf::ZERO;
        d.pivot_h_s = d.pivot_h;
        d.pitch_neg = 0;
        d.bias_v = Pf::ZERO;
        d.pivot_h_v = Pf::ZERO;
    }

    /// `0x313598(slot)`: take the follow camera's settings: 1 when free, already this slot's, or held by a lower
    /// priority (pvar +0x1c); 0 when held by an equal or higher one; −1 when the follow camera is not current.
    pub fn claim_owner(&mut self, slot: usize) -> i32 {
        if !self.follow_is_current() { return -1; }
        let lc = &mut self.level_cams;
        match lc.owner {
            None => {
                lc.owner = Some(slot);
                1
            }
            Some(o) if o == slot => 1,
            Some(o) if lc.slots[o].header.priority < lc.slots[slot].header.priority => {
                lc.owner = Some(slot);
                1
            }
            Some(_) => 0,
        }
    }

    /// `0x313560(slot)`: give the settings back if `slot` holds them (1), else 0; −1 when the follow camera is not
    /// current.
    pub fn release_owner(&mut self, slot: usize) -> i32 {
        if !self.follow_is_current() { return -1; }
        if self.level_cams.owner == Some(slot) {
            self.level_cams.owner = None;
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(activation: u8, priority: u8, hook: i32) -> Candidate { Candidate { class: 7, priority, activation, entered: false, hook } }

    #[test]
    fn activation_check_kinds_and_priorities() {
        let follow = Some(Best { priority: 5, releasing: false });
        let releasing = Some(Best { priority: 6, releasing: true });
        // Priority 0 never, even with a yes from the hook.
        assert!(!activation_check(cand(0, 0, 1), follow, || true, || true));
        // The hook decides first.
        assert!(activation_check(cand(3, 4, 1), follow, || false, || false));
        assert!(!activation_check(cand(0, 9, -1), follow, || true, || true));
        // Kind 0: a higher priority, or a releasing best.
        assert!(!activation_check(cand(0, 5, 0), follow, || true, || true));
        assert!(activation_check(cand(0, 5, 0), releasing, || true, || true));
        assert!(activation_check(cand(0, 6, 0), follow, || true, || true));
        // Kind 7: the hero's camera mode, then the priority.
        assert!(activation_check(cand(7, 6, 0), follow, || false, || true));
        assert!(!activation_check(cand(7, 6, 0), follow, || true, || false));
        assert!(!activation_check(cand(7, 4, 0), follow, || true, || true));
        // Kind 4: the priority before the cuboid (level 00's class-17 record 1: priority 4, never over the follow camera).
        assert!(!activation_check(cand(4, 4, 0), follow, || panic!("not tested"), || true));
        assert!(activation_check(cand(4, 6, 0), follow, || true, || false));
        // Kinds 1 / 2 need +0x7d; 3, 5, 6 never.
        assert!(!activation_check(cand(2, 6, 0), follow, || true, || true));
        assert!(activation_check(Candidate { entered: true, ..cand(2, 6, 0) }, follow, || true, || true));
        for k in [3, 5, 6, 8] { assert!(!activation_check(cand(k, 9, 0), None, || true, || true)); }
        // No best yet: the candidate's own answer.
        assert!(activation_check(cand(0, 1, 0), None, || true, || true));
    }

    // ---- class 17 on a synthetic level: one floor, cuboids around Ratchet ----

    use crate::follow_camera::{CamInput, Camera, CameraOptions};
    use crate::pad::{PadInput, PadState};
    use rc_formats::cameras::{CameraRecord, LevelCamera};
    use rc_formats::volumes::Shape;

    /// A cuboid of half-size `h` at `c` (axis-aligned).
    fn cuboid(c: [f32; 3], h: f32) -> Shape {
        Shape {
            matrix: [[h, 0.0, 0.0, 0.0], [0.0, h, 0.0, 0.0], [0.0, 0.0, h, 0.0], [c[0], c[1], c[2], 1.0]],
            inverse: [[1.0 / h, 0.0, 0.0, 0.0], [0.0, 1.0 / h, 0.0, 0.0], [0.0, 0.0, 1.0 / h, 0.0]],
            ..Default::default()
        }
    }

    /// A class-17 record: cuboid `cub`, priority, mode, turn, distance, pivot, look, pitch; the rest 0 (leash on).
    fn region(cub: i32, prio: u8, mode: i16, f: impl Fn(&mut [u8])) -> LevelCamera {
        let mut p = vec![0u8; 0x60];
        for o in [8, 0x10, 0x14, 0x4c] { p[o..o + 4].copy_from_slice(&(-1i32).to_le_bytes()); }
        p[0xc..0x10].copy_from_slice(&cub.to_le_bytes());
        p[0x18..0x1c].copy_from_slice(&1.5f32.to_le_bytes());
        p[0x1c..0x20].copy_from_slice(&[prio, 3, 3, 3]);
        p[0x2c..0x2e].copy_from_slice(&mode.to_le_bytes());
        p[0x34..0x36].copy_from_slice(&1i16.to_le_bytes());
        f(&mut p);
        LevelCamera { record: CameraRecord { class: 17, pos: [0.0; 3], rot: [0.0; 3], pvar_index: 0 }, pvar: Some(p) }
    }
    fn put(p: &mut [u8], o: usize, v: f32) { p[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
    fn put16(p: &mut [u8], o: usize, v: i16) { p[o..o + 2].copy_from_slice(&v.to_le_bytes()); }

    fn system() -> Vec<LevelCamera> {
        let rec = |class, prio, act| {
            let mut p = vec![0u8; 0x40];
            p[0x1c..0x20].copy_from_slice(&[prio, 0, 3, act]);
            LevelCamera { record: CameraRecord { class, pos: [0.0; 3], rot: [0.0; 3], pvar_index: 1 }, pvar: Some(p) }
        };
        vec![rec(0, 5, 0), rec(4, 6, 3), rec(5, 6, 2), rec(6, 6, 7), rec(7, 6, 7)]
    }

    struct Rig {
        coll: rc_formats::collision::Collision,
        hero: crate::hero::Hero,
        pad: PadState,
        cam: Camera,
    }

    fn rig(regions: Vec<LevelCamera>, shapes: Vec<Shape>) -> Rig {
        let coll = crate::hero::testkit::floor(100.0, 98, 108, 98, 108);
        let hero = crate::hero::Hero::spawn([410.0, 410.0, 100.0], 0.0);
        let pad = PadState::default();
        let mut cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }, CameraOptions::default());
        let vols = Volumes { cuboids: shapes, ..Default::default() };
        let mut cams = regions;
        cams.extend(system());
        cam.set_level(LevelCameras::new(1, &cams, Some(Arc::new(vols)), CameraPorts::default()));
        Rig { coll, hero, pad, cam }
    }

    impl Rig {
        /// The activation loop alone (the region's writes before the follow camera's update consumes them).
        fn hooks(&mut self) -> Option<i32> {
            let inp = CamInput { hero: &self.hero, pad: &self.pad, coll: &self.coll, mobys: None, hero_moby: None };
            self.cam.activation_loop(&inp)
        }
        fn tick(&mut self) {
            let inp = CamInput { hero: &self.hero, pad: &self.pad, coll: &self.coll, mobys: None, hero_moby: None };
            self.cam.update(&inp);
        }
        fn r(&self, i: usize) -> RegionTweak { self.cam.level_cams.slots[i].region.unwrap() }
    }

    const HERE: [f32; 3] = [410.0, 410.0, 100.0];

    /// A mode-0 region with every setting: its first tick captures the spring, takes the lock and writes the
    /// distance, heights, pitch, stick bits, leash and the eased spring; the follow camera's update then springs
    /// toward them and its spring-back clears the one-tick writes.
    #[test]
    fn region_writes_the_follow_camera() {
        let reg = region(0, 4, 0, |p| {
            put(p, 0x24, 6.5);
            put(p, 0x28, 3.0);
            put(p, 0x30, 2.5);
            put(p, 0x50, 23.0);
            put16(p, 0x54, 1);
            put16(p, 0x56, 1);
            put16(p, 0x34, 0);
            put(p, 0x38, 0.04);
            put(p, 0x3c, 0.3);
        });
        let mut r = rig(vec![reg], vec![cuboid(HERE, 5.0)]);
        assert_eq!(r.hooks(), None);
        let d = r.cam.cam;
        assert_eq!(r.cam.level_cams.owner, Some(0));
        assert_eq!(r.r(0).counter, 1);
        assert_eq!((r.r(0).from_k, r.r(0).from_d), (0.015, 0.2), "the spring captured from the follow camera");
        assert_eq!((d.dist_ovr, d.dist_tgt.to_f32(), d.dist_rate.to_f32().to_bits()), (1, 6.5, 0x3b44_9ba6));
        assert_eq!(d.toward_lock, 1, "the distance's run-toward lock");
        assert_eq!((d.ph_ovr, d.ph_tgt.to_f32(), d.ph_rate.to_f32().to_bits()), (1, 3.0, 0x3b44_9ba6));
        assert_eq!((d.look_h_ovr, d.look_h_tgt.to_f32(), d.look_h_rate.to_f32().to_bits()), (1, 2.5, 0x3ba3_d70a));
        assert_eq!(d.script_pitch.to_f32(), 23.0 * 0.017_453_292);
        assert_eq!(d.script & 3, 3);
        assert_eq!(d.leash, 0);
        let t = 1.0 / 120.0;
        assert_eq!(d.kh.to_f32(), 0.015 + (0.04 - 0.015) * t);
        assert_eq!(d.dh.to_f32(), 0.2 + (0.3 - 0.2) * t);
        // The update consumes them; the spring-back restores the defaults.
        r.tick();
        let d = r.cam.cam;
        assert_eq!((d.dist_ovr, d.ph_ovr, d.look_h_ovr, d.script, d.leash, d.toward_lock), (0, 0, 0, 0, 1, 0));
        assert!(d.dist > r.cam.d0.dist, "springing out toward 6.5");
        for _ in 0..300 { r.tick(); }
        assert!((r.cam.cam.dist.to_f32() - 6.5).abs() < 0.05, "distance {}", r.cam.cam.dist.to_f32());
        assert_eq!(r.r(0).counter, 200, "mode 0 holds the counter at 200");
        // 120 ticks in, the spring is the region's.
        r.hooks();
        assert_eq!((r.cam.cam.kh.to_f32(), r.cam.cam.dh.to_f32()), (0.04, 0.3));
    }

    /// Leaving: the counter to 0, "left", the lock released; a "once" region then stays off.
    #[test]
    fn leave_and_once() {
        let reg = region(0, 4, 0, |p| { put(p, 0x24, 6.5); put16(p, 0x48, 1); });
        let mut r = rig(vec![reg], vec![cuboid(HERE, 5.0)]);
        for _ in 0..3 { r.tick(); }
        assert_eq!((r.r(0).counter, r.cam.level_cams.owner), (3, Some(0)));
        r.hero.pos[0] = crate::ps2v::Pf::f(420.0);
        r.hooks();
        assert_eq!((r.r(0).counter, r.r(0).left, r.cam.level_cams.owner), (0, 1, None));
        r.hero.pos[0] = crate::ps2v::Pf::f(410.0);
        r.hooks();
        assert_eq!((r.r(0).counter, r.cam.cam.dist_ovr), (0, 0), "once: not again");
    }

    /// The lock: a higher priority takes it from a lower one; an equal one does not (it leaves without releasing).
    #[test]
    fn owner_lock_priorities() {
        let a = region(0, 4, 0, |p| put(p, 0x24, 6.0));
        let b = region(0, 5, 0, |p| put(p, 0x24, 8.0));
        let c = region(0, 5, 0, |p| put(p, 0x24, 9.0));
        let mut r = rig(vec![a, b, c], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        assert_eq!(r.cam.level_cams.owner, Some(1));
        // Slot 0 claimed first (its writes), slot 1 took it over (its writes after), slot 2 (equal) was refused.
        assert_eq!(r.cam.cam.dist_tgt.to_f32(), 8.0);
        assert_eq!([r.r(0).counter, r.r(1).counter, r.r(2).counter], [1, 1, 0]);
        r.cam.cam.dist_ovr = 0;
        r.hooks();
        assert_eq!([r.r(0).counter, r.r(1).counter, r.r(2).counter], [0, 2, 0], "the lower one is now refused");
        assert_eq!(r.r(0).left, 1);
    }

    /// Modes 1 / 7 / 9 / 10: the turn eases in over 200 ticks; the stick moving stops it (counter 200); held while
    /// Ratchet moves, it restarts from 1 at 400 idle ticks. Mode 3 holds the counter at 1 (the full turn at once).
    #[test]
    fn turn_counters() {
        let reg = region(0, 4, 1, |p| put(p, 0, 8.0));
        let mut r = rig(vec![reg], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        let rate = r.cam.cam.yaw_rate.to_f32();
        assert_eq!(rate, (8.0 * 0.017_453_292) * (1.0 / 200.0));
        assert_eq!((r.cam.cam.toward_lock, r.cam.cam.look_from_s, r.cam.cam.kv.to_f32(), r.cam.cam.dv.to_f32()), (1, 1, 0.01, 0.2));
        r.pad.update(Some(&PadInput::neutral().rstick(1.0, 0.0).bytes()), false);
        r.cam.cam.yaw_rate = crate::ps2v::Pf::ZERO;
        r.hooks();
        assert_eq!(r.r(0).counter, 200);
        assert_eq!(r.cam.cam.yaw_rate.to_f32(), 0.0, "stopped: no turn");
        r.pad.update(Some(&PadInput::neutral().bytes()), false);
        for _ in 0..199 { r.hooks(); }
        assert_eq!(r.r(0).counter, 399);
        r.hooks();
        assert_eq!(r.r(0).counter, 1, "restarted");
        let reg = region(0, 4, 3, |p| put(p, 0, 3.0));
        let mut r = rig(vec![reg], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        r.hooks();
        assert_eq!(r.r(0).counter, 1);
        assert_eq!(r.cam.cam.yaw_rate.to_f32(), 3.0 * 0.017_453_292);
    }

    /// Mode 2 (Kerwan's cables): only in group 0x1a (outside it the counter drops to 0 and the lock stays); inside,
    /// the stick is off and the lock toward set; once running, the region counts as entered without the cuboid.
    #[test]
    fn mode2_cable_group() {
        let reg = region(0, 4, 2, |p| put(p, 0x24, 6.5));
        let mut r = rig(vec![reg], vec![cuboid(HERE, 1.0)]);
        r.hooks();
        assert_eq!(r.r(0).counter, 0, "not on the cable");
        r.hero.group = 0x1a;
        r.hooks();
        assert_eq!((r.r(0).counter, r.cam.cam.script & 3, r.cam.cam.toward_lock), (1, 3, 1));
        r.hero.pos[0] = crate::ps2v::Pf::f(430.0);
        r.hooks();
        assert_eq!(r.r(0).counter, 2, "running: inside without the test");
        r.hero.group = 0;
        r.hooks();
        assert_eq!((r.r(0).counter, r.cam.level_cams.owner), (0, Some(0)), "off the cable: counter 0, the lock kept");
    }

    /// The facing check (+0x36): 80° or more between the camera yaw and the region's facing leaves.
    #[test]
    fn facing_check() {
        let mut reg = region(0, 4, 0, |p| { put(p, 0x24, 6.5); put16(p, 0x36, 1); });
        let yaw = r0_yaw();
        reg.record.rot[2] = yaw;
        let mut r = rig(vec![reg.clone()], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        assert_eq!(r.r(0).counter, 1);
        reg.record.rot[2] = yaw + 1.5;
        let mut r = rig(vec![reg], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        assert_eq!((r.r(0).counter, r.cam.cam.dist_ovr), (0, 0));
    }
    fn r0_yaw() -> f32 {
        let r = rig(vec![], vec![]);
        r.cam.out.euler[2].to_f32()
    }

    /// Another camera current: the regions leave and write nothing (the setters' guard).
    #[test]
    fn not_while_another_camera_is_current() {
        let reg = region(0, 4, 0, |p| put(p, 0x24, 6.5));
        let mut r = rig(vec![reg], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        assert_eq!(r.r(0).counter, 1);
        r.cam.script.active = true;
        r.cam.cam.dist_ovr = 0;
        r.hooks();
        assert_eq!((r.r(0).counter, r.r(0).left, r.cam.cam.dist_ovr), (0, 1, 0));
        assert_eq!(r.cam.level_cams.owner, Some(0), "the release answers −1 with another camera current");
    }

    /// Kind 7: the hero's camera mode picks the class-7 camera (priority 6 over the follow camera's 5), switched in
    /// when the level runs class 7's code; without it the choice
    /// is recorded as wanted and the follow camera stays.
    #[test]
    fn unported_class_is_wanted() {
        let mut r = rig(vec![], vec![]);
        r.hero.f15d4 = 7;
        assert_eq!(r.hooks(), Some(7));
        r.cam.level_cams.ports.swing = false;
        r.tick();
        assert_eq!(r.cam.level_cams.wanted, Some(7));
        assert!(r.cam.follow_is_current());
        r.cam.level_cams.ports.swing = true;
        r.tick();
        assert!(r.cam.swing.active, "switched in");
        // (Class 3, the rail camera, was the second example here; it runs since batch 5: `rail.rs` and its tests.)
    }

    /// A class-23 record at `at`: cuboid `cub`, priority 4, look angle `angle`, the rest 0 (leash off).
    fn placed(cub: i32, at: [f32; 3], angle: f32, f: impl Fn(&mut [u8])) -> LevelCamera {
        let mut p = vec![0u8; 0x60];
        for o in [8, 0x10, 0x14] { p[o..o + 4].copy_from_slice(&(-1i32).to_le_bytes()); }
        p[0xc..0x10].copy_from_slice(&cub.to_le_bytes());
        p[0x1c..0x20].copy_from_slice(&[4, 3, 3, 3]);
        put(&mut p, 0x2c, angle);
        f(&mut p);
        LevelCamera { record: CameraRecord { class: 23, pos: at, rot: [0.0; 3], pvar_index: 0 }, pvar: Some(p) }
    }

    /// Class 23: inside, the follow camera is placed at the record's position on the first two ticks (distance and
    /// height from it, look height = height − distance·tan(angle)), the lock taken, the setters written each tick;
    /// the counter holds at 200. Outside once: done for good.
    #[test]
    fn placed_view_places_the_camera() {
        let at = [404.0, 410.0, 103.0];
        let mut r = rig(vec![placed(0, at, -0.25, |_| {})], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        let v = r.cam.level_cams.slots[0].placed.unwrap();
        assert_eq!((v.counter, v.done, r.cam.level_cams.owner), (1, 0, Some(0)));
        assert_eq!((v.distance, v.height), (6.0, 3.0));
        assert!((v.look_height - (3.0 + 6.0 * 0.25f32.tan())).abs() < 1e-5);
        let d = r.cam.cam;
        assert_eq!(to_f32x3(d.pos), at, "placed at the record");
        assert_eq!((d.dist.to_f32(), d.pivot_h.to_f32()), (6.0, 3.0));
        assert_eq!(to_f32x3(d.pivot), [410.0, 410.0, 103.0], "pivot above Ratchet along −gravity");
        assert_eq!((d.dist_ovr, d.dist_tgt.to_f32(), d.toward_lock, d.ph_ovr, d.look_h_ovr, d.leash), (1, 6.0, 1, 1, 1, 0));
        for _ in 0..250 { r.tick(); }
        assert_eq!(r.cam.level_cams.slots[0].placed.unwrap().counter, 200);
        assert!((r.cam.cam.dist.to_f32() - 6.0).abs() < 0.05);
        r.hero.pos[0] = crate::ps2v::Pf::f(420.0);
        r.hooks();
        let v = r.cam.level_cams.slots[0].placed.unwrap();
        assert_eq!((v.counter, v.done, r.cam.level_cams.owner), (0, 1, None));
        r.hero.pos[0] = crate::ps2v::Pf::f(410.0);
        r.cam.cam.dist_ovr = 0;
        r.hooks();
        assert_eq!((r.cam.level_cams.slots[0].placed.unwrap().counter, r.cam.cam.dist_ovr), (0, 0), "done: never again");
        // Starting outside: done at once.
        let mut r = rig(vec![placed(0, at, 0.0, |_| {})], vec![cuboid([430.0, 410.0, 100.0], 2.0)]);
        r.hooks();
        assert_eq!(r.cam.level_cams.slots[0].placed.unwrap().done, 1);
    }

    /// Class 23's conditions: L1 / L2 leaves (done, lock released); +0x44 not as Clank, +0x46 only as Clank; +0x34
    /// only while the camera's yaw step and pitch input are still; the leash kept with +0x24.
    #[test]
    fn placed_view_conditions() {
        let at = [404.0, 410.0, 103.0];
        let mut r = rig(vec![placed(0, at, 0.0, |p| put16(p, 0x24, 1))], vec![cuboid(HERE, 5.0)]);
        r.hooks();
        assert_eq!(r.cam.cam.leash, 1, "+0x24: the leash kept");
        r.pad.update(Some(&PadInput::neutral().press(crate::pad::button::L1).bytes()), false);
        r.hooks();
        let v = r.cam.level_cams.slots[0].placed.unwrap();
        assert_eq!((v.counter, v.done, r.cam.level_cams.owner), (0, 1, None));
        for (flag, body, runs) in [(0x44usize, 1u8, false), (0x44, 0, true), (0x46, 0, false), (0x46, 1, true)] {
            let mut r = rig(vec![placed(0, at, 0.0, |p| put16(p, flag, 1))], vec![cuboid(HERE, 5.0)]);
            r.hero.mode = body;
            r.hooks();
            assert_eq!(r.cam.level_cams.slots[0].placed.unwrap().counter == 1, runs, "+{flag:#x} body {body}");
            assert_eq!(r.cam.level_cams.slots[0].placed.unwrap().done, 0, "a refused body is not 'outside'");
        }
        let mut r = rig(vec![placed(0, at, 0.0, |p| p[0x34] = 1)], vec![cuboid(HERE, 5.0)]);
        r.cam.cam.yaw_step = crate::ps2v::Pf::f(0.02);
        r.hooks();
        assert_eq!(r.cam.level_cams.slots[0].placed.unwrap().counter, 0, "+0x34: the camera turning");
        r.cam.cam.yaw_step = crate::ps2v::Pf::ZERO;
        r.hooks();
        assert_eq!(r.cam.level_cams.slots[0].placed.unwrap().counter, 1);
    }

    /// The avoidance's level branch (`0x312ef8`): on level 13 (0xd) in state 0x7b (sinking, no health) the camera
    /// line is not tested (the blocked counter stays 0), elsewhere a wall across the line counts.
    #[test]
    fn avoidance_level13_sinking_skips_the_line() {
        use crate::hero::testkit::{cell, mesh};
        let blocked_after = |level: u32, state: i32| {
            let open = crate::hero::testkit::floor(100.0, 98, 108, 98, 108);
            let mut cells = open.cells.clone();
            // A wall at x = 407 (between Ratchet's pivot and the camera behind him along −x).
            for cy in 100..104 {
                let (x, y) = (407.0, cy as f32 * 4.0);
                cells.push(cell([101, cy, 25], &[[x, y, 100.0], [x, y + 4.0, 100.0], [x, y + 4.0, 104.0], [x, y, 104.0]], &[([0, 1, 2, 3], 0x21)]));
            }
            let coll = mesh(cells);
            let mut hero = crate::hero::Hero::spawn(HERE, 0.0);
            let pad = PadState::default();
            // Placed behind him on the open floor; the wall appears for the update.
            let mut cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &open, mobys: None, hero_moby: None }, CameraOptions::default());
            cam.level_cams.level = level;
            hero.state = state;
            cam.update(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None });
            cam.cam.blocked
        };
        assert_eq!(blocked_after(1, 0x7b), 1, "the wall blocks the line");
        assert_eq!(blocked_after(0xd, 0), 1);
        assert_eq!(blocked_after(0xd, 0x7b), 0, "level 13, sinking: no line test");
    }
}
