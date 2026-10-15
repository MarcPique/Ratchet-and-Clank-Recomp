//! Novalis's non-enemy world classes headless (docs/plan/creatures.md §1, "World props and effects"): the level's
//! static mobys as the loader creates them on a first visit, the scheduler's load pass and moby loop (with the
//! level's volumes), driven without the hero tick. Skipped when `extracted/` is absent.
//!
//! * The waterfall foam / mist fields 760 and their scroll 809 (`moby_update::classes::fire_field`): the init, the element
//!   life cycle through the draw callback, the draws on the stream.

use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gameplay, level, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{MobyId, MobyTable};
use rc_game::moby_update::classes::fire_field::FireFieldState;
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::World;
use rc_game::moby_update::{ClassTable, Services};
use rc_game::ps2v::Pf;
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions};

struct Lv {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    tests: Vec<moby_spawn::SpawnTest>,
    gp: Vec<u8>,
    classes: ClassTable,
    spawnable: usize,
    death_z: f32,
}

fn load(level: u32) -> Option<Lv> {
    let dir = rc_formats::test_data::root().join(format!("levels/{level:02}"));
    let data = rc_formats::test_data::core_data(level)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let settings = rc_formats::test_data::gameplay_section(level, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(level, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: scheduler::port_update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    Some(Lv { mesh, instances, pvars, tests, gp: gp.to_vec(), classes, spawnable, death_z })
}

/// The level after its load pass, ticked through the scheduler alone (no hero update, no particles).
struct Sim {
    game: Game,
    svc: Services,
    sched: Scheduler,
    classes: ClassTable,
    load_draws: u64,
}

fn draws(from: u32, to: u32) -> u64 {
    let mut r = Rng::new();
    r.state = from;
    let mut n = 0;
    while r.state != to {
        r.rand();
        n += 1;
        assert!(n < 10_000_000, "stream distance too large");
    }
    n
}

impl Sim {
    fn new(lv: &Lv, level: u32) -> Sim {
        let mut ct = ClassTable { classes: lv.classes.classes.clone() };
        let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
        let mobys = statics.mobys.clone();
        let hero_idx = mobys.iter().position(|m| m.o_class == 0).expect("Ratchet");
        let table = MobyTable::new(mobys, lv.spawnable);
        let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
        let mut svc = Services::new();
        svc.level = level;
        svc.groups = statics.groups(&lv.gp);
        svc.set_volumes(rc_formats::volumes::parse_volumes(&lv.gp).unwrap());
        let classes = ClassTable { classes: lv.classes.classes.clone() };
        let mut sched = Scheduler::new();
        let r0 = game.rng.state;
        {
            let hero: Hero = game.hero.clone();
            let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &classes, &mut svc, 0);
            w.camera = game.camera.out.pos;
            w.coll = Some(&lv.mesh);
            sched.load_pass(&mut w);
        }
        let load_draws = draws(r0, game.rng.state);
        game.finish_load();
        Sim { game, svc, sched, classes, load_draws }
    }

    /// One moby loop with the camera at `cam` (no view: nothing is culled); returns its draws.
    fn tick(&mut self, cam: [f32; 3]) -> u64 {
        let r0 = self.game.rng.state;
        let hero: Hero = self.game.hero.clone();
        let counter = self.game.counter;
        let mut w = World::new(&mut self.game.mobys, &hero, &mut self.game.rng, &self.classes, &mut self.svc, counter);
        w.camera = [Pf::f(cam[0]), Pf::f(cam[1]), Pf::f(cam[2]), Pf::ZERO];
        self.sched.tick(&mut w);
        self.game.counter += 1;
        draws(r0, self.game.rng.state)
    }

    /// A hit record for `id` (`FUN_0026e968`), as the wrench or a shot leaves it before the moby loop.
    fn hit(&mut self, id: MobyId, t: &rc_game::moby_update::services::HitTemplate) {
        let hero: Hero = self.game.hero.clone();
        let counter = self.game.counter;
        let mut w = World::new(&mut self.game.mobys, &hero, &mut self.game.rng, &self.classes, &mut self.svc, counter);
        w.deliver_hit(id, t);
    }

    fn of_class(&self, oc: i16) -> Vec<MobyId> { self.game.mobys.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
}

// ---------------------------------------------------------------------------------------------------
// Fire fields 760 / smoke scroll 809

#[test]
fn novalis_fire_fields_init_like_the_game() {
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/"); return };
    let sim = Sim::new(&lv, 1);
    let f = &sim.svc.fire_fields;
    // 809's init (0x2ba658): the periods of 60 ticks.
    assert_eq!((f.wait_ticks, f.fade_in_ticks, f.fade_out_ticks), (60, 60, 60));
    assert_eq!(f.inv_fade_in, 1.0 / 60.0);
    // The four fields take their ranges in instance order (RAM, both savestates: P+0x00 = 0, 24, 68, 112 and the
    // allocator 0x161390 = 136).
    let fields = sim.of_class(760);
    assert_eq!(fields.len(), 4);
    let starts: Vec<i16> = fields.iter().map(|&id| rc_game::moby_update::services::pvar::i16(&sim.game.mobys.mobys[id].pvars, 0)).collect();
    assert_eq!(starts, vec![0, 24, 68, 112]);
    assert_eq!(f.alloc, 136);
    for &id in &fields {
        let pv = &sim.game.mobys.mobys[id].pvars;
        let m = &sim.game.mobys.mobys[id];
        assert_eq!((m.state, m.update_dist), (1, 0xff));
        // +0x38: the RGB word the init packs (RAM: 0x00204080 on all four).
        assert_eq!(rc_game::moby_update::services::pvar::u32(pv, 0x38), 0x0020_4080);
        let n_foam = rc_game::moby_update::services::pvar::i16(pv, 2) as usize;
        for (j, i) in FireFieldState::range(pv).enumerate() {
            let e = &f.elems[i];
            assert!(e.pos.iter().take(3).all(|x| (-1.0..=1.0).contains(x)) && e.pos[3] == 1.0);
            if j < n_foam {
                // Every foam element starts in its wait: (j + 1)·60 ticks, fade-in cleared.
                assert_eq!((e.kind, e.fade_in, e.fade_out, e.wait), (0, 0, 0, ((j + 1) * 60) as i16), "element {i}");
            } else {
                assert_eq!(e.kind, 1);
            }
        }
    }
    // P+0x44 / +0x48 / +0x50 of the first field as RAM holds them (0x3fed2eeb, 0x3fed2eeb, 0x3d4f6477).
    let pv = &sim.game.mobys.mobys[fields[0]].pvars;
    let ff = |o| rc_game::moby_update::services::pvar::ff(pv, o);
    assert!((ff(0x48) - f32::from_bits(0x3fed_2eeb)).abs() < 1e-6 && ff(0x44) == ff(0x48));
    assert!((ff(0x50) - f32::from_bits(0x3d4f_6477)).abs() < 1e-6);
    eprintln!("load pass draws {} (fire fields: 136 elements x 7 = 952)", sim.load_draws);
}

#[test]
fn novalis_fire_field_flames_cycle_every_j_plus_3_seconds() {
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/"); return };
    let run = |ticks: u32| {
        let mut sim = Sim::new(&lv, 1);
        let fields = sim.of_class(760);
        let (lo, hi) = (FireFieldState::range(&sim.game.mobys.mobys[fields[1]].pvars).start, FireFieldState::range(&sim.game.mobys.mobys[fields[1]].pvars).end);
        let mut respawn_ticks: Vec<Vec<u32>> = vec![Vec::new(); hi - lo];
        let mut per_tick = Vec::new();
        let mut last = sim.svc.fire_fields.respawns;
        for t in 0..ticks {
            let before: Vec<(i16, i16)> = (lo..hi).map(|i| (sim.svc.fire_fields.elems[i].fade_out, sim.svc.fire_fields.elems[i].fade_in)).collect();
            let d = sim.tick([100.0, 250.0, 60.0]);
            for (k, i) in (lo..hi).enumerate() {
                let e = &sim.svc.fire_fields.elems[i];
                // A respawn sets the fade-in to 60 out of the fade-out.
                if before[k].0 != 0 && e.fade_in == 60 { respawn_ticks[k].push(t); }
            }
            let r = sim.svc.fire_fields.respawns;
            per_tick.push((d, r - last));
            last = r;
        }
        (respawn_ticks, per_tick, sim.svc.fire_fields.elems.clone(), sim.game.rng.state)
    };
    let (respawns, per_tick, elems, rng) = run(1500);
    // Field 2 (40 foam elements): element j respawns every (j + 3)·60 ticks.
    for (j, ts) in respawns.iter().enumerate().take(40) {
        assert!(!ts.is_empty() || (j + 2) * 60 >= 1500, "element {j} never respawned");
        for w in ts.windows(2) { assert_eq!(w[1] - w[0], ((j + 3) * 60) as u32, "element {j}: {ts:?}"); }
    }
    // Every respawn is 7 draws of the tick (6 in `respawn`, the new mirror flag).
    let foam_draws: u64 = per_tick.iter().map(|(_, n)| n * 7).sum();
    let total: u64 = per_tick.iter().map(|(d, _)| d).sum();
    eprintln!("1500 ticks: {} respawns, {total} draws in all ({foam_draws} by the fire fields)", per_tick.iter().map(|p| p.1).sum::<u64>());
    assert!(foam_draws > 0 && foam_draws <= total);
    // Deterministic.
    let (r2, p2, e2, rng2) = run(1500);
    assert_eq!((respawns, per_tick, elems, rng), (r2, p2, e2, rng2));
}

#[test]
fn fire_fields_on_levels_0_and_14_init() {
    for level in [0u32, 14] {
        let Some(lv) = load(level) else { eprintln!("skipped: no extracted/levels/{level:02}"); continue };
        let mut sim = Sim::new(&lv, level);
        let fields = sim.of_class(760);
        assert!(!fields.is_empty(), "level {level}: no 760");
        let total: usize = fields.iter().map(|&id| FireFieldState::range(&sim.game.mobys.mobys[id].pvars).len()).sum();
        assert_eq!(sim.svc.fire_fields.alloc as usize, total, "level {level}");
        assert!(total <= rc_game::moby_update::classes::fire_field::MAX_ELEMENTS);
        let at_load = sim.svc.fire_fields.respawns;
        for _ in 0..200 { sim.tick([0.0, 0.0, 0.0]); }
        eprintln!("level {level}: {} fields, {total} elements, {at_load} set up at load, {} respawns in 200 ticks, unported {:?}", fields.len(), sim.svc.fire_fields.respawns - at_load, sim.svc.fx.part_spawns);
    }
}

// ---------------------------------------------------------------------------------------------------
// The waterfall foam: 751's zone-5 spray (particle types 57 / 56)

#[test]
fn novalis_waterfall_foam_spawns_rings_and_mist() {
    let root = rc_formats::test_data::root();
    let Ok(ov_bytes) = std::fs::read(root.join("levels/01/overlay.bin")) else { eprintln!("skipped: no extracted/"); return };
    let Some(gp) = rc_formats::test_data::gameplay(1) else { return };
    let ov = rc_formats::water::Overlay::parse(&ov_bytes).unwrap();
    let lo = rc_formats::level_overlay::LevelOverlay::parse(&ov_bytes).unwrap();
    let rel = rc_formats::level_overlay::Relocation::new(&lo, &lo);
    let (a, m) = (rc_formats::water::Ripple751Addrs::locate(&rel).unwrap(), rc_formats::water::RippleModuleAddrs::locate(&rel).unwrap());
    let tables = rc_formats::water::parse_ripple_tables(&ov, &a, &m).unwrap();
    let cuboids = rc_formats::water::parse_cuboids(&gp).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let pv = instances.iter().find(|m| m.o_class == 751).and_then(|m| m.pvar(&pvars)).unwrap();
    let zones = rc_formats::water::ripple_zone_cuboids(pv, tables.zones.len()).unwrap();
    let run = || {
        let mut rng = Rng::new();
        let mut sim = rc_game::water::RippleSim::new(&tables, zones.clone(), [0.0, 0.0], &mut rng);
        let mut parts = rc_game::particles::Particles::new(None, Vec::new());
        // The camera in zone 5 (centre (131.8, 199, 90)) above the pool at the foot of the fall.
        let cam = [150.0, 200.0, 60.0];
        let (mut rings, mut puffs, mut draws) = (0u64, 0u64, 0u64);
        for t in 0..600u64 {
            let r0 = rng.state;
            let info = sim.tick_with(cam, &cuboids, &mut rng, t, Some(&mut parts));
            assert_eq!(info.zone, 5);
            rings += 1;
            puffs += info.mist as u64;
            draws += super_draws(r0, rng.state);
            parts.update_parts(&mut rng);
        }
        let alive = |ty: u8| (0..=parts.pool.hw.max(-1)).filter(|&i| { let r = &parts.pool.recs[i as usize]; r[0] == ty && r[1] & 0x80 == 0 }).count();
        (rings, puffs, draws, alive(57), alive(56), rng.state)
    };
    let (rings, puffs, draws, a57, a56, state) = run();
    eprintln!("600 ticks in zone 5: {rings} rings, {puffs} mist puffs, {draws} draws; alive: {a57} rings, {a56} puffs");
    // A ring lives 128 updates: one a tick keeps 127 alive after the tick's update. Puffs: 20 tries a tick at 1/32.
    assert_eq!(a57, 127);
    assert!(puffs > 600 * 20 / 32 / 2 && puffs < 600 * 20 / 32 * 2, "{puffs}");
    // Draws: 751's own drops aside, 3 + 2 per ring, 20 (or 21) `randi(32)` and 4 + 2 per puff.
    assert!(draws >= rings * 5 + puffs * 6 + 600 * 20, "{draws}");
    assert_eq!(state, run().5, "deterministic");
}

fn super_draws(from: u32, to: u32) -> u64 { draws(from, to) }

// ---------------------------------------------------------------------------------------------------
// Props: spinners 705, elevators 703 / 715, sliding doors 768 / 769

#[test]
fn novalis_props_move_like_the_game() {
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/"); return };
    let run = || {
        let mut sim = Sim::new(&lv, 1);
        let spinners = sim.of_class(705);
        let lifts: Vec<MobyId> = [703, 715].iter().flat_map(|&c| sim.of_class(c)).collect();
        let doors: Vec<MobyId> = [768, 769].iter().flat_map(|&c| sim.of_class(c)).collect();
        assert_eq!((spinners.len(), lifts.len(), doors.len()), (5, 7, 8));
        // Ratchet next to the first door (its home = its placed position).
        let d0 = sim.game.mobys.mobys[doors[0]].position;
        sim.game.hero.pos = [Pf::f(d0[0]), Pf::f(d0[1]), Pf::f(d0[2]), Pf::ONE];
        let z0: Vec<f32> = lifts.iter().map(|&i| sim.game.mobys.mobys[i].position[2]).collect();
        let (mut zs, mut yaws, mut door_states) = (Vec::new(), Vec::new(), Vec::new());
        let s0 = sim.game.mobys.mobys[spinners[0]].position;
        for t in 0..900 {
            // The camera at the door, then at the first spinner (mobys update within their distance of it).
            let cam = if t < 450 { [d0[0] + 10.0, d0[1], d0[2] + 3.0] } else { [s0[0] + 8.0, s0[1], s0[2] + 3.0] };
            sim.tick(cam);
            if t == 450 {
                // Ratchet walks away: the doors close again.
                sim.game.hero.pos = [Pf::f(d0[0] + 40.0), Pf::f(d0[1]), Pf::f(d0[2]), Pf::ONE];
            }
            zs.push(lifts.iter().map(|&i| sim.game.mobys.mobys[i].position[2]).collect::<Vec<_>>());
            yaws.push(spinners.iter().map(|&i| sim.game.mobys.mobys[i].rotation[2]).collect::<Vec<_>>());
            door_states.push(sim.game.mobys.mobys[doors[0]].state);
        }
        (z0, zs, yaws, door_states, sim.game.rng.state)
    };
    let (z0, zs, yaws, door_states, rng) = run();
    // Every lift leaves its home height and comes back to it.
    for k in 0..z0.len() {
        let moved = zs.iter().any(|z| (z[k] - z0[k]).abs() > 1.0);
        let back = zs.iter().skip(1).any(|z| z[k] == z0[k]);
        assert!(moved && back, "lift {k}: home {} range {:?}", z0[k], zs.iter().map(|z| z[k]).fold((f32::MAX, f32::MIN), |a, z| (a.0.min(z), a.1.max(z))));
    }
    // The spinners turn (and reverse after their 90-tick hold at full speed).
    assert!(yaws[450..].windows(2).any(|w| w[0][0] != w[1][0]));
    // The door opens (2 → 3) while Ratchet is there, then closes (4 → 1) once he and the camera are away.
    let seq: Vec<u8> = door_states.iter().fold(Vec::new(), |mut v, &s| { if v.last() != Some(&s) { v.push(s); } v });
    eprintln!("door states {seq:?}");
    assert!(seq.windows(4).any(|w| w == [1, 2, 3, 4]) || seq.windows(3).any(|w| w == [2, 3, 4]), "{seq:?}");
    assert_eq!(*seq.last().unwrap(), 1);
    assert_eq!(rng, run().4, "deterministic");
}

// ---------------------------------------------------------------------------------------------------
// Breakables: rocks 704, shell walls 709–711, the big wall 729, the pipe 778 and its spray 779

#[test]
fn novalis_breakables_burst_into_rock_bits() {
    use rc_game::moby_update::services::HitTemplate;
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/"); return };
    let mut sim = Sim::new(&lv, 1);
    let bits = |sim: &Sim| sim.game.mobys.mobys.iter().filter(|m| (696..=698).contains(&m.o_class) && m.state < 0x80).count();
    let rock = sim.of_class(704)[0];
    let at = sim.game.mobys.mobys[rock].position;
    let cam = [at[0] + 5.0, at[1], at[2] + 2.0];
    sim.tick(cam);
    // The wrench: flags 0x10000, damage 1.
    sim.hit(rock, &HitTemplate { flags: 0x1_0000, damage: Pf::ONE, ..Default::default() });
    let d = sim.tick(cam);
    assert!(sim.game.mobys.mobys[rock].state >= 0x80, "the rock is deleted");
    assert_eq!(bits(&sim), 40);
    assert_eq!(sim.svc.fx.part_spawns.get(&22).copied(), Some(200));
    eprintln!("rock burst: {d} draws");
    // A shell wall: only a gunship shell (class 686) breaks it.
    let wall = sim.of_class(709)[0];
    let at = sim.game.mobys.mobys[wall].position;
    let cam = [at[0] + 5.0, at[1], at[2] + 2.0];
    sim.hit(wall, &HitTemplate { flags: 0x80_0000, damage: Pf::ONE, ..Default::default() });
    sim.tick(cam);
    assert!(sim.game.mobys.mobys[wall].state < 0x80, "a hit without a shell leaves the wall");
    let shell = (0..sim.game.mobys.mobys.len()).find(|&i| sim.game.mobys.mobys[i].o_class == 686);
    if let Some(shell) = shell {
        sim.hit(wall, &HitTemplate { attacker: Some(shell), flags: 0x80_0000, damage: Pf::ONE, ..Default::default() });
    } else {
        // No shell moby in the table yet: make one the way the gunship does.
        let hero: Hero = sim.game.hero.clone();
        let mut w = World::new(&mut sim.game.mobys, &hero, &mut sim.game.rng, &sim.classes, &mut sim.svc, 0);
        let s = w.create_moby(686).expect("a free slot");
        w.deliver_hit(wall, &HitTemplate { attacker: Some(s), flags: 0x80_0000, damage: Pf::ONE, ..Default::default() });
    }
    let before = bits(&sim);
    sim.tick(cam);
    assert!(sim.game.mobys.mobys[wall].state >= 0x80, "a shell breaks it");
    assert!(bits(&sim) >= before + 20 - 2, "20 bits (a few of the rock's may have expired)");
    // The pipe 778 leaves its spray 779 behind, which puffs two type-22 records a tick.
    let pipe = sim.of_class(778)[0];
    let at = sim.game.mobys.mobys[pipe].position;
    let cam = [at[0] + 5.0, at[1], at[2] + 2.0];
    sim.hit(pipe, &HitTemplate { flags: 0x1_0000, damage: Pf::ONE, ..Default::default() });
    sim.tick(cam);
    let spray = sim.of_class(779);
    assert_eq!(spray.len(), 1);
    assert_eq!(sim.game.mobys.mobys[spray[0]].position, at);
    let n0 = sim.svc.fx.part_spawns[&22];
    sim.tick(cam);
    assert_eq!(sim.svc.fx.part_spawns[&22], n0 + 2);
    // A pot: the wrench → state 2 → four shards 1817, the remains 1816 and bolts.
    let count = |sim: &Sim, c: i16| sim.of_class(c).len();
    let pot = sim.of_class(754)[0];
    let at = sim.game.mobys.mobys[pot].position;
    let cam = [at[0] + 5.0, at[1], at[2] + 2.0];
    // (BreakFxA drops the bolts only with Ratchet within 7: `BoltBurst` with flags 0 drops none.)
    sim.game.hero.pos = [Pf::f(at[0] + 2.0), Pf::f(at[1]), Pf::f(at[2]), Pf::ONE];
    let (shards, remains, bolts) = (count(&sim, 1817), count(&sim, 1816), (13..=16).map(|c| count(&sim, c)).sum::<usize>());
    sim.hit(pot, &HitTemplate { flags: 0x1_0000, damage: Pf::ONE, ..Default::default() });
    sim.tick(cam);
    sim.tick(cam);
    assert!(sim.game.mobys.mobys[pot].state >= 0x80, "the pot is deleted");
    assert_eq!(count(&sim, 1817), shards + 4);
    assert_eq!(count(&sim, 1816), remains + 1);
    assert!((13..=16).map(|c| count(&sim, c)).sum::<usize>() > bolts, "bolts");
}

#[test]
fn novalis_collapsing_platform_falls_once_ratchet_is_on_it() {
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/"); return };
    let mut sim = Sim::new(&lv, 1);
    let plat = sim.of_class(701)[0];
    let home = sim.game.mobys.mobys[plat].position;
    let c = rc_game::moby_update::services::pvar::i32(&sim.game.mobys.mobys[plat].pvars, 0x80);
    let ctr = sim.svc.volumes.cuboids[c as usize].matrix[3];
    // Ratchet on the trigger (above the cuboid's centre, within 10 in xy), on foot (movement group 0).
    sim.game.hero.pos = [Pf::f(ctr[0]), Pf::f(ctr[1]), Pf::f(ctr[2] + 0.5), Pf::ONE];
    let cam = [home[0] + 6.0, home[1], home[2] + 3.0];
    let mut states = Vec::new();
    let mut shakes = 0;
    for _ in 0..400 {
        sim.tick(cam);
        shakes += sim.svc.camera_shakes.len();
        sim.svc.camera_shakes.clear();
        states.push(sim.game.mobys.mobys[plat].state);
    }
    let seq: Vec<u8> = states.iter().fold(Vec::new(), |mut v, &s| { if v.last() != Some(&s) { v.push(s); } v });
    let m = &sim.game.mobys.mobys[plat];
    eprintln!("states {seq:?}, z {} -> {}, shakes {shakes}", home[2], m.position[2]);
    // (Triggered on the first tick: state 1 was the load pass's.)
    assert_eq!(seq, vec![2, 3, 4]);
    assert_eq!(m.position[2], home[2] - 20.0);
    // The three platforms share the trigger cuboid 0x12 and fall one after the other (waits 45 / 60 / 73 ticks):
    // a rumble and a landing each.
    assert!(sim.of_class(701).iter().all(|&i| sim.game.mobys.mobys[i].state == 4));
    assert_eq!(shakes, 6);
    assert!(sim.svc.save.death.contains(&(1, m.spawn_id)));
}
