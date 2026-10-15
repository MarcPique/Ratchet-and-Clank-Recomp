//! The gun-family weapons on Novalis, headless (docs/plan/hero_gameplay.md §9): the Blaster (`crate::hero::blaster`),
//! the Devastator, the R.Y.N.O. and the Tesla Claw, each given with `GiveItem(id, equip)` (their ammo, the hand
//! request) on the game state of a first Novalis arrival, then fired standing, running and in first person, at a
//! crate and at a critter. The level runs as the engine ticks it (the loader's spawn test, the scheduler's load pass
//! and moby loop, the moby collision, the particles with the hero's spawns, the tick's hit path). Every run twice:
//! identical. Skipped when `extracted/` is absent. The numbers checked are distilled from the runs themselves (no
//! personal files).

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::save_game::{ChunkTables, ItemTables, SaveGameLump};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::game_state::{GameState, SessionState};
use rc_game::hero::anim::{HoldClass, RatchetAnim, HOLD_CLASSES};
use rc_game::hero::items::{ItemClass, ItemData, ItemDef, WeaponDef, HERO_LISTS};
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
    /// Ratchet's glove-holding classes 1 / 2 (`rc_game::hero::weapons::hold_update`).
    hold: [Option<Arc<HoldClass>>; 2],
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
    let lump = SaveGameLump::parse(&std::fs::read(root.join("global/save_game.bin")).ok()?).unwrap();
    let ct = ChunkTables::from_boot_elf(&elf).unwrap();
    let mut state = GameState::new_game(ct, &lump.template).unwrap();
    let mut session = SessionState::default();
    state.apply_level_start(0, &ItemTables::load(&elf, &ov0).unwrap(), &mut session);
    state.on_veldin_clank_init(&mut session);
    state.apply_transition(1);
    state.apply_level_start(1, &tables, &mut session);
    let hold = HOLD_CLASSES.map(|oc| rc_formats::test_data::core_block(1, &format!("moby_class/{oc:04}")).and_then(|b| HoldClass::parse(&b)).map(Arc::new));
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, seconds, items, weapon_defs, tables, state, session, hold })
}

/// The hero's hit sink over the moby loop's services, as the engine's `CellHits` (with the guns' probe and class type).
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

type V4 = rc_game::hero::physics::V4;
type Tmpl = rc_game::moby_update::services::HitTemplate;

impl rc_game::hero::items::HitSink for CellHits<'_, '_> {
    fn sphere(&mut self, table: &mut MobyTable, r: rc_game::ps2v::Pf, centre: V4, flags: u32, ignore: Option<usize>, tmpl: &Tmpl) -> Option<usize> { self.with(|h| h.sphere(table, r, centre, flags, ignore, tmpl)) }
    fn line(&mut self, table: &mut MobyTable, a: V4, b: V4, flags: u32, ignore: Option<usize>, tmpl: &Tmpl) -> Option<Option<usize>> { self.with(|h| h.line(table, a, b, flags, ignore, tmpl)) }
    fn deliver(&mut self, table: &mut MobyTable, target: usize, tmpl: &Tmpl) {
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
    fn probe(&mut self, table: &mut MobyTable, a: V4, b: V4, flags: u32, ignore: Option<usize>) -> Option<Option<[f32; 3]>> { self.with(|h| h.probe(table, a, b, flags, ignore)) }
    fn probe_moby(&mut self, table: &mut MobyTable, a: V4, b: V4, flags: u32, ignore: Option<usize>) -> Option<Option<rc_game::hero::items::Probe>> { self.with(|h| h.probe_moby(table, a, b, flags, ignore)) }
    fn class_type(&self, o_class: i16) -> Option<u8> { use rc_game::moby_update::services::ClassData; self.classes.info(o_class).map(|i| i.ty) }
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
    /// The hand item: id, its state (+0x20), its key B sequence.
    hand: (i32, u8, u8),
    ammo: i32,
    /// 0x1413f8 / 0x1413fa: the weapon out, the arm raised; the arm layer's sequence (list 12).
    out: (u8, u8),
    layer: Option<u8>,
    /// The live projectiles of the weapon's class: (moby, position).
    shots: Vec<(usize, [f32; 3])>,
    /// Live particles of the given types.
    parts: usize,
    /// This tick's screen markers: (fx, rgba, point).
    markers: Vec<(usize, u32, Option<[f32; 3]>)>,
    /// The watched mobys' states (the crate, the critters).
    watched: Vec<u8>,
    /// The weapon's target this tick.
    target: Option<usize>,
    /// The Tesla Claw: firing, its chain's drawn points and the beam's end.
    beam: (bool, i16, [f32; 3]),
    light: bool,
    fp: u8,
    cam_pos: [f32; 3],
    rng: u32,
    /// The glove-holding layers (lists 12 / 13): key A / key B sequence and weight.
    hold: [Option<(u8, u8, f32)>; 2],
    /// The hand item as `HeroItemsAttach` placed it this tick: position and rows; Ratchet's facing row.
    item: Option<([f32; 3], [[f32; 3]; 3])>,
    facing: [f32; 3],
    /// The hand item's class sounds played this tick (`HeroSounds::item_sound`: class, index).
    sounds: Vec<(i16, i32)>,
}

/// The hero's sounds of a run: the hand item's class sounds, recorded per tick (no audio, no RNG draws).
#[derive(Default)]
struct RecSounds(Vec<(i16, i32)>);

impl rc_game::hero::HeroSounds for RecSounds {
    fn anim_advanced(&mut self, _: &rc_game::moby_runtime::Moby, _: &rc_game::hero::anim::AnimView, _: &rc_game::hero::anim::AnimView, _: &mut Rng) {}
    fn item_sound(&mut self, o_class: i16, _pos: [f32; 3], index: i32, _flags: u32, _rng: &mut Rng) -> i32 {
        self.0.push((o_class, index));
        -1
    }
}

/// What a run gives and watches.
struct Setup<'a> {
    item: i32,
    at: ([f32; 3], f32),
    /// Moby instances to watch (their states).
    watch: &'a [usize],
    shot_class: i16,
    part_types: &'a [u8],
}

fn give(lv: &Lv, item: i32) -> (GameState, SessionState) {
    let (mut gs, mut session) = (lv.state.clone(), lv.session);
    gs.give_item(item as usize, true, &lv.tables, &mut session);
    (gs, session)
}

fn run(lv: &Lv, s: &Setup, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> Vec<Row> { run_with(lv, s, input, ticks, true) }

/// [`run`], with (`hold`) or without Ratchet's glove-holding classes on his animation (without: the holding layers run
/// but are not evaluated, the arms play his own run).
fn run_with(lv: &Lv, s: &Setup, input: &dyn Fn(u32) -> PadInput, ticks: u32, hold: bool) -> Vec<Row> { run_swaps(lv, s, input, ticks, hold, &[], &[]) }

/// [`run_with`] owning also `more` items (not equipped), with the hand requests `swaps` (tick, item: the quick
/// select's request 0x141408) made before those ticks.
fn run_swaps(lv: &Lv, s: &Setup, input: &dyn Fn(u32) -> PadInput, ticks: u32, hold: bool, more: &[i32], swaps: &[(u32, i32)]) -> Vec<Row> {
    let (mut gs, mut session) = give(lv, s.item);
    for &m in more { gs.give_item(m as usize, false, &lv.tables, &mut session); }
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    let watch: Vec<usize> = s.watch.iter().map(|w| statics.moby_to_instance.iter().position(|&i| i == *w).expect("watched moby")).collect();
    let mut mobys = statics.mobys.clone();
    let hero_idx = mobys.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let (p, yaw) = s.at;
    mobys[hero_idx].position = [p[0], p[1], p[2], 1.0];
    mobys[hero_idx].rotation = [0.0, 0.0, yaw, 0.0];
    let mut table = MobyTable::new(mobys, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    game.hero.set_joint_targets(&lv.seconds);
    game.item_data = Some(lv.items.clone());
    game.hero.weapons.defs = lv.weapon_defs.clone();
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
    if hold { anim.hold = lv.hold.clone(); }
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
        let mut parts = |hero: &Hero, _: &rc_game::follow_camera::CameraView, rng: &mut Rng, _: u64| {
            let mut p = parts_cell.borrow_mut();
            rc_game::hero::fx::create_particles(hero, &mut p);
            p.update_parts(rng);
        };
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        game.hero.idle.counter = game.counter as i32;
        // The render's +0x31 ("drawn last frame", MobyProc's; the R.Y.N.O.'s search reads it): the watched mobys are
        // in view in these runs.
        for &i in &watch { if game.mobys.mobys[i].state < 0x80 { game.mobys.mobys[i].visible = 1; } }
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh };
        if let Some(&(_, id)) = swaps.iter().find(|x| x.0 == t) { game.item_globals.request = id; }
        let mut rec = RecSounds::default();
        let r = game.tick_with_hero_sounds(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits, None, &mut rec);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let p = parts_cell.borrow();
        let parts = (0..=p.pool.hw.max(-1)).map(|i| &p.pool.recs[i as usize]).filter(|r| s.part_types.contains(&r[0]) && r[1] & 0x80 == 0).count();
        let tick = game.counter - 1;
        let target = match s.item {
            15 => h.weapons.blaster.target,
            23 => h.weapons.ryno.target,
            11 => h.weapons.devastator.lock,
            19 => h.weapons.tesla.targets[0],
            _ => None,
        };
        rows.push(Row {
            state: h.state,
            pos: h.position(),
            seq: anim.state.seq_b,
            hand: h.items.slot.item.as_ref().map_or((h.items.slot.id, 0xff, 0xff), |m| (h.items.slot.id, m.mstate, m.anim.seq_b)),
            ammo: h.weapons.ammo[s.item as usize],
            out: (h.f13f8, h.f13fa),
            layer: h.weapons.layers[0].map(|l| l.seq_b),
            shots: game.mobys.mobys.iter().enumerate().filter(|(_, m)| m.o_class == s.shot_class && m.state < 0xfd).map(|(i, m)| (i, [m.position[0], m.position[1], m.position[2]])).collect(),
            parts,
            markers: h.weapons.markers.of_tick(tick).iter().map(|m| (m.fx, m.rgba, m.at)).collect(),
            watched: watch.iter().map(|&i| game.mobys.mobys[i].state).collect(),
            target,
            beam: (h.weapons.tesla.firing, h.weapons.tesla.count, h.weapons.tesla.end),
            light: svc_cell.borrow().point_lights.active().count() > 0,
            fp: h.f13f5,
            cam_pos: game.camera.out.pos_f32(),
            rng: game.rng.state,
            hold: [2, 3].map(|i| h.weapons.layers[i].map(|l| (l.seq_a, l.seq_b, l.weight))),
            item: h.items.slot.item.as_ref().map(|m| (m.position, m.rows.map(|r| [0, 1, 2].map(|k| f32::from_bits(r[k]))))),
            facing: rc_game::hero::physics::to_f32x3(h.moby_rows[0]),
            sounds: rec.0,
        });
    }
    rows
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_GUNS_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() { eprintln!("{t:4} {r:?}"); }
}

/// Ratchet's spawn and its yaw.
fn spawn(lv: &Lv) -> ([f32; 3], f32) {
    let h = lv.instances.iter().find(|m| m.o_class == 0).unwrap();
    ([h.position[0], h.position[1], h.position[2]], h.rotation[2])
}

/// Crate 376 (class 500 below the spawn plateau) and a place `d` units north of it, facing it (south).
const CRATE: usize = 376;
fn facing_crate(lv: &Lv, d: f32) -> ([f32; 3], f32) {
    let c = lv.instances[CRATE].position;
    ([c[0], c[1] + d, c[2]], -std::f32::consts::FRAC_PI_2)
}

/// The critters 577 in the pit north-west of the spawn (instances of the table's mobys 590 / 592), and a place `d`
/// units from `id`'s moby in direction `from`, facing it.
fn facing_moby(lv: &Lv, inst: usize, d: f32, from: f32) -> ([f32; 3], f32) {
    let c = lv.instances[inst].position;
    let p = [c[0] + d * from.cos(), c[1] + d * from.sin(), c[2] + 0.5];
    (p, (c[1] - p[1]).atan2(c[0] - p[0]))
}

/// The instance of the table's moby `id` (the moby loader's order).
fn instance_of(lv: &Lv, id: usize) -> usize {
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    statics.moby_to_instance[id]
}

fn ahead(r: &Row, yaw: f32, p: [f32; 3]) -> f32 { (p[0] - r.pos[0]) * yaw.cos() + (p[1] - r.pos[1]) * yaw.sin() }

fn hold(from: u32, to: u32) -> impl Fn(u32) -> PadInput { move |t| if (from..=to).contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

// ------------------------------------------------------------------------------------------------------------------
// The Blaster

#[test]
fn blaster_given_with_its_ammo_and_defs() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (gs, session) = give(&lv, 15);
    assert_eq!((gs.global.owned[15], session.temp_hand), (1, 15));
    assert!(gs.global.ammo[15] > 0);
    assert_eq!(lv.items.defs[15].o_class, 168);
    assert_eq!(lv.weapon_defs[15], WeaponDef { w18: 1, anims: [56, 57, 62], w30: 1 }, "stance 56, arm 57 / crouched 62, the arm kept raised");
}

/// Standing at the crate: ○ draws the Blaster (the stance 56), shots leave every 6 ticks from the 6th tick of the arm
/// on, one ammo each, fly 40 u/s along the facing, and one breaks the crate; the item plays its firing sequence 4;
/// released, the weapon goes away.
#[test]
fn novalis_blaster_standing_breaks_a_crate() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    // 6 units: the crate stands in a dip 0.86 below Ratchet and is about a unit high. With the Blaster in hand he holds
    // it up in the idle too (the glove-holding layer, `0x22e660`), so from 9 units every shot passes over it (the
    // search takes the nanotech crate 533 beyond); before the holding layer, the first shot left from the lower
    // no-weapon idle arm during the stance blend and broke it.
    let s = Setup { item: 15, at: facing_crate(&lv, 6.0), watch: &[CRATE], shot_class: 305, part_types: &[27, 72, 26] };
    let input = hold(60, 120);
    let rows = run(&lv, &s, &input, 220);
    dump(&rows);
    assert!(rows[40..].iter().all(|r| r.hand.0 == 15), "the Blaster is not in the hand");
    let drawn = rows.iter().position(|r| r.out == (1, 1)).expect("never drawn");
    assert!((60..64).contains(&drawn), "drawn at {drawn}");
    assert!(rows[drawn + 12..=120].iter().all(|r| r.state == 0 && r.seq == 56), "stance: {:?}", rows[drawn + 12]);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no shot");
    assert!((drawn + 5..drawn + 10).contains(&first), "first shot {first} (drawn {drawn})");
    let ammo0 = rows[59].ammo;
    let used = ammo0 - rows[125].ammo;
    // From the first shot to the release: one every 6 ticks.
    let expect = (121 - first as i32 + 5) / 6;
    assert!((used - expect).abs() <= 1, "ammo used {used}, expected about {expect}");
    assert!(rows[first + 2].parts > 10, "muzzle sparks, trail and glow");
    assert!(rows[first..first + 20].iter().any(|r| r.hand.2 == 4), "the firing sequence 4");
    // A shot moves 40·dt a tick along the facing (south).
    let (id, p0) = rows[first].shots[0];
    let p1 = rows[first + 1].shots.iter().find(|x| x.0 == id).expect("the shot lives").1;
    let step = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt();
    assert!((step - 40.0 / 60.0).abs() < 0.01, "step {step}");
    assert!(p1[1] < p0[1], "south");
    let broke = rows.iter().position(|r| r.watched[0] != rows[0].watched[0]).expect("the crate was not hit");
    assert!(broke > first, "broken at {broke}");
    // Released: put away within a few ticks, no shots after the last ones die.
    assert!(rows[130..].iter().all(|r| r.out.0 == 0), "put away");
    assert!(rows[200..].iter().all(|r| r.shots.is_empty()));
    assert_eq!(rows, run(&lv, &s, &input, 220), "deterministic");
}

fn run_and_fire(t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if (40..=140).contains(&t) { p = p.stick(0.0, -1.0); }
    if (70..=130).contains(&t) { p = p.press(button::CIRCLE); }
    p
}

/// Running: Ratchet keeps running with the arm layer (sequence 57) over his run, and the shots fly ahead of him.
#[test]
fn novalis_blaster_running_arm_layer() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = spawn(&lv);
    let s = Setup { item: 15, at, watch: &[], shot_class: 305, part_types: &[27, 72, 26] };
    let rows = run(&lv, &s, &run_and_fire, 180);
    dump(&rows);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no shot");
    assert!(rows[first..first + 30].iter().all(|r| r.state == 2 && r.out.0 == 1 && r.layer == Some(57)), "{:?}", rows[first + 5]);
    let r = &rows[first + 20];
    let yaw = (r.pos[1] - rows[first].pos[1]).atan2(r.pos[0] - rows[first].pos[0]);
    assert!(r.shots.iter().any(|x| ahead(r, yaw, x.1) > 3.0), "shots ahead: {:?}", r.shots);
    assert_eq!(rows, run(&lv, &s, &run_and_fire, 180), "deterministic");
}

/// The critters in the pit: the Blaster's search takes one (its green marker over it) and the shots home in and hit it.
#[test]
fn novalis_blaster_targets_and_hits_a_critter() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (instance_of(&lv, 590), instance_of(&lv, 592));
    let at = facing_moby(&lv, b, 7.0, std::f32::consts::PI);
    let s = Setup { item: 15, at, watch: &[a, b], shot_class: 305, part_types: &[27, 72, 26] };
    let input = hold(60, 200);
    let rows = run(&lv, &s, &input, 240);
    dump(&rows);
    let tg = rows[70..200].iter().filter_map(|r| r.target).next().expect("no target");
    let tick = rows.iter().position(|r| r.target == Some(tg)).unwrap();
    assert!(rows[tick].markers.iter().any(|m| m.0 == 0x26 && m.1 == 0xff0f_ff0f && m.2.is_some()), "green marker: {:?}", rows[tick].markers);
    // A critter hit: its state leaves its idle states (hit reaction or death).
    assert!(rows.iter().any(|r| r.watched != rows[0].watched), "no critter was hit");
    assert_eq!(rows, run(&lv, &s, &input, 240), "deterministic");
}

fn first_person(t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if (20..=200).contains(&t) { p = p.press(button::L1); }
    if (90..=150).contains(&t) { p = p.press(button::L1 | button::CIRCLE); }
    p
}

/// First person: the look stance becomes 0x1e, the red crosshair at the screen centre, the shots leave from below
/// the eye along the view.
#[test]
fn novalis_blaster_first_person() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = spawn(&lv);
    let s = Setup { item: 15, at, watch: &[], shot_class: 305, part_types: &[27, 72, 26] };
    let rows = run(&lv, &s, &first_person, 200);
    dump(&rows);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no shot in first person");
    let r = &rows[first];
    assert_eq!((r.state, r.fp), (0x1e, 1), "the look stance 0x1e under the first-person camera");
    assert!(r.markers.iter().any(|m| m.0 == 0x26 && m.1 == 0xff0f_0fff && m.2.is_none()), "red crosshair: {:?}", r.markers);
    let (_, p) = r.shots[0];
    assert!(p[2] < r.cam_pos[2] && (p[2] - r.cam_pos[2]).abs() < 0.5, "shot {p:?} eye {:?}", r.cam_pos);
    assert_eq!(rows, run(&lv, &s, &first_person, 200), "deterministic");
}

// ------------------------------------------------------------------------------------------------------------------
// The R.Y.N.O.

#[test]
fn ryno_given_with_its_ammo_and_defs() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (gs, session) = give(&lv, 23);
    assert_eq!((gs.global.owned[23], session.temp_hand), (1, 23));
    assert!(gs.global.ammo[23] > 0);
    assert_eq!(lv.items.defs[23].o_class, 454);
    assert_eq!(lv.weapon_defs[23], WeaponDef { w18: 2, anims: [81, -1, -1], w30: 1 }, "stance 81, no arm layer, the arm kept raised");
}

fn press(at: u32) -> impl Fn(u32) -> PadInput { move |t| if t == at { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

/// At the critters: the R.Y.N.O. locks one (the green marker, FX 0x23), ○ fires a salvo of seven missiles from its
/// barrels, 9 ticks apart, for one ammo; they home in and one hits: the critter reacts.
#[test]
fn novalis_ryno_salvo_hits_a_critter() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (instance_of(&lv, 590), instance_of(&lv, 592));
    let at = facing_moby(&lv, b, 9.0, std::f32::consts::PI);
    let s = Setup { item: 23, at, watch: &[a, b], shot_class: 457, part_types: &[4] };
    let input = press(80);
    let rows = run(&lv, &s, &input, 260);
    dump(&rows);
    assert!(rows[60..80].iter().any(|r| r.target.is_some()), "no lock before the salvo");
    let r = rows[60..80].iter().find(|r| r.target.is_some()).unwrap();
    assert!(r.markers.iter().any(|m| m.0 == 0x23 && m.1 == 0xff0f_ff0f), "green marker: {:?}", r.markers);
    let ammo0 = rows[79].ammo;
    assert_eq!(rows[120].ammo, ammo0 - 1, "one ammo a salvo");
    // Seven missiles, the first on the salvo's first tick, 9 ticks apart.
    let mut seen = std::collections::BTreeSet::new();
    let mut births = Vec::new();
    for (t, r) in rows.iter().enumerate() {
        for (id, _) in &r.shots {
            if seen.insert((*id, t.saturating_sub(0))) && !rows[t.saturating_sub(1)].shots.iter().any(|x| x.0 == *id) { births.push(t); }
        }
    }
    births.dedup();
    assert_eq!(births.len(), 7, "births {births:?}");
    assert!(births.windows(2).all(|w| w[1] - w[0] == 9), "births {births:?}");
    assert!(rows[births[3]].parts > 5, "smoke trails");
    assert!(rows.iter().any(|r| r.watched != rows[0].watched), "no critter was hit");
    assert_eq!(rows, run(&lv, &s, &input, 260), "deterministic");
}

/// Running at the spawn: Ratchet keeps running (no arm layer: the def has none) and the missiles leave ahead.
#[test]
fn novalis_ryno_running() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { item: 23, at: spawn(&lv), watch: &[], shot_class: 457, part_types: &[4] };
    let input = |t: u32| {
        let mut p = PadInput::neutral();
        if (40..=160).contains(&t) { p = p.stick(0.0, -1.0); }
        if t == 80 { p = p.press(button::CIRCLE); }
        p
    };
    let rows = run(&lv, &s, &input, 170);
    dump(&rows);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no missile");
    assert!(rows[first..first + 40].iter().all(|r| r.state == 2), "running: {:?}", rows[first + 10]);
    assert_eq!(rows, run(&lv, &s, &input, 170), "deterministic");
}

/// First person: the look stance 0x1e; the missiles leave along the view.
#[test]
fn novalis_ryno_first_person() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { item: 23, at: spawn(&lv), watch: &[], shot_class: 457, part_types: &[4] };
    let input = |t: u32| {
        let mut p = PadInput::neutral();
        if (20..=200).contains(&t) { p = p.press(button::L1); }
        if t == 100 { p = p.press(button::L1 | button::CIRCLE); }
        p
    };
    let rows = run(&lv, &s, &input, 200);
    dump(&rows);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no missile in first person");
    assert_eq!((rows[first].state, rows[first].fp), (0x1e, 1));
    assert_eq!(rows, run(&lv, &s, &input, 200), "deterministic");
}

// ------------------------------------------------------------------------------------------------------------------
// The Devastator

#[test]
fn devastator_given_with_its_ammo_and_defs() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (gs, session) = give(&lv, 11);
    assert_eq!((gs.global.owned[11], session.temp_hand), (1, 11));
    assert!(gs.global.ammo[11] > 0);
    assert_eq!(lv.items.defs[11].o_class, 157);
    assert_eq!(lv.weapon_defs[11], WeaponDef { w18: 2, anims: [54, 73, 73], w30: 0 }, "shot 54, arm 73 on both lists, not kept raised");
}

/// At the critters: ○ held fires a missile every 35 ticks (one ammo each) aimed by the search; when a critter is up
/// (targetable) the search locks it and the missile homes in and blows up on it (the critter reacts); Ratchet plays
/// the standing shot 54.
#[test]
fn novalis_devastator_locks_and_blows_up_a_critter() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (instance_of(&lv, 590), instance_of(&lv, 592));
    let at = facing_moby(&lv, b, 9.0, -std::f32::consts::FRAC_PI_2);
    let s = Setup { item: 11, at, watch: &[a, b], shot_class: 153, part_types: &[15, 11, 8] };
    let input = hold(60, 260);
    let rows = run(&lv, &s, &input, 340);
    dump(&rows);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no missile");
    assert!((60..90).contains(&first), "first missile at {first}");
    assert!(rows[first..first + 5].iter().any(|r| r.seq == 54), "the standing shot 54: {:?}", rows[first].seq);
    let used = rows[59].ammo - rows[262].ammo;
    let expect = (262 - first as i32) / 35 + 1;
    assert!((used - expect).abs() <= 1, "ammo used {used}, expected about {expect}");
    assert!(rows.iter().any(|r| r.target.is_some()), "never locked");
    // The first missile's end: the explosion's streaks, rings and puffs.
    let m0 = rows[first].shots[0].0;
    let gone = rows[first..].iter().position(|r| !r.shots.iter().any(|x| x.0 == m0)).expect("the missile never ended") + first;
    assert!(rows[gone].parts > rows[gone - 1].parts + 5, "explosion particles: {} → {}", rows[gone - 1].parts, rows[gone].parts);
    assert!(rows.iter().any(|r| r.watched.iter().zip(&rows[0].watched).any(|(x, y)| x != y && *x != 17 && *x != 3)), "no critter was hit");
    assert_eq!(rows, run(&lv, &s, &input, 340), "deterministic");
}

/// Running: the arm layers (sequence 73) over the run.
#[test]
fn novalis_devastator_running_arm_layer() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { item: 11, at: spawn(&lv), watch: &[], shot_class: 153, part_types: &[15] };
    let rows = run(&lv, &s, &run_and_fire, 180);
    dump(&rows);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no missile");
    assert!(rows[first + 2..first + 12].iter().all(|r| r.state == 2 && r.layer == Some(73)), "{:?}", rows[first + 3]);
    assert_eq!(rows, run(&lv, &s, &run_and_fire, 180), "deterministic");
}

/// First person: the lock-on crosshair (FX 0x27) turns at the screen centre; the missile leaves below the eye.
#[test]
fn novalis_devastator_first_person() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { item: 11, at: spawn(&lv), watch: &[], shot_class: 153, part_types: &[15] };
    let rows = run(&lv, &s, &first_person, 200);
    dump(&rows);
    let first = rows.iter().position(|r| !r.shots.is_empty()).expect("no missile in first person");
    let r = &rows[first];
    assert_eq!((r.state, r.fp), (0x1e, 1));
    assert!(r.markers.iter().any(|m| m.0 == 0x27 && m.1 == 0xff18_0a65 && m.2.is_none()), "{:?}", r.markers);
    let (_, p) = r.shots[0];
    assert!(p[2] < r.cam_pos[2], "missile {p:?} eye {:?}", r.cam_pos);
    assert_eq!(rows, run(&lv, &s, &first_person, 200), "deterministic");
}

// ------------------------------------------------------------------------------------------------------------------
// The Tesla Claw

#[test]
fn tesla_given_with_its_ammo_and_defs() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (gs, session) = give(&lv, 19);
    assert_eq!((gs.global.owned[19], session.temp_hand), (1, 19));
    assert!(gs.global.ammo[19] > 0);
    assert_eq!(lv.items.defs[19].o_class, 177);
    assert_eq!(lv.weapon_defs[19], WeaponDef { w18: 1, anims: [51, -1, -1], w30: 1 }, "stance 51, no arm layer, the arm kept raised");
}

/// At the critters: ○ held after the warm-up fires the beam (the stance 51, one ammo every 10 ticks), it locks a
/// critter and hits it; released, the beam stops and the weapon goes away.
#[test]
fn novalis_tesla_beam_hits_a_critter() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (instance_of(&lv, 590), instance_of(&lv, 592));
    let at = facing_moby(&lv, b, 7.0, -std::f32::consts::FRAC_PI_2);
    let s = Setup { item: 19, at, watch: &[a, b], shot_class: -1, part_types: &[53] };
    let input = hold(60, 200);
    let rows = run(&lv, &s, &input, 240);
    dump(&rows);
    let fire = rows.iter().position(|r| r.beam.0).expect("never fired");
    assert!((60..64).contains(&fire), "fired at {fire}");
    assert!(rows[fire + 12..=200].iter().all(|r| r.beam.0 && r.state == 0 && r.seq == 51 && r.out == (1, 1)), "{:?}", rows[fire + 12]);
    let used = rows[59].ammo - rows[205].ammo;
    assert!((13..=16).contains(&used), "ammo used {used}");
    assert!(rows[fire + 5].parts > 0, "sparks");
    assert!(rows.iter().any(|r| r.target.is_some()), "never locked");
    assert!(rows.iter().any(|r| r.watched.iter().zip(&rows[0].watched).any(|(x, y)| x != y && *x != 17 && *x != 3)), "no critter was hit");
    assert!(rows[210..].iter().all(|r| !r.beam.0 && r.out.0 == 0), "stopped and put away");
    assert_eq!(rows, run(&lv, &s, &input, 240), "deterministic");
}

/// Standing at the crate: the beam runs 12 along the facing and stops at the crate (its end there), which it breaks.
#[test]
fn novalis_tesla_beam_stops_at_a_crate() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { item: 19, at: facing_crate(&lv, 5.0), watch: &[CRATE], shot_class: -1, part_types: &[53] };
    let input = hold(60, 160);
    let rows = run(&lv, &s, &input, 200);
    dump(&rows);
    let fire = rows.iter().position(|r| r.beam.0).expect("never fired");
    assert!(rows[fire..fire + 60].iter().any(|r| r.beam.1 < 20), "the chain never stopped short");
    assert!(rows.iter().any(|r| r.watched[0] != rows[0].watched[0]), "the crate was not hit");
    assert_eq!(rows, run(&lv, &s, &input, 200), "deterministic");
}


// ------------------------------------------------------------------------------------------------------------------
// Holding the weapon (the glove-holding layers `0x22e660`, docs/plan/hero_gameplay.md §11)

/// Running forward from the spawn, no fire.
fn run_only(t: u32) -> PadInput { if (20..=200).contains(&t) { PadInput::neutral().stick(0.0, -1.0) } else { PadInput::neutral() } }

/// The hand item's barrel while running: its axis most along Ratchet's facing (sign kept), the axis's pitch and its
/// yaw off the facing (degrees), the item origin's height above Ratchet; per tick of `from..to`.
fn barrel(rows: &[Row], from: usize, to: usize) -> Vec<(f32, f32, f32)> {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let score = |k: usize| rows[from..to].iter().map(|r| r.item.map_or(0.0, |(_, m)| dot(m[k], r.facing))).sum::<f32>();
    let k = (0..3).max_by(|&a, &b| score(a).abs().total_cmp(&score(b).abs())).unwrap();
    let sign = score(k).signum();
    rows[from..to].iter().filter_map(|r| {
        let (p, m) = r.item?;
        let a = m[k].map(|x| x * sign);
        let n = dot(a, a).sqrt();
        let pitch = (a[2] / n).asin().to_degrees();
        let yaw = (a[1].atan2(a[0]) - r.facing[1].atan2(r.facing[0])).to_degrees();
        let yaw = (yaw + 540.0) % 360.0 - 180.0;
        Some((pitch, yaw, p[2] - r.pos[2]))
    }).collect()
}

fn spread(v: impl Iterator<Item = f32> + Clone) -> f32 { v.clone().fold(f32::MIN, f32::max) - v.fold(f32::MAX, f32::min) }

/// Which items hold with which arms (def +0x18 → 0x1413fb): the Devastator and the R.Y.N.O. (2) both arms, the
/// Blaster (1) the right arm only, the Bomb Glove (0) none; running, the layers follow Ratchet's run (his key B, a
/// sequence below 0x17) at full weight.
#[test]
fn novalis_holding_layers_per_item() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    for (item, arms) in [(11, 2), (23, 2), (15, 1), (10, 0)] {
        let s = Setup { item, at: spawn(&lv), watch: &[], shot_class: -1, part_types: &[] };
        let rows = run(&lv, &s, &run_only, 150);
        dump(&rows);
        assert_eq!(lv.items.defs[item as usize].b18, arms, "item {item}: def +0x18");
        for r in &rows[100..150] {
            assert_eq!(r.state, 2, "item {item}: running");
            let n = r.hold.iter().filter(|h| h.is_some()).count();
            assert_eq!(n, arms as usize, "item {item}: holding layers {:?}", r.hold);
            for h in r.hold.iter().flatten() {
                assert_eq!(h.2, 1.0, "item {item}: full weight");
                assert_eq!(h.1, r.seq, "item {item}: the layer follows Ratchet's key B");
                assert!(h.1 < rc_game::hero::anim::HOLD_SEQS, "item {item}: a held sequence");
            }
        }
    }
}

/// The weapon stays level in his hands while he runs: with the holding classes the barrel (the hand item's axis most
/// along his facing) keeps its pitch and its yaw off his facing within a few degrees and its height within 0.2 over the
/// run cycle; his own run animation (no holding classes: the port before) swings it by tens of degrees. The holding
/// pose is the same for every gun (classes 1 / 2 are item-independent), so the Devastator, the R.Y.N.O. and the
/// Blaster measure alike. The shots' direction is Ratchet's facing (`0x2c7d68`, `0x2e4e60`, `0x2ca610` read his
/// moby rows, not the muzzle's), so a steady muzzle is a steady line of fire.
#[test]
fn novalis_weapon_steady_while_running() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    for item in [11, 23, 15] {
        let s = Setup { item, at: spawn(&lv), watch: &[], shot_class: -1, part_types: &[] };
        let held = barrel(&run_with(&lv, &s, &run_only, 150, true), 90, 150);
        let free = barrel(&run_with(&lv, &s, &run_only, 150, false), 90, 150);
        let sp = |v: &[(f32, f32, f32)]| (spread(v.iter().map(|x| x.0)), spread(v.iter().map(|x| x.1)), spread(v.iter().map(|x| x.2)));
        let (hp, hy, hz) = sp(&held);
        let (fp, fy, fz) = sp(&free);
        eprintln!("item {item}: held pitch {hp:.2}° yaw {hy:.2}° height {hz:.3}; free pitch {fp:.2}° yaw {fy:.2}° height {fz:.3}");
        if std::env::var("RC_GUNS_DUMP").is_ok() { for (a, b) in held.iter().zip(&free) { eprintln!("  {a:?} {b:?}"); } }
        // The bounds are the holding classes' own sway over the walk / run cycle (measured: 13.1° / 7.2° / 0.16) with
        // a margin; the port before them (his own arm swinging the gun) was 98° / 125° / 0.30.
        assert!(hp < 16.0 && hy < 10.0 && hz < 0.2, "item {item}: held {hp} {hy} {hz}");
        assert!(fp > 4.0 * hp && fy > 4.0 * hy, "item {item}: the holding layers are what keeps it steady");
    }
}

/// Firing standing, then pushing the stick while still firing (the user's recording 16.47.33 of the original):
/// Ratchet leaves the weapon's stance and runs on firing (SetState 2 has no `0x242858`; `0x22eca0` brings the arm up
/// on his run), and when the stick is let go he stops back into the stance (the walk's slow stop → `0x242858`).
/// The Blaster (arm layer 57 while moving) and the Tesla Claw (no moving arm layer: the holding layer keeps the claw).
#[test]
fn novalis_standing_fire_then_run() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let input = |t: u32| {
        let mut p = PadInput::neutral();
        if (110..=200).contains(&t) { p = p.stick(0.0, -1.0); }
        if (60..=300).contains(&t) { p = p.press(button::CIRCLE); }
        p
    };
    for (item, stance, shot_class) in [(15, 56u8, 305i16), (19, 51, -1)] {
        let s = Setup { item, at: spawn(&lv), watch: &[], shot_class, part_types: &[] };
        let rows = run(&lv, &s, &input, 320);
        dump(&rows);
        let firing = |r: &Row| if item == 15 { !r.shots.is_empty() } else { r.beam.0 };
        assert!(rows[90..110].iter().all(|r| r.state == 0 && r.seq == stance && r.out == (1, 1) && firing(r)), "item {item}: standing and firing: {:?}", rows[100].state);
        let run_from = rows[110..].iter().position(|r| r.state == 2).map(|k| k + 110).expect("never ran off while firing");
        assert!(run_from < 125, "item {item}: ran at {run_from}");
        let moved = ((rows[195].pos[0] - rows[105].pos[0]).powi(2) + (rows[195].pos[1] - rows[105].pos[1]).powi(2)).sqrt();
        assert!(moved > 3.0, "item {item}: moved {moved}");
        assert!(rows[140..195].iter().all(|r| r.state == 2 && r.out == (1, 1) && firing(r)), "item {item}: running and firing");
        if item == 15 {
            assert!(rows[140..195].iter().all(|r| r.layer == Some(57)), "the Blaster's arm layer 57 over the run");
            let ids: std::collections::BTreeSet<usize> = rows[140..195].iter().flat_map(|r| r.shots.iter().map(|x| x.0)).collect();
            assert!(ids.len() >= 5, "shots while running: {}", ids.len());
        } else {
            assert!(rows[140..195].iter().all(|r| r.hold[0].is_some_and(|h| h.2 == 1.0)), "the Tesla Claw in the holding arm");
        }
        assert!(rows[260..300].iter().all(|r| r.state == 0 && r.seq == stance), "item {item}: back in the stance: {:?}", (rows[260].state, rows[260].seq));
        assert_eq!(rows, run(&lv, &s, &input, 320), "deterministic");
    }
}

/// Regression (the R.Y.N.O. crash, 2026-09-28): a salvo's sounds — seven missiles' class sound 1 (class 457), 9 ticks
/// apart, and their seven blasts (its sound 0) together, next to the listener (a salvo into a near target) — mix
/// without an overflow. The engine panicked in the SPU's master stage (`rc_game::audio::master_out`: the 32-bit product
/// of the voices' sum and the master volume) once the sum passed about two full-scale voices; this test panics on
/// that arithmetic.
#[test]
fn novalis_ryno_salvo_sounds_mix_without_overflow() {
    use rc_game::audio::voices::{Listener, SoundSlots};
    use rc_game::audio::{AudioSystem, FrameInput, LevelAudio};
    use rc_game::moby_update::services::SoundEvent;
    let root = rc_formats::test_data::level_dir(1);
    let (Ok(bank), Some(c), Some(gp), Ok(header)) = (std::fs::read(root.join("sound_bank.bin")), rc_formats::test_data::core(1), rc_formats::test_data::gameplay(1), std::fs::read(root.join("level_header.bin"))) else {
        eprintln!("skipped: no extracted/");
        return;
    };
    let data = LevelAudio::from_parts(&bank, &c.index, &c.core, &c.data, &gp, &header, &Default::default(), None).unwrap();
    let mut sys = AudioSystem::new(data);
    sys.slots = SoundSlots::new();
    let pos = [150.0f32, 177.0, 41.0];
    let listener = Listener { pos: [pos[0], pos[1], pos[2] + 1.0], rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], ..Default::default() };
    let input = FrameInput { listener, hero_pos: pos };
    let mut rng = Rng::new();
    let (mut played, mut out, mut peak) = (0, Vec::new(), 0i32);
    for frame in 0..90u32 {
        // The salvo's seven launches, then its seven blasts at once.
        if frame % 9 == 0 && frame < 63 {
            let ev = SoundEvent { index: 1, flags: 0, moby: 900 + (frame / 9) as usize, o_class: 457, sound_class: 457, pos, tick: frame as u64 };
            if sys.play_class_sound(&ev, None, &listener, &mut rng) >= 0 { played += 1; }
        }
        if frame == 60 {
            for k in 0..7 {
                let ev = SoundEvent { index: 0, flags: 0, moby: 900 + k, o_class: 457, sound_class: 457, pos, tick: frame as u64 };
                if sys.play_class_sound(&ev, None, &listener, &mut rng) >= 0 { played += 1; }
            }
        }
        out.clear();
        sys.tick_with(&input, frame, &mut rng, &|_| Some(pos), &mut out);
        peak = peak.max(out.iter().map(|s| (s[0] as i32).abs().max((s[1] as i32).abs())).max().unwrap_or(0));
    }
    eprintln!("played {played}, peak {peak}");
    assert!(played >= 7, "the salvo's sounds play: {played}");
    assert!(peak > 0, "and are heard");
}

// ------------------------------------------------------------------------------------------------------------------
// Hand swaps (docs/plan/hero_gameplay.md §11.1): the holding layers, the take-out sounds

/// The holding layers and the hand item through a run with hand requests: per tick (hand id, [layer weight or None]).
fn swap_run(lv: &Lv, first: i32, more: &[i32], swaps: &[(u32, i32)], ticks: u32) -> Vec<Row> {
    let s = Setup { item: first, at: spawn(lv), watch: &[], shot_class: -1, part_types: &[] };
    run_swaps(lv, &s, &|_| PadInput::neutral(), ticks, true, more, swaps)
}

fn weights(r: &Row) -> [Option<f32>; 2] { r.hold.map(|h| h.map(|h| h.2)) }

/// `UpdateWrenchSelected` 0x2307e0 sets +0x34 on both holding nodes when it commits a swap: they fade out by 0.1 a
/// tick and are freed; the new item's layers (0x1413fb from its def, `HeroItemsCreate`) are made after that and fade
/// in by 0.1. Two-hand → wrench: none left; two-hand → one-hand: list 12 only; one-hand → two-hand: both; quick
/// repeats end on the last item's layers.
#[test]
fn novalis_swap_releases_the_holding_layers() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let full = |r: &Row, n: usize| weights(r).iter().filter(|w| **w == Some(1.0)).count() == n && weights(r).iter().flatten().count() == n;
    // Two-hand (Devastator 11) → the wrench.
    let rows = swap_run(&lv, 11, &[], &[(60, 8)], 140);
    dump(&rows);
    assert!(full(&rows[59], 2), "two hands before: {:?}", rows[59].hold);
    // The swap commits in tick 60's item update (after the layers' own upkeep): 0.1 less a tick from tick 61.
    let fade: Vec<f32> = rows[60..72].iter().map(|r| weights(r)[1].unwrap_or(0.0)).collect();
    assert!(fade.windows(2).all(|w| w[1] <= w[0]) && fade[0] == 1.0 && (fade[1] - 0.9).abs() < 1e-6 && fade[10] == 0.0, "fading out: {fade:?}");
    assert!(rows[75..].iter().all(|r| r.hand.0 == 8 && r.hold == [None, None]), "wrench: no holding layer {:?}", rows[80].hold);
    // Two-hand → one-hand (Blaster 15): list 13 gone for good, list 12 made again and faded in.
    let rows = swap_run(&lv, 11, &[15], &[(60, 15)], 140);
    assert!(rows[100..].iter().all(|r| r.hand.0 == 15 && weights(r)[0] == Some(1.0) && weights(r)[1].is_none()), "one hand: {:?}", rows[100].hold);
    let back = rows[60..100].iter().position(|r| weights(r)[0].is_none()).expect("the old list-12 layer is freed first");
    assert!(rows[60 + back + 1..100].iter().any(|r| weights(r)[0].is_some_and(|w| w < 1.0)), "then the new one fades in");
    // One-hand → two-hand.
    let rows = swap_run(&lv, 15, &[11], &[(60, 11)], 140);
    assert!(full(&rows[59], 1) && rows[59].hold[1].is_none());
    assert!(rows[100..].iter().all(|r| r.hand.0 == 11 && full(r, 2)), "two hands: {:?}", rows[100].hold);
    // Quick-select repeats: 11 → 15 → 11 → 8 → 23 a few ticks apart, then the wrench again.
    let swaps = [(60, 15), (64, 11), (68, 8), (72, 23), (160, 8)];
    let rows = swap_run(&lv, 11, &[15, 8, 23], &swaps, 220);
    dump(&rows);
    assert!(rows[130..160].iter().all(|r| r.hand.0 == 23 && full(r, 2)), "R.Y.N.O.: two hands {:?}", rows[130].hold);
    assert!(rows[190..].iter().all(|r| r.hand.0 == 8 && r.hold == [None, None]), "wrench at last: {:?}", rows[190].hold);
    assert_eq!(rows, swap_run(&lv, 11, &[15, 8, 23], &swaps, 220), "deterministic");
}

/// The swap's sounds are the new item's own: `HeroItemsCreate`'s `CreateMoby` runs `update_moby_animation_state` on
/// sequence 0 (the take-out), so its trigger words play as it runs (`MobyAnimAdvance` in `HeroItemsAttach`); the
/// put-away (sequence 2, one frame) has none and the wrench starts on sequence 1 (no take-out sounds). No other sound
/// call in the swap path (0x2307e0, 0x231088, 0x2305e8, 0x22f3c0) or the quick select (0x24d238).
#[test]
fn novalis_swap_take_out_sounds() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let swaps = [(60, 15), (120, 23), (180, 8), (240, 11)];
    let rows = swap_run(&lv, 11, &[15, 23, 8], &swaps, 300);
    let played: Vec<(usize, i16, i32)> = rows.iter().enumerate().flat_map(|(t, r)| r.sounds.iter().map(move |&(c, i)| (t, c, i))).collect();
    eprintln!("{played:?}");
    let of = |c: i16, from: usize, to: usize| played.iter().filter(|p| p.1 == c && (from..to).contains(&p.0)).map(|p| p.2).collect::<Vec<_>>();
    assert_eq!(of(168, 60, 120), vec![2, 3], "the Blaster's take-out");
    assert_eq!(of(454, 120, 180), vec![0, 1], "the R.Y.N.O.'s take-out");
    assert!(played.iter().all(|p| !(180..240).contains(&p.0)), "the wrench: none");
    assert_eq!(of(157, 240, 300), vec![2, 3], "the Devastator's take-out");
}
