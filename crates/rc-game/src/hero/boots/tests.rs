//! Unit tests of the boots states on hand-built rails and floors (`hero::testkit`).
use super::*;
use crate::hero::platform::Carriers;
use crate::hero::testkit::{cell, floor, mesh, Runner};
use crate::hero::{hero_update, HeroTick};
use crate::pad::PadInput;
use rc_formats::collision::Collision;

/// A grind path through `pts` (w = chord), bounding sphere around it.
fn path(pts: &[[f32; 3]], closed: bool) -> GrindPath {
    let n = pts.len() as f32;
    let c: V3 = std::array::from_fn(|k| pts.iter().map(|p| p[k]).sum::<f32>() / n);
    let r = pts.iter().map(|&p| spline::dist3(p, c)).fold(0.0, f32::max) + 2.0;
    GrindPath { bsphere: [c[0], c[1], c[2], r], flag: closed as i32, points: spline::with_lengths(pts) }
}

fn world(paths: Vec<GrindPath>) -> Carriers {
    let mut w = Carriers::default();
    w.grind = std::sync::Arc::new(paths);
    w
}

/// One tick with the grind paths of `w` in `Env::world`.
fn tick(r: &mut Runner, coll: &Collision, w: &Carriers, input: PadInput) -> HeroTick {
    r.pad.update(Some(&input.bytes()), false);
    let env = Env { coll, pad: &r.pad, cam_yaw: r.cam_yaw, cam_rows: r.cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: Some(w) };
    let t = hero_update(&mut r.hero, &mut r.moby, &env, &mut r.anim, &mut r.rng);
    r.log.push((r.hero.state, r.hero.timer, r.hero.position()));
    t
}

fn run(r: &mut Runner, coll: &Collision, w: &Carriers, input: PadInput, n: usize) {
    for _ in 0..n { assert_eq!(tick(r, coll, w, input), HeroTick::Ran, "state {:#x}", r.hero.state); }
}

fn n() -> PadInput { PadInput::neutral() }

/// Ratchet standing on a straight rail along +x at z 100 (a rail level, the Grind Boots owned).
fn on_rail(level: i32, rails: Vec<GrindPath>) -> (Runner, Collision, Carriers) {
    let coll = floor(100.0, 100, 112, 100, 106);
    let mut r = Runner::new([402.0, 410.0, 100.0], 0.0);
    r.hero.idle.level = level;
    r.hero.grant_items(&[GRIND_BOOTS]);
    (r, coll, world(rails))
}

fn rail_a() -> GrindPath { path(&[[400.0, 410.0, 100.0], [420.0, 410.0, 100.0], [440.0, 410.0, 100.0]], false) }

#[test]
fn gravity_mode_rule_is_the_games() {
    let mut h = Hero::new();
    assert_eq!(gravity_mode(&h), 0);
    // Idle on surface 2 without the boots: mode 0 (the old port forced 1 after 4 air ticks, without the boots).
    h.f0637 = 1;
    h.air_ticks = 5;
    assert_eq!(gravity_mode(&h), 0);
    h.air_ticks = 0;
    assert_eq!(gravity_mode(&h), 0);
    h.grant_items(&[MAGNEBOOTS]);
    assert_eq!(gravity_mode(&h), 1);
    h.air_ticks = 4;
    assert_eq!(gravity_mode(&h), 0);
    h.f658 = 1;
    assert_eq!(gravity_mode(&h), 1);
    let mut h = Hero::new();
    for s in [0x3f, 0x70, 0x71] {
        h.state = s;
        assert_eq!(gravity_mode(&h), 1);
    }
}

/// No rail contact without the Grind Boots or off a rail level; with both, idle on the rail takes 0x28 and the
/// grind runs along the rail, side-on, the speed rising to 12 u/s.
#[test]
fn grind_along_a_rail() {
    let (mut r, coll, w) = on_rail(18, vec![rail_a()]);
    r.hero.owned = Default::default();
    run(&mut r, &coll, &w, n(), 3);
    assert_eq!(r.hero.state, 0);
    let (mut r, coll, w) = on_rail(1, vec![rail_a()]);
    run(&mut r, &coll, &w, n(), 3);
    assert_eq!(r.hero.state, 0);
    let (mut r, coll, w) = on_rail(18, vec![rail_a()]);
    run(&mut r, &coll, &w, n(), 2);
    assert_eq!(r.hero.state, 0x28);
    assert_eq!(r.hero.group, 0xf);
    assert_eq!(r.anim.calls.last().map(|c| c.1), Some(0x31));
    let x0 = r.hero.position()[0];
    run(&mut r, &coll, &w, n(), 80);
    let p = r.hero.position();
    assert_eq!(r.hero.state, 0x28);
    assert!(p[0] - x0 > 8.0 && (p[1] - 410.0).abs() < 0.01 && (p[2] - 100.0).abs() < 0.01, "{p:?}");
    assert!((r.hero.boots.speed - DTF * 12.0).abs() < 1e-5, "speed {}", r.hero.boots.speed / DTF);
    assert!((r.hero.yaw().to_f32() + HALF_PI).abs() < 0.01, "yaw {}", r.hero.yaw().to_f32());
    assert!(r.hero.fx.parts.iter().any(|p| matches!(p, crate::hero::fx::PartSpawn::Spark { .. })));
    assert_eq!(r.hero.packs.loops[0], -1, "no sound layer: the loop is retried every tick");
}

/// Downhill the slope term adds speed (and the downhill lean anim), uphill it takes it away.
#[test]
fn grind_slopes() {
    let down = path(&[[400.0, 410.0, 100.0], [440.0, 410.0, 88.0]], false);
    let coll = floor(100.0, 100, 112, 100, 106);
    let mut r = Runner::new([402.0, 410.0, 100.0], 0.0);
    r.hero.idle.level = 18;
    r.hero.grant_items(&[GRIND_BOOTS]);
    let w = world(vec![down]);
    // Start on the rail (the floor is flat: put him onto the grind directly).
    run(&mut r, &coll, &w, n(), 2);
    assert_eq!(r.hero.state, 0x28);
    run(&mut r, &coll, &w, n(), 40);
    assert_eq!(r.hero.boots.lean, 1);
    assert!(r.hero.boots.slope_acc > DTF * 3.0, "slope term {}", r.hero.boots.slope_acc / DTF);
    assert_eq!(r.anim.calls.last().map(|c| c.1), Some(0x4b));
}

/// ✕ on the rail: the grind jump 0x29 (up, along the rail) and the landing back into 0x28.
#[test]
fn grind_jump_lands_on_the_rail() {
    let (mut r, coll, w) = on_rail(18, vec![rail_a()]);
    run(&mut r, &coll, &w, n(), 20);
    assert_eq!(r.hero.state, 0x28);
    tick(&mut r, &coll, &w, n().press(button::CROSS));
    assert_eq!(r.hero.state, 0x29);
    assert_eq!(r.anim.calls.last().map(|c| (c.1, c.2)), Some((0x50, 4)));
    let x0 = r.hero.position()[0];
    let mut top: f32 = 0.0;
    let mut back = None;
    for t in 0..120 {
        tick(&mut r, &coll, &w, n());
        top = top.max(r.hero.position()[2] - 100.0);
        if r.hero.state == 0x28 { back = Some(t); break; }
    }
    let t = back.expect("back on the rail");
    eprintln!("grind jump: apex {top}, back on the rail after {t} ticks, x +{}", r.hero.position()[0] - x0);
    assert!((1.2..3.0).contains(&top), "apex {top}");
    assert!(r.hero.position()[0] - x0 > 3.0 && (r.hero.position()[1] - 410.0).abs() < 0.05);
}

/// ✕ with the stick to the side: the rail switch 0x2a onto the parallel rail 3 to the left.
#[test]
fn rail_switch_to_the_side() {
    let b = path(&[[400.0, 413.0, 100.0], [440.0, 413.0, 100.0]], false);
    let (mut r, coll, w) = on_rail(18, vec![rail_a(), b]);
    run(&mut r, &coll, &w, n(), 20);
    assert_eq!(r.hero.state, 0x28);
    let left = n().stick(-1.0, 0.0);
    run(&mut r, &coll, &w, left, 2);
    tick(&mut r, &coll, &w, left.press(button::CROSS));
    let mut seen = Vec::new();
    for _ in 0..140 {
        tick(&mut r, &coll, &w, left);
        if seen.last() != Some(&r.hero.state) { seen.push(r.hero.state); }
        if r.hero.state == 0x28 { break; }
    }
    eprintln!("rail switch: states {seen:x?}, rail {:?}, pos {:?}", r.hero.boots.rail, r.hero.position());
    assert!(seen.contains(&0x2a), "{seen:x?}");
    assert_eq!(r.hero.state, 0x28);
    assert_eq!(r.hero.boots.rail, Some(1));
    assert!((r.hero.position()[1] - 413.0).abs() < 0.1, "{:?}", r.hero.position());
}

/// The end of an open rail: he flies on, then falls (6) and lands.
#[test]
fn off_the_end_of_the_rail() {
    let short = path(&[[400.0, 410.0, 100.0], [406.0, 410.0, 100.0]], false);
    let (mut r, coll, w) = on_rail(18, vec![short]);
    run(&mut r, &coll, &w, n(), 2);
    assert_eq!(r.hero.state, 0x28);
    let mut states = Vec::new();
    for _ in 0..200 {
        tick(&mut r, &coll, &w, n());
        if states.last() != Some(&r.hero.state) { states.push(r.hero.state); }
    }
    eprintln!("off the end: {states:x?} at {:?}", r.hero.position());
    assert!(states.contains(&6), "{states:x?}");
    assert!(r.hero.position()[0] > 406.0);
}

/// □: the grind wrench 0x2b (anim 0x4e, the swing's hit sphere queued) and back to 0x28 after the row's idle frame.
#[test]
fn grind_wrench() {
    let (mut r, coll, w) = on_rail(18, vec![rail_a()]);
    run(&mut r, &coll, &w, n(), 10);
    tick(&mut r, &coll, &w, n().press(button::SQUARE));
    assert_eq!(r.hero.state, 0x2b);
    assert_eq!(r.anim.calls.last().map(|c| c.1), Some(0x4e));
    tick(&mut r, &coll, &w, n());
    assert_eq!(r.hero.packs.hits.len(), 1);
    // (□ still in the 7-tick buffer past frame 14 would swing again.)
    run(&mut r, &coll, &w, n(), 7);
    assert_eq!(r.hero.state, 0x2b);
    r.anim.v.frame = 40.0;
    tick(&mut r, &coll, &w, n());
    assert_eq!(r.hero.state, 0x28);
}

/// A hit on the rail: the grind hurt 0x42 with health to spare (one damage), else knocked off (0x16).
#[test]
fn grind_hurt() {
    let (mut r, coll, w) = on_rail(18, vec![rail_a()]);
    run(&mut r, &coll, &w, n(), 30);
    r.hero.damage.grind_hit = 1;
    tick(&mut r, &coll, &w, n());
    assert_eq!(r.hero.state, 0x42);
    assert_eq!(r.hero.health, 3);
    let mut back = None;
    for t in 0..100 {
        tick(&mut r, &coll, &w, n());
        if r.hero.state == 0x28 { back = Some(t); break; }
    }
    assert!(back.is_some(), "never back on the rail: {:#x}", r.hero.state);
    let (mut r, coll, w) = on_rail(18, vec![rail_a()]);
    run(&mut r, &coll, &w, n(), 30);
    r.hero.health = 1;
    r.hero.damage.grind_hit = 1;
    tick(&mut r, &coll, &w, n());
    assert_eq!(r.hero.state, 0x16);
    assert_eq!(r.hero.f518, 70);
}

/// Flat floor quads of type `ty` at z over cells x0..x1 × y0..y1.
fn floor_ty(z: f32, x0: i16, x1: i16, y0: i16, y1: i16, ty: u8) -> Collision {
    let cz = ((z - 2.0) / 4.0).floor() as i16;
    let mut cells = Vec::new();
    for cx in x0..x1 {
        for cy in y0..y1 {
            let (x, y) = (cx as f32 * 4.0, cy as f32 * 4.0);
            cells.push(cell([cx, cy, cz], &[[x, y, z], [x, y + 4.0, z], [x + 4.0, y + 4.0, z], [x + 4.0, y, z]], &[([0, 1, 2, 3], ty)]));
        }
    }
    mesh(cells)
}

/// The Magneboots on a magnetic floor (surface 2, Orxon's rules): idle is gravity mode 1, the walk becomes 0x3f,
/// ✕ the hop 0x71; without the boots the plain walk.
#[test]
fn magneboots_walk_and_hop() {
    let coll = floor_ty(100.0, 100, 112, 100, 106, 0x22);
    let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
    r.hero.idle.level = 10;
    let w = world(vec![]);
    run(&mut r, &coll, &w, n(), 3);
    assert_eq!((r.hero.state, r.hero.gravity_mode, r.hero.f658), (0, 0, 0));
    run(&mut r, &coll, &w, n().stick(0.0, -1.0), 10);
    assert_eq!(r.hero.state, 2);

    let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
    r.hero.idle.level = 10;
    r.hero.grant_items(&[MAGNEBOOTS]);
    run(&mut r, &coll, &w, n(), 3);
    assert_eq!((r.hero.state, r.hero.gravity_mode, r.hero.f658), (0, 1, 1));
    let x0 = r.hero.position()[0];
    run(&mut r, &coll, &w, n().stick(0.0, -1.0), 60);
    assert_eq!(r.hero.state, 0x3f);
    assert_eq!(r.hero.gravity_mode, 1);
    let dx = r.hero.position()[0] - x0;
    let sp = r.hero.speed.to_f32() / DTF;
    eprintln!("magnet walk: {dx} units, speed {sp} u/s");
    assert!((3.0..3.6).contains(&sp) && dx > 2.0, "speed {sp}, dx {dx}");
    assert!((r.hero.position()[2] - 100.0).abs() < 0.01);
    // No stick after 30 ticks in 0x3f: idle at once.
    tick(&mut r, &coll, &w, n());
    assert_eq!(r.hero.state, 0);
    // ✕ in idle with 0x13f658: the hop 0x71, back to idle after frame 25.
    run(&mut r, &coll, &w, n(), 8);
    tick(&mut r, &coll, &w, n().press(button::CROSS));
    assert_eq!(r.hero.state, 0x71);
    assert_eq!(r.anim.calls.last().map(|c| c.1), Some(0x5e));
    r.anim.v.frame = 26.0;
    tick(&mut r, &coll, &w, n());
    assert_eq!(r.hero.state, 0);
}

/// A 60° magnetic slope: without the boots he cannot stand on it; with them gravity pulls into the floor (mode 1),
/// he stays put and his up axis turns to the floor normal.
#[test]
fn magneboots_hold_a_steep_floor() {
    // A slope rising 7 over 4 along x (60.3°), type 0x22 (surface 2).
    let slope = || mesh(vec![cell([100, 102, 25], &[[400.0, 408.0, 100.0], [400.0, 412.0, 100.0], [404.0, 412.0, 107.0], [404.0, 408.0, 107.0]], &[([0, 1, 2, 3], 0x22)])]);
    let coll = slope();
    let w = world(vec![]);
    let start = [402.0, 410.0, 103.5];
    let mut r = Runner::new(start, 0.0);
    r.hero.idle.level = 10;
    run(&mut r, &coll, &w, n(), 60);
    let drift = spline::dist3(r.hero.position(), start);
    eprintln!("no boots: state {:#x}, drift {drift}", r.hero.state);
    assert!(drift > 1.0);

    let mut r = Runner::new(start, 0.0);
    r.hero.idle.level = 10;
    r.hero.grant_items(&[MAGNEBOOTS]);
    run(&mut r, &coll, &w, n(), 20);
    let settled = r.hero.position();
    run(&mut r, &coll, &w, n(), 40);
    let drift = spline::dist3(r.hero.position(), settled);
    assert!(spline::dist3(settled, start) < 0.5);
    let up_axis = rows_of(r.hero.rot)[2];
    let nrm = set_len([-7.0, 0.0, 4.0], 1.0);
    eprintln!("boots: state {:#x} mode {}, drift {drift}, up·n {}", r.hero.state, r.hero.gravity_mode, dot(up_axis, nrm));
    assert_eq!(r.hero.gravity_mode, 1);
    assert!(drift < 0.01, "drift {drift}");
    assert!(dot(up_axis, nrm) > 0.99);
}

/// The cable 0x74 (a cable level): caught from the fall by the hands, the slide along it at up to 14 u/s, off the
/// end into the fall.
#[test]
fn cable_slide() {
    let cable = path(&[[400.0, 410.0, 110.0], [430.0, 410.0, 104.0]], false);
    let w = world(vec![cable]);
    let coll = floor(60.0, 100, 112, 100, 106);
    // Hands (0.3 ahead, 1.34 up) 0.3 above the cable.
    let x = 401.0f32;
    let cz = 110.0 - 6.0 * (x + 0.3 - 400.0) / 30.0;
    let mut r = Runner::new([x, 410.0, cz - 1.34 + 0.3], 0.0);
    r.hero.idle.level = 3;
    let mut states = Vec::new();
    let mut top_speed: f32 = 0.0;
    for _ in 0..300 {
        tick(&mut r, &coll, &w, n());
        if states.last() != Some(&r.hero.state) { states.push(r.hero.state); }
        if r.hero.state == 0x74 { top_speed = top_speed.max(r.hero.boots.cable_speed / DTF); }
    }
    eprintln!("cable: {states:x?}, top speed {top_speed} u/s, end {:?}", r.hero.position());
    assert!(states.windows(2).any(|s| s == [6, 0x74]), "{states:x?}");
    assert!(states.windows(2).any(|s| s == [0x74, 6]), "{states:x?}");
    assert!(top_speed > 13.0 && top_speed <= 14.0 + 1e-3);
    assert!(r.hero.position()[0] > 425.0);
}

// ------------------------------------------------------------------------------------------------
// The cable caught from a jump (distilled from Kerwan's cable 0: 58.9 long, 26.74 drop; the platform under its
// start 2.9 below it), with and without the wrench's □.

/// Kerwan cable 0's slope (drop / length).
const KERWAN_SLOPE: f32 = 26.74 / 58.9;

/// A cable along +x through (`x0`, 410, `z0`) falling at Kerwan's slope, 40 long.
fn sloped_cable(x0: f32, z0: f32) -> GrindPath {
    let back = 2.0;
    path(&[[x0 - back, 410.0, z0 + back * KERWAN_SLOPE], [x0 - back + 40.0, 410.0, z0 + (back - 40.0) * KERWAN_SLOPE]], false)
}

/// Ratchet standing at (402, 410) on a floor at z 100, facing +x, the wrench in hand (slot ready, fire mask □).
fn standing(cable_z: Option<f32>) -> (Runner, Collision, Carriers) {
    let coll = floor(100.0, 100, 112, 100, 106);
    let mut r = Runner::new([402.0, 410.0, 100.0], 0.0);
    r.hero.idle.level = 3;
    let anim = rc_formats::moby_anim::AnimState { seq_a: 1, frame_a: 0, seq_b: 1, frame_b: 0, t: 0.0, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
    let wrench = super::super::items::HandItem { o_class: super::super::melee::WRENCH_CLASS, mstate: 0, anim, snapshot: None, scale: 1.0, position: [0.0; 3], rows: [[0; 4]; 3], hit_timer: 0, flight: Default::default() };
    r.hero.items.slot = super::super::items::HandSlot { item: Some(wrench), fire_mask: crate::pad::button::SQUARE, state: 2, id: 8, ..Default::default() };
    // The cable's point above the hands (0.3 ahead of the feet).
    let w = world(cable_z.map(|z| sloped_cable(402.3, z)).into_iter().collect());
    run(&mut r, &coll, &w, n(), 3);
    (r, coll, w)
}

/// One jump (✕ at tick 0, `sq` at tick `sq_at`), 90 ticks: the states entered and each tick's (state, hand z,
/// descending).
fn jump(cable_z: Option<f32>, sq_at: Option<usize>) -> (Vec<i32>, Vec<(i32, f32, i32)>, Runner) {
    let (mut r, coll, w) = standing(cable_z);
    let mut states = vec![r.hero.state];
    let mut recs = Vec::new();
    for t in 0..90 {
        let mut inp = if t == 0 { n().press(crate::pad::button::CROSS) } else { n() };
        if Some(t) == sq_at { inp = inp.press(crate::pad::button::SQUARE); }
        tick(&mut r, &coll, &w, inp);
        if states.last() != Some(&r.hero.state) { states.push(r.hero.state); }
        recs.push((r.hero.state, hand_point(&r.hero)[2], r.hero.jump.descending as i32));
    }
    (states, recs, r)
}

/// A rising jump catches a cable whose hands come within 0.7 of it (Kerwan's platform: the cable 2.9 above the
/// feet); the jump group's line → 0x74 (the port lacked it: only the fall 6 caught cables).
#[test]
fn a_rising_jump_catches_the_cable() {
    let (states, _, r) = jump(Some(100.0 + 2.9), None);
    eprintln!("jump under the cable: {states:x?}");
    assert_eq!(&states[..3], &[0, 7, 0x74], "{states:x?}");
    assert_eq!((r.hero.group, r.hero.items.f13f7, r.hero.f15d4), (0x1a, 1, 3));
    // No cable level (Novalis): the same jump lands.
    let (mut r, coll, w) = standing(Some(102.9));
    r.hero.idle.level = 1;
    let mut seen = Vec::new();
    for t in 0..90 {
        tick(&mut r, &coll, &w, if t == 0 { n().press(crate::pad::button::CROSS) } else { n() });
        seen.push(r.hero.state);
    }
    assert!(!seen.contains(&0x74));
}

/// □ pressed at the cable reaches 1.2 below it instead of 0.7 (0x13cae4 & 0x80 in `0x20d330`), and the weapon check
/// (level00 0x227fa0) leaves the jump attack 0x14 to the grab: the wrench catches the cable.
#[test]
fn square_is_the_wrench_grab_not_the_jump_attack() {
    // The apex of the hands while rising, without a cable.
    let (_, base, _) = jump(None, None);
    let top = base.iter().filter(|r| r.0 == 7 && r.2 == 0).map(|r| r.1).fold(f32::MIN, f32::max);
    // A cable 0.95 above the highest rising hands: out of the bare hands' reach, inside □'s.
    let cable = top + 0.95;
    let t_sq = base.iter().position(|r| r.0 == 7 && r.2 == 0 && cable - 1.2 < r.1 - 0.05).expect("the hands rise into the reach of □");
    eprintln!("hands' rising apex {top}, cable {cable}, □ at tick {t_sq}");
    let (bare, _, _) = jump(Some(cable), None);
    assert!(!bare.contains(&0x74), "{bare:x?}");
    let (with_sq, _, r) = jump(Some(cable), Some(t_sq));
    eprintln!("with □: {with_sq:x?}");
    assert!(with_sq.windows(2).any(|s| s == [7, 0x74]), "{with_sq:x?}");
    assert!(!with_sq.contains(&0x14));
    assert_eq!(r.hero.items.slot.id, 8, "the wrench stays in hand");
    // Without the cable, □ late in the rise is the jump attack.
    let late = base.iter().enumerate().position(|(t, r)| 11 < t && r.0 == 7 && 0.1 < r.1 - 101.34).expect("a late airborne tick");
    let (no_cable, _, _) = jump(None, Some(late));
    assert!(no_cable.contains(&0x14), "{no_cable:x?}");
}

/// On the cable: no jump-off (0x74's case has no pad test), the slide speeds up to 14 u/s at 9·dt², the drop into
/// the fall 5 ticks past the end with the 8-tick cable lockout 0x13f534; the grab anim 0x73.
#[test]
fn ride_to_the_end_no_jump_off() {
    let (mut r, coll, w) = standing(Some(102.9));
    let mut states = vec![r.hero.state];
    let (mut speeds, mut on_off) = (Vec::new(), None);
    for t in 0..400 {
        // ✕ and □ mashed while hanging.
        let inp = if t == 0 {
            n().press(crate::pad::button::CROSS)
        } else if t > 30 && t % 7 == 0 {
            n().press(crate::pad::button::CROSS | if t % 2 == 0 { crate::pad::button::SQUARE } else { 0 })
        } else {
            n()
        };
        tick(&mut r, &coll, &w, inp);
        if states.last() != Some(&r.hero.state) {
            if states.last() == Some(&0x74) { on_off = Some((t, r.hero.f534)); }
            states.push(r.hero.state);
        }
        if r.hero.state == 0x74 { speeds.push(r.hero.boots.cable_speed * 60.0); }
    }
    eprintln!("ride: {states:x?}, off at {on_off:?}, speeds {:?}..{:?}", speeds.first(), speeds.last());
    assert_eq!(&states[..4], &[0, 7, 0x74, 6], "{states:x?}");
    assert!(speeds.windows(2).all(|s| s[1] >= s[0] - 1e-4 && s[1] - s[0] <= 9.0 / 60.0 + 1e-4), "speed rises at 9 u/s²");
    assert!((speeds.last().unwrap() - 14.0).abs() < 1e-3);
    assert_eq!(on_off.unwrap().1, 8);
    // The grab anim (0x66 follows at 0x73's end: the real anim, tests/hero/hero_cable_kerwan.rs).
    assert!(r.anim.calls.iter().any(|c| c.1 == 0x73));
}
