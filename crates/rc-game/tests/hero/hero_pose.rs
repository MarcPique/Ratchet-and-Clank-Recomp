//! Ratchet's and Clank's pose producers (`rc_game::hero::pose`, docs/plan/hero_gameplay.md §19): the side probes and
//! the slope tilt of the feet `0x22c5c0`, the foot motes `0x248920`, `HeroScanTargets` 0x22c080, the Magneboots lean
//! `0x2352e0` and Clank's sway `0x235e60`, on hand-built floors (the testkit) and direct calls.

use rc_formats::moby_anim::MobyAnimClass;
use rc_game::hero::fx::PartSpawn;
use rc_game::hero::idle::{joint, Manip};
use rc_game::hero::melee::MeleeTarget;
use rc_game::hero::physics::{Env, V4};
use rc_game::hero::platform::HeroWorld;
use rc_game::hero::pose::motes;
use rc_game::hero::testkit::{cam_x, cell, floor, mesh, Runner};
use rc_game::hero::Hero;
use rc_game::pad::{PadInput, PadState};
use rc_game::ps2v::Pf;
use rc_game::rng::Rng;

const DEG45: f32 = std::f32::consts::FRAC_PI_4;

fn near(a: f32, b: f32) -> bool { (a - b).abs() < 1e-4 }

/// The floor height and cell range of the hand-built levels (inside the world bounds 2..1022).
const Z: f32 = 100.0;
const C0: i16 = 100;
const X0: f32 = 400.0;

/// A floor at Z over x 400..408, y 400..404 and 0.25 lower over y 404..408 (two quads, no wall between them).
fn drop_floor() -> rc_formats::collision::Collision {
    let q = |cx: i16, cy: i16, z: f32| {
        let (x, y) = (cx as f32 * 4.0, cy as f32 * 4.0);
        cell([cx, cy, 24], &[[x, y, z], [x, y + 4.0, z], [x + 4.0, y + 4.0, z], [x + 4.0, y, z]], &[([0, 1, 2, 3], 0x21)])
    };
    mesh(vec![q(C0, C0, Z), q(C0 + 1, C0, Z), q(C0, C0 + 1, Z - 0.25), q(C0 + 1, C0 + 1, Z - 0.25)])
}

/// Standing by a drop of 0.25 on his left (0.15 from its edge): the left side probe finds the lower floor, the right
/// one his own; record 6 (list 7) tilts the left leg (1.2·atan2(−0.25, 0.2) held at −10°) and record 4 (list 22) the
/// foot against it (−(roll + leg) with the floor's roll 0: +10°); records 7 / 5 stay level; the tilted records link
/// into his list.
#[test]
fn side_drop_tilts_the_left_leg() {
    let coll = drop_floor();
    let mut r = Runner::new([X0 + 2.0, X0 + 3.85, Z], 0.0);
    r.run(&coll, PadInput::neutral(), 90);
    assert_eq!(r.hero.state, 0);
    assert!((r.hero.position()[1] - (X0 + 3.85)).abs() < 0.01, "stayed {:?}", r.hero.position());
    let s = r.hero.idle.side;
    assert!(near(s.dz[0], -0.25) && s.dz[1] == 0.0, "{s:?}");
    assert!(near(s.pitch[0], 0.0) && near(s.roll[0], 0.0), "{s:?}");
    let j = &r.hero.idle.joints;
    let ten = 10f32.to_radians();
    assert!((j[joint::LEG_L].cur[0] + ten).abs() < 0.005, "leg {:?}", j[joint::LEG_L].cur);
    assert!((j[joint::FOOT_L].cur[0] - ten).abs() < 0.005, "foot {:?}", j[joint::FOOT_L].cur);
    assert!(j[joint::LEG_R].cur[0].abs() < 0.005 && j[joint::FOOT_R].cur[0].abs() < 0.005);
    for k in [joint::LEG_L, joint::FOOT_L] { assert!(r.hero.idle.manips.contains(&Manip::Rec(k as u8)), "record {k} linked"); }
    assert!(!r.hero.idle.manips.contains(&Manip::Rec(joint::LEG_R as u8)));
}

/// On a flat floor nothing tilts; on a magnetic floor (0x13f658 = 1) the tilt is skipped too.
#[test]
fn flat_floor_and_magnet_do_not_tilt() {
    let coll = floor(Z, C0, C0 + 2, C0, C0 + 2);
    let mut r = Runner::new([X0 + 2.0, X0 + 2.0, Z], 0.0);
    r.run(&coll, PadInput::neutral(), 60);
    assert_eq!((r.hero.state, r.hero.air_ticks), (0, 0));
    assert_eq!(r.hero.idle.side.dz, [0.0; 2]);
    for k in [joint::FOOT_L, joint::FOOT_R, joint::LEG_L, joint::LEG_R] { assert_eq!(r.hero.idle.joints[k].cur, [0.0; 3], "record {k}"); }
    // The tilt branch alone (0x22c5c0): a side drop with 0x13f658 = 1 writes nothing.
    let mut h = r.hero.clone();
    h.idle.side.dz = [0.2, 0.0];
    h.f658 = 1;
    let mut rng = Rng::new();
    h.feet_update(&r.anim, &mut rng);
    assert_eq!(h.idle.joints[joint::LEG_L].target, [0.0; 3]);
    h.f658 = 0;
    h.feet_update(&r.anim, &mut rng);
    assert!(near(h.idle.joints[joint::LEG_L].target[0], (0.2f32.atan2(0.2) * 1.2).min(DEG45)));
    // Right side: −1.2·atan2(dz, 0.2) within −45°..10°.
    h.idle.side.dz = [0.0, -0.05];
    h.feet_update(&r.anim, &mut rng);
    assert!(near(h.idle.joints[joint::LEG_R].target[0], (-(-0.05f32).atan2(0.2) * 1.2).min(10f32.to_radians())));
}

/// The foot motes: `0x248920`'s parameters, then per tick at both feet `per` motes (one jitter draw and the spawner's
/// 7 draws with Ratchet's rows, 9 without), the count running down; disabled (0x80): the count is cleared.
#[test]
fn foot_motes_spawn_and_draw() {
    let coll = floor(Z, C0, C0 + 2, C0, C0 + 2);
    let mut r = Runner::new([X0 + 2.0, X0 + 2.0, Z], 0.0);
    r.run(&coll, PadInput::neutral(), 2);
    let mut h = r.hero.clone();
    h.fx.parts.clear();
    h.land_motes(motes::LAND);
    assert_eq!((h.idle.motes.count, h.idle.motes.per, h.idle.motes.flags), (5, 2, 0));
    let mut rng = Rng::new();
    let before = rng;
    h.feet_update(&r.anim, &mut rng);
    let n = h.fx.parts.iter().filter(|p| matches!(p, PartSpawn::Mote28 { rows: None, .. })).count();
    assert_eq!((n, h.idle.motes.count), (4, 4));
    let mut probe = before;
    for _ in 0..4 * (1 + 9) { probe.rand(); }
    assert_eq!(rng, probe, "4 motes × (jitter + 9)");
    // With the rows flag (0x248920(.., 1, 0)): 7 spawner draws.
    h.fx.parts.clear();
    h.land_motes(motes::LAND_ROWS);
    let before = rng;
    h.feet_update(&r.anim, &mut rng);
    assert!(h.fx.parts.iter().all(|p| matches!(p, PartSpawn::Mote28 { rows: Some(_), .. })));
    let mut probe = before;
    for _ in 0..4 * (1 + 7) { probe.rand(); }
    assert_eq!(rng, probe);
    // The spread of each: 0.016 ± 0.002.
    for p in &h.fx.parts { if let PartSpawn::Mote28 { spread, .. } = p { assert!((spread - 0.016).abs() <= 0.002 + 1e-6); } }
    // Created by the particle hook as type-28 records.
    let mut sys = rc_game::particles::Particles::new(None, Vec::new());
    rc_game::hero::fx::create_particles(&h, &mut sys);
    assert_eq!(sys.live_by_type()[28], 4);
    assert_eq!(sys.gravity, [0.0, 0.0, -1.0]);
    // The hook also hands the particles the gravity direction 0x13f5e0 (type 78 homes around it).
    h.gravity_dir = [Pf::ZERO, Pf::f(-1.0), Pf::ZERO, Pf::ZERO];
    rc_game::hero::fx::create_particles(&h, &mut sys);
    assert_eq!(sys.gravity, [0.0, -1.0, 0.0]);
    // Off (flag 0x80): the count is cleared, nothing spawns.
    h.fx.parts.clear();
    h.foot_motes(0.1, 0.0, 9, 2, 0x80, 0);
    h.feet_update(&r.anim, &mut rng);
    assert_eq!((h.idle.motes.count, h.fx.parts.len()), (0, 0));
}

/// A fall onto the floor: the landing sets the motes (0x248920(0.016, 0.002, 5, 2, 0, 1) for the plain landing) and
/// the feet update spawns them the same tick.
#[test]
fn landing_puffs_motes() {
    let coll = floor(Z, C0, C0 + 2, C0, C0 + 2);
    let mut r = Runner::new([X0 + 2.0, X0 + 2.0, Z + 3.0], 0.0);
    let mut seen = 0;
    for _ in 0..90 {
        r.tick(&coll, PadInput::neutral());
        seen += r.hero.fx.parts.iter().filter(|p| matches!(p, PartSpawn::Mote28 { .. })).count();
    }
    assert!(seen >= 4, "no landing motes");
    assert!(r.hero.idle.motes.count < 5);
}

/// A world with only a target list (`HeroWorld::melee_targets`).
struct Targets(Vec<MeleeTarget>);
impl HeroWorld for Targets {
    fn melee_targets(&self) -> &[MeleeTarget] { &self.0 }
}

fn target(id: usize, pos: [f32; 3], look: [u8; 3]) -> MeleeTarget { MeleeTarget { id, pos, health: 1.0, targetable: true, is_crate: false, look } }

/// `HeroScanTargets`: every 7th tick the best target within its record's range (+0x38), 110° of the facing and 60° of
/// elevation (score `d + off·d`, a higher priority +0x39 wins); a clear line from the head; the head (record 3) and
/// neck (record 2) turn to its aim point (+0x3a / 8 up); outside the scanning groups the target is dropped.
#[test]
fn scan_picks_and_looks() {
    let o = |x: f32, y: f32, z: f32| [X0 + x, X0 + y, Z + z];
    let coll = floor(Z, C0 - 2, C0 + 4, C0 - 2, C0 + 4);
    let pad = PadState::default();
    let (cam_rows, cam_yaw) = cam_x();
    let far = target(3, o(9.0, 2.0, 0.0), [5, 0, 8]); // 7 away: out of its range 5.
    let side = target(4, o(2.0, 6.0, 0.0), [8, 0, 8]); // 90° left, 4 away.
    let ahead = target(5, o(5.0, 2.5, 0.0), [8, 0, 8]); // ahead, 3 away.
    let behind = target(6, o(-1.0, 2.0, 0.0), [8, 9, 8]); // behind: outside 110°, priority ignored.
    let world = Targets(vec![far, side, ahead, behind]);
    let env = Env { coll: &coll, pad: &pad, cam_yaw, cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: Some(&world) };
    let mut h = Hero::spawn(o(2.0, 2.0, 0.0), 0.0);
    // Not a scan tick: nothing picked.
    h.scan_targets(&env, 0, 8);
    assert_eq!(h.idle.look_target, 0);
    h.scan_targets(&env, 0, 14);
    assert_eq!(h.idle.look_target, 6, "the lowest score of the ones in range and cone");
    // The head: yaw toward (5, 2.5, 1) from the head point (2.3, 2, 0.85), pitch down 0 → up.
    let (hy, hz) = (h.idle.joints[joint::HEAD].target[1], h.idle.joints[joint::HEAD].target[2]);
    let yaw = (0.5f32).atan2(2.7);
    assert!((hz - yaw * 0.7).abs() < 1e-3, "{hz}");
    assert!((hy - (0.85f32 - 1.0).atan2((2.7f32 * 2.7 + 0.25).sqrt())).abs() < 1e-3, "{hy}");
    assert!(near(h.idle.joints[joint::REC2].target[2], hz * 0.55));
    assert_eq!((h.idle.joints[joint::HEAD].k, h.idle.joints[joint::REC2].k), (0.021, 0.014));
    // A priority beats a lower score.
    let prio = target(7, o(2.0, 5.0, 0.0), [8, 3, 8]);
    let world = Targets(vec![ahead, prio]);
    let env2 = Env { world: Some(&world), ..env };
    h.scan_targets(&env2, 0, 21);
    assert_eq!(h.idle.look_target, 8);
    // In the first 27 ticks of the hit invulnerability: the yaw within ±17°, no pitch.
    h.f510 = 70;
    h.scan_targets(&env2, 0, 22);
    assert!(near(h.idle.joints[joint::HEAD].target[2], 0.296_705_96 * 0.7) && h.idle.joints[joint::HEAD].target[1] == 0.0);
    // Sequence 0x54: nothing (the target kept); another group: dropped.
    h.scan_targets(&env2, 0x54, 28);
    assert_eq!(h.idle.look_target, 8);
    h.group = 3;
    h.scan_targets(&env2, 0, 28);
    assert_eq!(h.idle.look_target, 0);
    // A wall between: the line from the head blocks it.
    let wall = {
        let mut cells = Vec::new();
        for cx in C0 - 2..C0 + 4 {
            for cy in C0 - 2..C0 + 4 {
                let (x, y) = (cx as f32 * 4.0, cy as f32 * 4.0);
                let mut v = vec![[x, y, Z], [x, y + 4.0, Z], [x + 4.0, y + 4.0, Z], [x + 4.0, y, Z]];
                let mut q = vec![([0, 1, 2, 3], 0x21)];
                cells.push(cell([cx, cy, 24], &v, &q));
                // A wall 3 high across x = 404, y 400..404 (in the cell above the floor's, where the line runs).
                if (cx, cy) == (C0 + 1, C0) {
                    let (wx, y0) = (X0 + 4.0, X0);
                    v = vec![[wx, y0, Z], [wx, y0 + 4.0, Z], [wx, y0 + 4.0, Z + 3.0], [wx, y0, Z + 3.0]];
                    q = vec![([0, 1, 2, 3], 0x21)];
                    cells.push(cell([cx, cy, 25], &v, &q));
                }
            }
        }
        mesh(cells)
    };
    let world = Targets(vec![ahead]);
    let env3 = Env { coll: &wall, world: Some(&world), ..env };
    let mut h = Hero::spawn(o(2.0, 2.0, 0.0), 0.0);
    h.scan_targets(&env3, 0, 0);
    assert_eq!(h.idle.look_target, 0, "blocked by the wall");
}

/// Rows whose z column is −(the world down in the hero's frame): `0x2352e0` reads only that.
fn rows_down(x: f32, y: f32, z: f32) -> [V4; 4] {
    let r = |v: f32| [Pf::ZERO, Pf::ZERO, Pf::f(-v), Pf::ZERO];
    [r(x), r(y), r(z), [Pf::ZERO; 4]]
}

/// `0x2352e0` on a magnetic floor: the neck (record 1) within the state's limit, the head (3) against it, the body
/// (13..16) from the pitch (at least 80° upside down) and the side tilt.
#[test]
fn magnet_lean_follows_the_down_direction() {
    let (x, y, z) = (0.6f32, -0.3f32, 0.9f32);
    let k40 = 0.698_131_7f32;
    let k70 = 1.221_730_5f32;
    for (state, moby, lim) in [(0, false, 0.436_332_32f32), (0x3f, false, 0.296_705_96), (0x70, false, 0.087_266_46), (0x71, false, 0.191_986_22), (4, false, 0.191_986_22), (0, true, 0.157_079_64)] {
        let mut h = Hero::spawn([0.0; 3], 0.0);
        h.state = state;
        h.f658 = 1;
        h.ground_moby = moby.then_some(1);
        h.rows = rows_down(x, y, z);
        h.magnet_lean();
        let j = &h.idle.joints;
        let n1 = (x * k40).clamp(-lim, lim);
        let n0 = (-y * k40).clamp(-lim, lim);
        assert!(near(j[joint::NECK].target[1], n1) && near(j[joint::NECK].target[0], n0), "state {state:#x}: {:?}", j[joint::NECK].target);
        assert!(near(j[joint::HEAD].target[1], -n1 * 0.5) && near(j[joint::HEAD].target[0], -n0 * 0.25));
        let p = 1.396_263_4f32;
        let q = (y * k70).clamp(-k70, k70);
        let s = joint::SECONDARY;
        for (k, m) in [(0, 0.57f32), (1, 0.5), (2, 0.5), (3, 0.35)] { assert!(near(j[s + k].target[1], p * m), "record {}", 13 + k); }
        assert!(near(j[s + 3].target[0], q) && near(j[s].target[0], q * 0.7) && near(j[s + 2].target[0], q * 0.7));
        assert_eq!((j[joint::NECK].k, j[joint::NECK].d), (0.013, 0.3));
    }
    // Half over (0.5 < z ≤ 0.85): the pitch at least 70°; upright: the pitch clamp −7°..57°.
    let mut h = Hero::spawn([0.0; 3], 0.0);
    h.f658 = 1;
    h.rows = rows_down(0.6, 0.0, 0.7);
    h.magnet_lean();
    assert!(near(h.idle.joints[joint::SECONDARY + 3].target[1], k70 * 0.35));
    h.rows = rows_down(-0.9, 0.0, -0.4);
    h.magnet_lean();
    assert!(near(h.idle.joints[joint::SECONDARY + 3].target[1], 0.994_837_64 * 0.35));
    // Off the magnetic floor: nothing.
    let mut h = Hero::spawn([0.0; 3], 0.0);
    h.rows = rows_down(x, y, z);
    h.magnet_lean();
    assert_eq!(h.idle.joints[joint::NECK].target, [0.0; 3]);
    assert_eq!(h.idle.joints[joint::NECK].k, Hero::spawn([0.0; 3], 0.0).idle.joints[joint::NECK].k);
}

fn empty_class() -> MobyAnimClass { MobyAnimClass { joint_count: 0, skeleton: vec![], rest: vec![], parent_word: vec![], sequences: vec![] } }

/// `0x235e60` in the walk: record 18 (Clank's list 0) sways against the turn and rises 0.07 (in Clank's joint units,
/// ×1024 / his scale); it links into Clank's list (`Hero::clank_modifiers`), not Ratchet's; without Clank nothing.
#[test]
fn clank_sways_in_the_walk() {
    let coll = floor(Z, C0 - 4, C0 + 8, C0 - 4, C0 + 8);
    let mut r = Runner::new([X0 + 2.0, X0 + 2.0, Z], 0.0);
    r.hero.set_back_classes(empty_class(), empty_class());
    r.hero.set_clank_scale(0.25);
    r.run(&coll, PadInput::neutral(), 3);
    assert!(r.hero.back.is_some(), "Clank created");
    // Walk and turn: state 2 with a turn residual.
    let mut attached = false;
    for t in 0..60 {
        let input = if t < 30 { PadInput::neutral().stick(0.0, -1.0) } else { PadInput::neutral().stick(-1.0, 0.0) };
        r.tick(&coll, input);
        attached |= r.hero.idle.joints[joint::CLANK0].attached;
    }
    let rec = r.hero.idle.joints[joint::CLANK0];
    assert_eq!((r.hero.state, rec.k, rec.d), (2, 0.02, 0.35));
    assert!(attached && rec.trans[2] > 0.0, "{rec:?}");
    assert!(rec.trans[2] <= 0.07 * 1024.0 / 0.25 + 1e-3);
    assert!(!r.hero.idle.manips.contains(&Manip::Rec(joint::CLANK0 as u8)), "not in Ratchet's list");
    let m = r.hero.clank_modifiers(&[3, 0xff, 0xff, 0xff, 0xff, 0xff], 0.25);
    assert!(m.iter().any(|n| n.joint == 3 && n.trans[2] == rec.trans[2]), "{m:?}");
    // The formula (direct): residual 1.0 → z = −1.2 (within ±75°), translation 0.07 × 1024 / 0.25.
    let mut h = r.hero.clone();
    h.state = 2;
    h.yaw_residual = Pf::f(1.0);
    h.clank_sway();
    let rec = h.idle.joints[joint::CLANK0];
    assert!(near(rec.target[2], -1.2) && near(rec.trans_target[2], 0.07 * 4096.0), "{rec:?}");
    h.yaw_residual = Pf::f(-2.0);
    h.clank_sway();
    assert!(near(h.idle.joints[joint::CLANK0].target[2], 1.308_997));
    // Without Clank: nothing.
    let mut h = Hero::spawn([0.0; 3], 0.0);
    h.state = 2;
    h.yaw_residual = Pf::f(1.0);
    h.clank_sway();
    assert_eq!(h.idle.joints[joint::CLANK0].target, [0.0; 3]);
}
