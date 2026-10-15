//! Class 18 on a synthetic floor: a cuboid around Ratchet, the region's moby fed through `CamWorld`.

use super::*;
use crate::follow_camera::level::{CameraPorts, LevelCameras};
use crate::follow_camera::{CamInput, Camera, CameraOptions};
use crate::hero::testkit::floor;
use crate::pad::{PadInput, PadState};
use rc_formats::cameras::{CameraRecord, LevelCamera};
use rc_formats::collision::Collision;
use rc_formats::volumes::{Shape, Volumes};
use std::sync::Arc;

const HERE: [f32; 3] = [410.0, 410.0, 100.0];

fn put(p: &mut [u8], o: usize, v: f32) { p[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
fn put16(p: &mut [u8], o: usize, v: i16) { p[o..o + 2].copy_from_slice(&v.to_le_bytes()); }
fn put32(p: &mut [u8], o: usize, v: i32) { p[o..o + 4].copy_from_slice(&v.to_le_bytes()); }

/// A class-18 record: cuboid 0, mode, turn 10°/tick, moby 7, no group; `f` edits the block.
fn focus(mode: i16, rot_z: f32, f: impl Fn(&mut [u8])) -> LevelCamera {
    let mut p = vec![0u8; 0x60];
    for o in [8, 0x10, 0x14] { put32(&mut p, o, -1); }
    put32(&mut p, 0xc, 0);
    p[0x1c..0x20].copy_from_slice(&[4, 3, 3, 3]);
    put(&mut p, 0, 10.0);
    put32(&mut p, 0x28, 7);
    put16(&mut p, 0x3c, mode);
    put32(&mut p, 0x44, -1);
    f(&mut p);
    LevelCamera { record: CameraRecord { class: 18, pos: [0.0; 3], rot: [0.0, 0.0, rot_z], pvar_index: 0 }, pvar: Some(p) }
}

fn system() -> Vec<LevelCamera> {
    let rec = |class, prio, act| {
        let mut p = vec![0u8; 0x40];
        p[0x1c..0x20].copy_from_slice(&[prio, 0, 3, act]);
        LevelCamera { record: CameraRecord { class, pos: [0.0; 3], rot: [0.0; 3], pvar_index: 1 }, pvar: Some(p) }
    };
    vec![rec(0, 5, 0), rec(4, 6, 3), rec(5, 6, 2), rec(6, 6, 7), rec(7, 6, 7)]
}

fn cuboid(c: [f32; 3], h: f32) -> Shape {
    Shape {
        matrix: [[h, 0.0, 0.0, 0.0], [0.0, h, 0.0, 0.0], [0.0, 0.0, h, 0.0], [c[0], c[1], c[2], 1.0]],
        inverse: [[1.0 / h, 0.0, 0.0, 0.0], [0.0, 1.0 / h, 0.0, 0.0], [0.0, 0.0, 1.0 / h, 0.0]],
        ..Default::default()
    }
}

struct Rig {
    coll: Collision,
    hero: crate::hero::Hero,
    pad: PadState,
    cam: Camera,
}

fn rig(rec: LevelCamera, moby: Option<CamMoby>) -> Rig {
    let coll = floor(100.0, 98, 108, 98, 108);
    let hero = crate::hero::Hero::spawn(HERE, 0.0);
    let pad = PadState::default();
    let mut cam = Camera::new(&CamInput { hero: &hero, pad: &pad, coll: &coll, mobys: None, hero_moby: None }, CameraOptions::default());
    let mut cams = vec![rec];
    cams.extend(system());
    let vols = Volumes { cuboids: vec![cuboid(HERE, 5.0)], ..Default::default() };
    cam.set_level(LevelCameras::new(1, &cams, Some(Arc::new(vols)), CameraPorts::default()));
    if let Some(m) = moby { cam.world.mobys.insert(7, m); }
    Rig { coll, hero, pad, cam }
}

impl Rig {
    fn hooks(&mut self) {
        let inp = CamInput { hero: &self.hero, pad: &self.pad, coll: &self.coll, mobys: None, hero_moby: None };
        self.cam.activation_loop(&inp);
    }
    fn f(&self) -> MobyFocus { self.cam.level_cams.slots[0].focus.unwrap() }
}

/// A moby ahead of the camera (+x of Ratchet; the camera looks along +x from behind him), a bit to the left.
const AHEAD: CamMoby = CamMoby { state: 1, pos: [420.0, 413.0, 101.0] };

/// Mode 3: toward the moby (`0x313b48`), eased in over 90 ticks on top of the 200-tick ramp; the leash off, the run-
/// toward lock, the smoothed look, the vertical spring 0.01 / 0.2, the horizontal one easing 0.01 → 0.03 over 120
/// ticks; the distance and heights from the block.
#[test]
fn mode3_turns_toward_the_moby() {
    let mut r = rig(focus(3, 0.0, |p| { put(p, 0x34, 7.0); put(p, 0x38, 3.0); put(p, 0x4c, 1.0); }), Some(AHEAD));
    r.hooks();
    let d = r.cam.cam;
    assert_eq!((r.f().counter, r.cam.level_cams.owner), (1, Some(0)));
    assert!((d.yaw_rate.to_f32() - 10.0 * DEG / 200.0 / 90.0).abs() < 1e-9, "{}", d.yaw_rate.to_f32());
    assert!(d.script_yaw.to_f32() != 0.0, "the yaw input toward the moby");
    assert_eq!((d.dist_ovr, d.dist_tgt.to_f32(), d.ph_tgt.to_f32(), d.look_h_tgt.to_f32(), d.leash, d.toward_lock, d.look_from_s), (1, 7.0, 3.0, 1.0, 0, 1, 1));
    assert_eq!((d.kv.to_f32(), d.dv.to_f32(), d.dh.to_f32()), (0.01, 0.2, 0.2));
    assert!((d.kh.to_f32() - (0.01 + 0.02 / 120.0)).abs() < 1e-7);
    for _ in 0..300 { r.hooks(); }
    assert_eq!(r.f().counter, 200);
    assert_eq!(r.cam.cam.kh.to_f32(), 0.03);
    // Out of the region: the counter 0, the lock released.
    r.hero.pos[0] = crate::ps2v::Pf::f(430.0);
    r.hooks();
    assert_eq!((r.f().counter, r.cam.level_cams.owner), (0, None));
}

/// The moby gone (state ≥ 0x80): −1 for good; a group takes its first live member; all dead: gone; the suppress
/// word leaves for one tick and is cleared by the hook.
#[test]
fn moby_gone_group_and_suppress() {
    let mut r = rig(focus(3, 0.0, |_| {}), Some(CamMoby { state: 0xfe, pos: AHEAD.pos }));
    r.hooks();
    assert_eq!(r.f().counter, -1);
    r.cam.world.mobys.insert(7, AHEAD);
    r.hooks();
    assert_eq!(r.f().counter, -1, "gone for good");
    let mut r = rig(focus(3, 0.0, |p| put32(p, 0x44, 2)), None);
    r.cam.world.groups.insert(2, vec![8, 9]);
    r.cam.world.mobys.insert(8, CamMoby { state: 0xfd, pos: [0.0; 3] });
    r.cam.world.mobys.insert(9, AHEAD);
    r.hooks();
    assert_eq!(r.f().counter, 1, "the live member");
    r.cam.world.mobys.insert(9, CamMoby { state: 0x80, pos: AHEAD.pos });
    r.hooks();
    assert_eq!(r.f().counter, -1, "all dead");
    let mut r = rig(focus(3, 0.0, |_| {}), Some(AHEAD));
    r.hooks();
    r.cam.level_cams.slots[0].focus.as_mut().unwrap().suppress = 1;
    r.hooks();
    assert_eq!((r.f().counter, r.f().suppress, r.cam.level_cams.owner), (0, 0, None));
}

/// Mode 6 only while Ratchet stands on the moby (0x13f64c); +0x22 = 1: within +0x24 of the moby instead of the shape.
#[test]
fn mode6_and_near_kind() {
    let mut r = rig(focus(6, 0.0, |_| {}), Some(AHEAD));
    r.hooks();
    assert_eq!(r.f().counter, 0);
    r.hero.ground_moby = Some(7);
    r.hooks();
    assert_eq!(r.f().counter, 1);
    let mut r = rig(focus(3, 0.0, |p| { p[0x22] = 1; put(p, 0x24, 5.0); put32(p, 0xc, -1); }), Some(AHEAD));
    r.hooks();
    assert_eq!(r.f().counter, 0, "10.5 from the moby");
    r.cam.world.mobys.insert(7, CamMoby { state: 1, pos: [413.0, 411.0, 100.0] });
    r.hooks();
    assert_eq!(r.f().counter, 1);
}

/// Modes 1 / 2: the end-sphere flags 0xb0 and the sphere chain 1 / 12 / 0.11 while inside; the turn eases in to 200
/// (the second counter then 300); the stick stops it (counter 300), and once the second counter reaches 400 (mode 1)
/// with the moby within 30° of the view the turn starts again from 1.
#[test]
fn modes_1_2_stick_and_restart() {
    let mut r = rig(focus(1, 0.0, |_| {}), Some(AHEAD));
    r.hooks();
    assert_eq!((r.cam.cam.end_flags, r.cam.cam.sph_count.to_f32(), r.cam.cam.sph_step.to_f32().to_bits()), (0xb0, 12.0, 0x3de1_47ae));
    for _ in 0..200 { r.hooks(); }
    assert_eq!((r.f().counter, r.f().counter2), (200, 300), "held at 200 with the second counter at 300");
    r.pad.update(Some(&PadInput::neutral().rstick(1.0, 0.0).bytes()), false);
    r.cam.cam.yaw_rate = crate::ps2v::Pf::ZERO;
    r.hooks();
    assert_eq!((r.f().counter, r.f().counter2), (300, 301));
    assert_eq!(r.cam.cam.yaw_rate.to_f32(), 0.0, "stopped");
    r.pad.update(Some(&PadInput::neutral().bytes()), false);
    for _ in 0..98 { r.hooks(); }
    assert_eq!((r.f().counter, r.f().counter2), (398, 399));
    r.hooks();
    assert_eq!((r.f().counter, r.f().counter2), (1, 0), "restarted after 400 idle ticks with the moby in view");
}

/// Mode 7: the stick stops the turn at 300; 30 ticks later a tenth of it runs; at 400 it restarts from 1.
#[test]
fn mode7_tenth() {
    let mut r = rig(focus(7, 0.0, |_| {}), Some(AHEAD));
    r.pad.update(Some(&PadInput::neutral().rstick(1.0, 0.0).bytes()), false);
    r.hooks();
    r.pad.update(Some(&PadInput::neutral().bytes()), false);
    assert_eq!(r.f().counter, 300);
    for _ in 0..30 { r.hooks(); }
    r.cam.cam.yaw_rate = crate::ps2v::Pf::ZERO;
    r.hooks();
    assert_eq!(r.f().counter, 331);
    assert!((r.cam.cam.yaw_rate.to_f32() - 10.0 * 0.1 * DEG * 331.0 / 200.0).abs() < 1e-6);
    for _ in 0..69 { r.hooks(); }
    assert_eq!(r.f().counter, 1);
}

/// Modes 4 / 5: along the record's facing (4: its reverse when that is nearer the camera's yaw) or the camera's yaw
/// when the stick moved (5), the leash off, distance 5.84 (4: 8 when the moby is more than 110° off the facing),
/// the end-sphere flags 0xb0.
#[test]
fn modes_4_5_facing() {
    let mut r = rig(focus(4, PI, |_| {}), Some(CamMoby { state: 1, pos: [400.0, 410.0, 100.0] }));
    r.hooks();
    assert!((r.f().yaw - 0.0).abs() < 1e-5, "the reverse of π, nearer the camera's yaw 0: {}", r.f().yaw);
    let d = r.cam.cam;
    assert_eq!((d.leash, d.end_flags, d.dist_tgt.to_f32()), (0, 0xb0, 8.0), "the moby behind: 8");
    let mut r = rig(focus(5, 1.0, |_| {}), Some(AHEAD));
    r.hooks();
    assert_eq!(r.f().yaw, 1.0);
    assert_eq!(r.cam.cam.dist_tgt.to_f32(), f32::from_bits(0x40ba_e148));
    r.hooks();
    r.pad.update(Some(&PadInput::neutral().rstick(1.0, 0.0).bytes()), false);
    r.hooks();
    assert!((r.f().yaw - 0.0).abs() < 1e-3, "the camera's yaw: {}", r.f().yaw);
}

/// The view test: the flat angle to the moby within `max_angle`, its elevation from the camera within `max_pitch`
/// of the camera's.
#[test]
fn view_limits() {
    let r = rig(focus(3, 0.0, |_| {}), None);
    let cam = to_f32x3(r.cam.cam.pos);
    let fwd = to_f32x3(r.cam.cam.rows[0]);
    let yaw = fwd[1].atan2(fwd[0]);
    let at = |a: f32, up: f32| Some(CamMoby { state: 1, pos: [cam[0] + 10.0 * (yaw + a).cos(), cam[1] + 10.0 * (yaw + a).sin(), cam[2] + up] });
    assert!(r.cam.focus_view(0, 0.0, 0.0, None), "no limits");
    assert!(r.cam.focus_view(0, 30.0, 0.0, at(0.4, 0.0)));
    assert!(!r.cam.focus_view(0, 30.0, 0.0, at(0.6, 0.0)));
    assert!(!r.cam.focus_view(0, 30.0, 0.0, None));
    let pitch = fwd[2].atan2((fwd[0] * fwd[0] + fwd[1] * fwd[1]).sqrt());
    let up = |deg: f32| 10.0 * (pitch + deg * DEG).tan();
    assert!(r.cam.focus_view(0, 0.0, 20.0, at(0.0, up(15.0))));
    assert!(!r.cam.focus_view(0, 0.0, 20.0, at(0.0, up(25.0))));
}
