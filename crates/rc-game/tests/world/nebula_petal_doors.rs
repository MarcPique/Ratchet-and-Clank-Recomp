//! Nebula G34's airlock petal doors (class 1021) driven by the Clank lift (class 1061), headless on the level
//! (docs/plan/creatures.md §1, as novalis_world.rs): the lift's field_open / field_lower / field_close must act on
//! the door's own seven petals (the pvar slots +0x24..+0x3c hold the moby index + 1, 0 = none) — a raw read skips
//! the door's first petal and hits the next door's, desyncing the airlock doors by one petal each once the Clank
//! section runs. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gameplay, level, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId, MobyTable};
use rc_game::moby_update::classes::units::blarg_clank_lift::{field_close, field_lower, field_open};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{pvar as p, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::ps2v::Pf;
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
    let ports = crate::common::ports(level, &[])?;
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
            let info = class_info(&c, slot as u8, ports.update_fn(oc));
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: ports.update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    Some(Lv { mesh, instances, pvars, tests, gp: gp.to_vec(), classes, spawnable, death_z })
}

struct Sim {
    game: Game,
    svc: Services,
    sched: Scheduler,
    classes: ClassTable,
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
        {
            let hero: Hero = game.hero.clone();
            let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &classes, &mut svc, 0);
            w.camera = game.camera.out.pos;
            w.coll = Some(&lv.mesh);
            sched.load_pass(&mut w);
        }
        game.finish_load();
        Sim { game, svc, sched, classes }
    }

    fn tick(&mut self, cam: [f32; 3]) {
        let hero: Hero = self.game.hero.clone();
        let counter = self.game.counter;
        let mut w = World::new(&mut self.game.mobys, &hero, &mut self.game.rng, &self.classes, &mut self.svc, counter);
        w.camera = [Pf::f(cam[0]), Pf::f(cam[1]), Pf::f(cam[2]), Pf::ZERO];
        self.sched.tick(&mut w);
        self.game.counter += 1;
    }

    fn with_world<R>(&mut self, f: impl FnOnce(&mut World) -> R) -> R {
        let hero: Hero = self.game.hero.clone();
        let counter = self.game.counter;
        let mut w = World::new(&mut self.game.mobys, &hero, &mut self.game.rng, &self.classes, &mut self.svc, counter);
        f(&mut w)
    }

    fn doors_at(&self, pos: [f32; 2]) -> MobyId {
        self.game.mobys.mobys.iter().enumerate().find(|(_, m)| m.o_class == 1021 && m.state < 0x80 && (m.position[0] - pos[0]).abs() < 1.0 && (m.position[1] - pos[1]).abs() < 1.0).map(|(i, _)| i).expect("door")
    }
}

/// The door's seven petals (the slots hold the moby index + 1).
fn petals(sim: &Sim, d: MobyId) -> Vec<MobyId> { (0..7).map(|k| (p::i32(&sim.game.mobys.mobys[d].pvars, 0x24 + 4 * k) - 1) as usize).collect() }

fn assert_open(sim: &Sim, d: MobyId, what: &str) {
    assert_eq!((sim.game.mobys.mobys[d].state, sim.game.mobys.mobys[d].mode & mode::HIDDEN), (6, mode::HIDDEN), "{what}: the door open and hidden");
    for (k, &q) in petals(sim, d).iter().enumerate() {
        assert_eq!((sim.game.mobys.mobys[q].state, sim.game.mobys.mobys[q].mode & mode::HIDDEN), (7, mode::HIDDEN), "{what}: petal {k} (#{q}) open and hidden");
    }
}

fn assert_shut(sim: &Sim, d: MobyId, what: &str) {
    assert_eq!((sim.game.mobys.mobys[d].state, sim.game.mobys.mobys[d].mode & mode::HIDDEN), (2, 0), "{what}: the door shut");
    for (k, &q) in petals(sim, d).iter().enumerate() {
        assert_eq!((sim.game.mobys.mobys[q].state, sim.game.mobys.mobys[q].mode & mode::HIDDEN), (3, 0), "{what}: petal {k} (#{q}) shut");
    }
}

#[test]
fn nebula_airlock_doors_stay_in_sync_through_the_clank_section() {
    let Some(lv) = load(6) else { eprintln!("skipped: no extracted/"); return };
    let mut sim = Sim::new(&lv, 6);
    // The airlock: instances 575 / 576 / 577, the lift's +0x60 / +0x64 / +0x68.
    let d575 = sim.doors_at([221.64, 147.37]);
    let d576 = sim.doors_at([220.12, 152.74]);
    let d577 = sim.doors_at([212.46, 158.18]);
    let lift = sim.game.mobys.mobys.iter().enumerate().find(|(_, m)| m.o_class == 1061).map(|(i, _)| i).expect("lift");
    let (l60, l64, l68) = sim.with_world(|w| {
        let g = |o: usize| usize::try_from(p::i32(&w.m(lift).pvars, o)).ok();
        (g(0x60), g(0x64), g(0x68))
    });
    assert_eq!((l60, l64, l68), (Some(d575), Some(d576), Some(d577)), "the lift's linked doors");
    let cam = [221.0, 150.0, 139.0];
    for _ in 0..5 { sim.tick(cam); }
    for &d in &[d575, d576, d577] { assert_shut(&sim, d, "after load"); }

    // The Trespasser lock (class 615 by door 575) solved: door 575's own link trigger opens it.
    let lock = sim.game.mobys.mobys.iter().enumerate().find(|(_, m)| m.o_class == 615 && (m.position[0] - 225.14).abs() < 1.0 && (m.position[1] - 146.59).abs() < 1.0).map(|(i, _)| i).expect("lock");
    sim.game.mobys.mobys[lock].state = 4;
    for _ in 0..150 { sim.tick(cam); }
    assert_open(&sim, d575, "lock solved");

    // Ratchet on the pad without the O2 mask (the lift's state 3, the !o2 branch): field_lower(l64),
    // field_close(l68), field_open(l60), then the switch to Clank.
    sim.with_world(|w| field_lower(w, l64));
    sim.with_world(|w| field_close(w, l68));
    sim.with_world(|w| field_open(w, l60));
    for _ in 0..150 { sim.tick(cam); }
    assert_shut(&sim, d575, "entry: l60 reset");
    assert_open(&sim, d576, "entry: l64 lowered");
    assert_shut(&sim, d577, "entry: l68 shut");

    // Clank back on the pad (the body == CLANK branch): field_open(l64), field_open(l68), field_lower(l60).
    sim.with_world(|w| field_open(w, l64));
    sim.with_world(|w| field_open(w, l68));
    sim.with_world(|w| field_lower(w, l60));
    for _ in 0..150 { sim.tick(cam); }
    assert_open(&sim, d575, "exit: l60 lowered");
    assert_shut(&sim, d576, "exit: l64 reset");
    assert_shut(&sim, d577, "exit: l68 reset");
}
