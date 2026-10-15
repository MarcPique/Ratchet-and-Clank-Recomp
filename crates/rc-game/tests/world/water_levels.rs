//! Water on every level (docs/plan/level_generalisation.md W1–W4): the per-level water inventory (the ripple module,
//! the managers, strips, the water plane, the liquid faces, all found by code identity and read from the level's
//! overlay), the managers running on their levels, the Novalis 751 port against the ripple module alone, and the hero
//! swimming and wading on Rilgar (05) and Pokitaru (11) headless. Skipped without `extracted/`.

use rc_formats::level_overlay::{LevelOverlay, Relocation};
use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, moby_spawn};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, ClassInfo, MobyTable, Seq0Info};
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::PadInput;
use rc_game::particles::{BSphereView, Particles};
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use rc_game::water::managers::PORTS;
use rc_game::water::world::{LevelWaterData, WaterWorld};
use std::collections::HashMap;
use std::sync::Arc;

fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }

fn water_data(level: u32) -> Option<LevelWaterData> { crate::common::water_data(level) }

fn ports(level: u32) -> LevelPorts { crate::common::ports(level, &[]).unwrap() }

/// Faces of the world mesh with a liquid surface id (0 water, 3 / 0xb / 0xd sinking or deadly liquids, 0xe shallow).
fn liquid_faces(level: u32) -> Vec<(u8, usize)> {
    let core = rc_formats::test_data::core(level).unwrap();
    let coll = collision::parse_collision(&core.core, &core.data).unwrap();
    let mut n: std::collections::BTreeMap<u8, usize> = Default::default();
    for t in collision::collision_triangles(&coll) {
        let s = t.surface & 0x1f;
        if matches!(s, 0 | 3 | 0xb | 0xd | 0xe) { *n.entry(s).or_default() += 1; }
    }
    n.into_iter().collect()
}

#[test]
fn water_inventory_all_levels() {
    let Some(r01) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut with_module = Vec::new();
    for level in 0..19u32 {
        let d = water_data(level).unwrap();
        let t = overlay(level).unwrap();
        let p = ports(level);
        let strips = rc_formats::water::strip_tables(&t, &Relocation::new(&r01, &t));
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        // The water ports the level's class table runs, with their placed instances.
        let mut port_classes: Vec<(usize, i16, usize)> = Vec::new();
        for e in t.vtbl() {
            if let Some(ClassUpdate::Water(i)) = p.get(e.o_class as i16).filter(|_| p.in_table(e.o_class as i16)) {
                let placed = inst.iter().filter(|m| m.o_class == e.o_class).count();
                port_classes.push((i as usize, e.o_class as i16, placed));
            }
        }
        let names: Vec<String> = d.ports.iter().map(|pd| format!("{} ({} patches, {} masks)", PORTS[pd.port].name, pd.patches.len(), pd.masks.len())).collect();
        eprintln!(
            "level {level:02}: module {} | 751 {} | ports {names:?} | classes (port, class, placed) {port_classes:?} | strips {:?} | managers {:?} | liquid faces {:?} | look tint {:02x?} fog {:02x?} far {}",
            d.module.is_some(),
            d.z751.as_ref().map_or("-".to_string(), |(t, z)| format!("{} patches, {} zones, cuboids {z:?}", t.patches.len(), t.zones.len())),
            strips.iter().map(|s| (s.0, s.2)).collect::<Vec<_>>(),
            d.manager_classes,
            liquid_faces(level),
            d.look.tint,
            d.look.fog.color,
            d.look.fog.far_dist,
        );
        // Every port found on a level resolved its tables.
        for pd in &d.ports {
            let port = &PORTS[pd.port];
            if let Some((_, n)) = port.patches { assert_eq!(pd.patches.len(), n, "level {level:02} {}", port.name); }
            if port.masks.is_some() { assert_eq!(pd.masks.len(), pd.patches.len(), "level {level:02} {}", port.name); }
            for &(label, n) in port.words { for k in 0..n as u32 { assert!(pd.words.contains_key(&(label + 4 * k)), "level {level:02} {} word {label:#x}", port.name); } }
            // Each port runs its classes on its reference level, and they are placed.
            assert!(port_classes.iter().any(|c| c.0 == pd.port && c.2 > 0), "level {level:02}: {} runs no placed class", port.name);
        }
        // The underwater look was found (the same `.lit` bytes on every level).
        assert_eq!(d.look, r_look(), "level {level:02} underwater look");
        if d.module.is_some() { with_module.push(level); }
        // A module user exists exactly where the module is.
        assert_eq!(d.patch_records().is_some(), d.module.is_some(), "level {level:02}");
        assert_eq!(!strips.is_empty(), level == 1, "level {level:02}: strip module");
        assert_eq!(d.z751.is_some(), level == 1);
    }
    assert_eq!(with_module, [1, 5, 7, 11, 12, 13]);
    // Each patch manager and the plane on its level, nowhere else.
    for (i, port) in PORTS.iter().enumerate() {
        for level in 0..19u32 {
            let d = water_data(level).unwrap();
            assert_eq!(d.port(i).is_some(), i != 0 && level == port.level, "{} on level {level:02}", port.name);
        }
    }
    // The Novalis zone tables as documented (docs/plan/world_animation.md §2).
    let d = water_data(1).unwrap();
    let (t, zones) = d.z751.as_ref().unwrap();
    assert_eq!(*zones, [2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(t.zones.iter().map(|z| z.patch_count).collect::<Vec<_>>(), [4, 2, 6, 1, 0, 4, 1]);
    // Level 05: 48 patches, 7 of them in the flooding sewer at 15.2.
    let d5 = water_data(5).unwrap();
    let p5 = d5.port(rc_game::water::managers::RILGAR).unwrap();
    assert_eq!(p5.patches.len(), 48);
}

/// Novalis's `0x161200..0x161214` (the static bytes; `fog_zones::UnderwaterLook::default`).
fn r_look() -> rc_game::fog_zones::UnderwaterLook { rc_game::fog_zones::UnderwaterLook::default() }

// ---------------------------------------------------------------------------------------------------
// A headless level: the mobys, the load pass, the game tick with the moby loop and the level's water

struct Sim {
    game: Game,
    svc: Services,
    classes: Arc<ClassTable>,
    sched: Scheduler,
    particles: Particles,
    mesh: collision::Collision,
    anim_class: MobyAnimClass,
    anim: Option<RatchetAnim>,
    /// The hero's swim events so far.
    events: Vec<rc_game::hero::swim::SwimEvent>,
}

fn view_of(cam: &rc_game::follow_camera::CameraView) -> BSphereView {
    let r = cam.rows_f32();
    BSphereView::from_camera(cam.pos_f32(), r[0], r[1], r[2], 0.63, 0.63 * 0.775)
}

/// Loads `level` as the engine does (class table through the level's ports, the water data), with Ratchet placed at
/// `hero_at` before the hero init, and runs the load pass.
fn load(level: u32, hero_at: Option<[f32; 3]>) -> Option<Sim> {
    let core = rc_formats::test_data::core(level)?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let ports = ports(level);
    let settings = rc_formats::test_data::gameplay_section(level, "level_settings").unwrap();
    let mesh = collision::parse_collision(&core.core, &core.data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let ok: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &ok).unwrap();
    let splines = gameplay::parse_splines(&gp).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4).max(1) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    let mut spheres = HashMap::new();
    for (slot, e) in core.core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = core.block(&format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, ports.update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            spheres.insert(oc, c.header.bsphere);
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = ClassInfo { slot: slot as u8, no_header: true, update_fn: ports.update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let ratchet_blob = core.block("moby_class/0000")?.to_vec();
    let rclass = rc_formats::moby::parse_moby_class(&ratchet_blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256).map(|i| core.block(&format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(b, 0).ok())).collect();
    let anim_class = MobyAnimClass::new(&rclass, seqs);
    let classes = Arc::new(classes);
    let mut ct = ClassTable { classes: classes.classes.clone() };
    let statics = load_level_mobys(&instances, &mut ct, &pvars, &tests);
    let hero_idx = statics.mobys.iter().position(|m| m.o_class == 0)?;
    let mut table = MobyTable::new(statics.mobys.clone(), spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    if let Some(p) = hero_at { table.mobys[hero_idx].position = [p[0], p[1], p[2], table.mobys[hero_idx].position[3]]; }
    let mut game = Game::new(&mesh, table, hero_idx, GameOptions::default(), death_z);
    game.hero.idle.level = level as i32;
    let mut svc = Services::new();
    svc.level = level;
    svc.set_splines(&splines);
    svc.groups = statics.groups(&gp);
    svc.set_moby_collision(rc_formats::moby_collision::parse_level(&core.core, &core.data).unwrap());
    svc.set_volumes(rc_formats::volumes::parse_volumes(&gp).unwrap());
    svc.build_grid(&mut game.mobys);
    svc.creatures.class_spheres = spheres;
    svc.water = WaterWorld::new(water_data(level)?);
    let mut particles = Particles::new(None, Vec::new());
    let mut sched = Scheduler::new();
    {
        let hero: Hero = game.hero.clone();
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&mesh);
        w.particles = Some(&mut particles);
        sched.load_pass(&mut w);
    }
    game.finish_load();
    Some(Sim { game, svc, classes, sched, particles, mesh, anim_class, anim: None, events: Vec::new() })
}

impl Sim {
    /// One game tick with `pad`: the moby loop (with the last frame's view), the hero, the particles, the camera;
    /// the hero's ripples applied to the level's water after the tick (as the engine does).
    fn tick(&mut self, pad: PadInput) -> rc_game::tick::TickReport {
        if self.anim.is_none() { self.anim = Some(RatchetAnim::new(&self.anim_class)); }
        let view = view_of(&self.game.camera.out);
        let svc_cell = std::cell::RefCell::new(&mut self.svc);
        let parts_cell = std::cell::RefCell::new(&mut self.particles);
        let (sched, classes) = (&mut self.sched, &self.classes);
        let classes_ref: &ClassTable = classes;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut p = parts_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.particles = Some(&mut **p);
            w.view = Some(&view);
            sched.tick(&mut w);
        };
        let mut parts = |_: &Hero, _: &rc_game::follow_camera::CameraView, rng: &mut Rng, _: u64| { parts_cell.borrow_mut().update_parts(rng); };
        self.game.hero.idle.counter = self.game.counter as i32;
        let r = {
            let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
            let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
            self.game.tick(Some(&pad.bytes()), &self.mesh, &mut self.anim.as_mut().unwrap().ctl(&self.anim_class), &mut hooks)
        };
        for e in std::mem::take(&mut self.game.hero.swim.events) {
            if let rc_game::hero::swim::SwimEvent::Ripple { x, y, r, amp } = e { self.svc.water.disturb(x, y, r, amp, false); }
            self.events.push(e);
        }
        r
    }
}

// ---------------------------------------------------------------------------------------------------
// The managers on their levels

/// Runs every level with a patch manager for 400 ticks with the camera looking at the manager's patch 0: the module
/// is initialised by the load pass (patch 0's moby), the clock runs, patches are active while their mobys are in view,
/// the random drops land, and every patch's level is its moby's z.
#[test]
fn managers_run_on_their_levels() {
    if overlay(1).is_none() { eprintln!("skipped: no extracted/"); return; }
    for level in [1u32, 5, 7, 11, 12, 13] {
        let mut s = load(level, None).unwrap();
        let n = s.svc.water.sim.as_ref().map(|m| m.patches.len());
        assert!(n.is_some(), "level {level:02}: the load pass initialised the ripple module");
        let (mut steps, mut active_max) = (0, 0);
        for _ in 0..400 {
            let clock0 = s.svc.water.sim.as_ref().unwrap().clock;
            s.tick(PadInput::neutral());
            let sim = s.svc.water.sim.as_ref().unwrap();
            if sim.clock < clock0 { steps += 1; }
            active_max = active_max.max(sim.patches.iter().filter(|p| p.mask != 0).count());
        }
        let registered = s.svc.draw_callbacks.list1.iter().any(|(c, _)| *c == rc_game::moby_update::classes::draw_callbacks::Callback::RipplePatches);
        let sim = s.svc.water.sim.as_ref().unwrap();
        let energy: f32 = sim.patches.iter().map(|p| p.buf[sim.cur].iter().map(|h| h.to_f32().abs()).sum::<f32>()).sum();
        eprintln!("level {level:02}: {} patches, {steps} steps in 400 ticks, most active {active_max}, registered {registered}, height energy {energy:.3}", sim.patches.len());
        // One step every 9 ticks: 44 in 400.
        assert!((43..=45).contains(&steps), "level {level:02}: {steps} clock steps");
        assert!(registered, "level {level:02}: the patch draw callback");
        // Every patch's level is its moby's z (patch managers: pvar idx at +0x14 / +4 / +0xc / +0x10).
        if level != 1 {
            let off = match level { 5 | 7 => 0x14, 11 => 4, 12 => 0xc, _ => 0x10 };
            // (A moby that never ran, beyond its update distance, leaves its patch at the stored level.)
            for m in s.game.mobys.mobys.iter().filter(|m| m.state != 0 && m.state < 0x80 && m.update_fn.is_some_and(|f| PORTS.iter().any(|p| p.func == f && p.patches.is_some()))) {
                let idx = rc_game::moby_update::services::pvar::i32(&m.pvars, off);
                if let Some(p) = usize::try_from(idx).ok().and_then(|i| sim.patches.get(i)) {
                    assert_eq!(p.centre[2].to_f32(), m.position[2], "level {level:02} patch {idx}");
                }
            }
        }
    }
}

/// The 751 port against the ripple module alone on the same stream: the port's init and per-tick update are
/// `RippleSim::new` / `tick_with` (the Novalis guard: the patch heights, the clock and the stream stay identical) until
/// the first drip. From then on the port creates the drip moby 787 in the game's order (its creation draws), which the
/// direct `RippleSim` does not model, so the two streams and the heights part there by design.
#[test]
fn novalis_751_port_is_the_ripple_module() {
    if overlay(1).is_none() { eprintln!("skipped: no extracted/"); return; }
    let d = water_data(1).unwrap();
    let (t, zones) = d.z751.clone().unwrap();
    let mut rng = Rng::new();
    rng.srand(1234);
    let mut direct = rc_game::water::RippleSim::new(&t, zones, d.light_xy, &mut rng);
    // The port through a World with only the 751 moby.
    let m = rc_game::moby_runtime::Moby { o_class: 751, ..Default::default() };
    let mut table = MobyTable::new(vec![m], 1);
    let hero = Hero::default();
    let classes = ClassTable::default();
    let mut svc = Services::new();
    svc.water = WaterWorld::new(d.clone());
    let mut rng2 = Rng::new();
    rng2.srand(1234);
    let cams = [[150.0f32, 200.0, 60.0], [160.0, 96.0, 61.0], [120.0, 98.0, 57.0]];
    let mut compared = 0;
    for k in 0..300u64 {
        let cam = cams[(k / 100) as usize];
        {
            let mut w = World::new(&mut table, &hero, &mut rng2, &classes, &mut svc, k);
            w.camera = [cam[0], cam[1], cam[2], 1.0].map(rc_game::ps2v::Pf::f);
            rc_game::moby_update::classes::dispatch(ClassUpdate::Water(0), &mut w, 0);
        }
        if k > 0 { direct.tick_with(cam, &d.cuboids, &mut rng, k, None); }
        // The first drip: the paths part here (module doc above); everything before it matched.
        if table.mobys.iter().any(|m| m.o_class == 787) { break; }
        let port = svc.water.sim.as_ref().unwrap();
        assert_eq!(rng.state, rng2.state, "tick {k}: the stream");
        assert_eq!(port.clock, direct.clock, "tick {k}");
        for (a, b) in port.patches.iter().zip(&direct.patches) {
            assert_eq!(a.buf, b.buf, "tick {k}");
            assert_eq!(a.mask, b.mask, "tick {k}");
        }
        compared += 1;
    }
    assert!(compared > 0, "no tick before the first drip");
    assert_eq!(table.mobys[0].state, 1);
}

// ---------------------------------------------------------------------------------------------------
// Swimming and wading

/// Where a water face lies over a floor `depth` below it with nothing solid above: (point on the water, floor z). A
/// water face over nothing (Rilgar's open sea) counts as deep water, floor −∞.
fn find_water(mesh: &collision::Collision, depth: std::ops::Range<f32>) -> Vec<([f32; 3], f32)> {
    let tris = collision::collision_triangles(mesh);
    let inside = |t: &collision::CollisionTriangle, x: f32, y: f32| -> Option<f32> {
        let (a, b, c) = (t.a, t.b, t.c);
        let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if d.abs() < 1e-6 { return None; }
        let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
        let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
        let l3 = 1.0 - l1 - l2;
        (l1 >= 0.0 && l2 >= 0.0 && l3 >= 0.0).then(|| l1 * a[2] + l2 * b[2] + l3 * c[2])
    };
    let water: Vec<&collision::CollisionTriangle> = tris.iter().filter(|t| t.surface & 0x1f == 0).collect();
    let mut out = Vec::new();
    for t in water.iter().step_by((water.len() / 400).max(1)) {
        let (x, y) = ((t.a[0] + t.b[0] + t.c[0]) / 3.0, (t.a[1] + t.b[1] + t.c[1]) / 3.0);
        let Some(wz) = inside(t, x, y) else { continue };
        let mut floor = f32::MIN;
        let mut blocked = false;
        for u in tris.iter().filter(|u| u.surface & 0x1f != 0) {
            let Some(z) = inside(u, x, y) else { continue };
            if z < wz && z > floor { floor = z; }
            if z > wz && z < wz + 4.0 { blocked = true; }
        }
        let d = if floor > f32::MIN { wz - floor } else if depth.end > 10.0 { depth.start } else { continue };
        if !blocked && depth.contains(&d) { out.push(([x, y, wz], floor)); }
    }
    out
}

/// Drops Ratchet into deep water on `level` and pushes the stick: he enters the water (surface idle 0x37, then the
/// swim 0x36), floats on the level the probe reads, and makes his entry ripple; then, on a shallow spot, he wades.
fn swim_and_wade(level: u32) {
    let Some(probe) = load(level, None) else { eprintln!("skipped: no extracted/"); return };
    let deep = find_water(&probe.mesh, 3.0..40.0);
    let shallow = find_water(&probe.mesh, 0.3..0.8);
    eprintln!("level {level:02}: {} deep water spots, {} shallow (depth 0.3..0.8)", deep.len(), shallow.len());
    let &(spot, floor) = deep.first().expect("deep water on the level");
    let mut s = load(level, Some([spot[0], spot[1], spot[2] + 2.0])).unwrap();
    let mut states = Vec::new();
    let mut floats = None;
    for t in 0..240 {
        let pad = if t >= 90 { PadInput::neutral().stick(0.0, -1.0) } else { PadInput::neutral() };
        s.tick(pad);
        let h = &s.game.hero;
        if states.last() != Some(&h.state) { states.push(h.state); }
        if h.state == 0x37 && floats.is_none() && t > 40 { floats = Some((h.position(), h.water_level.to_f32())); }
    }
    let ripples = s.events.iter().filter(|e| matches!(e, rc_game::hero::swim::SwimEvent::Ripple { .. })).count();
    let splashes = s.game.mobys.mobys.iter().filter(|m| m.o_class == 775).count();
    eprintln!("level {level:02} swim at {spot:?} (floor {floor}): states {states:x?}, floats {floats:?}, ripples {ripples}, splash mobys {splashes}, end {:?}", s.game.hero.position());
    assert!(states.contains(&0x37), "treading water: {states:x?}");
    assert!(states.contains(&0x36), "swimming: {states:x?}");
    let (p, w) = floats.unwrap();
    assert!((w - spot[2]).abs() < 0.5, "water level {w} at the surface {}", spot[2]);
    assert!((p[2] - (w - 0.12)).abs() < 0.3, "floats at {} on {w}", p[2]);
    assert!(ripples > 0, "the entry ripple");

    let Some(&(ws, wfloor)) = shallow.first() else { panic!("no shallow water on level {level:02}") };
    let mut s = load(level, Some([ws[0], ws[1], wfloor + 1.0])).unwrap();
    let (mut wading, mut wade_state) = (0, 0);
    for t in 0..120 {
        let pad = if (30..60).contains(&t) { PadInput::neutral().stick(0.0, -0.4) } else { PadInput::neutral() };
        s.tick(pad);
        if s.game.hero.f13f9 != 0 { wading += 1; }
        if s.game.hero.state == 0x73 { wade_state += 1; }
    }
    eprintln!("level {level:02} wade at {ws:?} (floor {wfloor}): {wading} ticks wading, {wade_state} in state 0x73, water level {}", s.game.hero.water_level.to_f32());
    assert!(wading > 20, "wading flag 0x1413f9");
}

/// Rilgar (05): the canal (surface-0 faces at 59.5, the flat plane of class 982) and the patch managers' water.
#[test]
fn rilgar_swim_and_wade() { swim_and_wade(5); }

/// Pokitaru (11): the sea and the tide pools of class 1158.
#[test]
fn pokitaru_swim_and_wade() { swim_and_wade(11); }

/// Pokitaru's tide pools: the water is the 1158 mobys' collision (surface-0 faces at the moby's z) and the height
/// query reads their ripple patches, so Ratchet dropped into a pool treads water on the patch's surface.
///
/// The spot is a pool with nothing solid within 16 units above its surface. At patch 0's moby (577.7, 489.5) the
/// pool lies under world faces (surface 0xc at z 141.47 and 144.11): Ratchet placed there is ground-snapped by the
/// hero init (`0x26e618`: a world-mesh line, the pool is moby collision) onto the pool floor 7.6 below the water,
/// his ground probe starts under the surface, and the idle's periodic submerged check (a line from 4 / 16 above his
/// feet, flags 2) meets those faces before the water: he stands on the floor, as the game's rules do there.
#[test]
fn pokitaru_tide_pool_swim() {
    let Some(probe) = load(11, None) else { eprintln!("skipped: no extracted/"); return };
    let scene = probe.svc.hero_scene(&probe.game.mobys, probe.classes.clone(), None);
    let open = probe.game.mobys.mobys.iter().enumerate().find(|(i, m)| {
        let p = m.position;
        m.o_class == 1158
            && rc_game::collision_query::coll_line_m(&probe.mesh, Some(&scene.scene()), [p[0], p[1], p[2] + 16.0], [p[0], p[1], p[2] - 1.0], rc_game::collision_query::QueryFlags(2), None)
                .is_some_and(|o| o.moby == Some(*i) && o.surface_id() == 0)
    });
    let (_, m) = open.expect("an open tide pool");
    let (x, y, zw) = (m.position[0], m.position[1], m.position[2]);
    let mut s = load(11, Some([x, y, zw + 2.0])).unwrap();
    let (mut states, mut level_seen, mut patch_hits) = (Vec::new(), None, 0);
    for _ in 0..240 {
        s.tick(PadInput::neutral());
        let h = &s.game.hero;
        if states.last() != Some(&h.state) { states.push(h.state); }
        if h.state == 0x37 { level_seen = Some(h.water_level.to_f32()); }
        let p = h.position();
        if s.svc.water.water_height([p[0], p[1], zw]).is_some() { patch_hits += 1; }
    }
    eprintln!("level 11 tide pool at ({x}, {y}, {zw}): states {states:x?}, water level {level_seen:?}, ticks over an active patch {patch_hits}, end {:?}", s.game.hero.position());
    assert!(states.contains(&0x37), "treading water in the pool: {states:x?}");
    let w = level_seen.unwrap();
    // The level is the patch's rippled surface: the moby's z ± the wave.
    assert!((w - zw).abs() < 0.3, "the pool's level {w} (moby z {zw})");
    assert_ne!(w, zw, "the level comes from the ripple patch, not the face");
    assert!(patch_hits > 0, "the patch under him is active (in view)");
}
