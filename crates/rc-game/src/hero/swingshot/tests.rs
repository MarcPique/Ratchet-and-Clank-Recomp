//! Unit tests of the Swingshot states on a hand-built floor with hand-placed targets (`hero::testkit`): the target
//! searches, 0x24 → the hook's flight → 0x25 → the release, 0x26, 0x2c with the rope and the release into 0x2d.
use super::*;
use crate::hero::items::{HandItem, ItemData};
use crate::hero::platform::Carriers;
use crate::hero::testkit::{floor, Runner};
use crate::hero::{hero_update, HeroTick};
use crate::pad::{button, PadInput};
use rc_formats::collision::Collision;
use rc_formats::moby_anim::AnimState;

fn n() -> PadInput { PadInput::neutral() }
fn o() -> PadInput { PadInput::neutral().press(button::CIRCLE) }

fn record(kind: i32) -> Record { Record { kind, length: 7.0, max_speed: 0.366_666_7, cuboid: -1, mission: -1, link: -1, ..Record::default() } }

fn target(id: usize, o_class: i16, pos: V3, rec: Record) -> Target {
    Target { id, o_class, pos, yaw: 0.0, visible: true, hidden: false, rec, cuboid: None }
}

struct Sim {
    r: Runner,
    coll: Collision,
    w: Carriers,
    data: ItemData,
}

impl Sim {
    /// Ratchet on a floor at z 100 around (402, 410), facing +x, with the Swingshot ready in hand and `ts` in view.
    fn new(ts: Vec<Target>) -> Sim {
        let coll = floor(100.0, 96, 116, 96, 112);
        let mut r = Runner::new([402.0, 410.0, 100.0], 0.0);
        r.hero.idle.level = 3;
        let anim = AnimState { seq_a: 1, frame_a: 0, seq_b: 1, frame_b: 0, t: 0.0, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
        r.hero.items.slot.item = Some(HandItem { o_class: SWINGSHOT_CLASS, mstate: 0, anim, snapshot: None, scale: 1.0, position: [402.3, 410.0, 101.0], rows: [[0; 4]; 3], hit_timer: 0, flight: Default::default() });
        (r.hero.items.slot.state, r.hero.items.slot.id, r.hero.items.slot.fire_mask, r.hero.items.slot.ticks_ready) = (2, SWINGSHOT, button::CIRCLE, 60);
        let mut w = Carriers::default();
        w.targets = Targets { list: ts, camera: [390.0, 410.0, 104.0], cam_yaw: 0.0, cam_pitch: 0.0 };
        Sim { r, coll, w, data: ItemData::default() }
    }

    /// One tick: the hero update, then the hand item's update (the slot loop's, with the hero's context).
    fn tick(&mut self, input: PadInput) -> HeroTick {
        let r = &mut self.r;
        r.pad.update(Some(&input.bytes()), false);
        let env = Env { coll: &self.coll, pad: &r.pad, cam_yaw: r.cam_yaw, cam_rows: r.cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: Some(&self.w) };
        let t = hero_update(&mut r.hero, &mut r.moby, &env, &mut r.anim, &mut r.rng);
        let it = r.hero.items.slot.item.as_mut().unwrap();
        let p = r.hero.pos;
        it.position = [p[0].to_f32() + 0.3, p[1].to_f32(), p[2].to_f32() + 1.0];
        let mut c = Ctx { env: &env, anim: &mut r.anim, rng: &mut r.rng, voice: None };
        item_update(&mut r.hero, &mut c, &self.data);
        r.log.push((r.hero.state, r.hero.timer, r.hero.position()));
        t
    }

    fn run(&mut self, input: PadInput, k: usize) {
        for _ in 0..k { assert_eq!(self.tick(input), HeroTick::Ran, "state {:#x}", self.r.hero.state); }
    }

    /// Ticks with `input` until the state is `s` (at most `k`); the tick count.
    fn until(&mut self, input: PadInput, s: i32, k: usize) -> Option<usize> {
        for i in 0..k {
            self.tick(input);
            if self.r.hero.state == s { return Some(i + 1); }
        }
        None
    }

    fn states(&self) -> Vec<i32> {
        let mut v: Vec<i32> = Vec::new();
        for &(s, _, _) in &self.r.log { if v.last() != Some(&s) { v.push(s); } }
        v
    }
}

/// The pull search: the nearest in the ±30° cone wins, nothing within 4 or out of range, hidden ones and ones whose
/// cuboid holds the hero are skipped; the look stance narrows the cone to 5° about the camera.
#[test]
fn pull_search_rules() {
    let p = |id, pos| target(id, PULL_CLASS, pos, record(0));
    let mut s = Sim::new(vec![p(1, [420.0, 410.0, 104.0]), p(2, [414.0, 418.0, 104.0]), p(3, [405.0, 410.0, 101.0]), p(4, [440.0, 410.0, 104.0])]);
    let env_pull = |s: &mut Sim| {
        s.r.pad.update(Some(&n().bytes()), false);
        let env = Env { coll: &s.coll, pad: &s.r.pad, cam_yaw: s.r.cam_yaw, cam_rows: s.r.cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: Some(&s.w) };
        pull_search(&mut s.r.hero, &env)
    };
    assert!(env_pull(&mut s));
    // 2 is 33.7° off the facing, 3 within 4, 4 beyond 30: 1 wins.
    assert_eq!(s.r.hero.swing.pull, Some(1));
    assert_eq!(s.r.hero.swing.pull_len, 7.0);
    s.w.targets.list[0].hidden = true;
    assert!(!env_pull(&mut s), "no other target qualifies");
    assert_eq!(s.r.hero.swing.pull, Some(1), "0x13fcb4 keeps the last target");
    s.w.targets.list[0].hidden = false;
    s.w.targets.list[0].rec.range = 10.0;
    assert!(!env_pull(&mut s), "out of its own reach");
    s.w.targets.list[0].rec.range = 0.0;
    // A cuboid around the hero excludes the target.
    let cube = rc_formats::volumes::Shape {
        matrix: [[5.0, 0.0, 0.0, 0.0], [0.0, 5.0, 0.0, 0.0], [0.0, 0.0, 5.0, 0.0], [402.0, 410.0, 100.0, 1.0]],
        inverse: [[0.2, 0.0, 0.0, 0.0], [0.0, 0.2, 0.0, 0.0], [0.0, 0.0, 0.2, 0.0]],
        ..Default::default()
    };
    s.w.targets.list[0].rec.cuboid = 0;
    s.w.targets.list[0].cuboid = Some(cube);
    assert!(!env_pull(&mut s), "the hero is in the target's cuboid");
    s.w.targets.list[0].cuboid = None;
    s.w.targets.list[0].rec.cuboid = -1;
    // Not drawn last frame: +10, so the farther visible one would win if it qualified.
    s.w.targets.list[0].visible = false;
    assert!(env_pull(&mut s));
    let hid = s.r.hero.swing.pull_score;
    s.w.targets.list[0].visible = true;
    env_pull(&mut s);
    assert!((hid - s.r.hero.swing.pull_score - 10.0).abs() < 1e-3);
    // Look stance: 5° about the camera (at (390, 410, 104), looking along +x level): 1 (at z 104) qualifies.
    s.r.hero.state = 1;
    assert!(env_pull(&mut s));
    s.w.targets.list[0].pos[1] = 414.0;
    assert!(!env_pull(&mut s), "7.6° off the camera's yaw");
}

/// The swing search: class 803 within 16 and ±45° / 45°, the hero at most 3 above it; 0x2d widens the cone.
#[test]
fn swing_search_rules() {
    let sw = |id, pos| target(id, SWING_CLASS, pos, record(1));
    let mut s = Sim::new(vec![sw(1, [414.0, 410.0, 108.0]), sw(2, [412.0, 410.0, 96.0])]);
    let run = |s: &mut Sim| {
        s.r.pad.update(Some(&n().bytes()), false);
        let env = Env { coll: &s.coll, pad: &s.r.pad, cam_yaw: s.r.cam_yaw, cam_rows: s.r.cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: Some(&s.w) };
        swing_search(&mut s.r.hero, &env);
        (s.r.hero.swing.swing, s.r.hero.swing.swing_ok)
    };
    // 2 is 4 below the hero.
    assert_eq!(run(&mut s), (Some(1), true));
    s.w.targets.list[0].pos = [412.0, 410.0, 111.0];
    assert_eq!(run(&mut s), (None, false), "48° up: outside 45°");
    s.r.hero.state = 0x2d;
    assert_eq!(run(&mut s), (Some(1), true), "0x2d allows 65°");
}

/// ○ at a pull target: 0x24 (facing it, anim 0x2f), the hook flies at 24 u/s and holds after the rope settles, 0x25
/// flies him there (speed → 27 u/s, braking before 1.9), and ○ released drops him into the fall.
#[test]
fn pull_fire_fly_release() {
    let mut s = Sim::new(vec![target(1, PULL_CLASS, [418.0, 412.0, 104.0], record(0))]);
    s.run(n(), 2);
    s.tick(o());
    assert_eq!(s.r.hero.state, 0x24);
    assert_eq!(s.r.hero.group, 0xd);
    assert!(s.r.anim.calls.iter().any(|c| c.1 == 0x2f));
    assert_eq!(s.r.hero.swing.item.state, 2, "the hook fires");
    assert_eq!(s.r.hero.swing.item.sounds, vec![0]);
    // The hook covers ~16.6 at 24 u/s: 42 ticks, then holds, then the rope settles (0.28 → < 0.168: 5 ticks).
    let held = o();
    let k = s.until(held, 0x25, 80).expect("the pull starts");
    assert!((45..=60).contains(&k), "pull after {k} ticks");
    assert_eq!(s.r.hero.items.f13fc, 1);
    let yaw = s.r.hero.rot[2].to_f32();
    assert!((yaw - (2.0f32).atan2(16.0)).abs() < 0.05, "faces the target: {yaw}");
    let d0 = dist(f3(s.r.hero.pos), [418.0, 412.0, 104.0]);
    s.run(held, 20);
    let d1 = dist(f3(s.r.hero.pos), [418.0, 412.0, 104.0]);
    assert!(d1 < d0 - 3.0, "flies at it: {d0} → {d1}");
    assert!(s.r.hero.swing.pull_speed > 0.0 && s.r.hero.swing.pull_speed <= DTF * 27.0 + 1e-6);
    // Released: the fall 6 (in the air), speed ≤ 7 u/s, gravity 29.
    s.tick(n());
    assert_eq!(s.r.hero.state, 6, "{:x?}", s.states());
    assert!(s.r.hero.speed.to_f32() <= DTF * 7.0 + 1e-6);
    assert!((s.r.hero.group_gravity.to_f32() - DT2F * 29.0).abs() < 1e-7);
    // He lands and the hook retracts into the hand.
    s.run(n(), 200);
    assert_eq!(s.r.hero.state, 0, "{:x?}", s.states());
    assert_eq!(s.r.hero.swing.item.state, 1);
}

/// Held all the way: the pull brakes (substate 1) and ends by itself below 4 u/s near the target.
#[test]
fn pull_arrives_by_itself() {
    let mut s = Sim::new(vec![target(1, PULL_CLASS, [416.0, 410.0, 103.0], record(0))]);
    s.run(n(), 2);
    s.tick(o());
    s.until(o(), 0x25, 80).expect("the pull starts");
    let end = s.until(o(), 6, 200).or_else(|| (s.r.hero.state == 0).then_some(0));
    assert!(end.is_some(), "the pull ended: {:x?}", s.states());
    let d = dist(f3(s.r.hero.pos), [416.0, 410.0, 103.0]);
    assert!(d < 3.5, "ends near the target: {d}");
}

/// Mode ≠ 0 (record +0x08): 0x26, the pendulum at the record's length; ○ released above 0.4 → fall.
#[test]
fn arrive_pendulum() {
    let mut rec = record(0);
    rec.mode = 1;
    let mut s = Sim::new(vec![target(1, PULL_CLASS, [412.0, 410.0, 106.0], rec)]);
    s.run(n(), 2);
    s.tick(o());
    s.until(o(), 0x26, 80).expect("0x26 starts");
    s.run(o(), 60);
    assert_eq!(s.r.hero.state, 0x26);
    let d = dist(f3(s.r.hero.pos), [412.0, 410.0, 106.0]);
    assert!((d - 7.0).abs() < 0.5, "hangs at the rope's length: {d}");
    s.tick(n());
    assert!(s.r.hero.state == 6 || s.r.hero.state == 0);
}

/// The hand item is no longer the Swingshot: 0x24 returns to idle.
#[test]
fn fire_ends_without_the_swingshot() {
    let mut s = Sim::new(vec![target(1, PULL_CLASS, [420.0, 410.0, 104.0], record(0))]);
    s.run(n(), 2);
    s.tick(o());
    assert_eq!(s.r.hero.state, 0x24);
    s.r.hero.items.slot.id = 8;
    s.tick(o());
    assert_eq!(s.r.hero.state, 0);
}

/// ○ at a swing target: 0x2c (anim 0x34), the hook flies in 7 ticks and holds, the rope springs to 7 (pulling him
/// up off the floor), he swings under the target; released after 10 ticks → 0x2d with gravity 27, then lands.
#[test]
fn swing_rope_release() {
    let t = [414.0, 410.0, 108.0];
    let mut s = Sim::new(vec![target(1, SWING_CLASS, t, record(1))]);
    s.run(n(), 2);
    s.tick(o());
    assert_eq!(s.r.hero.state, 0x2c);
    assert_eq!(s.r.hero.group, 0xe);
    assert!(s.r.anim.calls.iter().any(|c| c.1 == 0x34));
    let rope0 = s.r.hero.swing.rope;
    assert!((rope0 - dist([402.0, 410.0, 100.0], t)).abs() < 0.01);
    assert_eq!(s.r.hero.swing.item.state, 9);
    assert_eq!(s.r.hero.f536 as i32, ticks(40), "the 40-tick refire lock (set after this tick's countdown)");
    s.run(o(), 12);
    assert!(s.r.hero.swing.hooked, "item state {}", s.r.hero.swing.item.state);
    s.run(o(), 90);
    assert_eq!(s.r.hero.state, 0x2c);
    assert!((s.r.hero.swing.rope - 7.0).abs() < 0.05, "rope {}", s.r.hero.swing.rope);
    let p = f3(s.r.hero.pos);
    assert!(dist(p, t) <= 7.2 && p[2] < t[2], "hangs under the target: {p:?}");
    assert!(p[2] > 100.5, "off the floor");
    // The swing moves him (a pendulum), and the body leans.
    let mut xs = Vec::new();
    for _ in 0..60 {
        s.tick(o());
        xs.push(s.r.hero.pos[0].to_f32());
    }
    let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    assert!(hi - lo > 0.5, "swings: x in {lo}..{hi}");
    s.tick(n());
    assert_eq!(s.r.hero.state, 0x2d);
    assert_eq!(s.r.hero.group, 2);
    assert!((s.r.hero.group_gravity.to_f32() - DT2F * 27.0).abs() < 1e-7);
    s.until(n(), 0, 300).expect("lands");
    assert!(s.r.hero.rot[0].to_f32().abs() < 0.05 && s.r.hero.rot[1].to_f32().abs() < 0.05, "upright again");
}

/// ○ tapped: released within 10 ticks the swing holds on; after that it lets go.
#[test]
fn swing_needs_ten_ticks() {
    let mut s = Sim::new(vec![target(1, SWING_CLASS, [414.0, 410.0, 108.0], record(1))]);
    s.run(n(), 2);
    s.tick(o());
    s.run(n(), 9);
    assert_eq!(s.r.hero.state, 0x2c);
    s.tick(n());
    s.tick(n());
    assert_eq!(s.r.hero.state, 0x2d);
    assert_eq!(s.r.hero.swing.help, 1, "released before 40 ticks");
}

/// The record parser reads a disc record (Aridia's first swing target).
#[test]
fn record_parse() {
    let hex = "010000000000e04000000000abaaaa3eabaaaa3d00000000ffffffffffffffff00000000000000000000000000000000ffffffff0000000000000000ffffffff";
    let b: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
    let r = Record::parse(&b).unwrap();
    assert_eq!((r.kind, r.length, r.mode, r.cuboid, r.mission, r.link), (1, 7.0, 0, -1, -1, -1));
    assert!((r.max_speed - 1.0 / 3.0).abs() < 1e-6 && (r.f10 - 1.0 / 12.0).abs() < 1e-6);
}
