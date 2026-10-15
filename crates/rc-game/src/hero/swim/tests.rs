//! Swim unit tests on hand-built pools: floor faces (surface 1) under water faces (surface 0).

use super::*;
use crate::hero::testkit::*;
use crate::hero::HeroTick;
use crate::pad::{button, PadInput};
use rc_formats::collision::Collision;
use std::collections::BTreeMap;

const WATER: f32 = 14.0;

/// A pool over cells `x0..x1 × y0..y1` (cell units of 4): the floor height `floor(x)` at each cell corner (a
/// ramp when it varies) and a water face at `water` where `water` is above the floor (None: dry).
/// Vertices and quads per cell key.
type CellFaces = BTreeMap<(i16, i16, i16), (Vec<[f32; 3]>, Vec<([u8; 4], u8)>)>;

fn pool(floor: impl Fn(f32) -> f32, water: Option<f32>, x0: i16, x1: i16, y0: i16, y1: i16) -> Collision {
    let mut cells = CellFaces::new();
    let mut quad = |cx: i16, cy: i16, z: [f32; 2], ty: u8| {
        let (x, y) = (cx as f32 * 4.0, cy as f32 * 4.0);
        let cz = (z[0].min(z[1]) / 4.0).floor() as i16;
        let e = cells.entry((cx, cy, cz)).or_default();
        let b = e.0.len() as u8;
        e.0.extend_from_slice(&[[x, y, z[0]], [x, y + 4.0, z[0]], [x + 4.0, y + 4.0, z[1]], [x + 4.0, y, z[1]]]);
        e.1.push(([b, b + 1, b + 2, b + 3], ty));
    };
    for cx in x0..x1 {
        for cy in y0..y1 {
            let (xa, xb) = (cx as f32 * 4.0, cx as f32 * 4.0 + 4.0);
            let z = [floor(xa), floor(xb)];
            quad(cx, cy, z, 0x21);
            if let Some(w) = water {
                if z[0] < w || z[1] < w { quad(cx, cy, [w, w], 0x00); }
            }
        }
    }
    mesh(cells.into_iter().map(|((cx, cy, cz), (v, q))| cell([cx, cy, cz], &v, &q)).collect())
}

/// A 4-deep pool (floor 10, water 14) over x, y in 0..48.
fn deep() -> Collision { pool(|_| 10.0, Some(WATER), 0, 12, 0, 12) }

fn runner(pos: [f32; 3]) -> Runner {
    let mut r = Runner::new(pos, 0.0);
    // The recording anim advances the readout by the playback speed; the stroke curve reads how far it moved.
    r.anim.v.frame_step = 0.6;
    r
}

fn run(r: &mut Runner, coll: &Collision, input: PadInput, n: usize) {
    for _ in 0..n {
        assert_eq!(r.tick(coll, input), HeroTick::Ran, "state {:#x}", r.hero.state);
    }
}

fn states(r: &Runner) -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    for l in &r.log { if v.last() != Some(&l.0) { v.push(l.0); } }
    v
}

/// Drop into the deep pool and settle on the surface.
fn floating() -> (Collision, Runner) {
    let coll = deep();
    let mut r = runner([24.0, 24.0, 18.0]);
    run(&mut r, &coll, PadInput::neutral(), 150);
    (coll, r)
}

#[test]
fn fall_into_deep_water_floats_on_the_surface() {
    let (_, r) = floating();
    let s = states(&r);
    eprintln!("states {s:x?}, z {}, bob {} level {}", r.hero.position()[2], r.hero.swim.bob.to_f32(), r.hero.swim.level.to_f32());
    assert_eq!(s, vec![0, 6, id::SURFACE_IDLE]);
    let entry = r.log.iter().find(|l| l.0 == id::SURFACE_IDLE).unwrap();
    // The fall reaches the level: entered within the band |W − (z + 0.45)| < max(0.2, |vz| + 0.07).
    assert!(entry.2[2] < WATER && entry.2[2] > WATER - 1.5, "entered at z {}", entry.2[2]);
    assert_eq!(r.hero.group, 0x12);
    assert_eq!(r.hero.water_level.to_f32(), WATER);
    assert!((r.hero.position()[2] - (WATER - 0.12)).abs() < 0.03, "floats at {}", r.hero.position()[2]);
    assert_eq!(r.hero.f0634, 1);
    assert_eq!(r.hero.swim.oxygen, OXYGEN_MAX);
    // The splash voice, played at its call point by the hero update (after the entry's splash draws).
    assert!(r.hero.swim.events.contains(&SwimEvent::Played(3)));
    assert!(r.hero.swim.events.iter().any(|e| matches!(e, SwimEvent::Splash { big: true, .. })));
}

#[test]
fn surface_swim_speed_and_stop() {
    let (coll, mut r) = floating();
    let x0 = r.hero.position()[0];
    run(&mut r, &coll, PadInput::neutral().stick(0.0, -1.0), 120);
    assert_eq!(r.hero.state, id::SURFACE_SWIM);
    // Target 3 u/s × full stick, reached through SpeedStep(4·dt²).
    assert!((r.hero.speed.to_f32() - 3.0 / 60.0).abs() < 1e-6, "speed {}", r.hero.speed.to_f32());
    let x1 = r.hero.position()[0];
    // 0.6 of the readout per tick on the curve's tail (0.4): 3·dt × 0.24 × 5.5 per tick.
    let per_tick = 3.0 / 60.0 * 0.24 * 5.5;
    let moved = x1 - x0;
    eprintln!("moved {moved} in 120 ticks (steady {per_tick}/tick), z {}", r.hero.position()[2]);
    assert!(moved > 40.0 * per_tick && moved < 120.0 * per_tick, "moved {moved}");
    assert!((r.hero.position()[1] - 24.0).abs() < 1e-3);
    assert!((r.hero.position()[2] - (WATER - 0.12)).abs() < 0.03);
    // Release: the speed runs out, then treading water again.
    run(&mut r, &coll, PadInput::neutral(), 60);
    assert_eq!(r.hero.state, id::SURFACE_IDLE);
    assert_eq!(r.hero.speed, Pf::ZERO);
}

#[test]
fn no_pack_dives_slowly_and_r1_does_nothing() {
    let (coll, mut r) = floating();
    assert!(!r.hero.owned.has(ITEM_HYDRO_PACK));
    // R1 alone on the surface: no dive without the pack.
    run(&mut r, &coll, PadInput::neutral().press(button::R1), 10);
    assert_eq!(r.hero.state, id::SURFACE_IDLE);
    // □: the plain dive 0x33 (dive-in substate, pitched down), R1 held changes nothing.
    run(&mut r, &coll, PadInput::neutral(), 2);
    run(&mut r, &coll, PadInput::neutral().press(button::SQUARE), 1);
    assert_eq!(r.hero.state, id::UNDERWATER);
    assert_eq!(r.hero.group, 0x11);
    assert_eq!(r.hero.substate, 1);
    let z0 = r.hero.position()[2];
    run(&mut r, &coll, PadInput::neutral().press(button::R1 | button::SQUARE), 40);
    assert_eq!(r.hero.state, id::UNDERWATER);
    let z1 = r.hero.position()[2];
    eprintln!("dive: z {z0} → {z1}, pitch {}, speed {}", r.hero.rot[1].to_f32(), r.hero.speed.to_f32());
    assert!(z1 < z0 - 0.3, "dove {z0} → {z1}");
    assert!(r.hero.rot[1].to_f32() > 0.5, "pitched down");
    assert!(r.hero.speed.to_f32() <= 6.0 / 60.0 + 1e-6);
    // Air drains 11 per tick under water.
    assert!(r.hero.swim.oxygen < OXYGEN_MAX && r.hero.swim.oxygen > OXYGEN_MAX - 11 * 45);
}

#[test]
fn hydro_pack_dive_thrust_and_surface() {
    let (coll, mut r) = floating();
    r.hero.owned.set(ITEM_HYDRO_PACK, true);
    run(&mut r, &coll, PadInput::neutral(), 2);
    // R1 pressed on the surface dives with the pack, straight into 0x35 while held.
    run(&mut r, &coll, PadInput::neutral().press(button::R1), 1);
    assert_eq!(r.hero.state, id::HYDRO);
    let dive = PadInput::neutral().press(button::R1 | button::SQUARE).stick(0.0, -1.0);
    run(&mut r, &coll, dive, 50);
    assert_eq!(r.hero.state, id::HYDRO);
    let deep_z = r.hero.position()[2];
    eprintln!("thrust dive: z {deep_z}, speed {}, pitch {}", r.hero.speed.to_f32(), r.hero.rot[1].to_f32());
    assert!(deep_z < WATER - 1.5, "z {deep_z}");
    assert!((r.hero.speed.to_f32() - 7.0 / 60.0).abs() < 1e-6, "full thrust {}", r.hero.speed.to_f32());
    // Climb: R1 + ✕ pitch up; back on the surface when rising within 0.4 of the level.
    let climb = PadInput::neutral().press(button::R1 | button::CROSS).stick(0.0, -1.0);
    let mut surfaced = None;
    for t in 0..240 {
        r.tick(&coll, climb);
        if r.hero.state == id::SURFACE_IDLE { surfaced = Some(t); break; }
    }
    eprintln!("surfaced after {surfaced:?} ticks at z {}, pitch {}", r.hero.position()[2], r.hero.rot[1].to_f32());
    assert!(surfaced.is_some(), "never surfaced; states {:x?}", states(&r));
    // Released pack: the pitch straightens (0x236520) on the surface.
    run(&mut r, &coll, PadInput::neutral(), 120);
    assert!(r.hero.rot[1].to_f32().abs() < 0.01 && r.hero.rot[0].to_f32().abs() < 0.01, "pitch {} roll {}", r.hero.rot[1].to_f32(), r.hero.rot[0].to_f32());
    assert!((r.hero.position()[2] - (WATER - 0.12)).abs() < 0.05);
}

#[test]
fn out_of_air_drowns_unless_masked() {
    let (coll, mut r) = floating();
    r.hero.owned.set(ITEM_O2_MASK, true);
    run(&mut r, &coll, PadInput::neutral().press(button::SQUARE), 1);
    run(&mut r, &coll, PadInput::neutral(), 1000);
    assert_eq!(r.hero.group, 0x11, "state {:#x}", r.hero.state);
    assert_eq!(r.hero.swim.oxygen, OXYGEN_MAX);
    r.hero.owned.set(ITEM_O2_MASK, false);
    let mut t = 0;
    while r.hero.state != id::DROWN && t < 1000 { r.tick(&coll, PadInput::neutral()); t += 1; }
    eprintln!("drowned after {t} ticks");
    assert_eq!(r.hero.state, id::DROWN);
    assert!((908..=910).contains(&t), "10000 / 11 per tick");
    assert_eq!(r.hero.health, 0);
    let z = r.hero.position()[2];
    run(&mut r, &coll, PadInput::neutral(), 60);
    assert!(r.hero.position()[2] < z, "sinks");
    assert_eq!(r.hero.state, id::DROWN, "locked in the death group");
}

#[test]
fn jump_out_of_deep_water() {
    let (coll, mut r) = floating();
    run(&mut r, &coll, PadInput::neutral().press(button::CROSS), 1);
    assert_eq!(r.hero.state, id::WATER_JUMP);
    assert_eq!(r.hero.jump.curve_on, 1);
    let n0 = r.log.len();
    run(&mut r, &coll, PadInput::neutral(), 90);
    let top = r.log[n0..].iter().map(|l| l.2[2]).fold(f32::MIN, f32::max);
    eprintln!("water jump apex {} (level {WATER}), states {:x?}", top, states(&r));
    assert!(top > WATER + 0.5, "apex {top}");
    // Falls back in and floats again.
    assert_eq!(r.hero.state, id::SURFACE_IDLE);
}

#[test]
fn swim_up_a_ramp_wades_out() {
    // Deep for x < 16, a ramp from 10 up to 15 over x 16..32, dry ground at 15 beyond.
    let floor = |x: f32| if x <= 16.0 { 10.0 } else if x >= 32.0 { 15.0 } else { 10.0 + (x - 16.0) * 0.3125 };
    let coll = pool(floor, Some(WATER), 0, 12, 0, 8);
    let mut r = runner([8.0, 16.0, 18.0]);
    run(&mut r, &coll, PadInput::neutral(), 150);
    assert_eq!(r.hero.state, id::SURFACE_IDLE);
    let mut seen = Vec::new();
    for _ in 0..600 {
        r.tick(&coll, PadInput::neutral().stick(0.0, -1.0));
        if seen.last() != Some(&r.hero.state) { seen.push(r.hero.state); }
        if r.hero.position()[0] > 36.0 { break; }
    }
    eprintln!("ramp: states {seen:x?}, end {:?}, depth {}", r.hero.position(), r.hero.f15f4.to_f32());
    assert!(seen.contains(&id::SURFACE_SWIM));
    assert!(seen.contains(&id::WADE), "never waded: {seen:x?}");
    assert!(r.hero.position()[0] > 32.0, "stuck at {:?}", r.hero.position());
    assert!(!matches!(r.hero.group, 0x11 | 0x12));
}

/// The ground probe takes the water level from the level's tables (a ripple patch) when they cover the hit.
#[test]
fn water_level_comes_from_the_water_tables() {
    struct Wave;
    impl WaterQuery for Wave {
        fn water_height(&self, p: [f32; 3]) -> Option<f32> { (p[0] < 30.0).then_some(p[2] + 0.1) }
    }
    let coll = deep();
    let mut r = runner([24.0, 24.0, 18.0]);
    r.water = Some(Box::new(Wave));
    run(&mut r, &coll, PadInput::neutral(), 150);
    assert_eq!(r.hero.state, id::SURFACE_IDLE);
    eprintln!("tables: level {} z {} bob {} smoothed {}", r.hero.water_level.to_f32(), r.hero.position()[2], r.hero.swim.bob.to_f32(), r.hero.swim.level.to_f32());
    assert_eq!(r.hero.water_level.to_f32(), WATER + 0.1);
    // The hero rides the table's level (the bob still settling from the fall).
    assert_eq!(r.hero.swim.level.to_f32(), WATER + 0.1);
    assert!((r.hero.position()[2] - (WATER + 0.1 - 0.12 + r.hero.swim.bob.to_f32())).abs() < 1e-5);
}

// ------------------------------------------------------------------------------------------------
// The water effects (super::effects): what each tick queued, and the rand ledger.

use crate::hero::fx::{MobySpawn, PartSpawn};

/// Draws between two stream states.
fn draws(from: crate::rng::Rng, to: crate::rng::Rng) -> usize {
    let mut r = from;
    (0..1_000_000).find(|_| { let hit = r == to; r.rand(); hit }).expect("stream never reached")
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Queued { rings45: usize, rings46: usize, drops: usize, bubbles: usize, splashes: usize }

fn queued(r: &Runner) -> Queued {
    let mut q = Queued { splashes: r.hero.fx.mobys.iter().filter(|m| matches!(m, MobySpawn::Splash { .. })).count(), ..Default::default() };
    for p in &r.hero.fx.parts {
        match p {
            PartSpawn::Ring45 { .. } => q.rings45 += 1,
            PartSpawn::Ring46 { .. } => q.rings46 += 1,
            PartSpawn::Drop35 { .. } => q.drops += 1,
            PartSpawn::Bubble { .. } => q.bubbles += 1,
            _ => {}
        }
    }
    q
}

/// The fall into deep water: SetState 0x37's splash `0x22b3a8(3, n, 1)` with n = min(300·|dz|, 40) on the entry tick
/// (the splash moby 775 of size 2.25 on the level, three type-45 rings, n type-35 drops), and the rand ledger of that
/// tick and the next ones: entry = 1 (moby) + 3 × (3 + 4) (rings: their draws + the spawner's) + n × (6 + 2) (drops),
/// and nothing else; each tick after = the splash countdown's min(t / 8, 6) = 6 bubbles × (3 + 3 + 1 + 6) (position,
/// drift, size, the spawner's six) — no wake yet (its counter needs 31 ticks), no spray (not moving) — plus, on the
/// first, the two draws of the head's secondary look (`0x22bdd0`, every state).
#[test]
fn entry_splash_and_rand_ledger() {
    let coll = deep();
    let mut r = runner([24.0, 24.0, 18.0]);
    let mut entry = None;
    for t in 0..150 {
        let s0 = r.rng;
        let was = r.hero.state;
        r.tick(&coll, PadInput::neutral());
        if was != id::SURFACE_IDLE && r.hero.state == id::SURFACE_IDLE {
            let n = r.hero.swim.events.iter().rev().find_map(|e| match e { SwimEvent::Splash { drops, big: true, .. } => Some(*drops), _ => None }).unwrap();
            let q = queued(&r);
            let mut pos = [0.0; 4];
            if let Some(MobySpawn::Splash { size, pos: p, .. }) = r.hero.fx.mobys.first() {
                assert_eq!(*size, 2.25);
                pos = *p;
            }
            entry = Some((t, n, q, draws(s0, r.rng), pos));
            for k in 1..=10 {
                let s1 = r.rng;
                r.tick(&coll, PadInput::neutral());
                assert_eq!(queued(&r), Queued { bubbles: 6, ..Default::default() });
                assert_eq!(draws(s1, r.rng), 6 * 13 + if k == 1 { 2 } else { 0 }, "tick {k} after the entry");
            }
            break;
        }
    }
    let (t, n, q, d, pos) = entry.expect("never entered the water");
    eprintln!("entry at tick {t}: {n} drops, queued {q:?}, {d} draws, splash at {pos:?}");
    assert!(n > 0 && n <= 40);
    assert_eq!(q, Queued { rings45: 3, drops: n as usize, splashes: 1, ..Default::default() });
    assert_eq!(pos[2], WATER);
    assert_eq!(d, 1 + 3 * 7 + n as usize * 8, "entry tick ledger");
}

/// Treading water: the wake `0x22ac40(15, 30)` every 17..31 ticks (one type-45 ring on the level).
#[test]
fn treading_water_leaves_a_wake() {
    let (coll, mut r) = floating();
    let mut n = 0;
    for _ in 0..300 {
        r.tick(&coll, PadInput::neutral());
        assert_eq!(r.hero.state, id::SURFACE_IDLE);
        n += queued(&r).rings45;
    }
    assert!((300 / 31..=300 / 17 + 1).contains(&n), "{n} wake rings in 300 ticks");
}

/// Swimming on the surface at full stick: bow rings `0x22ad38(0, 1)` (two type-46 every other tick), spray
/// `0x22af48(4, 12)` (a type-35 drop every 10..13 ticks) once faster than 1.5 u/s — 3·dt × the stroke is under that on
/// the curve's tail, so none here — and four bubbles at the hands in the stroke's first 10 frames.
#[test]
fn surface_swim_bow_rings() {
    let (coll, mut r) = floating();
    let mut total = Queued::default();
    for _ in 0..120 {
        r.tick(&coll, PadInput::neutral().stick(0.0, -1.0));
        let q = queued(&r);
        total.rings46 += q.rings46;
        total.drops += q.drops;
        total.bubbles += q.bubbles;
    }
    eprintln!("surface swim: {total:?}, |eff.xy| {}", r.hero.eff_len_xy.to_f32() * 60.0);
    assert_eq!(r.hero.state, id::SURFACE_SWIM);
    assert!(total.rings46 >= 60 && total.rings46 % 2 == 0, "{total:?}");
}

/// The deep-water jump 0x12: at its tick 20 the big splash `0x22b3a8(3, 16, 1)` and the ripple (0.4, 0.35).
#[test]
fn water_jump_splashes_at_tick_20() {
    let (coll, mut r) = floating();
    run(&mut r, &coll, PadInput::neutral().press(button::CROSS), 1);
    assert_eq!(r.hero.state, id::WATER_JUMP);
    let mut at = None;
    for _ in 0..40 {
        r.tick(&coll, PadInput::neutral());
        let q = queued(&r);
        if q.splashes == 1 { at = Some((r.hero.timer, q)); break; }
    }
    let (timer, q) = at.expect("no splash");
    // The physics sees tick 20; the transitions after it count the timer on.
    assert_eq!(timer, 0x14 + 1);
    assert_eq!((q.rings45, q.drops), (3, 16));
    assert!(r.hero.swim.events.contains(&SwimEvent::Ripple { x: r.hero.position()[0], y: r.hero.position()[1], r: 0.4, amp: 0.35 }));
}

/// Wading (0.5 deep, state 0x73) and ankle-deep water (0.19 deep, state 2 in water): walking leaves five bubbles at the
/// feet every tick (`0x22b140(5, 2)`), the bow rings every third tick while faster than 1 u/s, spray every 3..5 ticks
/// while faster than 0.5 u/s (kind 1 when 0.25 or deeper, else 3); a jump from the water splashes `0x22b3a8(3, 16, 0)`,
/// and landing back in it plays the voice 0x11 with the big splash `0x22b3a8(3, 24, 1)`.
#[test]
fn wading_and_shallow_water_splash() {
    for (depth, want_state, kind) in [(0.5f32, id::WADE, 1), (0.1875, 2, 3)] {
        let coll = pool(|_| WATER - depth, Some(WATER), 0, 12, 0, 12);
        let mut r = runner([8.0, 24.0, WATER + 1.0]);
        run(&mut r, &coll, PadInput::neutral(), 60);
        let mut total = Queued::default();
        let mut kinds = Vec::new();
        let mut walking = 0;
        for _ in 0..60 {
            // The block runs in the walk / wade case (the state at the physics: before the tick's transitions).
            if matches!(r.hero.state, 2 | 0x73) { walking += 1; }
            r.tick(&coll, PadInput::neutral().stick(0.0, -1.0));
            let q = queued(&r);
            total.bubbles += q.bubbles;
            total.rings46 += q.rings46;
            total.drops += q.drops;
            kinds.extend(r.hero.fx.parts.iter().filter_map(|p| match p { PartSpawn::Drop35 { kind, .. } => Some(*kind), _ => None }));
        }
        eprintln!("depth {depth}: state {:#x}, {total:?}, drop kinds {kinds:?}, speed {}", r.hero.state, r.hero.eff_len_xy.to_f32() * 60.0);
        assert_eq!(r.hero.state, want_state, "depth {depth}");
        assert!(walking > 50);
        assert_eq!(total.bubbles, 5 * walking);
        assert!(total.rings46 >= 2 * 15, "{total:?}");
        assert!(total.drops >= 10 && kinds.iter().all(|&k| k == kind), "{total:?} {kinds:?}");
        // A jump out of the water: the small splash in the jump's entry.
        r.tick(&coll, PadInput::neutral().press(button::CROSS));
        assert_eq!(r.hero.group, 4, "jumped");
        // The jump tick's physics still ran the walk / wade prologue (0x23a2e4), so its spray `0x22af48(2, 4)` may
        // add one drop (0.1 above the water, the splash's are at or below it) when its counter, restarted at
        // `randi(2)`, runs out this tick — which it does depends on the rand stream (the initial landing's foot motes,
        // `0x248920` → `0x22c5c0`'s type-28 draws, shift it). Count the splash's drops alone.
        let q = queued(&r);
        let splash_drops = r.hero.fx.parts.iter().filter(|p| matches!(p, PartSpawn::Drop35 { pos, .. } if pos[2] <= WATER)).count();
        assert!(q.drops - splash_drops <= 1, "{q:?}");
        assert_eq!((q.rings45, splash_drops, q.splashes), (3, 16, 0), "jump splash at depth {depth}");
        // Landing back in it: the voice 0x11 and the big splash.
        let mut landed = None;
        for _ in 0..90 {
            r.tick(&coll, PadInput::neutral());
            let q = queued(&r);
            if q.splashes == 1 { landed = Some(q); break; }
        }
        let q = landed.expect("no landing splash");
        assert_eq!((q.rings45, q.drops), (3, 24));
        assert!(r.hero.swim.events.contains(&SwimEvent::Played(0x11)));
    }
}

/// The swim lean (0x238634..): diving, the pitch keeps changing, so the neck record leans in y by 47·Δpitch (within
/// ±50°) and the feet kick by 40·Δpitch, with the swim springs; the neck's node reaches Ratchet's joint-modifier list.
#[test]
fn diving_leans_the_neck_and_kicks_the_feet() {
    use crate::hero::idle::joint::{FOOT_L, FOOT_R, NECK};
    let (coll, mut r) = floating();
    run(&mut r, &coll, PadInput::neutral(), 2);
    run(&mut r, &coll, PadInput::neutral().press(button::SQUARE), 1);
    let p0 = r.hero.rot[1];
    run(&mut r, &coll, PadInput::neutral().press(button::SQUARE), 1);
    assert_eq!(r.hero.state, id::UNDERWATER);
    let dp = f(fast_subtract_rotations(r.hero.rot[1], p0));
    assert!(dp > 0.0, "pitching down");
    let j = &r.hero.idle.joints;
    // Targets are cleared by the record springs after the tick: check the springs moved the angles that way and
    // the constants.
    assert_eq!((j[NECK].k, j[NECK].d), (f32::from_bits(0x3c13_74bc), f32::from_bits(0x3e61_47ae)));
    assert_eq!((j[FOOT_L].k, j[FOOT_R].d), (f32::from_bits(0x3cf5_c28f), f32::from_bits(0x3e61_47ae)));
    assert!(j[NECK].cur[1] > 0.0 && j[FOOT_L].cur[1] > 0.0 && j[FOOT_R].cur[1] > 0.0, "{:?} {:?}", j[NECK].cur, j[FOOT_L].cur);
    assert!(j[NECK].attached);
    // Direct: the targets the lean writes.
    let mut h = crate::hero::Hero::new();
    h.swim_lean(0.2, 0.05);
    let j = &h.idle.joints;
    assert_eq!(j[NECK].target, [0.2 * f32::from_bits(0x3fb3_3333), f32::from_bits(0x3f5f_66f3), 0.2 * 1.5]);
    assert_eq!(j[FOOT_R].target, [0.0, 0.05 * 40.0, 0.2 * f32::from_bits(0x3f8c_cccd)]);
}
