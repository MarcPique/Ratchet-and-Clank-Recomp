//! The census "cheap win" units of round 3, part 2 (G-CLS-027; `rc_game::moby_update::classes::units`): each runs
//! headless on its level (or on a synthetic table) covering the side effects its coverage table marks ported. The
//! level tests skip when `extracted/` is absent.
//!
//! `cargo xtask test-job --test classes --filter cheap_classes_e:: --nocapture` prints the per-unit survey.

use crate::cheap_classes_d::{hero_at, load};
use rc_game::moby_update::classes::units::{flame_jet, pressure_pad};
use crate::cheap_classes_d::Lv;
use crate::creature_classes::RecSink;
use rc_game::hero::Hero;
use rc_game::moby_runtime::MobyId;
use rc_game::moby_update::services::{pvar as p, World};

/// One update with a recording sound sink (so a kept loop reads alive).
fn run_s(lv: &mut Lv, sink: &mut RecSink, hero: &Hero, id: MobyId, f: fn(&mut World, MobyId)) {
    lv.counter += 1;
    let mut w = lv.world(hero);
    w.sound = Some(sink);
    f(&mut w, id);
}

// ---------------------------------------------------------------------------------------------------
// U487: Quartu's pressure pads 1209 (level 15)

#[test]
fn pressure_pads_time_their_group_and_stay_solved() {
    let Some(mut lv) = load(15) else { eprintln!("skipped"); return };
    let pads = lv.of_class(1209);
    assert_eq!(pads.len(), 9, "level 15's (level 17 has 6)");
    let far = hero_at([1.0, 1.0, 1.0]);
    for &q in &pads { lv.run(&far, q, pressure_pad::update); }
    let leaders: Vec<usize> = pads.iter().copied().filter(|&q| p::i32(&lv.table.mobys[q].pvars, 0x10) == q as i32 + 1).collect();
    let counts: Vec<u8> = leaders.iter().map(|&l| lv.table.mobys[l].pvars[0xb]).collect();
    eprintln!("leaders {leaders:?}, their counts {counts:?}");
    assert!(pads.iter().all(|&q| lv.table.mobys[q].state == 1 && lv.table.mobys[q].ambient == [0, 0, 0, 0]));
    let l = leaders[0];
    let g = lv.table.mobys[l].group;
    let group: Vec<usize> = pads.iter().copied().filter(|&q| lv.table.mobys[q].group == g).collect();
    assert_eq!(group.len(), counts[0] as usize, "the leader counts its group");
    assert!(group.iter().all(|&q| p::i32(&lv.table.mobys[q].pvars, 0x10) == l as i32 + 1), "the followers point at it");
    let on = |lv: &crate::cheap_classes_d::Lv, q: usize| { let c = lv.table.mobys[q].position; hero_at([c[0], c[1], c[2]]) };
    // One pad pressed: the leader's clock starts, sound 3, green.
    let h = on(&lv, group[0]);
    lv.run(&h, group[0], pressure_pad::update);
    assert_eq!(lv.table.mobys[l].state, 2, "the leader's clock runs");
    assert_eq!(lv.table.mobys[group[0]].ambient, [0, 0xff, 0, 0]);
    assert_eq!(lv.svc.sounds.first().map(|e| (e.index, e.flags)), Some((3, 0x31)), "the press sound (then the leader's first tick, 0)");
    // Let the clock run out on the leader: ticking sounds (0), then the reset sound (2), group back to 1.
    let far = hero_at([1.0, 1.0, 1.0]);
    for _ in 0..2000 {
        for &q in &group { lv.run(&far, q, pressure_pad::update); }
        if lv.svc.sounds.iter().any(|e| e.index == 2) { break; }
    }
    let idx: Vec<i32> = lv.svc.sounds.iter().map(|e| e.index).collect();
    assert!(idx.iter().filter(|&&i| i == 0).count() > 5 && idx.last() == Some(&2), "ticks then the reset: {idx:?}");
    for &q in &group { lv.run(&far, q, pressure_pad::update); lv.run(&far, q, pressure_pad::update); }
    assert!(group.iter().all(|&q| lv.table.mobys[q].state == 1 && lv.table.mobys[q].ambient == [0, 0, 0, 0]), "reset");
    // Every pad pressed in time: sound 1, the group solved (4), blinking, death bits.
    lv.svc.sounds.clear();
    for &q in &group { let h = on(&lv, q); lv.run(&h, q, pressure_pad::update); }
    lv.run(&far, l, pressure_pad::update);
    assert!(lv.svc.sounds.iter().any(|e| e.index == 1), "the solved sound");
    let mut colours = std::collections::BTreeSet::new();
    for _ in 0..45 { for &q in &group { lv.run(&far, q, pressure_pad::update); colours.insert(lv.table.mobys[q].ambient); } }
    assert!(group.iter().all(|&q| lv.table.mobys[q].state == 4));
    let b2 = lv.table.mobys[l].spawn_id;
    assert!(b2 < 0 || lv.svc.save.death.contains(&(15, b2)), "its death bits");
    assert_eq!(colours.into_iter().collect::<Vec<_>>(), vec![[0, 0xff, 0, 0], [0xff, 0xff, 0xff, 0]], "blinks white / green");
}

// ---------------------------------------------------------------------------------------------------
// U211: Blarg's flame jets 911 (level 6)

#[test]
fn flame_jets_sweep_or_cycle_with_their_loop_puffs_and_hit_lines() {
    let Some(mut lv) = load(6) else { eprintln!("skipped"); return };
    let jets = lv.of_class(911);
    assert_eq!(jets.len(), 14);
    let far = hero_at([1.0, 1.0, 1.0]);
    let mut sink = RecSink::default();
    for &j in &jets { run_s(&mut lv, &mut sink, &far, j, flame_jet::update); }
    let sweep: Vec<usize> = jets.iter().copied().filter(|&j| lv.table.mobys[j].state == 1).collect();
    let timed: Vec<usize> = jets.iter().copied().filter(|&j| lv.table.mobys[j].state == 3).collect();
    eprintln!("sweeping {sweep:?}, timed {timed:?}");
    assert_eq!(sweep.len() + timed.len(), jets.len());
    // A sweeping jet near Ratchet: its loop, two type-2 puffs a tick, its yaw swinging.
    if let Some(&j) = sweep.first() {
        let c = lv.table.mobys[j].position;
        let near = hero_at([c[0] + 3.0, c[1], c[2]]);
        let y0 = lv.table.mobys[j].rotation[2];
        let before = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0);
        for _ in 0..10 { run_s(&mut lv, &mut sink, &near, j, flame_jet::update); }
        let puffs = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0) - before;
        assert_eq!(puffs, 20, "two puffs a tick near Ratchet");
        assert_ne!(lv.table.mobys[j].rotation[2], y0, "sweeps");
        let loops: Vec<_> = lv.svc.sounds.iter().filter(|e| e.moby == j).map(|e| (e.index, e.flags)).collect();
        assert_eq!(loops.first(), Some(&(0, 4)), "the loop");
        assert!(loops.len() <= 2, "kept alive, not restarted every tick: {loops:?}");
    }
    // A timed jet: off, then on for +0x70 s with its loop (released when it goes off).
    let j = timed[0];
    lv.svc.sounds.clear();
    let (on_s, off_s) = (p::ff(&lv.table.mobys[j].pvars, 0x70), p::ff(&lv.table.mobys[j].pvars, 0x74));
    eprintln!("timed jet {j}: on {on_s} s, off {off_s} s, delay {}", p::ff(&lv.table.mobys[j].pvars, 0x78));
    let mut n = 0;
    while lv.table.mobys[j].state == 3 && n < 20_000 { run_s(&mut lv, &mut sink, &far, j, flame_jet::update); n += 1; }
    assert_eq!(lv.table.mobys[j].state, 2, "burns");
    let mut on_ticks = 0;
    while lv.table.mobys[j].state == 2 && on_ticks < 20_000 { run_s(&mut lv, &mut sink, &far, j, flame_jet::update); on_ticks += 1; }
    assert_eq!(lv.table.mobys[j].state, 3, "off again");
    assert!((on_ticks as f32 - on_s * 60.0).abs() <= 2.0, "on {on_ticks} ticks");
    assert_eq!(lv.svc.sounds.first().map(|e| (e.index, e.flags)), Some((0, 4)));
    assert_eq!(p::i32(&lv.table.mobys[j].pvars, 0x94), -1, "the loop released");
    // Its jet lines hit a hittable moby (mode 0x4000, with collision) standing 2 before it: damage 1, flags 0x10001.
    let h = lv.table.mobys.iter().position(|m| m.mode & 0x4000 != 0 && m.has_collision && m.o_class != 0 && m.o_class != 911 && m.state < 0x80).expect("a hittable moby");
    let c = lv.table.mobys[h].bsphere.map(|x| x / 1024.0);
    let yaw = lv.table.mobys[j].rotation[2];
    lv.table.mobys[j].position = [c[0] - yaw.cos() * 2.0, c[1] - yaw.sin() * 2.0, c[2] - 0.125, 1.0];
    p::set_i32(&mut lv.table.mobys[j].pvars, 0x88, 0);
    { let mut w = lv.world(&far); flame_jet::jet_lines(&mut w, j); }
    let hit = lv.svc.hits.records.iter().find(|r| r.attacker == Some(j) && r.target == h).copied();
    eprintln!("moby {h} (class {}) in the jet of {j}: {hit:?}", lv.table.mobys[h].o_class);
    let hit = hit.expect("the jet's template hit");
    assert_eq!((hit.flags, hit.damage), (0x1_0001, rc_game::ps2v::Pf::ONE));
}

// ---------------------------------------------------------------------------------------------------
// U542: the fleet's underwater laser spinners 669 (level 17)

#[test]
fn water_lasers_spin_in_the_water_with_their_fans_particles_and_hits() {
    use rc_game::moby_update::classes::draw_callbacks::Callback;
    use rc_game::moby_update::classes::units::{self, water_laser};
    let Some(mut lv) = load(17) else { eprintln!("skipped"); return };
    let lasers = lv.of_class(669);
    assert_eq!(lasers.len(), 15);
    let l = lasers[0];
    let c = lv.table.mobys[l].position;
    let dry = hero_at([c[0] + 3.0, c[1], c[2]]);
    lv.run(&dry, l, water_laser::update);
    assert_eq!(lv.table.mobys[l].state, 0, "Ratchet out of the water: nothing runs");
    let mut wet = hero_at([c[0] + 3.0, c[1], c[2]]);
    wet.group = 0x11;
    lv.run(&wet, l, water_laser::update);
    let speed = p::ff(&lv.table.mobys[l].pvars, 0);
    let step = p::ff(&lv.table.mobys[l].pvars, 4);
    eprintln!("laser {l}: speed {speed}, step {step}");
    assert_eq!((lv.table.mobys[l].state, lv.table.mobys[l].update_dist), (1, 0xff));
    assert_eq!(step, speed * water_laser::SPIN * 0.017_453_292 * (1.0 / 60.0));
    // Spinning: its step on rot.x a tick, the draw callback (six quads of FX 0x28), and with the view on it
    // 2 edge lines × 3 arms × 8 type-60 particles.
    let x0 = lv.table.mobys[l].rotation[0];
    let view = rc_game::particles::BSphereView::from_camera([c[0] - 10.0, c[1], c[2]], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.63, 0.63 * 0.775);
    let before = lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0);
    lv.counter += 1;
    {
        let mut w = lv.world(&wet);
        w.view = Some(&view);
        water_laser::update(&mut w, l);
    }
    assert!((lv.table.mobys[l].rotation[0] - rc_game::moby_update::creature::add_rot(x0, step)).abs() < 1e-6);
    assert_eq!(lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0) - before, 48);
    let row = units::row(water_laser::REFERENCE_LEVEL, water_laser::UPDATE_FN).unwrap();
    assert!(lv.svc.draw_callbacks.list1.contains(&(Callback::UnitQuads(row), l)));
    let q = units::fx_quads(&lv.table, &lv.svc, row, l).unwrap();
    assert_eq!((q.fx, q.additive, q.quads.len()), (water_laser::FX, true, 6));
    let apex = q.quads[0].corners[0];
    assert!(apex.iter().zip(&c).all(|(a, b)| (a - b).abs() < 1e-4), "each fan starts at the centre");
    assert!(q.quads.iter().all(|q| q.corners[1..].iter().all(|k| ((k[0] - c[0]).powi(2) + (k[1] - c[1]).powi(2) + (k[2] - c[2]).powi(2)).sqrt() - water_laser::LEN < 1e-3)));
    // A hittable moby on the middle line of arm 0 (state 2: no spin) takes the template hit.
    let h = lv.table.mobys.iter().position(|m| m.mode & 0x4000 != 0 && m.has_collision && m.o_class != 0 && m.o_class != 669 && m.state < 0x80).expect("a hittable moby");
    let hc = lv.table.mobys[h].bsphere.map(|x| x / 1024.0);
    lv.table.mobys[l].state = 2;
    lv.table.mobys[l].rotation = [0.0, 0.0, 0.0, 0.0];
    let b = 95.0f32.to_radians();
    lv.table.mobys[l].position = [hc[0], hc[1] - 2.5 * b.cos(), hc[2] - 2.5 * b.sin(), 1.0];
    let near = { let mut h = hero_at([hc[0], hc[1], hc[2] + 5.0]); h.group = 0x11; h };
    lv.run(&near, l, water_laser::update);
    let hit = lv.svc.hits.records.iter().find(|r| r.attacker == Some(l) && r.target == h).copied();
    eprintln!("moby {h} (class {}) on arm 0: {hit:?}", lv.table.mobys[h].o_class);
    let hit = hit.expect("the fan's template hit");
    assert_eq!((hit.flags, hit.damage), (0x1_0001, rc_game::ps2v::Pf::ONE));
}

// ---------------------------------------------------------------------------------------------------
// U349: Orxon's particle vents 1544 (level 10)

#[test]
fn orxon_vents_emit_their_kinds_of_type2_blobs() {
    use rc_game::moby_update::classes::units::orxon_vent;
    let Some(mut lv) = load(10) else { eprintln!("skipped"); return };
    let vents = lv.of_class(1544);
    assert_eq!(vents.len(), 14);
    let kinds: Vec<(usize, i32, i32)> = vents.iter().map(|&v| (v, p::i32(&lv.table.mobys[v].pvars, 0), p::i32(&lv.table.mobys[v].pvars, 4))).collect();
    eprintln!("vents (moby, kind, spline) {kinds:?}");
    let mut seen = std::collections::BTreeMap::new();
    for &(v, kind, spline) in &kinds {
        let c = lv.table.mobys[v].position;
        let at = if spline == -1 { c } else { let q = lv.svc.splines[spline as usize][0].map(f32::from_bits); [q[0], q[1], q[2], 1.0] };
        let h = hero_at([at[0], at[1], at[2]]);
        lv.run(&h, v, orxon_vent::update);
        assert_eq!(lv.table.mobys[v].state, [1u8, 2, 3][kind as usize], "vent {v} kind {kind}");
        if kind == 1 {
            let g = if spline == -1 { p::ff(&lv.table.mobys[v].pvars, orxon_vent::GROUND) } else { f32::from_bits(lv.svc.splines[spline as usize][0][3]) };
            eprintln!("vent {v}: ground z {g} (vent z {})", at[2]);
            assert!(g < at[2], "the ground below the drip point");
        }
        let before = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0);
        for _ in 0..1200 { lv.run(&h, v, orxon_vent::update); }
        let n = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0) - before;
        *seen.entry(kind).or_insert(0u64) += n;
        assert!(n > 0, "vent {v} kind {kind} emits");
        // Out of the camera's 32 (kinds 0 and 2; the camera is set to Ratchet's position explicitly): nothing.
        if kind != 1 {
            let far = hero_at([at[0] + 10_000.0, at[1], at[2]]);
            let before = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0);
            for _ in 0..400 { lv.run(&far, v, orxon_vent::update); }
            assert_eq!(lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0), before, "vent {v}: camera too far");
        }
    }
    eprintln!("type-2 blobs in 20 s by kind {seen:?}");
}
