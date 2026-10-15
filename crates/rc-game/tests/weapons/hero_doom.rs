//! The Glove of Doom (docs/plan/hero_gameplay.md §18), headless: the shared glove update (`0x2dd6c0` = the Bomb
//! Glove's `0x2d8330`) throwing the canister 230, the canister opening and letting out four bots 186, the bots finding,
//! chasing and blowing up creatures of two levels (Novalis 577, Rilgar 866), their effects and sounds. Harness as
//! `hero_gloves.rs` (the level as the engine ticks it; `GiveItem(id, equip)` on a first arrival's game state). Every
//! run twice: identical. Skipped when `extracted/` is absent; the numbers are distilled from the runs.


use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::save_game::{ChunkTables, ItemTables, SaveGameLump};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::game_state::{GameState, SessionState};
use rc_game::hero::anim::RatchetAnim;
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
    level: u32,
    reactions: std::collections::HashMap<i16, rc_game::moby_update::creature::react::Table>,
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

fn load() -> Option<Lv> { load_level(1) }

fn load_level(lvl: u32) -> Option<Lv> {
    let root = rc_formats::test_data::root();
    let dir = root.join(format!("levels/{lvl:02}"));
    let data = rc_formats::test_data::core_data(lvl)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(lvl)?;
    let settings = rc_formats::test_data::gameplay_section(lvl, "level_settings")?;
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
        let parsed = rc_formats::test_data::core_block(lvl, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
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
    let ratchet_blob = rc_formats::test_data::core_block(lvl, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&ratchet_blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(lvl, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
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
    state.apply_transition(lvl as i32);
    state.apply_level_start(lvl as i32, &tables, &mut session);
    let ov_target = rc_formats::level_overlay::LevelOverlay::parse(&ov1).ok()?;
    let ov_ref = rc_formats::level_overlay::LevelOverlay::parse(&std::fs::read(root.join("levels/01/overlay.bin")).ok()?).ok()?;
    let reactions = rc_game::moby_update::creature::react::tables_from_overlay(&ov_target, &ov_ref);
    Some(Lv { level: lvl, reactions, mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, seconds, items, weapon_defs, tables, state, session })
}

/// The hero's hit sink over the moby loop's services, as the engine's `CellHits` (with the guns' probe and class type).
struct CellHits<'a, 'b> {
    svc: &'a std::cell::RefCell<&'b mut Services>,
    classes: &'a ClassTable,
    coll: &'a collision::Collision,
    parts: &'a std::cell::RefCell<&'b mut Particles>,
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
    fn world(&mut self, table: &mut MobyTable, hero: &Hero, rng: &mut Rng, counter: u64, f: &mut dyn FnMut(&mut World)) -> bool {
        let mut s = self.svc.borrow_mut();
        let mut p = self.parts.borrow_mut();
        let mut w = World::new(table, hero, rng, self.classes, &mut s, counter);
        w.coll = Some(self.coll);
        w.particles = Some(&mut **p);
        f(&mut w);
        true
    }
}

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    pos: [f32; 3],
    /// The hand item: id, its state (+0x20), its key B sequence.
    hand: (i32, u8, u8),
    /// The ammo of the item given, the throws so far, the object in the glove.
    ammo: i32,
    throws: u32,
    held: Option<usize>,
    /// The watched mobys: (state, position, class, their target record's health).
    watched: Vec<(u8, [f32; 3], i16, f32)>,
    /// Ratchet's health.
    health: f32,
    /// The gloves' objects alive: (id, class, state, position, a pvar word of the class (the decoy's health, the mine's
    /// and the drone's target + 1), the ambient's red (the decoy's hit flash)).
    objects: Vec<(usize, i16, u8, [f32; 3], f32, u8)>,
    /// The reticles registered this tick.
    reticles: usize,
    /// The objects' class sounds of this tick (class, index, flags), the hero-side sounds ([`FakeSounds::heard`]).
    sounds: Vec<(i16, i32, u32)>,
    heard: Vec<(i16, i32)>,
    /// The particle spawns so far of the types in [`PARTS`], the explosion lights alive, the camera shake's timer.
    parts: [u64; 9],
    lights: usize,
    shake: i32,
    /// Bolts (classes 13..16) alive; mines the Taunter lures (+0x78).
    bolts: usize,
    lured: usize,
    rng: u32,
}

/// What a run gives and watches.
struct Setup<'a> {
    item: i32,
    at: ([f32; 3], f32),
    /// Moby instances to watch (their states).
    watch: &'a [usize],
    /// How long a hand-item class sound plays (the stand-in sound layer's `SoundIsAlive`; 0: no sound layer).
    sound_ticks: u64,
    /// A tougher record for the watched mobys after the load (health, the s16 +0x04): the other levels' big creatures
    /// (level 07's 871: 3 / 3, level 13's 111: 9 / 9, level 10's 1229: 20 / 20).
    record: Option<(f32, i16)>,
    /// A second item given too and asked for (the quick-select request 0x141408) at a tick.
    then: Option<(u32, i32)>,
    /// Lose the glove's object on the tick before the throw's trigger (the throw state 0x23 at tick 15): the case-3
    /// branch without an object.
    lose_held: bool,
    /// Before this tick, an explosion's hit record (flags 0x800000) for every bot alive (their `MobyGetHitMessage
    /// (0x800001)` sets them off where they are).
    boom_at: Option<u32>,
}

/// A stand-in sound layer: every hand-item class sound plays for `ticks` ticks (the Taunter's whistles, the Suck
/// Cannon's suction), so `SoundIsAlive` answers as an audio layer would; nothing is heard.
struct FakeSounds {
    ticks: u64,
    now: u64,
    started: Vec<u64>,
    /// The hero-side sounds of this tick: the hand item's class sounds (class, index) and Ratchet's voices (0, index).
    heard: Vec<(i16, i32)>,
}

impl rc_game::hero::HeroSounds for FakeSounds {
    fn anim_advanced(&mut self, _: &rc_game::moby_runtime::Moby, _: &rc_game::hero::anim::AnimView, _: &rc_game::hero::anim::AnimView, _: &mut Rng) {}
    fn voice(&mut self, _: &rc_game::moby_runtime::Moby, index: i32, _: u32, _: &mut Rng) -> i32 {
        self.heard.push((0, index));
        -1
    }
    fn item_sound(&mut self, o_class: i16, _: [f32; 3], index: i32, _: u32, _: &mut Rng) -> i32 {
        self.heard.push((o_class, index));
        if self.ticks == 0 { return -1; }
        self.started.push(self.now);
        self.started.len() as i32 - 1
    }
    fn alive(&mut self, slot: i32) -> bool { self.started.get(slot as usize).is_some_and(|&t| self.now < t + self.ticks) }
}

fn give(lv: &Lv, item: i32) -> (GameState, SessionState) {
    let (mut gs, mut session) = (lv.state.clone(), lv.session);
    gs.give_item(item as usize, true, &lv.tables, &mut session);
    // Buying the Drone Device launches its drones (the vendor 0x2af7e8: 0x141345 = 1).
    if item == 24 { session.drone = true; }
    (gs, session)
}

fn run(lv: &Lv, s: &Setup, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> Vec<Row> {
    let (mut gs, mut session) = give(lv, s.item);
    if let Some((_, second)) = s.then {
        gs.give_item(second as usize, false, &lv.tables, &mut session);
    }
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
    game.hero.idle.level = lv.level as _;
    game.hero.set_joint_targets(&lv.seconds);
    game.item_data = Some(lv.items.clone());
    game.hero.weapons.defs = lv.weapon_defs.clone();
    game.hero.owned.0 = gs.global.owned;
    game.hero.weapons.ammo = gs.global.ammo;
    for (i, r) in lv.tables.records.iter().enumerate() { game.hero.weapons.uses_ammo[i] = r.has_ammo(); }
    game.item_globals.wrench_flag = 1;
    game.item_globals.request = session.temp_hand;
    game.item_globals.saved = session.temp_hand;
    game.item_globals.drone = session.drone;
    let mut svc = Services::new();
    svc.level = lv.level;
    svc.creatures.react.tables = lv.reactions.clone();
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
    if let Some((h, s4)) = s.record {
        for &i in &watch {
            let m = &mut game.mobys.mobys[i];
            if let Some(r) = rc_game::targeting::record(m) {
                m.pvars[r..r + 4].copy_from_slice(&h.to_le_bytes());
                m.pvars[r + 4..r + 6].copy_from_slice(&s4.to_le_bytes());
            }
        }
    }
    let mut anim = RatchetAnim::new(&lv.ratchet);
    anim.layer_joints = rc_game::hero::anim::LAYER_LISTS.map(|l| lv.seconds[l as usize].clone());
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let parts_cell = std::cell::RefCell::new(&mut particles);
    let mut rows = Vec::new();
    let mut fake = FakeSounds { ticks: s.sound_ticks, now: 0, started: Vec::new(), heard: Vec::new() };
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
        if let Some((at, second)) = s.then { if t == at { game.item_globals.request = second; } }
        // The render's +0x31 ("drawn last frame", MobyProc's; the R.Y.N.O.'s search reads it): the watched mobys are
        // in view in these runs.
        for &i in &watch { if game.mobys.mobys[i].state < 0x80 { game.mobys.mobys[i].visible = 1; } }
        if s.boom_at == Some(t) {
            let tmpl = Tmpl { dir: [rc_game::ps2v::Pf::ZERO; 4], attacker: None, flags: 0x80_0000, b18: 2, b19: 1, h1a: 0, damage: rc_game::ps2v::Pf::f(2.0), w20: 0x80_0000 };
            let bots: Vec<usize> = game.mobys.mobys.iter().enumerate().filter(|(_, x)| x.o_class == 186 && x.state < 0xfd).map(|(i, _)| i).collect();
            let mut sv = svc_cell.borrow_mut();
            for i in bots { rc_game::moby_update::services::deliver_hit_in(&mut game.mobys, &mut sv.hits, i, &tmpl); }
        }
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh, parts: &parts_cell };
        fake.now = game.counter;
        fake.heard.clear();
        let sounds0 = svc_cell.borrow().sounds.len();
        let r = game.tick_with_hero_sounds(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits, None, &mut fake);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let tick = game.counter - 1;
        if s.lose_held && game.hero.state == 0x23 && game.hero.timer == 15 {
            if let Some(o) = game.hero.weapons.glove.held {
                let mut sv = svc_cell.borrow_mut();
                rc_game::moby_update::classes::bomb::delete_from_hero(&mut game.mobys, &mut sv, o, game.counter);
            }
        }
        let objects = game.mobys.mobys.iter().enumerate().filter(|(_, x)| OBJECTS.contains(&x.o_class) && x.state < 0xfd)
            .map(|(i, x)| (i, x.o_class, x.state, [x.position[0], x.position[1], x.position[2]], object_word(x), x.ambient[0])).collect();
        rows.push(Row {
            state: h.state,
            pos: h.position(),
            hand: h.items.slot.item.as_ref().map_or((h.items.slot.id, 0xff, 0xff), |x| (h.items.slot.id, x.mstate, x.anim.seq_b)),
            ammo: h.weapons.ammo[s.item as usize],
            throws: h.weapons.throws,
            held: h.weapons.glove.held,
            health: h.health as f32,
            watched: watch.iter().map(|&i| { let x = &game.mobys.mobys[i]; (x.state, [x.position[0], x.position[1], x.position[2]], x.o_class, rc_game::targeting::record(x).map_or(-1.0, |r| f32::from_le_bytes(x.pvars[r..r + 4].try_into().unwrap()))) }).collect(),
            objects,
            reticles: svc_cell.borrow().reticles.of_tick(tick).len(),
            sounds: svc_cell.borrow().sounds[sounds0..].iter().filter(|e| OBJECTS.contains(&e.o_class)).map(|e| (e.o_class, e.index, e.flags)).collect(),
            heard: fake.heard.clone(),
            parts: PARTS.map(|t| svc_cell.borrow().fx.part_spawns.get(&t).copied().unwrap_or(0)),
            lights: game.mobys.mobys.iter().filter(|x| x.o_class == rc_game::moby_update::creature::fx::LIGHT_CLASS && x.state < 0xfd).count(),
            shake: game.camera.shake.iter().map(|k| k.timer).max().unwrap_or(0),
            bolts: game.mobys.mobys.iter().filter(|x| (13..=16).contains(&x.o_class) && x.state < 0xfd).count(),
            lured: game.mobys.mobys.iter().filter(|x| x.o_class == 74 && x.state < 0xfd && x.pvars.len() >= 0x7c && x.pvars[0x78..0x7c] != [0; 4]).count(),
            rng: game.rng.state,
        });
    }
    rows
}

/// The particle types the rows count: the canister's glows (32), its pop (5), the beam explosion's sparks / streaks /
/// puffs (11 / 15 / 8), the debris fireballs' smoke (16), the water's bubbles / drops / scorch (34 / 35 / 64).
const PARTS: [u8; 9] = [32, 5, 11, 15, 8, 16, 34, 35, 64];

/// The Glove of Doom's canister 230 and its bots 186.
const OBJECTS: [i16; 2] = [230, 186];

/// The object's word the rows show: the canister's bots made (+0x3c), a bot's target (+0x38: the moby + 1).
fn object_word(m: &rc_game::moby_runtime::Moby) -> f32 {
    match m.o_class {
        230 if m.pvars.len() >= 0x40 => i32::from_le_bytes(m.pvars[0x3c..0x40].try_into().unwrap()) as f32,
        186 if m.pvars.len() >= 0x3c => i32::from_le_bytes(m.pvars[0x38..0x3c].try_into().unwrap()) as f32,
        _ => 0.0,
    }
}

/// The instance of the table's moby `id` (the moby loader's order).
#[allow(dead_code)]
fn instance_of(lv: &Lv, id: usize) -> usize {
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    statics.moby_to_instance[id]
}

/// A place `d` units from instance `inst` in direction `from`, facing it.
fn facing(lv: &Lv, inst: usize, d: f32, from: f32) -> ([f32; 3], f32) {
    let c = lv.instances[inst].position;
    let p = [c[0] + d * from.cos(), c[1] + d * from.sin(), c[2] + 0.5];
    (p, (c[1] - p[1]).atan2(c[0] - p[0]))
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_DOOM_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() { eprintln!("{t:4} {r:?}"); }
}

fn tap(at: &[u32]) -> impl Fn(u32) -> PadInput + '_ { move |t| if at.contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

/// The first instance of class `class` (the gameplay file's order).
#[allow(dead_code)]
fn first_of(lv: &Lv, class: i16, skip: usize) -> usize {
    lv.instances.iter().enumerate().filter(|(i, m)| m.o_class as i16 == class && lv.tests.get(*i).is_some_and(|t| t.spawn)).nth(skip).map(|(i, _)| i).expect("instance")
}

/// The states an object went through.
fn states_of(rows: &[Row], id: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for r in rows { if let Some(o) = r.objects.iter().find(|o| o.0 == id) { if v.last() != Some(&o.2) { v.push(o.2); } } }
    v
}

/// The first object thrown (held at the throw, then out of the glove).
fn thrown(rows: &[Row]) -> Option<(usize, usize)> {
    let t = rows.iter().position(|r| r.throws > 0)?;
    let id = rows[t - 1].held?;
    Some((t, id))
}

/// The first tick each bot appears, in order (a slot reused later counts again).
fn bot_births(rows: &[Row]) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = Vec::new();
    for (t, r) in rows.iter().enumerate() {
        for o in r.objects.iter().filter(|o| o.1 == 186) {
            let prev = t.checked_sub(1).and_then(|p| rows[p].objects.iter().find(|x| x.0 == o.0 && x.1 == 186).map(|x| x.2));
            if o.2 == 0 && prev != Some(0) { v.push((t, o.0)); }
        }
    }
    v
}

/// The ticks a bot blew up: it was there the tick before, is gone now, and its class sound 0 played.
fn blasts(rows: &[Row]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for t in 1..rows.len() {
        for o in rows[t - 1].objects.iter().filter(|o| o.1 == 186) {
            if !rows[t].objects.iter().any(|x| x.0 == o.0 && x.1 == 186 && x.2 != 0) && rows[t].sounds.contains(&(186, 0, 0)) { v.push((t, o.0)); }
        }
    }
    v
}

/// Novalis, the Glove of Doom (item 20, class 229) 7 units east of critter group 2 (moby 592), facing it: ○ throws
/// from the throw state 0x23 (voice 0x1a, one ammo; the next canister in the glove at once); the canister 230 flies
/// (2), glowing (type 32) every 4 ticks, stops and opens (3), lets out four bots 186 eight ticks apart and fades (4)
/// away. A bot takes the critter as its target (the target list's search), runs at it and blows up on it: the blast
/// (a light, 2 spark pairs, 5 streaks, 4 puffs, the shake, class sound 0) and the hit (damage 3: the critter's 1 →
/// −2), and the critter dies; the other bots go for the critters farther off.
#[test]
fn novalis_bots_blow_up_a_critter() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(lv.items.defs[20].o_class, 229, "item 20 is the Glove of Doom (class 229)");
    let b = instance_of(&lv, 592);
    let at = facing(&lv, b, 7.0, 0.0);
    let s = Setup { item: 20, at, watch: &[b], sound_ticks: 0, record: None, then: None, lose_held: false, boom_at: None };
    let input = tap(&[40]);
    let rows = run(&lv, &s, &input, 360);
    dump(&rows);
    let (t, c) = thrown(&rows).expect("never threw");
    assert!(rows[t - 16..t].iter().all(|r| r.state == 0x23), "thrown from the throw state");
    assert!(rows[t].heard.contains(&(0, 0x1a)), "the throw voice");
    assert_eq!(rows[t].ammo + 1, rows[30].ammo, "one ammo a throw");
    let next = rows[t].held.expect("a new canister in the glove");
    assert_ne!(next, c);
    assert_eq!(states_of(&rows, c), [0, 1, 2, 3, 4], "held, flew, opened, faded");
    assert!(rows.last().unwrap().objects.iter().all(|o| o.0 != c || o.1 != 230), "gone");
    let made = rows.iter().filter_map(|r| r.objects.iter().find(|o| o.0 == c && o.1 == 230).map(|o| o.4)).fold(0.0f32, f32::max);
    assert_eq!(made, 4.0, "four bots let out");
    let births = bot_births(&rows);
    assert_eq!(births.len(), 4, "{births:?}");
    let bt: Vec<usize> = births.iter().map(|b| b.0).collect();
    assert_eq!([bt[1] - bt[0], bt[2] - bt[1], bt[3] - bt[2]], [8, 8, 8], "8 ticks apart");
    let glows_held = rows[30].parts[0];
    let glows_out = rows[bt[0]].parts[0];
    assert!(glows_held > 0 && glows_out > glows_held, "type-32 glows held and flying: {glows_held} / {glows_out}");
    // The bot that took the critter (its target word = moby 592 + 1) and its blast.
    let hunter = births.iter().map(|b| b.1).find(|&x| rows.iter().any(|r| r.objects.iter().any(|o| o.0 == x && o.1 == 186 && o.4 == 593.0))).expect("a bot took the critter");
    let (k, _) = *blasts(&rows).iter().find(|b| b.1 == hunter).expect("the hunter blew up");
    let (r0, r1) = (&rows[k - 1], &rows[k]);
    assert_eq!((r0.watched[0].3, r1.watched[0].3), (1.0, -2.0), "damage 3");
    assert!(r1.lights > r0.lights, "the explosion light");
    let d: Vec<u64> = (0..9).map(|i| r1.parts[i] - r0.parts[i]).collect();
    assert!(d[2] >= 4 && d[3] >= 5 && d[4] >= 4, "sparks, streaks, puffs: {d:?}");
    assert!(r1.shake > 0, "the shake");
    assert!(rows[k..].iter().any(|r| r.watched[0].0 >= 0xfd), "the critter dies");
    assert!(blasts(&rows).len() >= 2, "another bot blew up on another critter");
    assert_eq!(rows, run(&lv, &s, &input, 360), "deterministic");
}

/// Rilgar, the small amoeboid 866 (instance 1262) 6 units ahead: the canister lands by it and the first bot out
/// touches it (a creature's primitive in its sphere) and blows up at once: the amoeboid's record health goes to 0 and
/// it dies; the other bots then go for another creature.
#[test]
fn rilgar_bot_blows_up_an_amoeboid() {
    let Some(lv) = load_level(5) else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, 1262, 6.0, -std::f32::consts::FRAC_PI_2);
    let s = Setup { item: 20, at, watch: &[1262], sound_ticks: 0, record: None, then: None, lose_held: false, boom_at: None };
    let input = tap(&[30]);
    let rows = run(&lv, &s, &input, 300);
    dump(&rows);
    let (_, c) = thrown(&rows).expect("never threw");
    assert_eq!(states_of(&rows, c), [0, 1, 2, 3, 4], "held, flew, opened, faded");
    let births = bot_births(&rows);
    let (k, x) = blasts(&rows)[0];
    assert_eq!(x, births[0].1, "the first bot");
    assert!(rows[k].watched[0].0 >= 0xfd && rows[k - 1].watched[0].0 < 0xfd, "the amoeboid dies with the blast");
    assert!(rows[k].watched[0].3 < rows[k - 1].watched[0].3, "its health taken");
    assert!(rows.iter().all(|r| r.health == 4.0), "Ratchet untouched");
    assert!(births.len() == 4, "four bots: {births:?}");
    assert_eq!(rows, run(&lv, &s, &input, 300), "deterministic");
}

/// The Glove of Doom with its canister lost before the trigger (the throw state 0x23 at tick 16): class sound 0 (the
/// empty click, `fun_0022da68(0, 0, glove)`), nothing thrown, state 4, a new canister from the tail (ammo).
#[test]
fn novalis_glove_of_doom_clicks_without_a_canister() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let b = instance_of(&lv, 592);
    let at = facing(&lv, b, 7.0, 0.0);
    let s = Setup { item: 20, at, watch: &[b], sound_ticks: 0, record: None, then: None, lose_held: true, boom_at: None };
    let input = tap(&[40]);
    let rows = run(&lv, &s, &input, 80);
    dump(&rows);
    let t = rows.iter().position(|r| r.ammo < rows[30].ammo).expect("no trigger");
    assert!(rows[t].heard.contains(&(229, 0)), "the click: {:?}", rows[t].heard);
    assert_eq!(rows[t].throws, 0, "nothing thrown");
    assert_eq!(rows[t].hand.1, 4, "state 4");
    assert!(rows[t].held.is_some(), "a new canister from the tail");
    assert!(rows[t..].iter().all(|r| r.objects.iter().all(|o| o.2 <= 1)), "no canister out");
    assert_eq!(rows, run(&lv, &s, &input, 80), "deterministic");
}

/// Novalis crate 376, the canister thrown from 3 units: it lands by the crate and its bots come out; an explosion's
/// hit (0x800000) given to every bot at tick 125 sets off the first one updated (its `MobyGetHitMessage(0x800001)`):
/// its blast's hit (flags 0x10000, damage 3, pushed from it) breaks the crate (its bolts fly) and, being stronger than
/// the 2-damage explosion records the other bots hold, replaces them (`0x26e968`'s damage rule), so those do not go
/// off: only one blast.
#[test]
fn novalis_bot_blast_breaks_a_crate() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, 376, 3.0, std::f32::consts::PI);
    let s = Setup { item: 20, at, watch: &[376], sound_ticks: 0, record: None, then: None, lose_held: false, boom_at: Some(125) };
    let input = tap(&[40]);
    let rows = run(&lv, &s, &input, 200);
    dump(&rows);
    let births = bot_births(&rows);
    assert_eq!(births.len(), 4, "{births:?}");
    let b = blasts(&rows);
    assert_eq!(b.len(), 1, "one blast: {b:?}");
    let (k, x) = b[0];
    assert_eq!((k, x), (125, births[0].1), "the first bot, at once");
    assert!(rows[k - 1].watched[0].0 < 0xfd && rows[k..k + 5].iter().any(|r| r.watched[0].0 >= 0xfd), "the crate broken");
    assert!(rows[k..].iter().any(|r| r.bolts > rows[k - 1].bolts), "its bolts");
    assert_eq!(rows[199].objects.iter().filter(|o| o.1 == 186).count(), 3, "the other bots stay");
    assert_eq!(rows, run(&lv, &s, &input, 200), "deterministic");
}
