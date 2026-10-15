//! Weapons batch 4 (docs/plan/hero_gameplay.md §14–§16), headless. The Walloper and the gadget lunge 0x20: the weapon check's case 0x12,
//! the lunge's timing (speed window, after-images, hit window, the end past frame 20), its three hit spheres through the
//! hit records (a critter 577 and a crate on Novalis, a small amoeboid 866 on Rilgar), the item's swing and hit sounds,
//! the sparkles on creatures, the item update's arcs and glow, the draw registration and its flicker draw. Harness as
//! `hero_morph.rs`. Every run twice: identical. Skipped when `extracted/` is absent.

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
    timer: i32,
    pos: [f32; 3],
    speed: f32,
    hand: (i32, u8, u8),
    /// The watched mobys: (state, position, class, hit slot).
    watched: Vec<(u8, [f32; 3], i16)>,
    /// The after-image record: active, ghosts (alpha, back).
    trail: (bool, Vec<(u8, i32)>),
    /// The Walloper: live arcs, glow, drawn this tick, stats (spheres, listed, sparkle bursts, hit sounds).
    arcs: usize,
    glow: f32,
    drawn: bool,
    stats: [u32; 4],
    listed: Vec<(usize, i16)>,
    /// The hand item's class sounds this tick (index).
    sounds: Vec<i32>,
    /// The flicker the last frame's draw made.
    dim: Option<bool>,
    quads: usize,
    /// The screen markers registered this tick (colour, at the screen centre), the hand item's ammo, the R.Y.N.O.'s
    /// missiles alive.
    markers: Vec<(u32, bool)>,
    ammo: i32,
    missiles: usize,
    /// Mobys alive of the classes 122 (fireballs), 1192 (flashes), 775 (splashes), 660 (flyers); the live type-4 and
    /// type-15 particles.
    fx_mobys: [usize; 4],
    parts: [u32; 2],
    rng: u32,
}

struct Setup<'a> {
    item: i32,
    at: ([f32; 3], f32),
    watch: &'a [usize],
    /// At tick t, the hand item's state byte +0x20 set to s (the R.Y.N.O.'s unreachable state 3).
    force: Option<(u32, u8)>,
    /// At tick t, a hit (flags, damage) delivered to the first live moby of class c (`FUN_0026e968`).
    hit: Option<(u32, i16, u32, f32)>,
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

fn give(lv: &Lv, item: i32) -> (GameState, SessionState) {
    let (mut gs, mut session) = (lv.state.clone(), lv.session);
    gs.give_item(item as usize, true, &lv.tables, &mut session);
    (gs, session)
}

fn run(lv: &Lv, s: &Setup, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> Vec<Row> {
    let (gs, session) = give(lv, s.item);
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
        let mut parts = |hero: &Hero, _: &rc_game::follow_camera::CameraView, rng: &mut Rng, _: u64| {
            let mut p = parts_cell.borrow_mut();
            rc_game::hero::fx::create_particles(hero, &mut p);
            p.update_parts(rng);
        };
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        game.hero.idle.counter = game.counter as i32;
        if let Some((at, class, flags, damage)) = s.hit {
            if t == at {
                if let Some(id) = game.mobys.mobys.iter().position(|m| m.o_class == class && m.state < 0x80) {
                    let tmpl = rc_game::moby_update::services::HitTemplate { dir: [rc_game::ps2v::Pf::ZERO; 4], attacker: None, flags, b18: 2, b19: 1, h1a: 0, damage: rc_game::ps2v::Pf::f(damage), w20: 0 };
                    rc_game::moby_update::services::deliver_hit_in(&mut game.mobys, &mut svc_cell.borrow_mut().hits, id, &tmpl);
                }
            }
        }
        if let Some((at, st)) = s.force { if t == at { if let Some(it) = game.hero.items.slot.item.as_mut() { it.mstate = st; } } }
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh, parts: &parts_cell };
        let mut rec = RecSounds::default();
        let r = game.tick_with_hero_sounds(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits, None, &mut rec);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let tick = game.counter - 1;
        let w = &h.walloper;
        let tr = &h.fx.trails.hero;
        let dim = svc_cell.borrow().draw_callbacks.walloper_dim;
        let drawn = w.drawn == Some(tick);
        let cam = game.camera.out.pos_f32();
        rows.push(Row {
            state: h.state,
            timer: h.timer,
            pos: h.position(),
            speed: h.speed.to_f32(),
            hand: h.items.slot.item.as_ref().map_or((h.items.slot.id, 0xff, 0xff), |x| (h.items.slot.id, x.mstate, x.anim.seq_b)),
            watched: watch.iter().map(|&i| { let x = &game.mobys.mobys[i]; (x.state, [x.position[0], x.position[1], x.position[2]], x.o_class) }).collect(),
            trail: (tr.active, tr.ghosts.iter().map(|g| (g.alpha, g.back)).collect()),
            arcs: w.arcs.iter().filter(|a| a.timer != 0).count(),
            glow: w.glow,
            drawn,
            stats: w.stats,
            listed: w.listed.clone(),
            sounds: rec.0,
            dim,
            markers: h.weapons.markers.of_tick(tick).iter().map(|m| (m.rgba, m.at.is_none())).collect(),
            ammo: h.weapons.ammo[h.items.slot.id.clamp(0, 35) as usize] as i32,
            missiles: game.mobys.mobys.iter().filter(|x| x.o_class == 457 && x.state < 0xfd).count(),
            fx_mobys: [122, 1192, 775, 660].map(|c| game.mobys.mobys.iter().filter(|x| x.o_class == c && x.state < 0x80).count()),
            parts: { let l = parts_cell.borrow().live_by_type(); [l[4], l[15]] },
            quads: if drawn { rc_game::hero::walloper::draw_quads(w, cam, [0.0, 0.0, -1.0], dim.unwrap_or(true)).len() } else { 0 },
            rng: game.rng.state,
        });
    }
    rows
}

/// The instance of the table's moby `id` (the moby loader's order).
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
    if std::env::var("RC_WALLOPER_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() { eprintln!("{t:4} {r:?}"); }
}

fn press_at(ts: &'static [u32]) -> impl Fn(u32) -> PadInput { move |t| if ts.contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

/// The lunge's timeline (relative to its entry tick): the target speed only in 10 < T < 18, the ghosts made at T = 8
/// (0x30 / 0x17 / 0x0c at 2 / 4 / 6 back), fading by 5 a tick after T = 18, the spheres in 10 < T < 22 (three a tick),
/// the lunge ending past frame 20; the arcs active from the item's key time 6, the draw registered while they live.
fn check_lunge(rows: &[Row], start: usize) -> usize {
    assert_eq!(rows[start].state, 0x20, "the lunge");
    assert_eq!(rows[start].sounds, vec![0], "the swing sound (class sound 0) with the SetState");
    let end = start + rows[start..].iter().position(|r| r.state != 0x20).expect("the lunge ended");
    // After-images: made at T = 8 (the physics runs with the timer of the tick).
    let made = rows[start..end].iter().position(|r| r.trail.0).map(|k| start + k).expect("no after-images");
    assert_eq!(rows[made].trail.1, vec![(0x30, 2), (0x17, 4), (0x0c, 6)]);
    // The spheres: 3 per tick for the ticks 11..=21 of the lunge.
    let spheres = rows[end - 1].stats[0];
    assert!(spheres >= 3 * 9 && spheres.is_multiple_of(3), "spheres {spheres}");
    // The arcs and the glow.
    assert!(rows[start..end].iter().any(|r| r.drawn && r.arcs > 0 && r.glow > 0.0), "no arcs drawn");
    assert!(rows[start..end].iter().any(|r| r.quads > 1), "no quads");
    end
}

/// The first instance of class `class` (the gameplay file's order).
fn first_of(lv: &Lv, class: i16, skip: usize) -> usize {
    lv.instances.iter().enumerate().filter(|(i, m)| m.o_class as i16 == class && lv.tests.get(*i).is_some_and(|t| t.spawn)).nth(skip).map(|(i, _)| i).expect("instance")
}

/// Novalis: ○ with the Walloper facing a big amoeboid 572 two units off: the lunge, its after-images, the arcs, the hit
/// on the amoeboid (its hit record, the item's hit sound 1 once per lunge, sparkle bursts on it: class type 5), the
/// lunge's end back to the idle state, the arcs running out and the draw stopping.
#[test]
fn novalis_walloper_lunges_into_an_amoeboid() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(lv.items.defs[18].o_class, 180, "item 18 is the Walloper (class 180)");
    assert_eq!(lv.items.defs[18].b18, 0, "no holding layer");
    let a = first_of(&lv, 572, 0);
    let at = facing(&lv, a, 2.2, 0.0);
    let s = Setup { item: 18, at, watch: &[a], force: None, hit: None };
    let input = press_at(&[40]);
    let rows = run(&lv, &s, &input, 160);
    dump(&rows);
    let start = rows.iter().position(|r| r.state == 0x20).expect("never lunged");
    let end = check_lunge(&rows, start);
    let hit_tick = rows.iter().position(|r| r.listed.iter().any(|&(_, c)| c == 572)).expect("the spheres never listed the amoeboid");
    assert!(rows[..=hit_tick].iter().any(|r| r.sounds.contains(&1)), "the hit sound by the first listing");
    assert_eq!(rows.iter().map(|r| r.sounds.iter().filter(|&&s| s == 1).count()).sum::<usize>(), 1, "one hit sound per lunge");
    assert!(rows[end].stats[2] > 0, "sparkles on the amoeboid");
    assert_eq!(rows[end].state, 0);
    // The amoeboid reacted to the hit (its state changed within a few ticks of the first listing).
    let before = rows[hit_tick - 1].watched[0].0;
    assert!(rows[hit_tick..hit_tick + 4].iter().any(|r| r.watched[0].0 != before), "the amoeboid did not react: {:?}", rows[hit_tick].watched);
    // The arcs run out after the lunge, and the draw with them.
    assert!(rows[end + 5..].iter().all(|r| r.arcs == 0 && !r.drawn), "arcs after the lunge");
    // The after-images fade out (5 a tick after T = 18) and end.
    assert!(rows[end + 10..].iter().all(|r| !r.trail.0));
    assert_eq!(rows, run(&lv, &s, &input, 160), "deterministic");
}

/// Novalis: a lunge in the open (nothing within reach): no hit sound, no sparkles; the lunge carries Ratchet forward
/// (the target speed 18·dt only between T = 10 and 18); two lunges in a row.
#[test]
fn novalis_walloper_lunge_timing_in_the_open() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let h = lv.instances.iter().find(|m| m.o_class == 0).unwrap();
    let s = Setup { item: 18, at: ([h.position[0], h.position[1], h.position[2]], h.rotation[2]), watch: &[], force: None, hit: None };
    let input = press_at(&[30, 80]);
    let rows = run(&lv, &s, &input, 140);
    dump(&rows);
    let start = rows.iter().position(|r| r.state == 0x20).expect("never lunged");
    let end = check_lunge(&rows, start);
    // The game's spheres at yaw and yaw + 50° ignore the hand item, not Ratchet: his own moby is listed once his body
    // reaches them (the hit sound plays; his intake drops the 0x30000 record); nothing else, no sparkles.
    assert!(rows[start..end].iter().flat_map(|r| r.listed.iter()).all(|&(_, c)| c == 0), "only Ratchet listed in the open");
    assert_eq!(rows[end].stats[2], 0, "no sparkles in the open");
    let moved = {
        let (a, b) = (rows[start].pos, rows[end].pos);
        ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
    };
    assert!(moved > 1.0, "the lunge moved {moved}");
    // The speed: 0 before T = 11 (entry speed kept only by the SpeedStep after T = 10).
    for r in &rows[start..start + 10] { assert!(r.speed <= rows[start].speed + 1e-6, "{r:?}"); }
    let second = end + rows[end..].iter().position(|r| r.state == 0x20).expect("a second lunge");
    check_lunge(&rows, second);
    assert_eq!(rows, run(&lv, &s, &input, 140), "deterministic");
}

/// Novalis: the lunge breaks a crate (500 reads 0x1830000; the lunge's hits are 0x30000).
#[test]
fn novalis_walloper_breaks_a_crate() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let c = 376;
    let p = lv.instances[c].position;
    let s = Setup { item: 18, at: ([p[0], p[1] + 2.0, p[2]], -std::f32::consts::FRAC_PI_2), watch: &[c], force: None, hit: None };
    let input = press_at(&[30]);
    let rows = run(&lv, &s, &input, 120);
    dump(&rows);
    let start = rows.iter().position(|r| r.state == 0x20).expect("never lunged");
    check_lunge(&rows, start);
    assert!(rows.last().unwrap().watched[0].0 >= 0x80, "the crate broke: {:?}", rows.last().unwrap().watched);
    assert_eq!(rows, run(&lv, &s, &input, 120), "deterministic");
}

/// Rilgar (level 05): a small amoeboid 866 hit by the lunge.
#[test]
fn rilgar_walloper_hits_a_small_amoeboid() {
    let Some(lv) = load_level(5) else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, 1262, 2.2, -std::f32::consts::FRAC_PI_2);
    let s = Setup { item: 18, at, watch: &[1262], force: None, hit: None };
    let input = press_at(&[30]);
    let rows = run(&lv, &s, &input, 120);
    dump(&rows);
    let start = rows.iter().position(|r| r.state == 0x20).expect("never lunged");
    check_lunge(&rows, start);
    let hit = rows.iter().position(|r| r.stats[1] > 0).expect("nothing listed");
    assert!(rows[hit].sounds.contains(&1));
    assert!(rows.last().unwrap().stats[2] > 0, "sparkles on the amoeboid");
    assert_eq!(rows, run(&lv, &s, &input, 120), "deterministic");
}

/// The R.Y.N.O.'s update state 3 (0x2e5264; nothing in the level code sets it, the test does): the red crosshair at the
/// screen centre every tick; L1 held and ○ fire the salvo (one ammo, seven missiles); L1 released goes back to state 2.
#[test]
fn novalis_ryno_state_3() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let b = instance_of(&lv, 592);
    let at = facing(&lv, b, 9.0, std::f32::consts::PI);
    let s = Setup { item: 23, at, watch: &[b], force: Some((60, 3)), hit: None };
    let input = |t: u32| {
        let mut p = PadInput::neutral();
        if (60..=150).contains(&t) { p = p.press(rc_game::pad::button::L1); }
        if t == 70 { p = p.press(button::CIRCLE); }
        p
    };
    let rows = run(&lv, &s, &input, 220);
    dump(&rows);
    // Before: state 2, no centre crosshair; from tick 60 state 3 with the red one (0xff0f0fff) at the centre.
    assert!(rows[..60].iter().all(|r| !r.markers.iter().any(|m| m.1)), "no centre crosshair before");
    assert_eq!(rows[60].hand.1, 3);
    assert!(rows[61].markers.contains(&(0xff0f_0fff, true)), "the red crosshair: {:?}", rows[61].markers);
    // ○ with L1: the salvo (state 4), one ammo, seven missiles.
    assert_eq!(rows[70].hand.1, 4, "the salvo from state 3");
    assert_eq!(rows[69].ammo - rows[70].ammo, 1, "one ammo");
    assert!(rows.iter().map(|r| r.missiles).max().unwrap() >= 1, "missiles");
    // The missiles' blasts within 14 of the camera throw the beam explosion's debris fireball 122 (`0x2c4c20`,
    // hero_gameplay.md §14.1).
    assert!(rows.iter().any(|r| r.fx_mobys[0] > 0), "the blasts' debris fireballs");
    assert_eq!(rows, run(&lv, &s, &input, 220), "deterministic");
}

/// State 3 without L1 / L2 held goes back to state 2 at once (the crosshair drawn that tick).
#[test]
fn novalis_ryno_state_3_released() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let h = lv.instances.iter().find(|m| m.o_class == 0).unwrap();
    let s = Setup { item: 23, at: ([h.position[0], h.position[1], h.position[2]], h.rotation[2]), watch: &[], force: Some((60, 3)), hit: None };
    let input = |_: u32| PadInput::neutral();
    let rows = run(&lv, &s, &input, 70);
    assert_eq!(rows[60].hand.1, 2, "back to 2 in the same update");
    assert!(rows[60].markers.contains(&(0xff0f_0fff, true)));
}

/// The Blarg flyer 660's kill (a hit 0x800000: an explosion's): the ship's blast `SpawnBeamExplosion` (its streaks, type
/// 15), the flyer and its links deleted (hero_gameplay.md §14.1: the blast was missing).
#[test]
fn novalis_flyer_kill_blast() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let h = lv.instances.iter().find(|m| m.o_class == 0).unwrap();
    let s = Setup { item: 18, at: ([h.position[0], h.position[1], h.position[2]], h.rotation[2]), watch: &[], force: None, hit: Some((20, 660, 0x80_0000, 1.0)) };
    let input = |_: u32| PadInput::neutral();
    let rows = run(&lv, &s, &input, 40);
    dump(&rows);
    assert_eq!(rows[21].fx_mobys[3] + 1, rows[19].fx_mobys[3], "one flyer killed");
    assert!(rows[21].parts[1] >= rows[19].parts[1] + 20, "the blast's 20 streaks: {:?} → {:?}", rows[19].parts, rows[21].parts);
    assert_eq!(rows, run(&lv, &s, &input, 40), "deterministic");
}

/// The Pyrocitor's hits are the resolver's type 5 (the burning kind 4): a critter or amoeboid knocked by them carries
/// the burn marker, and its flight throws the burn's fire puffs (type 4, `0x271258`: hero_gameplay.md §14.1).
#[test]
fn novalis_pyrocitor_burn_puffs() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let a = first_of(&lv, 572, 0);
    let at = facing(&lv, a, 3.0, 0.0);
    let s = Setup { item: 16, at, watch: &[a], force: None, hit: None };
    let input = |t: u32| if (30..=120).contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() };
    let rows = run(&lv, &s, &input, 160);
    dump(&rows);
    assert!(rows.iter().any(|r| r.parts[0] > 0), "no burn puffs (type 4)");
    assert_eq!(rows, run(&lv, &s, &input, 160), "deterministic");
}
