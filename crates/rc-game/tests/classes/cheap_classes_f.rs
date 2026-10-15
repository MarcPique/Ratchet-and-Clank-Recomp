//! The census "cheap win" units of round 4 (G-CLS-027, G-CLS-028; `rc_game::moby_update::classes::units`): each runs
//! headless on its level (or on a synthetic table) covering the side effects its coverage table marks ported. The level
//! tests skip when `extracted/` is absent. The camera and the view are set explicitly in every test.
//!
//! `cargo xtask test-job --test classes --filter cheap_classes_f:: --nocapture` prints the per-unit survey.

use crate::cheap_classes_d::{hero_at, load, Lv};
use crate::creature_classes::RecSink;
use rc_formats::{gameplay, moby_spawn};
use rc_game::moby_runtime::{mode, MobyId};
use rc_game::moby_update::classes::units::{self, air_traffic};
use rc_game::moby_update::classes::ClassUpdate;
use rc_game::moby_update::services::{pvar as p, HitTemplate, World};
use rc_game::particles::BSphereView;
use rc_game::ps2v::Pf;

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap();
    ClassUpdate::Unit(i as u16)
}

/// A camera at `eye` looking along +x (the whole scene in front of it within the draw distance is in view).
fn view_at(eye: [f32; 3], fwd: [f32; 3]) -> BSphereView {
    let l = (fwd[0] * fwd[0] + fwd[1] * fwd[1]).sqrt();
    let f = [fwd[0] / l, fwd[1] / l, 0.0];
    let left = [-f[1], f[0], 0.0];
    BSphereView::from_camera(eye, f, left, [0.0, 0.0, 1.0], 0.63, 0.63 * 0.775)
}

/// One update with the camera at `cam`, the view `view` (None: the game's unbuilt view, everything culled) and a
/// recording sound sink.
fn run(lv: &mut Lv, sink: &mut RecSink, cam: [f32; 3], view: Option<&BSphereView>, id: MobyId, f: fn(&mut World, MobyId)) {
    lv.counter += 1;
    let hero = hero_at([cam[0], cam[1], cam[2] - 30.0]);
    let mut w = lv.world(&hero);
    w.camera = [Pf::f(cam[0]), Pf::f(cam[1]), Pf::f(cam[2]), Pf::ZERO];
    w.view = view;
    w.sound = Some(sink);
    f(&mut w, id);
}

fn with<R>(lv: &mut Lv, cam: [f32; 3], view: Option<&BSphereView>, f: impl FnOnce(&mut World) -> R) -> R {
    let hero = hero_at([cam[0], cam[1], cam[2] - 30.0]);
    let mut w = lv.world(&hero);
    w.camera = [Pf::f(cam[0]), Pf::f(cam[1]), Pf::f(cam[2]), Pf::ZERO];
    w.view = view;
    f(&mut w)
}

// ---------------------------------------------------------------------------------------------------
// U128: Kerwan's air traffic 75, 115–120, 132, 795 and its exhaust trail 235 (level 03)

#[test]
fn air_traffic_resolves_on_level_03_only() {
    let Some(_) = crate::common::overlay(3) else { eprintln!("skipped: no extracted/"); return };
    let mut created = 0;
    for level in 0..19u32 {
        let ports = crate::common::ports(level, &[]).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        for &c in &air_traffic::CLASSES {
            if level == 3 {
                assert_eq!(ports.get(c), Some(unit("U128")), "class {c}");
                created += inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == c && t.spawn).count();
            } else {
                assert_ne!(ports.get(c), Some(unit("U128")), "level {level} class {c}");
            }
        }
        if level == 3 { assert_eq!(ports.get(air_traffic::TRAIL_CLASS), Some(unit("U128 235"))); }
    }
    assert_eq!(created, 246, "the census's created instances");
}

/// The vehicles of level 03 after their state-0 update (no view: the out-of-view group freeze cannot run yet).
fn loaded() -> Option<(Lv, RecSink, Vec<MobyId>)> {
    let mut lv = load(3)?;
    let mut sink = RecSink { slots: Vec::new() };
    let all: Vec<MobyId> = air_traffic::CLASSES.iter().flat_map(|&c| lv.of_class(c)).collect();
    for &i in &all { run(&mut lv, &mut sink, [0.0; 3], None, i, air_traffic::update); }
    Some((lv, sink, all))
}

#[test]
fn air_traffic_flies_its_paths_with_its_sounds_and_freezes_unseen_groups() {
    let Some((mut lv, mut sink, all)) = loaded() else { eprintln!("skipped"); return };
    assert_eq!(all.len(), 246);
    let flying: Vec<MobyId> = all.iter().copied().filter(|&i| lv.table.mobys[i].state == 1).collect();
    let parked = all.iter().filter(|&&i| lv.table.mobys[i].state == 2).count();
    let delayed = all.iter().filter(|&&i| lv.table.mobys[i].state == 3).count();
    eprintln!("air traffic: {} flying, {parked} without a path, {delayed} delayed", flying.len());
    assert!(!flying.is_empty());
    for &i in &flying {
        let m = &lv.table.mobys[i];
        assert!(m.mode & mode::TARGETABLE != 0 && m.update_dist == 0xff && m.pvars[0x2b] == 1 && m.pvars[0x2c] == 0x25, "vehicle {i}");
        assert_eq!(p::ff(&m.pvars, air_traffic::pv::T), 1.0);
        assert_eq!(p::i32(&m.pvars, air_traffic::pv::LOOP_SLOT), -1, "state 0 clears the loop slot");
        assert_eq!(m.pvars[air_traffic::pv::B131], 7);
        assert_eq!(p::i16(&m.pvars, air_traffic::pv::WRECK_T), 0);
    }
    // One flying vehicle of class 75 (sounds 1 and the horn) watched from 30 behind it for 8 ticks.
    let v = flying.iter().copied().find(|&i| lv.table.mobys[i].o_class == 75).expect("a class-75 vehicle on a path");
    let start = lv.table.mobys[v].position;
    let eye = [start[0] - 30.0, start[1], start[2]];
    let view = view_at(eye, [1.0, 0.0, 0.0]);
    let g0 = lv.table.mobys[v].group;
    lv.table.mobys[v].group = -1;
    let horn0 = p::i16(&lv.table.mobys[v].pvars, air_traffic::pv::HORN_T);
    let mut moved = 0.0f32;
    for _ in 0..8 {
        let before = lv.table.mobys[v].position;
        lv.table.mobys[v].visible = 1;
        run(&mut lv, &mut sink, eye, Some(&view), v, air_traffic::update);
        let q = lv.table.mobys[v].position;
        moved += ((q[0] - before[0]).powi(2) + (q[1] - before[1]).powi(2) + (q[2] - before[2]).powi(2)).sqrt();
        let t = p::ff(&lv.table.mobys[v].pvars, air_traffic::pv::T);
        assert!((0.0..2.0).contains(&t), "t {t}");
    }
    let speed = p::ff(&lv.table.mobys[v].pvars, air_traffic::pv::SPEED);
    eprintln!("vehicle {v}: moved {moved} in 8 ticks (speed {speed} a tick), horn timer {horn0} → {}", p::i16(&lv.table.mobys[v].pvars, air_traffic::pv::HORN_T));
    assert!(moved > 0.0);
    let loops: Vec<(i32, u32)> = lv.svc.sounds.iter().filter(|s| s.moby == v && s.flags == 4).map(|s| (s.index, s.flags)).collect();
    assert_eq!(loops, vec![(1, 4)], "the loop sound 1 once on the vehicle's tick of 8, then kept alive");
    assert!(p::i32(&lv.table.mobys[v].pvars, air_traffic::pv::LOOP_SLOT) >= 0);
    assert_ne!(p::i16(&lv.table.mobys[v].pvars, air_traffic::pv::HORN_T), -1, "the horn interval is set");
    // The horn: its timer out with the camera 15..90 away → class sound 0 (flags 0) and a new interval ≥ ticks(2200).
    p::set_i16(&mut lv.table.mobys[v].pvars, air_traffic::pv::HORN_T, 1);
    run(&mut lv, &mut sink, eye, Some(&view), v, air_traffic::update);
    assert!(lv.svc.sounds.iter().any(|s| s.moby == v && (s.index, s.flags) == (0, 0)), "the horn");
    assert!(p::i16(&lv.table.mobys[v].pvars, air_traffic::pv::HORN_T) >= 2199);
    // A group none of whose members is in view: its first live member parks all of them for 5 ticks.
    let g = flying.iter().map(|&i| lv.table.mobys[i].group).find(|&g| g >= 0 && g != g0).expect("a grouped vehicle");
    let members: Vec<MobyId> = all.iter().copied().filter(|&i| lv.table.mobys[i].group == g).collect();
    let first = { let w = with(&mut lv, eye, None, |w| rc_game::moby_update::scheduler::group_ids(w, g)); w[0] };
    for &m in &members { lv.table.mobys[m].state = 1; }
    run(&mut lv, &mut sink, eye, None, first, air_traffic::update);
    // The first member's own state-3 branch counts its delay down in the same update.
    let delay = |m: MobyId| if m == first { 4 } else { 5 };
    assert!(members.iter().all(|&m| lv.table.mobys[m].state == 3 && p::i16(&lv.table.mobys[m].pvars, air_traffic::pv::DELAY) == delay(m)), "group {g} parked");
    for _ in 0..3 { run(&mut lv, &mut sink, eye, None, first, air_traffic::update); }
    assert_eq!(lv.table.mobys[first].state, 3);
    run(&mut lv, &mut sink, eye, None, first, air_traffic::update);
    assert_eq!(lv.table.mobys[first].state, 1, "back to flight once the delay is out (and parked again next tick while unseen)");
    eprintln!("group {g}: {} members parked while unseen", members.len());
}

#[test]
fn air_traffic_exhaust_per_class_and_the_trail_235() {
    let Some((mut lv, _sink, all)) = loaded() else { eprintln!("skipped"); return };
    let of = |lv: &Lv, c: i16| all.iter().copied().find(|&i| lv.table.mobys[i].o_class == c && lv.table.mobys[i].state == 1);
    let slow = [0.1, 0.0, 0.0, 0.0];
    let parts = |lv: &Lv| lv.svc.fx.part_spawns.get(&22).copied().unwrap_or(0);
    // 115 / 117 / 120 / 132: 10 puffs near the camera once the timer runs out, then a 7..20-tick wait.
    for (c, n, near) in [(115, 10, 75.0), (117, 10, 75.0), (120, 10, 75.0), (132, 10, 75.0), (118, 15, 90.0)] {
        let Some(v) = of(&lv, c) else { panic!("class {c}") };
        let q = lv.table.mobys[v].position;
        let cam = [q[0] - (near - 5.0), q[1], q[2]];
        let n0 = parts(&lv);
        lv.table.mobys[v].pvars[air_traffic::pv::PUFF_T] = 1;
        with(&mut lv, cam, None, |w| air_traffic::exhaust(w, v, slow, false));
        assert_eq!(parts(&lv) - n0, n, "class {c}");
        let t = lv.table.mobys[v].pvars[air_traffic::pv::PUFF_T];
        assert!((7..=20).contains(&t), "class {c}: wait {t}");
        // Too far, too fast or culled: none.
        lv.table.mobys[v].pvars[air_traffic::pv::PUFF_T] = 1;
        let far = [q[0] - (near + 5.0), q[1], q[2]];
        with(&mut lv, far, None, |w| air_traffic::exhaust(w, v, slow, false));
        lv.table.mobys[v].pvars[air_traffic::pv::PUFF_T] = 1;
        with(&mut lv, cam, None, |w| air_traffic::exhaust(w, v, [1.0, 0.0, 0.0, 0.0], false));
        lv.table.mobys[v].pvars[air_traffic::pv::PUFF_T] = 1;
        with(&mut lv, cam, None, |w| air_traffic::exhaust(w, v, slow, true));
        assert_eq!(parts(&lv) - n0, n, "class {c}: no more");
    }
    // 116: two puffs every tick it is seen, none culled.
    let v = of(&lv, 116).unwrap();
    let n0 = parts(&lv);
    let q = lv.table.mobys[v].position;
    with(&mut lv, [q[0] + 500.0, q[1], q[2]], None, |w| air_traffic::exhaust(w, v, [3.0, 0.0, 0.0, 0.0], false));
    with(&mut lv, [q[0] + 500.0, q[1], q[2]], None, |w| air_traffic::exhaust(w, v, [3.0, 0.0, 0.0, 0.0], true));
    assert_eq!(parts(&lv) - n0, 2);
    // 75 / 119: the trail moby 235 at the exhaust point (joint 0), its colours, then the trail's own update.
    for (c, c1, c2) in [(75, 0x30a0u32, 0x90a0u32), (119, 0xa040, 0xa080)] {
        let v = of(&lv, c).unwrap();
        let q = lv.table.mobys[v].position;
        let cam = [q[0] - 20.0, q[1], q[2]];
        let before = lv.of_class(air_traffic::TRAIL_CLASS);
        lv.table.mobys[v].pvars[air_traffic::pv::PUFF_T] = 1;
        lv.table.mobys[v].pvars[air_traffic::pv::TRAIL_T] = 0;
        with(&mut lv, cam, None, |w| air_traffic::exhaust(w, v, slow, false));
        let trails: Vec<MobyId> = lv.of_class(air_traffic::TRAIL_CLASS).into_iter().filter(|t| !before.contains(t)).collect();
        assert_eq!(trails.len(), 1, "class {c}: one trail");
        let t = trails[0];
        let joint = with(&mut lv, cam, None, |w| w.joint_point(v, 0));
        let m = &lv.table.mobys[t];
        assert_eq!(p::v4f(&lv.table.mobys[v].pvars, air_traffic::pv::EXHAUST), joint);
        assert_eq!((m.position, m.update_dist, m.draw_dist, m.visible), (joint, 0xff, 0x7f, 1));
        assert_eq!((p::i32(&m.pvars, 0) as MobyId, p::u32(&m.pvars, 4), p::u32(&m.pvars, 8), p::i16(&m.pvars, 0xc), p::i16(&m.pvars, 0xe)), (v, c1, c2, 50, 50));
        assert_eq!(&m.ambient[..3], &[c1 as u8, (c1 >> 8) as u8, 0]);
        assert_eq!(lv.table.mobys[v].pvars[air_traffic::pv::TRAIL_T], 50);
        assert_eq!(lv.table.mobys[v].pvars[air_traffic::pv::PUFF_T], 13);
        let s0 = m.scale;
        let mut sink = RecSink { slots: Vec::new() };
        run(&mut lv, &mut sink, cam, None, t, air_traffic::trail_update);
        let m = &lv.table.mobys[t];
        assert_eq!(m.alpha, (49.0f32 / 50.0 * 128.0) as u8);
        assert!((m.scale - s0 * (0.019_999_98f32 + 1.0)).abs() < 1e-6);
        let back = ((m.position[0] - joint[0]).powi(2) + (m.position[1] - joint[1]).powi(2) + (m.position[2] - joint[2]).powi(2)).sqrt();
        assert!((back - 2.5 / 60.0).abs() < 1e-4, "moved back 2.5·dt: {back}");
        for _ in 0..49 { run(&mut lv, &mut sink, cam, None, t, air_traffic::trail_update); }
        assert!(lv.table.mobys[t].state >= 0x80, "deleted when its life runs out");
    }
}

#[test]
fn air_traffic_wreck_blast_bolts_pieces_sound_and_hide() {
    let Some((mut lv, mut sink, all)) = loaded() else { eprintln!("skipped"); return };
    let bolts = |lv: &Lv| (13..=16).map(|c| lv.of_class(c).len()).sum::<usize>();
    for c in [115i16, 116, 117, 118, 119, 120, 132, 75, 795] {
        let Some(v) = all.iter().copied().find(|&i| lv.table.mobys[i].o_class == c && lv.table.mobys[i].state == 1) else { panic!("class {c}") };
        let q = lv.table.mobys[v].position;
        let cam = [q[0] - 30.0, q[1], q[2]];
        let (b0, fl0, sh0, un0) = (bolts(&lv), lv.svc.fx.flashes, lv.svc.camera_shakes.len(), lv.svc.fx.unported.values().sum::<u64>());
        let pieces0: Vec<usize> = air_traffic::pieces(c).iter().map(|&k| lv.of_class(k).len()).collect();
        let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: None, flags: 0x80_0000, b18: 0, b19: 0, h1a: 0, damage: Pf::f(1.0), w20: 0 };
        with(&mut lv, cam, None, |w| w.deliver_hit(v, &tmpl));
        lv.table.mobys[v].group = -1;
        lv.table.mobys[v].visible = 1;
        run(&mut lv, &mut sink, cam, None, v, air_traffic::update);
        let m = &lv.table.mobys[v];
        let nb = bolts(&lv) - b0;
        assert!((2..=20).contains(&nb), "class {c}: {nb} coins (10..20 bolts for a 0x800000 hit)");
        assert!(lv.svc.fx.flashes > fl0 && lv.svc.camera_shakes.len() > sh0, "class {c}: the beam explosion");
        for (k, &pc) in air_traffic::pieces(c).iter().enumerate() { assert_eq!(lv.of_class(pc).len(), pieces0[k] + 1, "class {c}: piece {pc:#x}"); }
        assert_eq!(lv.svc.level_defs_played.last(), Some(&(0, v)), "class {c}: level def 2 at the moby");
        assert_eq!(p::i16(&m.pvars, air_traffic::pv::WRECK_T), 599, "class {c}");
        assert!(m.mode & 0x41 == 0x41 && m.mode & mode::TARGETABLE == 0 && !m.has_collision && m.hit_slot == 0xff, "class {c}: hidden");
        assert_eq!(p::i32(&m.pvars, air_traffic::pv::LOOP_SLOT), -1);
        assert_eq!(lv.svc.fx.unported.values().sum::<u64>() - un0, 0, "class {c}: every row ported (795's skill point goes through `story::award_skill_point` since the story lane)");
        // Out of view (no view: culled) when the timer runs out → back, targetable, solid as its class.
        p::set_i16(&mut lv.table.mobys[v].pvars, air_traffic::pv::WRECK_T, 1);
        lv.table.mobys[v].visible = 0;
        run(&mut lv, &mut sink, cam, None, v, air_traffic::update);
        let m = &lv.table.mobys[v];
        assert!(m.mode & 0x41 == 0 && m.mode & mode::TARGETABLE != 0, "class {c}: back");
        eprintln!("class {c}: {nb} bolts, pieces {:?}", air_traffic::pieces(c));
    }
    // The timer out while seen: it stays hidden (the timer is 0 from then on: the game's own behaviour).
    let v = all.iter().copied().find(|&i| lv.table.mobys[i].o_class == 120 && lv.table.mobys[i].state == 1).unwrap();
    p::set_i16(&mut lv.table.mobys[v].pvars, air_traffic::pv::WRECK_T, 1);
    lv.table.mobys[v].visible = 1;
    run(&mut lv, &mut sink, [0.0; 3], None, v, air_traffic::update);
    assert!(lv.table.mobys[v].mode & 0x41 == 0x41);
    // A beam explosion's own hit (type 2 / 1) does not wreck it.
    let v = all.iter().copied().find(|&i| lv.table.mobys[i].o_class == 117 && lv.table.mobys[i].state == 1 && p::i16(&lv.table.mobys[i].pvars, air_traffic::pv::WRECK_T) == 0).unwrap();
    let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: None, flags: 0x81_0001 | 0x20_0000, b18: 2, b19: 1, h1a: 0, damage: Pf::f(1.0), w20: 0 };
    with(&mut lv, [0.0; 3], None, |w| w.deliver_hit(v, &tmpl));
    run(&mut lv, &mut sink, [0.0; 3], None, v, air_traffic::update);
    assert_eq!(p::i16(&lv.table.mobys[v].pvars, air_traffic::pv::WRECK_T), 0);
}

// ---------------------------------------------------------------------------------------------------
// U126: Kerwan's swinging path movers 868, 905, 928 (level 03)

#[test]
fn kerwan_movers_swing_along_their_paths_carry_and_loop_their_sound() {
    use rc_game::moby_update::classes::units::kerwan_mover::{self as km, pv};
    let Some(mut lv) = load(3) else { eprintln!("skipped"); return };
    let mut sink = RecSink { slots: Vec::new() };
    let all: Vec<MobyId> = km::CLASSES.iter().flat_map(|&c| lv.of_class(c)).collect();
    assert_eq!(all.len(), 12);
    let cam = [0.0; 3];
    for &i in &all { run(&mut lv, &mut sink, cam, None, i, km::update); }
    for &i in &all {
        let m = &lv.table.mobys[i];
        let pts: Vec<[f32; 4]> = lv.svc.splines[p::i32(&m.pvars, pv::PATH) as usize].iter().map(|q| q.map(f32::from_bits)).collect();
        let dir = m.pvars[pv::DIR] as i8;
        let start = if dir >= 0 { pts[pts.len() - 1] } else { pts[0] };
        eprintln!("mover {i} class {}: state {}, dir {dir}, {} points, wait {}, start timer {}", m.o_class, m.state, pts.len(), p::i32(&m.pvars, pv::WAIT), p::i32(&m.pvars, pv::START_T));
        assert_eq!(m.position, start, "mover {i}: at its start point");
        assert_eq!(p::i32(&m.pvars, pv::SLOT), -1);
    }
    // One mover through a whole swing: the tick count, the ease, the turn, the wait, and the carry block.
    let v = all.iter().copied().find(|&i| lv.table.mobys[i].state == 1).expect("a mover past its start timer");
    let oc = lv.table.mobys[v].o_class;
    let pts: Vec<[f32; 4]> = lv.svc.splines[p::i32(&lv.table.mobys[v].pvars, pv::PATH) as usize].iter().map(|q| q.map(f32::from_bits)).collect();
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let len = p::ff(&lv.table.mobys[v].pvars, pv::LENGTH);
    assert!((len - ((last[0] - first[0]).powi(2) + (last[1] - first[1]).powi(2) + (last[2] - first[2]).powi(2)).sqrt()).abs() < 1e-3);
    assert!((p::ff(&lv.table.mobys[v].pvars, pv::ACCEL) - len * 4.0 / (240.0 * 240.0)).abs() < 1e-9);
    let dir0 = lv.table.mobys[v].pvars[pv::DIR] as i8;
    let (from, to) = if dir0 >= 0 { (first, last) } else { (last, first) };
    let dist = |a: [f32; 4], b: [f32; 4]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    for k in 1..=240 {
        let before = lv.table.mobys[v].position;
        run(&mut lv, &mut sink, cam, None, v, km::update);
        let m = &lv.table.mobys[v];
        let d = p::v4f(&m.pvars, pv::BLOCK + 0x10);
        assert_eq!(d, [m.position[0] - before[0], m.position[1] - before[1], m.position[2] - before[2], m.position[3] - before[3]], "tick {k}: the carry delta");
        if k == 1 { assert!(dist(m.position, from) < len * 0.01, "tick 1 starts from the far end of the swing"); }
        if k == 120 { assert!((dist(m.position, from) - len / 2.0).abs() < len * 0.01, "half way at tick 120"); }
    }
    let m = &lv.table.mobys[v];
    assert!(dist(m.position, to) < 1e-3, "at the end after ticks(240)");
    assert_eq!((m.state, m.pvars[pv::DIR] as i8, p::i32(&m.pvars, pv::SLOT)), (2, -dir0, -1));
    assert_eq!(p::i32(&m.pvars, pv::WAIT_T), p::i32(&m.pvars, pv::WAIT));
    let snd: Vec<(i32, u32, i16)> = lv.svc.sounds.iter().filter(|s| s.moby == v).map(|s| (s.index, s.flags, s.sound_class)).collect();
    if oc == 928 { assert!(snd.is_empty()); } else { assert_eq!(snd, vec![(0, 4, km::SOUND_CLASS)], "class {oc}: class 905's loop once, kept alive"); }
    let wait = p::i32(&lv.table.mobys[v].pvars, pv::WAIT);
    for _ in 0..wait.max(1) { run(&mut lv, &mut sink, cam, None, v, km::update); }
    assert_eq!((lv.table.mobys[v].state, p::i32(&lv.table.mobys[v].pvars, pv::TICK)), (1, 0), "back after the wait");
    // The other classes' loop sounds: 868 and 905 play class 905's sound 0, 928 none.
    for &c in &km::CLASSES {
        let Some(i) = all.iter().copied().find(|&i| lv.table.mobys[i].o_class == c && lv.table.mobys[i].state == 1 && i != v) else { continue };
        let n0 = lv.svc.sounds.len();
        run(&mut lv, &mut sink, cam, None, i, km::update);
        let new: Vec<i16> = lv.svc.sounds[n0..].iter().map(|s| s.sound_class).collect();
        assert_eq!(new, if c == 928 { vec![] } else { vec![km::SOUND_CLASS] }, "class {c}");
    }
}
