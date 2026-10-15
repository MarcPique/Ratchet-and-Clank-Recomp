//! The Visibomb (docs/plan/hero_gameplay.md §17), headless: the gun (item 13, class 163), its missile 172 with the
//! type-6 camera, Ratchet's state 0x1d, the explosion on a crate and a critter, ○'s detonation, the steering, the range
//! limiter 832's static and its cut-off, the hand-back; on Novalis and Rilgar. The level runs as the engine ticks it
//! (harness as `hero_weapons4.rs`, with a recording sound sink for the moby loop's class sounds). Every run twice:
//! identical. Skipped when `extracted/` is absent. The numbers checked are distilled from the runs themselves.

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
    sound: &'a std::cell::RefCell<RecSink>,
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
        let mut snd = self.sound.borrow_mut();
        let mut w = World::new(table, hero, rng, self.classes, &mut s, counter);
        w.coll = Some(self.coll);
        w.particles = Some(&mut **p);
        w.sound = Some(&mut *snd);
        f(&mut w);
        true
    }
}

/// A class sound of the moby loop (or of the hand items' calls into it).
#[derive(Clone, Debug, PartialEq)]
enum Snd {
    Play { class: i16, index: i32, flags: u32, slot: i32 },
    Release(i32),
    Bend(i32, i32),
}

/// The moby loop's sound sink: every call recorded, slots handed out in order (no audio, no `rand` draws).
#[derive(Default)]
struct RecSink {
    next: i32,
    log: Vec<Snd>,
}

impl rc_game::moby_update::services::SoundSink for RecSink {
    fn play_class_sound(&mut self, ev: &rc_game::moby_update::services::SoundEvent, _: &mut Rng) -> i32 {
        let slot = self.next;
        self.next += 1;
        self.log.push(Snd::Play { class: ev.o_class, index: ev.index, flags: ev.flags, slot });
        slot
    }
    fn alive(&self, _: i32, _: usize) -> bool { true }
    fn release(&mut self, slot: i32, _: usize) { self.log.push(Snd::Release(slot)); }
    fn set_pitch_bend(&mut self, slot: i32, pb: i32) { self.log.push(Snd::Bend(slot, pb)); }
}

#[derive(Default)]
struct RecSounds(Vec<i32>);

impl rc_game::hero::HeroSounds for RecSounds {
    fn anim_advanced(&mut self, _: &rc_game::moby_runtime::Moby, _: &rc_game::hero::anim::AnimView, _: &rc_game::hero::anim::AnimView, _: &mut Rng) {}
    fn item_sound(&mut self, _: i16, _: [f32; 3], index: i32, _: u32, _: &mut Rng) -> i32 {
        self.0.push(index);
        -1
    }
}

use rc_game::moby_update::classes::visibomb as vb;

/// The missile: id, position, rotation (roll, pitch, yaw), +0xbc, timer, speed, target speed, hidden.
#[derive(Clone, Debug, PartialEq)]
struct Missile {
    id: usize,
    pos: [f32; 3],
    rot: [f32; 3],
    cmd: u8,
    timer: i32,
    speed: f32,
    target_speed: f32,
    hidden: bool,
}

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    group: i32,
    pos: [f32; 3],
    /// Ratchet's hand: item, its key B sequence; the ammo of item 13.
    hand: (i32, u8),
    ammo: i32,
    /// The glove-holding layers 0x140050 / 0x140054 (def +0x18 = 2: lists 12 and 13) made; 0x1413f5 (first person).
    hold: [bool; 2],
    fp: u8,
    missile: Option<Missile>,
    /// The glow 179: position and size factor.
    glow: Option<([f32; 3], f32)>,
    /// The camera: the type-6 camera up, its position / Euler and centre; the published position; the blend mode.
    type6: bool,
    cam: [f32; 3],
    cam_euler: [f32; 3],
    centre: [f32; 3],
    cur: [f32; 5],
    blend: u8,
    /// The globals: 0x17e988 and 0x15f608 set this tick, the view (overlay, no sky, fog writes, short far), the
    /// help box held, the prompt's owner, the range static drawn this tick (quads).
    hud_off: bool,
    all_visible: bool,
    view: (bool, bool, u32, bool),
    help_hold: bool,
    prompt: i32,
    statics: usize,
    range_counter: i32,
    /// The watched mobys: state, position.
    watched: Vec<(u8, [f32; 3])>,
    /// This tick's moby-loop sounds; the hand item's class sounds.
    sounds: Vec<Snd>,
    item_sounds: Vec<i32>,
    /// Live particles of types 13, 11, 15, 8; live flash mobys (0x70, 1192); point lights.
    parts: [usize; 4],
    flashes: usize,
    lights: usize,
    rng: u32,
}

struct Setup<'a> {
    at: ([f32; 3], f32),
    watch: &'a [usize],
}

fn give(lv: &Lv) -> (GameState, SessionState) {
    let (mut gs, mut session) = (lv.state.clone(), lv.session);
    gs.give_item(13, true, &lv.tables, &mut session);
    (gs, session)
}

fn f3(a: [f32; 4]) -> [f32; 3] { [a[0], a[1], a[2]] }

fn run(lv: &Lv, s: &Setup, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> Vec<Row> {
    let (gs, session) = give(lv);
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
    let mut svc = Services::new();
    svc.level = lv.level;
    svc.creatures.react.tables = lv.reactions.clone();
    svc.set_splines(&lv.splines);
    svc.groups = statics.groups(&lv.gp);
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    if let Ok(v) = rc_formats::volumes::parse_volumes(&lv.gp) { svc.set_volumes(v); }
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
    let sound_cell = std::cell::RefCell::new(RecSink::default());
    let range = game.mobys.mobys.iter().position(|m| m.o_class == 832 && m.state < 0x80);
    let mut rows = Vec::new();
    for t in 0..ticks {
        let classes_ref: &ClassTable = &classes;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut p = parts_cell.borrow_mut();
            let mut snd = sound_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.particles = Some(&mut **p);
            w.sound = Some(&mut *snd);
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
        for &i in &watch { if game.mobys.mobys[i].state < 0x80 { game.mobys.mobys[i].visible = 1; } }
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh, parts: &parts_cell, sound: &sound_cell };
        let mut rec = RecSounds::default();
        let n0 = sound_cell.borrow().log.len();
        let r = game.tick_with_hero_sounds(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits, None, &mut rec);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let tick = game.counter - 1;
        let svc = svc_cell.borrow();
        let g = &svc.visibomb;
        let missile = game.mobys.mobys.iter().enumerate().find(|(_, m)| m.o_class == vb::CLASS && m.state < 0x80).map(|(id, m)| {
            let pf = |o: usize| rc_game::moby_update::services::pvar::ff(&m.pvars, o);
            Missile {
                id,
                pos: f3(m.position),
                rot: f3(m.rotation),
                cmd: m.cmd,
                timer: rc_game::moby_update::services::pvar::i32(&m.pvars, vb::pv::TIMER),
                speed: pf(vb::pv::SPEED),
                target_speed: pf(vb::pv::TARGET_SPEED),
                hidden: m.mode & 1 != 0,
            }
        });
        let glow = game.mobys.mobys.iter().find(|m| m.o_class == 179 && m.state < 0x80).map(|m| (f3(m.position), rc_game::moby_update::services::pvar::ff(&m.pvars, 0x0c)));
        let p = parts_cell.borrow();
        let count = |ty: u8| (0..=p.pool.hw.max(-1)).map(|i| &p.pool.recs[i as usize]).filter(|r| r[0] == ty && r[1] & 0x80 == 0).count();
        let c6 = &game.camera.type6;
        rows.push(Row {
            state: h.state,
            group: h.group,
            pos: h.position(),
            hand: (h.items.slot.id, h.items.slot.item.as_ref().map_or(0xff, |it| it.anim.seq_b)),
            ammo: h.weapons.ammo[13],
            hold: [h.weapons.layers[2].is_some(), h.weapons.layers[3].is_some()],
            fp: h.f13f5,
            missile,
            glow,
            type6: c6.active,
            cam: game.camera.out.pos_f32(),
            cam_euler: [0, 1, 2].map(|k| game.camera.out.euler[k].to_f32()),
            centre: c6.centre,
            cur: c6.cur,
            blend: game.camera.blend.mode,
            hud_off: g.hud_off_at == Some(tick),
            all_visible: svc.occlusion_fallback == Some((tick, 1)),
            view: (g.view.overlay, g.view.no_sky, g.view.fog_seq, g.view.short_far),
            help_hold: svc.help.hold,
            prompt: svc.interact.prompt.owner,
            statics: if g.static_at == Some(tick) { g.static_quads.len() } else { 0 },
            range_counter: range.map_or(-1, |r| rc_game::moby_update::services::pvar::i32(&game.mobys.mobys[r].pvars, rc_game::moby_update::classes::rc_range::COUNTER)),
            watched: watch.iter().map(|&i| (game.mobys.mobys[i].state, f3(game.mobys.mobys[i].position))).collect(),
            sounds: sound_cell.borrow().log[n0..].to_vec(),
            item_sounds: rec.0,
            parts: [count(13), count(11), count(15), count(8)],
            flashes: game.mobys.mobys.iter().filter(|m| rc_game::moby_update::classes::debris::FLASH_CLASSES.contains(&m.o_class) && m.state < 0x80).count(),
            lights: svc.point_lights.active().count(),
            rng: game.rng.state,
        });
    }
    rows
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_VB_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() { eprintln!("{t:4} {r:?}"); }
}

/// Ratchet's spawn and its yaw.
fn spawn(lv: &Lv) -> ([f32; 3], f32) {
    let h = lv.instances.iter().find(|m| m.o_class == 0).unwrap();
    ([h.position[0], h.position[1], h.position[2]], h.rotation[2])
}

/// Crate 376 (class 500 below the Novalis spawn plateau) and a place `d` units north of it, facing it.
const CRATE: usize = 376;
fn facing_crate(lv: &Lv, d: f32) -> ([f32; 3], f32) {
    let c = lv.instances[CRATE].position;
    ([c[0], c[1] + d, c[2]], -std::f32::consts::FRAC_PI_2)
}

/// A place `d` units from instance `inst` in direction `from`, facing it.
#[allow(dead_code)]
fn facing_moby(lv: &Lv, inst: usize, d: f32, from: f32) -> ([f32; 3], f32) {
    let c = lv.instances[inst].position;
    let p = [c[0] + d * from.cos(), c[1] + d * from.sin(), c[2] + 0.5];
    (p, (c[1] - p[1]).atan2(c[0] - p[0]))
}

/// The instance of the table's moby `id` (the moby loader's order).
#[allow(dead_code)]
fn instance_of(lv: &Lv, id: usize) -> usize {
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    statics.moby_to_instance[id]
}

fn press_at(ticks: &'static [u32]) -> impl Fn(u32) -> PadInput { move |t| if ticks.contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

fn launched(rows: &[Row]) -> usize { rows.iter().position(|r| r.missile.is_some()).expect("no missile launched") }

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

// ------------------------------------------------------------------------------------------------------------------

/// The item table 0x179f40 gives item 13 the gun class 163; the level has the missile 172 (update 0x2cbda8) and the
/// glow 179, both on the port's class updates.
#[test]
fn visibomb_is_item_13_with_its_classes() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(lv.items.defs[13].o_class, 163);
    assert!(lv.items.classes.iter().any(|c| c.o_class == 163), "the gun class is a gadget class of the level");
    let (gs, session) = give(&lv);
    assert_eq!((gs.global.owned[13], session.temp_hand), (1, 13));
    assert!(gs.global.ammo[13] > 0, "given with its ammo");
    use rc_game::moby_update::services::ClassData;
    for (oc, f) in [(vb::CLASS, vb::UPDATE_FN), (179, 0x2d1068)] {
        assert!(lv.classes.info(oc).is_some(), "class {oc} loaded");
        assert_eq!(scheduler::port_update_fn(oc), Some(f), "class {oc}");
    }
    eprintln!("weapon def 13: {:?}", lv.weapon_defs[13]);
}

/// Standing at the spawn: ○ launches the missile 0.6 ahead of the gun along Ratchet's facing, one ammo; in the same
/// tick Ratchet is in 0x1d (group 9, no control, the gun kept), the type-6 camera is up at the missile, the HUD and the
/// occlusion flags are set, the missile view's look is on (the fog swapped), the help box is held and the prompt is
/// the Visibomb's (owner 6); the loop voice (class sound 0, flags 4) plays and its pitch bend follows every tick; the
/// glow 179 rides it. The flight goes on with the pad neutral.
#[test]
fn novalis_launch() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { at: spawn(&lv), watch: &[] };
    let input = press_at(&[60]);
    let rows = run(&lv, &s, &input, 160);
    dump(&rows);
    let l = launched(&rows);
    assert_eq!(l, 60, "launched on the press");
    let (r0, r) = (&rows[l - 1], &rows[l]);
    assert!(rows[..l].iter().all(|r| r.state != 0x1d && !r.type6 && !r.hud_off));
    assert_eq!(r.ammo, r0.ammo - 1, "one ammo");
    assert_eq!((r.state, r.group), (0x1d, 9), "Ratchet held");
    assert_eq!(r.hand.0, 13, "the gun stays in his hand");
    assert_eq!(r.hand.1, 3, "the gun's launch sequence 3");
    let m = r.missile.as_ref().unwrap();
    let yaw = s.at.1;
    // polar(0.6, yaw, 0) from the gun: 0.6 ahead along his facing.
    let ahead = (m.pos[0] - r0.pos[0]) * yaw.cos() + (m.pos[1] - r0.pos[1]) * yaw.sin();
    assert!(ahead > 0.4 && ahead < 1.2, "launch point {:?} from {:?}", m.pos, r0.pos);
    assert_eq!((m.timer, m.cmd, m.hidden), (3000, 0, false));
    assert!((m.rot[2] - yaw).abs() < 1e-3 && m.rot[1] == 0.0, "rotation {:?}", m.rot);
    assert!(r.type6, "the type-6 camera switched in");
    assert!(dist(r.centre, m.pos) < 0.2 && dist(r.cam, r.centre) < 0.05, "camera {:?} centre {:?} missile {:?}", r.cam, r.centre, m.pos);
    assert!(r.hud_off && r.all_visible, "0x17e988 / 0x15f608");
    assert_eq!(r.view, (true, true, 1, true), "0x15f30c, no sky, the fog swap, the short far");
    assert!(r.help_hold && r.prompt == 6);
    let voice = r.sounds.iter().find_map(|s| match *s { Snd::Play { class: vb::CLASS, index: 0, flags: 4, slot } => Some(slot), _ => None }).expect("the loop voice");
    assert!(r.glow.is_some(), "the glow 179");
    // In flight: the pitch bend every tick, the HUD off every tick, the speed rising toward 24 u/s.
    for (k, x) in rows[l + 1..l + 60].iter().enumerate() {
        assert!(x.hud_off && x.all_visible && x.type6 && x.state == 0x1d, "tick {k}: {x:?}");
        assert!(x.sounds.contains(&Snd::Bend(voice, -199)), "tick {k}: {:?}", x.sounds);
    }
    let ms = |t: usize| rows[t].missile.as_ref().unwrap().clone();
    assert!(ms(l + 1).speed > 0.0 && ms(l + 60).speed > ms(l + 30).speed && ms(l + 60).speed < 24.0 / 60.0);
    // The glow eases to 0.45 of its size by 0.05 a tick (then steps across it: the f32 sum of 0.05s misses 0.45, as
    // the game's does) and sits 0.11 from the missile.
    let (gp, gs) = rows[l + 30].glow.unwrap();
    assert!((gs - 0.45).abs() <= 0.05 + 1e-4 && (dist(gp, ms(l + 30).pos) - 0.11).abs() < 1e-3, "glow {gp:?} {gs}");
    assert_eq!(rows[l + 1].glow.unwrap().1, 0.05);
    assert_eq!(rows, run(&lv, &s, &input, 160), "deterministic");
}

/// Launched 8 units from crate 376 and dived onto it: the crate breaks (the path's hit and the
/// blast); the explosion: the loop voice released, `SpawnBeamExplosion` (the class sound 1, its streaks, sparks,
/// puffs, flashes and light), the six puffs (type 13) and the rings (type 11) of `0x2cb968` five times, the missile
/// hidden; the camera orbits the blast 6 away; ~100 ticks later the flight ends: Ratchet back in 0 with control, the
/// type-6 camera handed back (a cut: he is behind it), the look restored with the far distances, the help box
/// resumed, the missile and the glow gone.
#[test]
fn novalis_explosion_breaks_a_crate() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { at: facing_crate(&lv, 8.0), watch: &[CRATE] };
    // The crate stands in a dip below Ratchet: the stick pushed forward a moment dives the missile onto it.
    let input = |t: u32| match t {
        60 => PadInput::neutral().press(button::CIRCLE),
        66..=80 => PadInput::neutral().stick(0.0, -1.0),
        _ => PadInput::neutral(),
    };
    let rows = run(&lv, &s, &input, 300);
    dump(&rows);
    let l = launched(&rows);
    let boom = rows.iter().position(|r| r.missile.as_ref().is_some_and(|m| m.hidden)).expect("no explosion");
    assert!(boom > l + 30, "exploded at {boom} (launched {l})");
    assert_ne!(rows[boom].watched[0].0, rows[0].watched[0].0, "the crate broke");
    let r = &rows[boom];
    let m = r.missile.as_ref().unwrap();
    assert_eq!(m.timer, -1, "a long flight: the orbit");
    assert!(r.sounds.iter().any(|s| matches!(s, Snd::Release(_))), "the loop voice released: {:?}", r.sounds);
    assert!(r.sounds.iter().any(|s| matches!(s, Snd::Play { class: vb::CLASS, index: 1, .. })), "the explosion sound: {:?}", r.sounds);
    assert!(r.parts[0] >= 30, "5 × 6 puffs (type 13): {:?}", r.parts);
    assert!(r.parts[1] > 0 && r.parts[2] > 0 && r.parts[3] > 0, "rings, streaks, puffs: {:?}", r.parts);
    assert!(r.flashes >= 5 * 3 + 2, "the flashes of 0x2cb968 (five times) and of SpawnBeamExplosion: {}", r.flashes);
    assert!(rows[boom + 1].lights > 0, "the explosion light (radius 20)");
    // The orbit: 6 from the blast (+ the push), looking at it; the look restored (the fog written back).
    let o = &rows[boom + 5];
    assert!(o.type6 && (dist(o.cam, o.centre) - 6.0).abs() < 0.8, "orbit {:?} centre {:?}", o.cam, o.centre);
    assert_eq!((o.view.0, o.view.1, o.view.3), (false, false, true), "the look restored but the far distances kept");
    assert!(o.view.2 > r.view.2, "fog writes");
    assert!(o.hud_off && o.state == 0x1d);
    let end = boom + rows[boom..].iter().position(|r| r.missile.is_none() && r.state == 0).expect("the flight never ended");
    assert_eq!(end, boom + 99, "ended ticks(100) − 1 after the blast");
    let e = &rows[end];
    // The orbit looks at the blast from Ratchet's side, so he is behind the camera: `0x317e70` cuts (+0x7e = 4) and the
    // follow camera snaps behind him.
    let prev = &rows[end - 1];
    assert!(!rc_game::follow_camera::type6::release_blends(prev.cam, prev.cam_euler[2], prev.pos));
    assert!(!e.type6 && e.blend == 0 && dist(e.cam, e.pos) < 3.5, "the hand-back cuts behind Ratchet: {e:?}");
    assert!(!e.help_hold && e.view == (false, false, e.view.2, false));
    assert!(e.glow.is_none() || rows[end + 1].glow.is_none(), "the glow deletes itself");
    assert!(!rows[end + 1].hud_off);
    assert_eq!(rows, run(&lv, &s, &input, 300), "deterministic");
}

/// Pulled up with ✕ held from the spawn: the missile climbs out of the range limiter 832's areas (Novalis: paths 64–67
/// up to 106 high, cuboid 71). From the first tick outside its counter runs and, with the missile view's flag set, its
/// static is drawn (a 17 × 14 grid of 32-pixel quads, alpha `counter·127.5/90`, two `randi(32)` each); the tick the
/// counter passes `ticks(90)` the flight ends (`0x2cb788`): no explosion, Ratchet back, the camera handed back.
#[test]
fn novalis_range_limit_ends_the_flight() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { at: spawn(&lv), watch: &[] };
    let input = |t: u32| match t {
        60 => PadInput::neutral().press(button::CIRCLE),
        66.. => PadInput::neutral().stick(0.0, 1.0).press(button::CROSS),
        _ => PadInput::neutral(),
    };
    let rows = run(&lv, &s, &input, 700);
    dump(&rows);
    let l = launched(&rows);
    // ✕: the target speed toward 34 u/s.
    assert!(rows[l + 40].missile.as_ref().unwrap().target_speed > 30.0 / 60.0, "{:?}", rows[l + 40].missile);
    // Pulled up: the pitch at its limit (−89.94°).
    assert_eq!(rows[l + 160].missile.as_ref().unwrap().rot[1], -vb::PITCH_LIMIT, "{:?}", rows[l + 160].missile);
    let first = rows.iter().position(|r| r.range_counter >= 1).expect("never out of range");
    assert!(rows[..first].iter().all(|r| r.statics == 0));
    let end = l + rows[l..].iter().position(|r| r.missile.is_none() && r.state != 0x1d).expect("never ended");
    // The last stretch outside: the counter 1, 2, …, 90, then the end.
    let out = end - 90;
    for (k, r) in rows[out..end].iter().enumerate() { assert_eq!(r.range_counter, k as i32 + 1, "tick {}", out + k); }
    assert_eq!(rows[out - 1].range_counter, 0, "back in range before the last stretch");
    let r = &rows[out + 10];
    assert_eq!(r.statics, 17 * 14, "the static's grid");
    assert!(rows[out..end].iter().all(|r| r.missile.as_ref().is_some_and(|m| !m.hidden)), "no explosion");
    assert!(!rows[end].type6 && rows[end].range_counter == 0 || rows[end + 1].range_counter == 0);
    assert!(rows[end + 2].statics == 0);
    assert_eq!(rows, run(&lv, &s, &input, 700), "deterministic");
}

/// The stick steers from the 6th tick of the flight: the smoothed stick eases 10 % a tick, the yaw turns by
/// x·0.7° a tick (right: clockwise), the roll eases 6 % a tick toward x; pulled back (y +1) the pitch turns up (the
/// pitch is positive down).
/// ○ before the 60th tick does nothing; after it, the missile explodes where it is.
#[test]
fn novalis_steering_and_circle() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { at: spawn(&lv), watch: &[] };
    let input = |t: u32| match t {
        60 => PadInput::neutral().press(button::CIRCLE),
        61..=90 => PadInput::neutral().stick(1.0, 0.0),
        91..=100 => PadInput::neutral().stick(0.0, 1.0),
        110 => PadInput::neutral().press(button::CIRCLE),
        140 => PadInput::neutral().press(button::CIRCLE),
        _ => PadInput::neutral(),
    };
    let rows = run(&lv, &s, &input, 200);
    dump(&rows);
    let l = launched(&rows);
    let m = |t: usize| rows[t].missile.clone().unwrap();
    let yaw0 = m(l).rot[2];
    for t in l + 1..=l + 5 { assert_eq!(m(t).rot, m(l).rot, "no steering in the first 5 ticks ({t})"); }
    // The stick read by the moby loop is last tick's hero stick: 1.0 from tick 61 on, first used at l + 2.
    let mut sx = 0.0f32;
    let mut yaw = yaw0;
    let mut roll = 0.0f32;
    for t in l + 6..=l + 20 {
        sx += (1.0 - sx) * 0.1;
        roll += (sx - roll) * f32::from_bits(0x3d75_c28f);
        yaw -= sx * vb::TURN;
        let r = m(t).rot;
        assert!((r[2] - yaw).abs() < 1e-5 && (r[0] - roll).abs() < 1e-5 && r[1] == 0.0, "tick {t}: {r:?} want yaw {yaw} roll {roll}");
    }
    assert!(m(l + 40).rot[1] < -0.02, "pulled back: nose up {:?}", m(l + 40).rot);
    // ○ at 110 (the 50th flight tick) is ignored; at 140 it explodes.
    assert!(!m(110).hidden && !m(139).hidden && m(140).hidden, "{:?} {:?}", m(110), m(140));
    assert_eq!(m(140).timer, -1);
    // The camera follows the turn: its yaw trails the missile's on the 0.07 spring.
    assert!(rows[l + 30].cam_euler[2] < yaw0 && rows[l + 30].cam_euler[2] > m(l + 30).rot[2] - 0.01, "{:?} {:?}", rows[l + 30].cam_euler, m(l + 30).rot);
    assert_eq!(rows, run(&lv, &s, &input, 200), "deterministic");
}

/// At a critter 577 in the pit (8 units away since the Water Pump Worker's head look draws from the stream,
/// 2026-09-30): the missile flies into it; the critter is hit (the path's record and the blast) and
/// reacts.
#[test]
fn novalis_explosion_hits_a_critter() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (instance_of(&lv, 590), instance_of(&lv, 592));
    let at = facing_moby(&lv, b, 8.0, std::f32::consts::PI);
    let s = Setup { at, watch: &[a, b] };
    let input = press_at(&[60]);
    let rows = run(&lv, &s, &input, 220);
    dump(&rows);
    let boom = rows.iter().position(|r| r.missile.as_ref().is_some_and(|m| m.hidden)).expect("no explosion");
    assert!(rows[boom..].iter().any(|r| r.watched.iter().zip(&rows[0].watched).any(|(x, y)| x.0 != y.0)), "no critter reacted");
    assert_eq!(rows, run(&lv, &s, &input, 220), "deterministic");
}

/// On Rilgar (level 5; its range limiter 832 and the lighter scanline record 0x16cc30): the launch, the flight in
/// 0x1d under the type-6 camera, the explosion (into the wall ahead of the spawn, before the ○ at 140), the orbit and
/// the end.
#[test]
fn rilgar_launch_detonate_and_hand_back() {
    let Some(lv) = load_level(5) else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { at: spawn(&lv), watch: &[] };
    let input = press_at(&[60, 140]);
    let rows = run(&lv, &s, &input, 300);
    dump(&rows);
    let l = launched(&rows);
    assert_eq!(l, 60);
    let boom = rows.iter().position(|r| r.missile.as_ref().is_some_and(|m| m.hidden)).expect("no explosion");
    assert!(boom <= 140, "exploded at {boom} (a wall ahead, or ○)");
    assert!(rows[l..=boom].iter().all(|r| r.state == 0x1d && r.type6 && r.hud_off && r.view.0), "the flight");
    let end = boom + rows[boom..].iter().position(|r| r.missile.is_none()).expect("never ended");
    assert!(rows[end].state != 0x1d && !rows[end].type6 && !rows[end].help_hold);
    assert_eq!(rows, run(&lv, &s, &input, 300), "deterministic");
}

/// Held, the Visibomb's def +0x18 = 2 makes both glove-holding layers (lists 12 and 13). In first person (L1: the
/// look stance 1 with Ratchet hidden by the first-person camera) ○ launches along the view from the eye + 0.3 ahead
/// − 0.35 up, with the view's yaw and pitch.
#[test]
fn novalis_holding_layers_and_first_person_launch() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { at: spawn(&lv), watch: &[] };
    let input = |t: u32| {
        let mut p = PadInput::neutral();
        if (20..=120).contains(&t) { p = p.press(button::L1); }
        if t == 90 { p = p.press(button::L1 | button::CIRCLE); }
        p
    };
    let rows = run(&lv, &s, &input, 130);
    dump(&rows);
    assert_eq!(rows[15].hold, [true, true], "both holding layers while it is held");
    let l = launched(&rows);
    let prev = &rows[l - 1];
    assert_eq!((prev.state, prev.fp), (1, 1), "first person before the launch");
    let m = rows[l].missile.clone().unwrap();
    // The eye 0x167240 of the last camera update, and its forward row from the Euler (yaw, pitch positive down).
    let (p, y) = (prev.cam_euler[1], prev.cam_euler[2]);
    let fwd = [p.cos() * y.cos(), p.cos() * y.sin(), -p.sin()];
    let d = [m.pos[0] - prev.cam[0], m.pos[1] - prev.cam[1], m.pos[2] - prev.cam[2]];
    let along = d[0] * fwd[0] + d[1] * fwd[1] + d[2] * fwd[2];
    assert!((along - 0.3).abs() < 0.05 && d[2] < 0.0 && dist(m.pos, prev.cam) < 0.5, "launch point {:?} eye {:?}", m.pos, prev.cam);
    assert!((m.rot[2] - y).abs() < 1e-3 && (m.rot[1] - p).abs() < 1e-3, "rotation {:?} view {:?}", m.rot, prev.cam_euler);
    assert_eq!(rows, run(&lv, &s, &input, 130), "deterministic");
}
