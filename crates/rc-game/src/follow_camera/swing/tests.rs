//! The Swingshot camera (class 7) on a synthetic floor: the switch in and out, the init's aim, the placement, the
//! level-14 / 7 / 9 group look, the wall stop, the collision push and the orbit blend.

use super::*;
use crate::follow_camera::level::{CameraPorts, LevelCameras};
use crate::follow_camera::{CamInput, Camera, CameraOptions};
use crate::hero::testkit::{cell, floor, mesh};
use crate::pad::PadState;
use rc_formats::cameras::{CameraRecord, LevelCamera};
use rc_formats::collision::Collision;

const HERE: [f32; 3] = [410.0, 410.0, 100.0];

/// The system records every level has (follow 5 / kind 0, first person, script, type 6, Swingshot 6 / kind 7 / blend 3).
fn system() -> Vec<LevelCamera> {
    let rec = |class, prio, blend, act| {
        let mut p = vec![0u8; 0x40];
        for o in [8, 0xc, 0x10, 0x14] { p[o..o + 4].copy_from_slice(&(-1i32).to_le_bytes()); }
        p[0x1c..0x20].copy_from_slice(&[prio, blend, 3, act]);
        LevelCamera { record: CameraRecord { class, pos: [0.0; 3], rot: [0.0; 3], pvar_index: 1 }, pvar: Some(p) }
    };
    vec![rec(0, 5, 0, 0), rec(4, 6, 3, 3), rec(5, 6, 0, 2), rec(6, 6, 1, 7), rec(7, 6, 3, 7)]
}

struct Rig {
    coll: Collision,
    hero: crate::hero::Hero,
    pad: PadState,
    cam: Camera,
}

fn rig_on(level: u32, coll: Collision) -> Rig {
    let hero = crate::hero::Hero::spawn(HERE, 0.0);
    let pad = PadState::default();
    let mut cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }, CameraOptions::default());
    cam.set_level(LevelCameras::new(level, &system(), None, CameraPorts::default()));
    Rig { coll, hero, pad, cam }
}

fn rig(level: u32) -> Rig { rig_on(level, floor(100.0, 98, 108, 98, 108)) }

impl Rig {
    fn inp(&self) -> CamInput<'_> { CamInput { hero: &self.hero, pad: &self.pad, coll: &self.coll, mobys: None, hero_moby: None } }
    fn tick(&mut self) {
        let inp = CamInput { hero: &self.hero, pad: &self.pad, coll: &self.coll, mobys: None, hero_moby: None };
        self.cam.update(&inp);
    }
    /// Ratchet swinging (0x2c, camera mode 7) from target 5 at `on`, rope length `len`, yaw to it `yaw`.
    fn swing(&mut self, on: [f32; 3], len: f32, yaw: f32) {
        let h = &mut self.hero;
        h.state = 0x2c;
        h.f15d4 = 7;
        h.swing.on = Some(5);
        h.swing.on_pos = on;
        h.swing.rope_target = len;
        h.swing.yaw = yaw;
    }
}

fn len(v: [f32; 3]) -> f32 { fdot(v, v).sqrt() }

/// Camera mode 7 picks the class-7 record (priority 6 over the follow camera's 5); the switch copies the follow
/// camera's pose, aims along the swing yaw and keeps the target; the init starts the orbit blend (kind 2, 60 + 1
/// ticks). Mode 7 gone (not falling): the pre hook releases it (+0x7e = 3), the follow camera comes back with the
/// Swingshot camera's pose (no blend, no snap) and its row blend 0 on the ground, 90 in the air.
#[test]
fn switch_in_and_release() {
    let mut r = rig(1);
    let follow = r.cam.cam;
    r.swing([414.0, 410.0, 106.0], 8.0, 0.0);
    r.tick();
    let s = &r.cam.swing;
    assert!(s.active && !r.cam.follow_is_current());
    assert_eq!(r.cam.current_class(), CLASS_SWING);
    assert_eq!((s.target, s.yaw_vel == 0.0, s.release), (Some(5), true, 0));
    assert_eq!(s.dir, [1.0, 0.0, 0.0], "the init's aim along 0x13fcfc (the spring moved the yaw only a little)");
    assert_eq!((r.cam.blend.running, r.cam.blend.orbit_len, r.cam.blend.orbit.left), (2, 61, 60), "orbit blend, one step taken");
    assert_eq!(r.cam.level_cams.wanted, None);
    let _ = follow;
    // Falling (state 6) keeps it even out of camera mode 7.
    r.hero.f15d4 = 0;
    r.hero.state = 6;
    r.tick();
    assert!(r.cam.swing.active && r.cam.swing.release == 0);
    // Out of mode 7 on the ground: released, the follow camera from its pose.
    r.hero.state = 0;
    let (rows, pos) = r.cam.swing_view();
    let inp = CamInput { hero: &r.hero, pad: &r.pad, coll: &r.coll, mobys: None, hero_moby: None };
    r.cam.swing_pre(&inp);
    assert_eq!(r.cam.swing.release, 3);
    r.cam.swing.release = 0;
    r.tick();
    assert!(!r.cam.swing.active && r.cam.follow_is_current());
    assert_eq!(r.cam.cam.saved_fwd, crate::follow_camera::rows_pf(rows)[0], "the copied forward saved");
    assert_eq!(r.cam.d0.row_blend, 0, "after the Swingshot camera on the ground: no row blend");
    assert!(len(fsub(to_f32x3(r.cam.d0.pos), pos)) < 1e-4, "the follow camera starts where the Swingshot camera was");
    assert_eq!(r.cam.d0.target, r.hero.pos, "0x311dd0: T = Ratchet");
    // In the air the row blend runs 90 ticks.
    let mut r = rig(1);
    r.swing([414.0, 410.0, 106.0], 8.0, 0.0);
    r.tick();
    r.hero.state = 0x2d;
    r.hero.f15d4 = 0;
    r.hero.f65c = 3;
    r.tick();
    assert_eq!(r.cam.d0.row_blend, 90);
}

/// Pulling (0x24..0x26): the init aims flat at the pull target 0x13fcb4 from the camera, keeps it; the look turn
/// ramps by 0.017 per tick in 0x25.
#[test]
fn pull_init_and_ramp() {
    let mut r = rig(1);
    let h = &mut r.hero;
    h.state = 0x25;
    h.f15d4 = 7;
    h.swing.pull = Some(9);
    h.swing.pull_pos = [410.0, 420.0, 130.0];
    r.tick();
    let s = &r.cam.swing;
    assert_eq!(s.target, Some(9));
    assert!(s.pitch_vel == 0.017, "one ramp step: {}", s.pitch_vel);
    for _ in 0..70 { r.tick(); }
    assert_eq!(r.cam.swing.pitch_vel, 1.0);
}

/// The placement: swinging, the distance and height ease (8 / 4 u/s) toward 3.65 + f and 2f with f the target's
/// height over Ratchet per rope length (clamped 0..1), the look height to 2f − 0.5 (level 14: 2f + 0.25, starting at
/// 2.0); the camera sits distance behind along the yaw, height above Ratchet.
#[test]
fn swing_placement() {
    for (level, look0, look_f) in [(1u32, 1.5f32, -0.5f32), (0xe, 2.0, 0.25)] {
        let mut r = rig(level);
        r.swing([414.0, 410.0, 104.0], 8.0, 0.0);
        r.tick();
        let f = 0.5;
        let s = &r.cam.swing;
        assert_eq!(s.dist, 4.64 - 8.0 / 60.0, "level {level}");
        assert_eq!(s.height, 2.0 - 4.0 / 60.0);
        assert_eq!(s.look_h, look0 - 8.0 / 60.0, "level {level}");
        for _ in 0..200 { r.tick(); }
        let s = &r.cam.swing;
        assert!((s.dist - (3.65 + f)).abs() < 1e-4 && (s.height - 2.0 * f).abs() < 1e-4, "level {level}: {} {}", s.dist, s.height);
        assert!((s.look_h - (2.0 * f + look_f)).abs() < 1e-4, "level {level}: look {}", s.look_h);
        let want = [HERE[0] - s.dist * s.yaw.cos(), HERE[1] - s.dist * s.yaw.sin(), HERE[2] + s.height];
        assert!(len(fsub(s.pos, want)) < 1e-3, "{:?} vs {want:?}", s.pos);
        // The forward has turned to the look point.
        let look = fsub([HERE[0], HERE[1], HERE[2] + s.look_h], s.pos);
        assert!(fdot(fnorm(look, 1.0), s.rows[0]) > 0.9999);
    }
}

/// The yaw springs toward Ratchet's yaw (k 0.00125 swinging); on levels 14 / 7 / 9 toward the best next target of
/// the group instead (ahead, lowest `distance − 10·cos`, not the swung-on one, only classes 803 / 758).
#[test]
fn group_look() {
    let g = SwingGroup {
        group: 3,
        on_pos: [0.0, 0.0, 0.0],
        members: vec![
            GroupMember { id: 5, class: 803, pos: [1.0, 0.0, 0.0] },
            GroupMember { id: 6, class: 803, pos: [-5.0, 0.0, 0.0] },
            GroupMember { id: 7, class: 500, pos: [2.0, 0.0, 0.0] },
            GroupMember { id: 8, class: 758, pos: [20.0, 10.0, 0.0] },
            GroupMember { id: 9, class: 803, pos: [12.0, 0.0, 0.0] },
        ],
    };
    // 9: 12 − 10 = 2; 8: 22.36 − 8.94 = 13.4; 5 is the swung-on one, 6 behind, 7 not a target.
    assert_eq!(next_target(&g, Some(5), 0.0), Some([12.0, 0.0, 0.0]));
    let ahead_x = SwingGroup { members: g.members.iter().copied().filter(|m| m.pos[0] > 0.0).collect(), ..g.clone() };
    assert_eq!(next_target(&ahead_x, Some(5), std::f32::consts::PI), None, "none ahead");
    for (level, looks) in [(1u32, false), (7, true), (9, true), (0xe, true)] {
        let mut r = rig(level);
        r.swing([414.0, 410.0, 106.0], 8.0, 0.0);
        r.cam.world.swing_group = Some(SwingGroup { group: 3, on_pos: [414.0, 410.0, 106.0], members: vec![GroupMember { id: 9, class: 803, pos: [420.0, 420.0, 106.0] }] });
        r.tick();
        let y0 = r.cam.swing.yaw;
        r.tick();
        let (y1, v) = (r.cam.swing.yaw, r.cam.swing.yaw_vel);
        assert_eq!(0.0 < v, looks, "level {level}: {y0} → {y1}, the spring pulls toward the next target (+y) or Ratchet's yaw 0");
    }
    // Group −1: Ratchet's yaw.
    let mut r = rig(9);
    r.swing([414.0, 410.0, 106.0], 8.0, 0.0);
    r.cam.world.swing_group = Some(SwingGroup { group: -1, on_pos: [414.0, 410.0, 106.0], members: vec![GroupMember { id: 9, class: 803, pos: [420.0, 420.0, 106.0] }] });
    r.tick();
    r.tick();
    assert!(r.cam.swing.yaw_vel <= 0.0);
}

/// A new target while swinging re-inits the camera from its own pose with a 45-tick orbit blend.
#[test]
fn target_change_reinits() {
    let mut r = rig(1);
    r.swing([414.0, 410.0, 106.0], 8.0, 0.0);
    for _ in 0..5 { r.tick(); }
    let before = r.cam.swing_view();
    r.hero.swing.on = Some(6);
    r.hero.swing.yaw = 1.0;
    let inp = CamInput { hero: &r.hero, pad: &r.pad, coll: &r.coll, mobys: None, hero_moby: None };
    let prev = r.cam.swing_update(&inp);
    assert_eq!(prev, Some(before), "the capture is its own pose");
    assert_eq!((r.cam.swing.target, r.cam.blend.mode, r.cam.blend.kind, r.cam.blend.orbit_len), (Some(6), 1, 2, 45));
    assert_eq!(r.cam.swing.dist, 4.64 - 8.0 / 60.0, "re-initialised");
}

/// A wall on the line from the camera to its place: the camera stops 0.5 short of it and faces Ratchet from there.
#[test]
fn wall_stops_short() {
    let open = floor(100.0, 98, 108, 98, 108);
    let mut cells = open.cells.clone();
    // A wall across x = 406 (between Ratchet at 410 and the camera's place ~405.4 behind him along −x).
    for cy in 100..104 {
        let (x, y) = (406.0, cy as f32 * 4.0);
        cells.push(cell([101, cy, 25], &[[x, y, 100.0], [x, y + 4.0, 100.0], [x, y + 4.0, 104.0], [x, y, 104.0]], &[([0, 1, 2, 3], 0x21)]));
    }
    let mut r = rig_on(1, mesh(cells));
    r.swing([414.0, 410.0, 102.0], 8.0, 0.0);
    // Start the camera in front of the wall (the follow camera's pose is copied).
    r.cam.cam.pos = crate::follow_camera::level::pos4([408.0, 410.0, 102.0]);
    r.tick();
    let s = &r.cam.swing;
    assert!(406.0 < s.pos[0] && s.pos[0] < 408.0, "stopped in front of the wall: {:?}", s.pos);
    assert_eq!(s.yaw_vel, 0.0);
    let d = fsub([HERE[0], HERE[1], HERE[2]], s.pos);
    assert!((s.yaw - d[1].atan2(d[0])).abs() < 1e-5, "faces Ratchet");
}

/// The collision push `0x20f2a8`: in the open the position is kept (in equal steps); the step count is
/// `trunc(d / 0.45) + 1`.
#[test]
fn collision_push_open() {
    let r = rig(1);
    let (p, pushed) = r.cam.collision_push(&r.inp(), 0.5, [405.0, 410.0, 103.0], [406.0, 410.0, 103.0]);
    assert!(!pushed && len(fsub(p, [406.0, 410.0, 103.0])) < 1e-4, "{p:?} {pushed}");
}

/// The orbit blend: the capture's (yaw, pitch, distance) rebuild the captured point; its steps end on the camera
/// (the last one lands); the first-person camera's blend resets the kind to 0.
#[test]
fn orbit_blend_lands() {
    let (f, l, u) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let c = [10.0, 20.0, 0.0];
    for p in [[5.0, 20.0, 2.0], [12.0, 25.0, -1.0], [10.0, 16.0, 3.0]] {
        let [yaw, pitch, dist] = super::super::orbit_coords(p, c, f, l, u);
        let v = frot(fnorm(f, dist), yaw, u);
        let v = frot(v, pitch, fnorm(vcross(v, u), 1.0));
        assert!(len(fsub(fadd(c, v), p)) < 1e-4, "{p:?}");
    }
    let mut r = rig(1);
    r.swing([414.0, 410.0, 106.0], 8.0, 0.0);
    let mut n = 0;
    loop {
        r.tick();
        n += 1;
        if r.cam.blend.mode == 0 { break; }
        assert!(n < 100);
    }
    assert_eq!(n, 62, "61 steps, then the blend ends");
    assert!(len(fsub(r.cam.out.pos_f32(), r.cam.swing.pos)) < 1e-5);
}

/// The collision push against a wall: the path from the previous position through the wall ends pushed out of it
/// on the near side (+0x89 set).
#[test]
fn collision_push_wall() {
    let open = floor(100.0, 98, 108, 98, 108);
    let mut cells = open.cells.clone();
    for cy in 100..104 {
        let (x, y) = (406.0, cy as f32 * 4.0);
        cells.push(cell([101, cy, 25], &[[x, y, 100.0], [x, y + 4.0, 100.0], [x, y + 4.0, 104.0], [x, y, 104.0]], &[([0, 1, 2, 3], 0x21)]));
    }
    let mut r = rig_on(1, mesh(cells));
    r.tick();
    let (p, pushed) = r.cam.collision_push(&r.inp(), 0.5, [407.0, 410.0, 102.0], [406.2, 410.0, 102.0]);
    assert!(pushed && 406.45 < p[0] && p[0] < 406.55, "{p:?} flags {:#x}", r.cam.sph_flags);
}

/// The look-up hint: an offer ahead and above within 22 raises the weight by 0.03 (the follow camera's look height
/// target = 1.5 + 0.35·weight through the callback) and the post-update keeps it while offers come, decays it by
/// 0.03 a tick without them; a target behind or the right stick's y moved does not count; the reset clears it.
#[test]
fn look_hint() {
    let mut r = rig(1);
    r.tick();
    let fwd = crate::follow_camera::rows_f(r.cam.cam.rows)[0];
    let hero = to_f32x3(r.hero.pos);
    let ahead = fadd(hero, fadd(fscale(fwd, 8.0), [0.0, 0.0, 4.0]));
    r.cam.look_hint(HintCall::Reset, 0.0);
    assert_eq!((r.cam.hint.dist, r.cam.hint.callback, r.cam.hint.weight), (10000.0, true, 0.0));
    r.cam.look_hint(HintCall::Offer { target: 3, pos: ahead, hero }, 0.2);
    assert_eq!(r.cam.hint.weight, 0.0, "the right stick's y moved");
    let behind = fsub(hero, fadd(fscale(fwd, 8.0), [0.0, 0.0, -4.0]));
    r.cam.look_hint(HintCall::Offer { target: 3, pos: behind, hero }, 0.0);
    assert_eq!(r.cam.hint.weight, 0.0, "behind the camera");
    for i in 1..=3 {
        r.cam.look_hint(HintCall::Offer { target: 3, pos: ahead, hero }, 0.0);
        assert_eq!(r.cam.hint.target, Some(3));
        r.tick();
        assert!((r.cam.hint.weight - 0.03 * i as f32).abs() < 1e-6);
        assert_eq!((r.cam.hint.target, r.cam.hint.dist), (None, 10000.0), "the post-update");
    }
    // The callback ran before the post-update: the look height target, then the spring-back of the update.
    r.cam.look_hint(HintCall::Offer { target: 3, pos: ahead, hero }, 0.0);
    r.cam.look_hint_callback();
    assert!((r.cam.cam.look_h_tgt.to_f32() - (1.5 + 0.35 * 0.12)).abs() < 1e-5);
    r.tick();
    for _ in 0..10 { r.tick(); }
    assert_eq!(r.cam.hint.weight, 0.0, "decayed");
    r.tick();
    assert!(!r.cam.hint.post);
}
