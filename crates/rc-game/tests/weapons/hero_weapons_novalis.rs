//! Weapons and first person on Novalis, headless (docs/plan/hero_states.md "Weapons + first person"): the level as
//! the engine ticks it (the loader's spawn test on a first visit, the static mobys, the scheduler's load pass and
//! moby loop, the moby collision, the hero with the wrench and the Bomb Glove, the tick's hit path, the camera) with
//! Ratchet placed in front of a crate. The Comet-Strike breaks it and the wrench comes back; a bomb breaks another;
//! the first-person camera comes up with L1, aims with the stick and goes; the wrench is thrown from first person.
//! Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::items::{ItemClass, ItemData, ItemDef, HERO_LISTS};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{SharedServices, World};
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
    items: ItemData,
}

fn load() -> Option<Lv> {
    let dir = rc_formats::test_data::root().join("levels/01");
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
    // The wrench (item 8 = class 71 on list 0) and the Bomb Glove (item 10 = class 192 on list 6), as the engine
    // builds them from the item definitions.
    let mut defs = vec![ItemDef::default(); 37];
    defs[8] = ItemDef { slot: 0, attach: 0, o_class: 71, b18: 0 };
    defs[10] = ItemDef { slot: 0, attach: 6, o_class: 192, b18: 0 };
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
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, items })
}

/// The hero's hit sink over the moby loop's services (as the engine's `CellHits`): the wrench's lines and spheres,
/// and the glove's bomb (`CreateMoby` / `DeleteMoby`).
struct CellHits<'a, 'b> {
    svc: &'a std::cell::RefCell<&'b mut Services>,
    classes: &'a ClassTable,
    coll: &'a collision::Collision,
}

impl rc_game::hero::items::HitSink for CellHits<'_, '_> {
    fn sphere(&mut self, table: &mut MobyTable, r: rc_game::ps2v::Pf, centre: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &rc_game::moby_update::services::HitTemplate) -> Option<usize> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::ServiceHits { svc: &mut s, classes: self.classes, coll: Some(self.coll) }.sphere(table, r, centre, flags, ignore, tmpl)
    }
    fn line(&mut self, table: &mut MobyTable, a: rc_game::hero::physics::V4, b: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &rc_game::moby_update::services::HitTemplate) -> Option<Option<usize>> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::ServiceHits { svc: &mut s, classes: self.classes, coll: Some(self.coll) }.line(table, a, b, flags, ignore, tmpl)
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
}

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    pos: [f32; 3],
    yaw: f32,
    /// The hand item: id, its moby state (+0x20), its position.
    hand: (i32, u8, [f32; 3]),
    /// The crate's state (0xfd / 0xfe once broken and deleted).
    crate_state: u8,
    /// The live bombs (class 121): (state, position).
    bombs: Vec<(u8, [f32; 3])>,
    ammo: i32,
    fp_active: bool,
    fp_flag: u8,
    hero_hidden: bool,
    cam_pos: [f32; 3],
    cam_yaw: f32,
    blend: u8,
    rng: u32,
}

/// Novalis with its mobys, Ratchet at `at` (yaw), `hand` the item requested into the hand (8 wrench, 10 the glove,
/// with 10 bombs), `input(t)` for `ticks` ticks; `watch` = the crate instance to record.
fn run(lv: &Lv, at: ([f32; 3], f32), hand: i32, watch: usize, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> Vec<Row> {
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
    game.item_data = Some(lv.items.clone());
    game.item_globals.wrench_flag = 1;
    game.hero.grant_items(&[10]);
    game.hero.weapons.uses_ammo[10] = true;
    game.hero.weapons.ammo[10] = 10;
    if hand != 8 {
        game.item_globals.request = hand;
        game.item_globals.saved = hand;
    }
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
        let mut parts = |_: &Hero, _: &rc_game::follow_camera::CameraView, rng: &mut Rng, _: u64| { parts_cell.borrow_mut().update_parts(rng); };
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        game.hero.idle.counter = game.counter as i32;
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh };
        let r = game.tick_with_hits(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let hand = h.items.slot.item.as_ref().map_or((h.items.slot.id, 0xff, [0.0; 3]), |m| (h.items.slot.id, m.mstate, m.position));
        let bombs = game.mobys.mobys.iter().filter(|m| m.o_class == 121 && m.state < 0xfd).map(|m| (m.state, [m.position[0], m.position[1], m.position[2]])).collect();
        let v = game.camera.out;
        rows.push(Row {
            state: h.state,
            pos: h.position(),
            yaw: h.rot[2].to_f32(),
            hand,
            crate_state: game.mobys.mobys[watch].state,
            bombs,
            ammo: h.weapons.ammo[10],
            fp_active: game.camera.first_person.active,
            fp_flag: h.f13f5,
            hero_hidden: game.mobys.mobys[hero_idx].mode & mode::HIDDEN != 0,
            cam_pos: v.pos_f32(),
            cam_yaw: v.yaw().to_f32(),
            blend: game.camera.blend.mode,
            rng: game.rng.state,
        });
    }
    rows
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_WEAPONS_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() {
        eprintln!("{t:4} {:#x} {:?} yaw {:.2} hand {:?} crate {:#x} bombs {:?} ammo {} fp {}/{} hid {} cam {:?} {:.2} blend {}", r.state, r.pos, r.yaw, r.hand, r.crate_state, r.bombs, r.ammo, r.fp_active, r.fp_flag, r.hero_hidden, r.cam_pos, r.cam_yaw, r.blend);
    }
}

/// A crate of Novalis (`inst`), and a place `d` units in front of it facing it (on the crate's ground).
fn facing(lv: &Lv, inst: usize, d: f32, from: f32) -> ([f32; 3], f32) {
    let c = lv.instances[inst].position;
    let p = [c[0] + d * from.cos(), c[1] + d * from.sin(), c[2]];
    (p, (c[1] - p[1]).atan2(c[0] - p[0]))
}

/// Survey helper: the crates near the spawn (`cargo xtask test-job --test weapons --filter hero_weapons_novalis:: --ignored --nocapture`).
#[test]
#[ignore]
fn weapons_survey() {
    let Some(lv) = load() else { return };
    let hero = lv.instances.iter().find(|m| m.o_class == 0).unwrap().position;
    println!("spawn {hero:?}");
    for (k, (i, t)) in lv.instances.iter().zip(&lv.tests).enumerate() {
        if !(500..=511).contains(&i.o_class) || !t.spawn { continue; }
        let d = ((i.position[0] - hero[0]).powi(2) + (i.position[1] - hero[1]).powi(2)).sqrt();
        if d < 60.0 {
            println!("inst {k} class {} pos {:?} d {d:.1}", i.o_class, i.position);
            // The ground around it (8 directions at 4 and 6 units): the hit z of a line from 3 above to 5 below.
            for dist in [4.0f32, 6.0] {
                let mut line = String::new();
                for q in 0..8 {
                    let a = q as f32 * std::f32::consts::FRAC_PI_4;
                    let (x, y) = (i.position[0] + dist * a.cos(), i.position[1] + dist * a.sin());
                    let z = i.position[2];
                    let hit = rc_game::hero::physics::line_world(&lv.mesh, rc_game::hero::physics::v4(x, y, z + 3.0), rc_game::hero::physics::v4(x, y, z - 5.0), 2);
                    line += &format!(" {q}:{}", hit.map_or("-".to_string(), |o| format!("{:.1}", o.point[2])));
                }
                println!("    d{dist}:{line}");
            }
        }
    }
}

/// Crate 376 (class 500 at (147.4, 125.8, 57.0), one of the trio below the spawn plateau); Ratchet west of it,
/// facing it (east).
const CRATE: usize = 376;
const WEST: f32 = std::f32::consts::PI;

fn crouch_square(t: u32) -> PadInput {
    if (30..=40).contains(&t) { PadInput::neutral().press(button::R1) } else if (41..=43).contains(&t) { PadInput::neutral().press(button::R1 | button::SQUARE) } else { PadInput::neutral() }
}

#[test]
fn novalis_comet_strike_breaks_a_crate_and_the_wrench_returns() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, CRATE, 4.0, WEST);
    let rows = run(&lv, at, 8, CRATE, &crouch_square, 240);
    dump(&rows);
    let entered = rows.iter().position(|r| r.state == 0x15).expect("no comet strike 0x15");
    let out = rows.iter().position(|r| r.hand.1 == 10).expect("the wrench was never thrown");
    let back = rows.iter().position(|r| r.hand.1 == 11).expect("the wrench never came back");
    assert!(entered < out && out < back);
    let caught = rows.iter().skip(back).position(|r| r.hand.1 == 0).expect("the wrench was never caught") + back;
    assert!(rows[caught..].iter().all(|r| r.hand.1 == 0));
    assert!(rows[entered..].iter().any(|r| r.crate_state >= 0xfd || r.crate_state != rows[0].crate_state), "the crate was not hit");
    assert_eq!(rows.last().unwrap().state, 0, "back to idle");
    // The wrench flew away from Ratchet (≥ 2 units) and ended in his hand.
    let far = rows[out..caught].iter().map(|r| ((r.hand.2[0] - r.pos[0]).powi(2) + (r.hand.2[1] - r.pos[1]).powi(2)).sqrt()).fold(0.0f32, f32::max);
    assert!(far > 2.0, "the wrench went only {far}");
    let again = run(&lv, at, 8, CRATE, &crouch_square, 240);
    assert_eq!(rows, again);
}

fn circle(t: u32) -> PadInput { if (40..=41).contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

#[test]
fn novalis_bomb_breaks_a_crate() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, CRATE, 4.0, WEST);
    let rows = run(&lv, at, 10, CRATE, &circle, 240);
    dump(&rows);
    assert!(rows[20..].iter().all(|r| r.hand.0 == 10), "the glove is not in the hand");
    assert!(rows.iter().any(|r| r.state == 0x23), "no throw state 0x23");
    assert!(rows.iter().any(|r| r.bombs.iter().any(|b| b.0 == 1)), "no bomb flew");
    assert!(rows.iter().any(|r| r.bombs.iter().any(|b| b.0 == 2)), "no bomb exploded");
    assert!(rows.iter().any(|r| r.crate_state >= 0xfd || r.crate_state != rows[0].crate_state), "the crate was not hit");
    assert_eq!(rows.last().unwrap().ammo, 9, "one bomb used");
    let again = run(&lv, at, 10, CRATE, &circle, 240);
    assert_eq!(rows, again);
}

fn look(t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if (20..=120).contains(&t) { p = p.press(button::L1); }
    if (70..=90).contains(&t) { p = p.stick(1.0, 0.0); }
    p
}

#[test]
fn novalis_first_person_enter_aim_and_exit() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, CRATE, 4.0, WEST);
    let rows = run(&lv, at, 8, CRATE, &look, 220);
    dump(&rows);
    let on = rows.iter().position(|r| r.fp_active).expect("no first-person camera");
    assert!(rows[on - 1].state == 1 && (26..=30).contains(&on), "the switch 7 ticks into the stance: {on}");
    let flag = rows.iter().position(|r| r.fp_flag != 0).expect("0x1413f5 never set");
    assert!(flag > on + 15, "the flag after the blend-in: {on} {flag}");
    // HeroSyncMoby hides him in the next hero update.
    assert!(!rows[flag].hero_hidden && rows[flag + 1].hero_hidden, "Ratchet shown in first person");
    // The eye 1.6 above the feet.
    let r = &rows[flag + 5];
    assert!((r.cam_pos[2] - r.pos[2] - 1.6).abs() < 0.05, "eye {:?} feet {:?}", r.cam_pos, r.pos);
    // The stick turns the view and Ratchet with it.
    let (y0, y1) = (rows[69].cam_yaw, rows[95].cam_yaw);
    assert!((y1 - y0).abs() > 0.2, "the view did not turn: {y0} {y1}");
    assert!((rows[96].yaw - rows[96].cam_yaw).abs() < 0.01, "Ratchet does not face the view");
    // Released: the follow camera again, Ratchet shown.
    let off = rows.iter().skip(121).position(|r| !r.fp_active).expect("first person never left") + 121;
    assert!(off <= 123);
    assert!(!rows[off + 1].hero_hidden && rows[off + 1].fp_flag == 0);
    assert!(rows.last().unwrap().blend == 0);
    let again = run(&lv, at, 8, CRATE, &look, 220);
    assert_eq!(rows, again);
}

fn look_square(t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if (20..=200).contains(&t) { p = p.press(button::L1); }
    if (70..=71).contains(&t) { p = p.press(button::L1 | button::SQUARE); }
    p
}

#[test]
fn novalis_first_person_comet_strike() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, CRATE, 4.0, WEST);
    let rows = run(&lv, at, 8, CRATE, &look_square, 220);
    dump(&rows);
    let out = rows.iter().position(|r| r.hand.1 == 10).expect("the wrench was not thrown in first person");
    assert_eq!(rows[out].state, 1, "the first-person throw keeps the look stance");
    assert!(rows[out].fp_flag != 0);
    assert!(rows[out..].iter().any(|r| r.hand.1 == 0), "the wrench never came back");
    assert!(rows.iter().any(|r| r.crate_state >= 0xfd || r.crate_state != rows[0].crate_state), "the crate was not hit");
}
