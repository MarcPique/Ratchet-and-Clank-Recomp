//! The Pyrocitor on Novalis, headless (crate::hero::pyrocitor): the level as the engine ticks it (the loader's spawn
//! test, the scheduler's load pass and moby loop, the moby collision, the particles with the hero's spawns, the tick's
//! hit path) with the game state of a first Novalis arrival. The vendor's purchase (`GiveItem(16, equip)`, what the
//! vendor screen's buy flow calls; the screen itself: `interaction_vendor.rs`) puts the Pyrocitor with its ammo into
//! the hand; then ○ fires it standing (the stance, the flames, the light, the pilot flame, the ammo), running at a crate
//! (Ratchet keeps running, the flames break the crate) and in first person (the flames leave from below the eye along
//! the view). Every run twice: identical. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::save_game::{ChunkTables, ItemTables, SaveGameLump};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::game_state::{GameState, SessionState};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::items::{ItemClass, ItemData, ItemDef, WeaponDef, HERO_LISTS};
use rc_game::hero::pyrocitor::{GLOW_CLASS, PYROCITOR};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{ServiceHits, SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
use rc_game::particles::Particles;
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

struct Lv {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    tests: Vec<moby_spawn::SpawnTest>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    spawnable: usize,
    death_z: f32,
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    ratchet: MobyAnimClass,
    seconds: Vec<Vec<u8>>,
    items: ItemData,
    weapon_defs: Vec<WeaponDef>,
    tables: ItemTables,
    state: GameState,
    session: SessionState,
}

fn load() -> Option<Lv> {
    let root = rc_formats::test_data::root();
    let dir = root.join("levels/01");
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let settings = rc_formats::test_data::gameplay_section(1, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let splines = gameplay::parse_splines(&gp).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: scheduler::port_update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    let ratchet_blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&ratchet_blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    let seconds = (0..64).map(|l| gadget::joint_list(&ratchet_blob, &class.header, l).map(|(_, b)| b).unwrap_or_default()).collect();
    // The item definitions and their weapon fields from the level overlay, as the engine reads them.
    let elf = std::fs::read(root.join("boot/SCUS_971.99")).ok()?;
    let ov0 = std::fs::read(root.join("levels/00/overlay.bin")).ok()?;
    let ov1 = std::fs::read(dir.join("overlay.bin")).ok()?;
    let tables = ItemTables::load(&elf, &ov1).unwrap();
    let sections = rc_formats::font::parse_overlay_sections(&ov1).unwrap();
    let (n, sz) = (rc_formats::save_game::ITEM_COUNT, rc_formats::save_game::ITEM_DEF_SIZE);
    let raw = rc_formats::font::read_overlay(&sections, tables.item_defs_addr, n * sz).unwrap().to_vec();
    let w = |i: usize, o: usize| i32::from_le_bytes(raw[i * sz + o..i * sz + o + 4].try_into().unwrap());
    let defs = (0..n).map(|i| ItemDef { slot: w(i, 8), attach: w(i, 0xc), o_class: w(i, 0x10), b18: raw[i * sz + 0x18] }).collect();
    let weapon_defs = (0..n).map(|i| WeaponDef { w18: w(i, 0x18), anims: [w(i, 0x24), w(i, 0x28), w(i, 0x2c)], w30: w(i, 0x30) }).collect();
    let hero_chains = HERO_LISTS.iter().map(|&l| gadget::joint_list(&ratchet_blob, &class.header, l).map(|(a, _)| a).unwrap_or_default()).collect();
    let item_classes = gadget::parse_gadget_classes(&core, &data)
        .unwrap()
        .iter()
        .map(|g| {
            let c = &g.moby.class;
            let chains = (0..16).map_while(|l| gadget::joint_list(&g.blob, &c.header, l).ok().map(|(a, _)| a)).collect();
            ItemClass { o_class: g.moby.o_class as i16, anim: MobyAnimClass::new(c, parse_sequences(&g.blob, c).unwrap_or_default()), scale: c.header.scale, chains }
        })
        .collect();
    let items = ItemData { defs, hero_chains, classes: item_classes };
    // The game state of a first Novalis arrival (game_state.md §4) with bolts to spend.
    let lump = SaveGameLump::parse(&std::fs::read(root.join("global/save_game.bin")).ok()?).unwrap();
    let ct = ChunkTables::from_boot_elf(&elf).unwrap();
    let mut state = GameState::new_game(ct, &lump.template).unwrap();
    let mut session = SessionState::default();
    state.apply_level_start(0, &ItemTables::load(&elf, &ov0).unwrap(), &mut session);
    state.on_veldin_clank_init(&mut session);
    state.apply_transition(1);
    state.apply_level_start(1, &tables, &mut session);
    state.global.bolts = 5000;
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, seconds, items, weapon_defs, tables, state, session })
}

/// The hero's hit sink over the moby loop's services, as the engine's `CellHits`.
struct CellHits<'a, 'b> {
    svc: &'a std::cell::RefCell<&'b mut Services>,
    classes: &'a ClassTable,
    coll: &'a collision::Collision,
}

impl CellHits<'_, '_> {
    fn with<R>(&mut self, f: impl FnOnce(&mut ServiceHits) -> R) -> R {
        let mut s = self.svc.borrow_mut();
        f(&mut ServiceHits { svc: &mut s, classes: self.classes, coll: Some(self.coll) })
    }
}

impl rc_game::hero::items::HitSink for CellHits<'_, '_> {
    fn sphere(&mut self, table: &mut MobyTable, r: rc_game::ps2v::Pf, centre: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &rc_game::moby_update::services::HitTemplate) -> Option<usize> {
        self.with(|h| h.sphere(table, r, centre, flags, ignore, tmpl))
    }
    fn line(&mut self, table: &mut MobyTable, a: rc_game::hero::physics::V4, b: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &rc_game::moby_update::services::HitTemplate) -> Option<Option<usize>> {
        self.with(|h| h.line(table, a, b, flags, ignore, tmpl))
    }
    fn deliver(&mut self, table: &mut MobyTable, target: usize, tmpl: &rc_game::moby_update::services::HitTemplate) {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::deliver_hit_in(table, &mut s.hits, target, tmpl);
    }
    fn create_moby(&mut self, table: &mut MobyTable, o_class: i16, counter: u64) -> Option<usize> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::classes::bomb::create_from_hero(table, &mut s, self.classes, o_class, counter)
    }
    fn delete_moby(&mut self, table: &mut MobyTable, id: usize, counter: u64) {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::classes::bomb::delete_from_hero(table, &mut s, id, counter);
    }
    fn probe(&mut self, table: &mut MobyTable, a: rc_game::hero::physics::V4, b: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>) -> Option<Option<[f32; 3]>> {
        self.with(|h| h.probe(table, a, b, flags, ignore))
    }
    fn light_alloc(&mut self, l: rc_game::point_lights::PointLight) -> i32 { self.with(|h| h.light_alloc(l)) }
    fn light_get(&mut self, slot: i32) -> Option<rc_game::point_lights::PointLight> { self.with(|h| h.light_get(slot)) }
    fn light_set(&mut self, slot: i32, l: rc_game::point_lights::PointLight) { self.with(|h| h.light_set(slot, l)) }
    fn light_free(&mut self, slot: i32) { self.with(|h| h.light_free(slot)) }
}

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    pos: [f32; 3],
    /// Ratchet's key B sequence.
    seq: u8,
    /// The hand item: id, its state (+0x20).
    hand: (i32, u8),
    ammo: i32,
    /// 0x1413f8 / 0x1413fa: the weapon out, the arm raised.
    out: (u8, u8),
    /// Live Pyrocitor flames (type 12 records) and their positions' mean.
    flames: usize,
    flame_mean: [f32; 3],
    /// The pilot flame (class 179) alive; the point light's slot.
    glow: bool,
    light: i32,
    crate_state: u8,
    fp: u8,
    cam_pos: [f32; 3],
    rng: u32,
}

/// The vendor's purchase on the arrival state: `GiveItem(16, equip)` (the buy flow's call), bolts paid.
fn buy(lv: &Lv) -> (GameState, SessionState) {
    let (mut gs, mut session) = (lv.state.clone(), lv.session);
    let price = i32::from_le_bytes(lv.tables.records[PYROCITOR as usize].0[0..4].try_into().unwrap());
    gs.global.bolts -= price;
    gs.give_item(PYROCITOR as usize, true, &lv.tables, &mut session);
    (gs, session)
}

/// Ratchet at `at` (yaw) with the bought Pyrocitor requested into the hand, `input(t)` for `ticks` ticks; `watch` =
/// the moby instance to record (a crate).
fn run(lv: &Lv, at: ([f32; 3], f32), watch: usize, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> Vec<Row> {
    let (gs, session) = buy(lv);
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    let watch = statics.moby_to_instance.iter().position(|&i| i == watch).expect("the crate is created");
    let mut mobys = statics.mobys.clone();
    let hero_idx = mobys.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let (p, yaw) = at;
    mobys[hero_idx].position = [p[0], p[1], p[2], 1.0];
    mobys[hero_idx].rotation = [0.0, 0.0, yaw, 0.0];
    let mut table = MobyTable::new(mobys, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    game.hero.set_joint_targets(&lv.seconds);
    game.item_data = Some(lv.items.clone());
    game.hero.weapons.defs = lv.weapon_defs.clone();
    // The engine's sync of the game state: owned items, ammo, "uses ammo", the hand request of the purchase.
    game.hero.owned.0 = gs.global.owned;
    game.hero.weapons.ammo = gs.global.ammo;
    for (i, r) in lv.tables.records.iter().enumerate() { game.hero.weapons.uses_ammo[i] = r.has_ammo(); }
    game.item_globals.wrench_flag = 1;
    game.item_globals.request = session.temp_hand;
    game.item_globals.saved = session.temp_hand;
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&lv.splines);
    svc.groups = statics.groups(&lv.gp);
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    let mut particles = Particles::new(None, Vec::new());
    let mut sched = Scheduler::new();
    {
        let hero: Hero = game.hero.clone();
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&lv.mesh);
        w.particles = Some(&mut particles);
        sched.load_pass(&mut w);
    }
    game.finish_load();
    let mut anim = RatchetAnim::new(&lv.ratchet);
    anim.layer_joints = rc_game::hero::anim::LAYER_LISTS.map(|l| lv.seconds[l as usize].clone());
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let parts_cell = std::cell::RefCell::new(&mut particles);
    let mut rows = Vec::new();
    for t in 0..ticks {
        let classes_ref: &ClassTable = &classes;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut p = parts_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.particles = Some(&mut **p);
            sched.tick(&mut w);
        };
        // The particle hook as the engine's: the hero's queued spawns, then UpdateParts.
        let mut parts = |hero: &Hero, _: &rc_game::follow_camera::CameraView, rng: &mut Rng, _: u64| {
            let mut p = parts_cell.borrow_mut();
            rc_game::hero::fx::create_particles(hero, &mut p);
            p.update_parts(rng);
        };
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        game.hero.idle.counter = game.counter as i32;
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh };
        let r = game.tick_with_hits(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let p = parts_cell.borrow();
        let live: Vec<[f32; 3]> = (0..=p.pool.hw.max(-1))
            .map(|i| &p.pool.recs[i as usize])
            .filter(|r| r[0] == 12 && r[1] & 0x80 == 0)
            .map(rc_game::particles::rec::pos)
            .collect();
        let n = live.len().max(1) as f32;
        let mean = [0, 1, 2].map(|k| live.iter().map(|q| q[k]).sum::<f32>() / n);
        rows.push(Row {
            state: h.state,
            pos: h.position(),
            seq: anim.state.seq_b,
            hand: h.items.slot.item.as_ref().map_or((h.items.slot.id, 0xff), |m| (h.items.slot.id, m.mstate)),
            ammo: h.weapons.ammo[PYROCITOR as usize],
            out: (h.f13f8, h.f13fa),
            flames: live.len(),
            flame_mean: mean,
            glow: game.mobys.mobys.iter().any(|m| m.o_class == GLOW_CLASS && m.state < 0xfd),
            light: h.weapons.pyro.light,
            crate_state: game.mobys.mobys[watch].state,
            fp: h.f13f5,
            cam_pos: game.camera.out.pos_f32(),
            rng: game.rng.state,
        });
    }
    rows
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_PYRO_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() { eprintln!("{t:4} {r:?}"); }
}

/// Crate 376 (class 500 below the spawn plateau) and a place `d` units north of it (flat ground), facing it (south).
const CRATE: usize = 376;
fn facing(lv: &Lv, d: f32) -> ([f32; 3], f32) {
    let c = lv.instances[CRATE].position;
    // The flame leaves from his right hand (west of him when he faces south).
    ([c[0] - 0.3, c[1] + d, c[2]], -std::f32::consts::FRAC_PI_2)
}

/// Ratchet's spawn (the level's placement) and its yaw.
fn spawn(lv: &Lv) -> ([f32; 3], f32) {
    let h = lv.instances.iter().find(|m| m.o_class == 0).unwrap();
    ([h.position[0], h.position[1], h.position[2]], h.rotation[2])
}

/// How far ahead of Ratchet (along `yaw`) a point is.
fn ahead(r: &Row, yaw: f32, p: [f32; 3]) -> f32 { (p[0] - r.pos[0]) * yaw.cos() + (p[1] - r.pos[1]) * yaw.sin() }

#[test]
fn the_purchase_puts_the_pyrocitor_in_hand_with_ammo() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (gs, session) = buy(&lv);
    assert_eq!(gs.global.owned[16], 1);
    assert_eq!(gs.global.bolts, 2500, "the Pyrocitor costs 2500");
    assert_eq!(session.temp_hand, 16, "bought with equip: requested into the hand");
    assert_eq!(gs.global.ammo[16], 120, "GiveItem's ammo");
    assert_eq!(lv.weapon_defs[16], WeaponDef { w18: 2, anims: [51, -1, -1], w30: 1 }, "stance 51, no moving layer, the arm kept raised");
}

fn hold_circle(t: u32) -> PadInput { if (60..=160).contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

#[test]
fn novalis_pyrocitor_standing() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = spawn(&lv);
    let rows = run(&lv, at, CRATE, &hold_circle, 260);
    dump(&rows);
    assert!(rows[40..].iter().all(|r| r.hand.0 == 16), "the Pyrocitor is not in the hand");
    let fire = rows.iter().position(|r| r.hand.1 == 3).expect("never fired");
    assert!((60..70).contains(&fire), "fired at {fire}: the draw sequence blends to 1 (5 ticks) once ○ is held");
    // The stance: idle state, sequence 51, the weapon out with the arm raised.
    assert!(rows[fire + 12..=160].iter().all(|r| r.state == 0 && r.seq == 51 && r.out == (1, 1)), "{:?}", rows[fire + 12]);
    // The flames: a stream of type-12 particles ahead of Ratchet, the light and the pilot flame.
    let r = &rows[120];
    assert!(r.flames > 40, "{} flames", r.flames);
    assert!(ahead(r, at.1, r.flame_mean) > 3.0, "flames at {:?}, Ratchet {:?}", r.flame_mean, r.pos);
    assert!(r.light >= 0 && r.glow);
    // One ammo every 10 ticks while firing.
    let used = rows[59].ammo - rows[170].ammo;
    assert!((9..=12).contains(&used), "ammo used {used}");
    // Released: the item back to 2, the weapon put away, Ratchet back to his idle sequence; the light fades out.
    let stop = rows.iter().skip(161).position(|r| r.hand.1 == 2).expect("never stopped") + 161;
    assert!(stop <= 172, "stopped at {stop}");
    assert_eq!(rows[stop].out, (0, 0));
    assert!(rows[stop + 20..].iter().all(|r| r.seq != 51));
    assert!(rows.last().unwrap().light == -1, "the light was freed");
    assert!(rows.last().unwrap().glow, "the pilot flame stays while the item is ready");
    assert_eq!(rows, run(&lv, at, CRATE, &hold_circle, 260), "deterministic");
}

fn run_and_fire(t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if (50..=150).contains(&t) { p = p.stick(0.0, -1.0); }
    if (60..=150).contains(&t) { p = p.press(button::CIRCLE); }
    p
}

#[test]
fn novalis_pyrocitor_running_breaks_a_crate() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, 6.0);
    let rows = run(&lv, at, CRATE, &run_and_fire, 200);
    dump(&rows);
    let fire = rows.iter().position(|r| r.hand.1 == 3).expect("never fired");
    // Ratchet keeps running (walk / run state 2) with the weapon out: the Pyrocitor has no moving arm layer.
    assert!(rows[fire + 5..fire + 30].iter().all(|r| r.state == 2 && r.out.0 == 1), "{:?}", rows[fire + 5]);
    assert!(ahead(&rows[fire], at.1, rows[fire + 30].pos) > 1.5, "not running");
    assert!(rows[fire + 20].flames > 20);
    let broke = rows.iter().position(|r| r.crate_state != rows[0].crate_state).expect("the crate was not hit");
    assert!(broke > fire, "hit at {broke}");
    // The same run without ○: the crate stays (the flames broke it, not Ratchet running into it).
    let dry = run(&lv, at, CRATE, &|t| { let mut p = run_and_fire(t); p.buttons &= !(button::CIRCLE as u16); p }, 200);
    assert!(dry.iter().all(|r| r.crate_state == rows[0].crate_state), "the crate broke without the flames");
    assert_eq!(rows, run(&lv, at, CRATE, &run_and_fire, 200), "deterministic");
}

fn first_person(t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if (20..=200).contains(&t) { p = p.press(button::L1); }
    if (80..=140).contains(&t) { p = p.press(button::L1 | button::CIRCLE); }
    p
}

#[test]
fn novalis_pyrocitor_first_person() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = spawn(&lv);
    let rows = run(&lv, at, CRATE, &first_person, 200);
    dump(&rows);
    let fire = rows.iter().position(|r| r.hand.1 == 3).expect("never fired in first person");
    let r = &rows[fire + 30];
    assert_eq!((r.state, r.fp), (1, 1), "the look stance under the first-person camera");
    assert!(r.flames > 20);
    // The flames leave from below the eye, along the view (Ratchet faces the view).
    assert!(ahead(r, at.1, r.flame_mean) > 2.0 && r.flame_mean[2] < r.cam_pos[2], "flames {:?} eye {:?}", r.flame_mean, r.cam_pos);
    assert_eq!(rows, run(&lv, at, CRATE, &first_person, 200), "deterministic");
}
