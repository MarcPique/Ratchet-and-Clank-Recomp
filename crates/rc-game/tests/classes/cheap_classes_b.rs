//! The census "cheap win" units whose first copy is on levels 09–18 (class_census.md, G-CLS-027;
//! `rc_game::moby_update::classes::units`): each unit resolves to its port on its levels, and runs headless on one of
//! them, covering the behaviour and the side effects its coverage table marks ported. Skipped when `extracted/` is
//! absent.
//!
//! `cargo xtask test-job --test classes --filter cheap_classes_b:: --nocapture` prints the per-unit survey.

use rc_formats::level_overlay::LevelOverlay;
use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gadget, gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId, MobyTable};
use rc_game::moby_update::classes::units::{self, asteroid, barricade, chain_link, explosive_tank, extending_piece, fleet_door, linked_cog, rising_float, tethered_platform, veldin_carrier};
use rc_game::moby_update::interact::GameWrite;
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{pvar as p, HitTemplate, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::rng::Rng;
use std::collections::HashMap;
use std::sync::Arc;

fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap();
    ClassUpdate::Unit(i as u16)
}

/// (unit, class, the levels whose table runs it, created instances on them).
const EXPECTED: &[(&str, i16, &[u32], usize)] = &[
    ("U408", asteroid::ROCK, &[13], 130),
    ("U408", asteroid::PIECE, &[13], 0),
    ("U303", chain_link::LINK, &[9], 124),
    ("U294", 1182, &[9], 0),
    ("U294", 1184, &[9], 0),
    ("U294", 1189, &[9], 0),
    ("U417", explosive_tank::TANK, &[13], 55),
    ("U533", 1359, &[17], 0),
    ("U533", 1373, &[17], 0),
    ("U477", 937, &[15], 48),
    ("U479", 1250, &[15], 18),
    ("U500", 650, &[16], 29),
    ("U499", 647, &[16], 16),
    ("U563", 1584, &[18], 38),
    ("U559", 1432, &[18], 13),
    ("U493", 482, &[16], 8),
    ("U484", 1425, &[15], 6),
    ("U456", 1397, &[14], 6),
    ("U553", 885, &[18], 0),
    ("U553", 888, &[18], 0),
    ("U553", 891, &[18], 0),
    ("U553", 892, &[18], 0),
    ("U553", 894, &[18], 0),
    ("U553", 900, &[18], 0),
    ("U553", 901, &[18], 0),
    ("U553", 936, &[18], 0),
];

/// Every unit's classes resolve to the unit on its levels (and to nothing else), with the census's instance counts;
/// the unit addresses are distinct from every other port's (the scheduler maps a moby's update address back).
#[test]
fn units_resolve_on_their_levels() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut addrs: HashMap<u32, ClassUpdate> = HashMap::new();
    for u in ClassUpdate::every() {
        if let Some(o) = addrs.insert(u.key(), u) { panic!("{u:?} and {o:?} share 0x{:x}", u.key()); }
    }
    let mut created: HashMap<(&str, i16), usize> = HashMap::new();
    for level in 0..19u32 {
        let p = ports(level).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        for &(name, oc, levels, _) in EXPECTED {
            let u = unit(name);
            if levels.contains(&level) {
                assert_eq!(p.get(oc), Some(u), "level {level:02} class {oc}");
                let n = inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == oc && t.spawn).count();
                *created.entry((name, oc)).or_default() += n;
            } else if p.in_table(oc) {
                assert_ne!(p.get(oc), Some(u), "level {level:02} class {oc}");
            }
        }
    }
    eprintln!("created instances now ported: {created:?}");
    let mut by_unit: HashMap<&str, usize> = HashMap::new();
    for (&(name, _), &n) in &created { *by_unit.entry(name).or_default() += n; }
    for &(name, oc, _, n) in EXPECTED { if n > 0 { assert_eq!(created[&(name, oc)], n, "{name} class {oc}"); } }
    eprintln!("by unit: {by_unit:?}");
    assert_eq!(by_unit["U553"], 119);
    let u294: usize = (1182..=1189).map(|c| {
        let gp = rc_formats::test_data::gameplay(9).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == c && t.spawn).count()
    }).sum();
    assert_eq!(u294, 80);
}

// ---------------------------------------------------------------------------------------------------
// Headless levels

struct Lv {
    table: MobyTable,
    classes: ClassTable,
    svc: Services,
    sched: Scheduler,
    rng: Rng,
    mesh: collision::Collision,
    counter: u64,
    missions: rc_game::moby_update::services::LevelMissions,
}

fn load(level: u32) -> Option<Lv> {
    let core = rc_formats::test_data::core(level)?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let ports = ports(level)?;
    let mesh = collision::parse_collision(&core.core, &core.data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4).max(1) as usize;
    let mut classes = ClassTable::default();
    let mut joints = HashMap::new();
    for (slot, e) in core.core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = core.block(&format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, ports.update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| rc_game::moby_runtime::Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            if ports.needs_joint_lists(oc) {
                joints.insert(oc, (0..16).map_while(|l| gadget::joint_list(blob, &c.header, l).ok().map(|(a, _)| a)).collect());
            }
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: ports.update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let statics = load_level_mobys(&instances, &mut classes, &pvars, &tests);
    let mut table = MobyTable::new(statics.mobys.clone(), spawnable);
    if let Some(h) = table.mobys.iter().position(|m| m.o_class == 0) { table.mobys[h].mode |= mode::NO_UPDATE; }
    let mut svc = Services::new();
    svc.level = level;
    svc.set_splines(&gameplay::parse_splines(&gp).unwrap());
    svc.groups = statics.groups(&gp);
    svc.set_moby_collision(rc_formats::moby_collision::parse_level(&core.core, &core.data).unwrap());
    svc.set_volumes(rc_formats::volumes::parse_volumes(&gp).unwrap());
    svc.joint_lists = joints;
    svc.build_grid(&mut table);
    Some(Lv { table, classes, svc, sched: Scheduler::new(), rng: Rng::new(), mesh, counter: 0, missions: rc_game::moby_update::services::LevelMissions::fresh_load(level, [0xff; 16]) })
}

impl Lv {
    fn world<'a>(&'a mut self, hero: &'a Hero) -> World<'a> {
        let mut w = World::new(&mut self.table, hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
        w.coll = Some(&self.mesh);
        w.camera = hero.pos;
        w.missions = &self.missions;
        w
    }
    fn load_pass(&mut self, hero: &Hero) {
        let mut sched = std::mem::take(&mut self.sched);
        { let mut w = self.world(hero); sched.load_pass(&mut w); }
        self.sched = sched;
    }
    fn tick(&mut self, hero: &Hero) {
        self.counter += 1;
        let c = self.counter;
        self.table.free_slot_pass(c);
        let mut sched = std::mem::take(&mut self.sched);
        { let mut w = self.world(hero); sched.tick(&mut w); }
        self.sched = sched;
    }
    fn of_class(&self, oc: i16) -> Vec<MobyId> { self.table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
    /// Runs one moby's update directly (a state forced by the test).
    fn run(&mut self, hero: &Hero, id: MobyId, u: ClassUpdate) {
        let mut w = self.world(hero);
        rc_game::moby_update::classes::dispatch(u, &mut w, id);
    }
}

fn hero_at(p: [f32; 3]) -> Hero {
    let mut h = Hero::new();
    h.pos = rc_game::hero::physics::v4(p[0], p[1], p[2]);
    h
}

// ---------------------------------------------------------------------------------------------------
// U408: Gemlik's asteroids 212 / 1412

#[test]
fn asteroids_idle_then_split_on_a_hit() {
    let Some(mut lv) = load(13) else { eprintln!("skipped"); return };
    let rocks = lv.of_class(asteroid::ROCK);
    assert_eq!(rocks.len(), 130);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for _ in 0..4 { lv.tick(&far); }
    let mut spinning = 0;
    for &r in &rocks {
        let m = &lv.table.mobys[r];
        assert_eq!(m.state, 1, "rock {r} idles (Ratchet not in state 0x32)");
        let home = p::v4f(&m.pvars, asteroid::pv_::HOME);
        assert_eq!(home[3], asteroid::LO);
        assert_eq!(m.position, [home[0], home[1], home[2], m.position[3]]);
        for k in 0..3 { assert!((asteroid::LO..=asteroid::HI).contains(&m.position[k])); }
        if p::v4f(&m.pvars, asteroid::pv_::SPIN).iter().any(|&a| a != 0.0) { spinning += 1; }
    }
    assert_eq!(spinning, rocks.len());
    // A drifting rock hit by a weapon (mask 0x330000): its class sound, the explosion, 1–2 pieces 1412 with
    // velocities, the split cool-down with collision off.
    let r = rocks[0];
    lv.table.mobys[r].state = 2;
    let at = lv.table.mobys[r].position;
    let hero = hero_at([at[0] + 5.0, at[1], at[2]]);
    let scale0 = lv.table.mobys[r].scale;
    let splits0 = p::i16(&lv.table.mobys[r].pvars, asteroid::pv_::SPLITS);
    {
        let mut w = lv.world(&hero);
        let t = HitTemplate { flags: 0x10_0000, ..Default::default() };
        w.deliver_hit(r, &t);
    }
    lv.counter = r as u64 + 1; // the contact sweep runs on the other parity: the hit message is read
    let sounds0 = lv.svc.sounds.len();
    let part0 = lv.svc.fx.part_spawns.values().sum::<u64>();
    lv.run(&hero, r, unit("U408"));
    let pieces = lv.of_class(asteroid::PIECE);
    let m = &lv.table.mobys[r];
    eprintln!("rock {r}: scale {scale0} -> {}, splits {splits0} -> {}, pieces {}", m.scale, p::i16(&m.pvars, asteroid::pv_::SPLITS), pieces.len());
    assert!((1..=2).contains(&pieces.len()));
    assert!(m.scale < scale0 && !m.has_collision && m.hit_slot == 0xff);
    assert_eq!(p::i16(&m.pvars, asteroid::pv_::COOL), 30);
    assert_eq!(p::i16(&m.pvars, asteroid::pv_::SPLITS) as usize, splits0 as usize - pieces.len());
    assert!(lv.svc.sounds[sounds0..].iter().any(|s| s.moby == r && s.index == 0));
    assert!(lv.svc.fx.part_spawns.values().sum::<u64>() > part0, "the explosion's particles");
    for &c in &pieces {
        let q = &lv.table.mobys[c];
        assert_eq!((q.state, q.update_dist, q.visible, q.has_collision), (2, 0xff, 1, false));
        assert_eq!(p::i32(&q.pvars, asteroid::pv_::PIECE), 1);
        assert_eq!(p::i32(&q.pvars, asteroid::pv_::MAKER), r as i32 + 1);
        assert!(p::v4f(&q.pvars, asteroid::pv_::VEL)[..3].iter().any(|&v| v != 0.0));
    }
    // A piece drifts; far away and undrawn it is deleted when its timer ends.
    let c = pieces[0];
    let p0 = lv.table.mobys[c].position;
    lv.run(&hero, c, unit("U408"));
    assert_ne!(lv.table.mobys[c].position, p0);
    lv.table.mobys[c].visible = 0;
    let away = hero_at([at[0] + 500.0, at[1], at[2]]);
    let mut n = 0;
    while lv.table.mobys[c].state < 0x80 && n < 200 { lv.run(&away, c, unit("U408")); n += 1; }
    assert!(lv.table.mobys[c].state >= 0x80, "piece deleted after {n} ticks");
}

// ---------------------------------------------------------------------------------------------------
// U303: Gaspar's chain links 1181

#[test]
fn chain_links_find_their_neighbours_and_break_in_turn() {
    let Some(mut lv) = load(9) else { eprintln!("skipped"); return };
    let links = lv.of_class(chain_link::LINK);
    assert_eq!(links.len(), 124);
    let hero = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&hero);
    for _ in 0..16 { lv.tick(&hero); }
    let (mut with_prev, mut with_next) = (0, 0);
    for &l in &links {
        let m = &lv.table.mobys[l];
        assert_eq!(m.state, 1);
        if p::i32(&m.pvars, chain_link::pv_::PREV) != -1 { with_prev += 1; }
        if p::i32(&m.pvars, chain_link::pv_::NEXT) != -1 { with_next += 1; }
    }
    let unfound = lv.svc.fx.unported.iter().filter(|(k, _)| k.starts_with("1181")).map(|(_, v)| *v).sum::<u64>();
    eprintln!("1181: {} links, {with_prev} with a previous link, {with_next} with a next, {unfound} searches found none", links.len());
    assert!(with_prev + with_next > links.len());
    // Break one link with both neighbours: the break runs down its chain.
    let start = *links.iter().find(|&&l| {
        let pv = &lv.table.mobys[l].pvars;
        p::i32(pv, chain_link::pv_::PREV) != -1 && p::i32(pv, chain_link::pv_::NEXT) != -1
    }).unwrap();
    lv.table.mobys[start].cmd = 1;
    let at = lv.table.mobys[start].position;
    let hero = hero_at([at[0] + 3.0, at[1], at[2]]);
    let parts0 = lv.svc.fx.part_spawns.values().sum::<u64>();
    let sounds0 = lv.svc.sounds.len();
    for _ in 0..120 { lv.tick(&hero); }
    let gone = links.iter().filter(|&&l| lv.table.mobys[l].state >= 0x80).count();
    eprintln!("after the break of link {start}: {gone} links gone");
    assert!(lv.table.mobys[start].state >= 0x80);
    assert!(gone >= 3, "the neighbours broke in turn");
    assert!(lv.svc.fx.part_spawns.values().sum::<u64>() > parts0);
    assert!(lv.svc.sounds[sounds0..].iter().any(|s| s.index == 0 && s.o_class == chain_link::LINK), "class sound 0 of the explosion");
}

// ---------------------------------------------------------------------------------------------------
// U553: Veldin's grouped breakable pieces 885…936

#[test]
fn barricade_group_flies_apart_on_a_hit() {
    let Some(mut lv) = load(18) else { eprintln!("skipped"); return };
    let gone = {
        let mut probe = load(18).unwrap();
        probe.load_pass(&hero_at([1.0, 1.0, 1.0]));
        barricade::CLASSES.iter().flat_map(|&c| probe.of_class(c)).count()
    };
    eprintln!("barricade: {gone} live after init with no mission started");
    // Their missions started (save bytes 0): they stay.
    lv.missions = rc_game::moby_update::services::LevelMissions::fresh_load(18, [0; 16]);
    let pieces: Vec<MobyId> = barricade::CLASSES.iter().flat_map(|&c| lv.of_class(c)).collect();
    let words: Vec<i32> = pieces.iter().map(|&m| p::i32(&lv.table.mobys[m].pvars, 0)).collect();
    let hero = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&hero);
    let live: Vec<MobyId> = pieces.iter().copied().filter(|&m| lv.table.mobys[m].state < 0x80).collect();
    eprintln!("barricade: {} pieces, {} live after init (mission-gated ones deleted), pvar word 0 non-zero: {} (e.g. {:x?})", pieces.len(), live.len(), words.iter().filter(|&&x| x != 0).count(), words.iter().filter(|&&x| x != 0).take(6).collect::<Vec<_>>());
    for &m in &live { assert_eq!(lv.table.mobys[m].state, 1); }
    // Hit the piece of the largest group.
    let g_of = |lv: &Lv, m: MobyId| lv.table.mobys[m].group;
    let target = *live.iter().filter(|&&m| g_of(&lv, m) >= 0).max_by_key(|&&m| live.iter().filter(|&&k| g_of(&lv, k) == g_of(&lv, m)).count()).unwrap();
    let g = g_of(&lv, target);
    let members: Vec<MobyId> = live.iter().copied().filter(|&m| g_of(&lv, m) == g).collect();
    let at = lv.table.mobys[target].position;
    let hero = hero_at([at[0] + 6.0, at[1], at[2]]);
    {
        let mut w = lv.world(&hero);
        w.deliver_hit(target, &HitTemplate { flags: barricade::HIT_MASK, ..Default::default() });
    }
    let sounds0 = lv.svc.sounds.len();
    lv.run(&hero, target, unit("U553"));
    assert!(members.iter().all(|&m| lv.table.mobys[m].state == 2), "group {g}: every member to state 2");
    assert!(lv.svc.sounds[sounds0..].iter().any(|s| s.moby == target && s.sound_class == barricade::SOUND_CLASS && s.index == 0));
    for &m in &members { lv.run(&hero, m, unit("U553")); }
    for &m in &members {
        let q = &lv.table.mobys[m];
        if q.o_class == barricade::BURST { assert!(q.state >= 0x80); continue; }
        assert_eq!((q.state, q.has_collision), (3, false));
        let v = p::v4f(&q.pvars, barricade::pv_::VEL);
        assert!(v[2] >= 0.0);
        // Away from Ratchet (level component), up to the random part.
        let d = [q.position[0] - hero.pos[0].to_f32(), q.position[1] - hero.pos[1].to_f32()];
        assert!(v[0] * d[0] + v[1] * d[1] > -0.5, "{m}: flies away");
    }
    let flashes0 = lv.svc.fx.flashes;
    for _ in 0..400 { for &m in &members { if lv.table.mobys[m].state < 0x80 { lv.run(&hero, m, unit("U553")); } } }
    let left = members.iter().filter(|&&m| lv.table.mobys[m].state < 0x80).count();
    eprintln!("group {g}: {} members; {left} still flying after 400 ticks", members.len());
    assert_eq!(left, 0, "every piece exploded");
    assert!(lv.svc.fx.flashes > flashes0, "the death explosions (their flashes; the sparks need a particle system)");
}

// ---------------------------------------------------------------------------------------------------
// U294: Gaspar's tethered platforms 1182–1189

#[test]
fn tethered_platform_breaks_and_sinks_into_the_lava() {
    let Some(mut lv) = load(9) else { eprintln!("skipped"); return };
    let all: Vec<MobyId> = tethered_platform::CLASSES.iter().flat_map(|&c| lv.of_class(c)).collect();
    let cores = lv.of_class(tethered_platform::CORE);
    lv.svc.interact.game.flags = vec![0; 128];
    let hero = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&hero);
    let made = tethered_platform::CLASSES.iter().flat_map(|&c| lv.of_class(c)).count() - all.len();
    let with_core = all.iter().filter(|&&m| m != 0 && lv.table.mobys[m].o_class != tethered_platform::CORE && p::i32(&lv.table.mobys[m].pvars, tethered_platform::pv_::CORE) > 0).count();
    eprintln!("1182..1189: {} placed ({} cores), {made} made by cores, {with_core} placed pieces found a core", all.len(), cores.len());
    for &m in &all { assert_eq!(lv.table.mobys[m].state, 1); }
    // The core bobs.
    let core = cores[0];
    let at = lv.table.mobys[core].position;
    let near = hero_at([at[0] + 10.0, at[1], at[2]]);
    let z0 = at[2];
    for _ in 0..30 { lv.tick(&near); }
    assert_ne!(lv.table.mobys[core].position[2], z0, "bob");
    // Told to break: the pieces follow, everything shakes loose, explodes and falls.
    let pieces: Vec<MobyId> = tethered_platform::CLASSES.iter().flat_map(|&c| lv.of_class(c)).filter(|&m| m != core && usize::try_from(p::i32(&lv.table.mobys[m].pvars, tethered_platform::pv_::CORE) - 1).ok() == Some(core)).collect();
    lv.table.mobys[core].cmd = 1;
    let sounds0 = lv.svc.sounds.len();
    let mut splashed = false;
    let mut seen = vec![false; 6];
    for t in 0..2000 {
        lv.tick(&near);
        let s = lv.table.mobys[core].state;
        if s < 6 { seen[s as usize] = true; }
        if lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0) > 0 { splashed = true; }
        if s >= 0x80 { eprintln!("core gone after {t} ticks"); break; }
    }
    eprintln!("core states seen {seen:?}; {} pieces; writes {:?}", pieces.len(), lv.svc.interact.writes);
    assert!(seen[2] && seen[4], "breaking then falling");
    assert!(lv.svc.interact.writes.contains(&GameWrite::Flag(0x38, 1)), "0x13d3c0 = 1");
    assert_eq!(lv.svc.interact.game.flags[0x38], 1);
    assert!(lv.svc.sounds[sounds0..].iter().any(|e| e.moby == core && e.index == 2), "the core's class sound 2");
    assert!(lv.svc.fx.flashes > 0, "the death explosions' flashes");
    if splashed { assert!(lv.svc.sounds.iter().any(|e| e.index == 0 && tethered_platform::CLASSES.contains(&e.o_class))); }
    for &m in &pieces { assert!(lv.table.mobys[m].state >= 3, "piece {m} shook loose too"); }
    // Revisit: the flag deletes it at init.
    let f = p::i32(&lv.table.mobys[core].pvars, tethered_platform::pv_::FLAG);
    eprintln!("core flag byte {f}; splashed {splashed}");
}

// ---------------------------------------------------------------------------------------------------
// U417: Gemlik's explosive tanks 1261

#[test]
fn explosive_tank_takes_hits_then_explodes() {
    let Some(mut lv) = load(13) else { eprintln!("skipped"); return };
    let p13 = ports(13).unwrap();
    eprintln!("1261's fireball {}: level13 update {:x?}, port {:?}", explosive_tank::FIREBALL, p13.level_update(explosive_tank::FIREBALL), p13.get(explosive_tank::FIREBALL));
    let tanks = lv.of_class(explosive_tank::TANK);
    assert_eq!(tanks.len(), 55);
    let t = tanks[0];
    let hp0 = p::ff(&lv.table.mobys[t].pvars, explosive_tank::pv_::HEALTH);
    let col = lv.table.mobys[t].pvars[explosive_tank::pv_::D + 8];
    eprintln!("tank {t}: health {hp0}, damage column {col}");
    // Most attacks' reactions (1 / 2) blow a tank up at once; a column-3 record with more health takes the damage.
    lv.table.mobys[t].pvars[explosive_tank::pv_::D + 8] = 3;
    p::set_ff(&mut lv.table.mobys[t].pvars, explosive_tank::pv_::HEALTH, 100.0);
    let hp0 = 100.0;
    let at = lv.table.mobys[t].position;
    let hero = hero_at([at[0] + 6.0, at[1], at[2]]);
    // A small hit: health down, the flash starts (red byte 0xfa), it rocks.
    let small = HitTemplate { flags: 0x1_0000, damage: rc_game::ps2v::Pf::f(0.25), ..Default::default() };
    { let mut w = lv.world(&hero); w.deliver_hit(t, &small); }
    lv.run(&hero, t, unit("U417"));
    let hp1 = p::ff(&lv.table.mobys[t].pvars, explosive_tank::pv_::HEALTH);
    eprintln!("tank {t}: health {hp0} -> {hp1}");
    assert!(hp1 < hp0);
    assert_eq!(lv.table.mobys[t].pvars[explosive_tank::pv_::F_RED], 0xfa);
    assert_ne!(lv.table.mobys[t].ambient, [0x40, 0x40, 0x40, 0]);
    // A big one: it explodes.
    let big = HitTemplate { flags: 0x1_0000, damage: rc_game::ps2v::Pf::f(1000.0), ..Default::default() };
    lv.table.mobys[t].pvars[explosive_tank::pv_::D + 8] = col;
    // Let the resolver's per-kind cooldown run out first.
    for _ in 0..70 { lv.run(&hero, t, unit("U417")); }
    { let mut w = lv.world(&hero); w.deliver_hit(t, &big); }
    let sounds0 = lv.svc.sounds.len();
    let flashes0 = lv.table.mobys.iter().filter(|m| m.o_class == 1192 && m.state < 0x80).count();
    lv.run(&hero, t, unit("U417"));
    let m = &lv.table.mobys[t];
    assert_eq!((m.state, m.mode & 0x1000), (1, 0));
    let balls = lv.of_class(explosive_tank::FIREBALL).len();
    let flashes = lv.table.mobys.iter().filter(|m| m.o_class == 1192 && m.state < 0x80).count() - flashes0;
    eprintln!("burst: {balls} fireballs, {flashes} flashes, scorch spawns {:?}", lv.svc.fx.part_spawns.get(&52));
    assert_eq!(balls, 15);
    assert!(flashes >= 3);
    assert!(lv.svc.sounds[sounds0..].iter().any(|e| e.moby == t && e.index == 0));
    assert!(lv.svc.fx.part_spawns.get(&52).copied().unwrap_or(0) >= 1, "the scorch");
    // The damage sphere (`attack::area_hit`, shared with the bomb and the chicken) runs each tick; the tanks' own
    // collision lists nothing next to it headless, so its hits are covered by those ports' tests.
    for _ in 0..30 { if lv.table.mobys[t].state < 0x80 { lv.run(&hero, t, unit("U417")); } }
    assert!(lv.table.mobys[t].state >= 0x80, "deleted after 20 ticks");
    // The fireballs: the high ones trail type-2 blobs; all fall and are gone within their lives.
    let balls = lv.of_class(explosive_tank::FIREBALL);
    let fb = ClassUpdate::Unit(units::PORTS.iter().position(|u| u.unit == "U417 fireball").unwrap() as u16);
    assert_eq!(ports(13).unwrap().get(explosive_tank::FIREBALL), Some(fb));
    let blobs0 = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0);
    for _ in 0..200 { for &b in &balls { if lv.table.mobys[b].state < 0x80 { lv.run(&hero, b, fb); } } }
    let blobs = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0) - blobs0;
    eprintln!("fireballs: {blobs} trail blobs; left {}", balls.iter().filter(|&&b| lv.table.mobys[b].state < 0x80).count());
    assert!(blobs >= 4);
    assert!(balls.iter().all(|&b| lv.table.mobys[b].state >= 0x80));
}

// ---------------------------------------------------------------------------------------------------
// U533: Drek's Fleet sliding doors 1359…1373

#[test]
fn fleet_door_opens_for_ratchet_and_shuts_after() {
    let Some(mut lv) = load(17) else { eprintln!("skipped"); return };
    let doors: Vec<MobyId> = fleet_door::CLASSES.iter().flat_map(|&c| lv.of_class(c)).collect();
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    let live: Vec<MobyId> = doors.iter().copied().filter(|&d| lv.table.mobys[d].state < 0x80).collect();
    eprintln!("fleet doors: {} placed, {} with both cuboids", doors.len(), live.len());
    assert_eq!(doors.len(), 52);
    for &d in &live { assert_eq!(lv.table.mobys[d].state, 1); }
    // A flat door (|rot x| < 45°) and Ratchet in its trigger cuboid.
    let d = *live.iter().find(|&&d| lv.table.mobys[d].rotation[0].abs() < 0.78).unwrap();
    let trig = p::i32(&lv.table.mobys[d].pvars, fleet_door::pv_::TRIGGER);
    let c = lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, trig).unwrap().centre();
    let inside = hero_at(c);
    let home = lv.table.mobys[d].position;
    let sounds0 = lv.svc.sounds.len();
    for _ in 0..45 { lv.run(&inside, d, unit("U533")); }
    let m = &lv.table.mobys[d];
    let moved = ((m.position[0] - home[0]).powi(2) + (m.position[1] - home[1]).powi(2)).sqrt();
    eprintln!("door {d} (class {}): state {}, slid {moved}", m.o_class, m.state);
    assert_eq!(m.state, 4);
    assert!((moved - 2.0).abs() < 0.1);
    assert!(lv.svc.sounds[sounds0..].iter().any(|e| e.moby == d && e.index == 0 && e.flags == 4), "the slide sound");
    // Ratchet (and the camera) gone: it shuts back home.
    for _ in 0..60 { lv.run(&far, d, unit("U533")); }
    let m = &lv.table.mobys[d];
    assert_eq!(m.state, 1);
    assert_eq!(&m.position[..3], &home[..3]);
}

// ---------------------------------------------------------------------------------------------------
// The small units: U477 cogs 937, U479 belts 1250 (15), U500 floats 650, U499 pieces 647 (16), U563 carriers 1584 (18)

#[test]
fn small_units_on_their_levels() {
    let Some(mut lv) = load(15) else { eprintln!("skipped"); return };
    let hero = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&hero);
    // Cogs turn with their linked moby's state 2 / 4.
    let cogs = lv.of_class(937);
    let cog = *cogs.iter().find(|&&c| p::i32(&lv.table.mobys[c].pvars, 0) != -1).unwrap();
    let l = p::i32(&lv.table.mobys[cog].pvars, 0) as usize;
    eprintln!("cog {cog} linked to {l} (class {})", lv.table.mobys[l].o_class);
    for (st, flag, sign) in [(2u8, 0, -1.0f32), (2, 1, 1.0), (4, 0, 1.0), (4, 1, -1.0), (1, 0, 0.0)] {
        lv.table.mobys[l].state = st;
        if lv.table.mobys[l].pvars.len() >= 0x14 { p::set_i32(&mut lv.table.mobys[l].pvars, 0x10, flag); }
        lv.table.mobys[cog].rotation[2] = 0.0;
        lv.run(&hero, cog, unit("U477"));
        let want = sign * linked_cog::RATE.abs() * 0.017453292 / 60.0;
        assert!((lv.table.mobys[cog].rotation[2] - want).abs() < 1e-6, "state {st} flag {flag}");
    }
    // Belts: a moving floor whose block carries 2.5 u/s across the heading.
    let belt = lv.of_class(1250)[0];
    lv.run(&hero, belt, unit("U479"));
    lv.run(&hero, belt, unit("U479"));
    let m = &lv.table.mobys[belt];
    let d = rc_game::moby_update::triggers::platform_delta(m).unwrap();
    let a = m.rotation[2] + std::f32::consts::FRAC_PI_2;
    eprintln!("belt {belt}: block {:?}", d);
    assert!(d.flags & 4 != 0);
    assert!((d.displacement[0] - a.cos() * 2.5 / 60.0).abs() < 1e-5 && (d.displacement[1] - a.sin() * 2.5 / 60.0).abs() < 1e-5);

    let Some(mut lv) = load(16) else { return };
    lv.load_pass(&hero);
    // A float held up by its timer rises 3 and comes back, with class sound 1 when the timer ends.
    let f = lv.of_class(650)[0];
    lv.run(&hero, f, unit("U500"));
    let rest = lv.table.mobys[f].position[2];
    p::set_i32(&mut lv.table.mobys[f].pvars, 8, 120);
    for _ in 0..119 { lv.run(&hero, f, unit("U500")); }
    let up = lv.table.mobys[f].position[2];
    let sounds0 = lv.svc.sounds.len();
    lv.counter = 1000;
    lv.run(&hero, f, unit("U500"));
    assert!(lv.svc.sounds[sounds0..].iter().any(|e| e.moby == f && e.index == 1), "class sound 1 at the end");
    for _ in 0..200 { lv.run(&hero, f, unit("U500")); }
    eprintln!("float {f}: rest {rest}, up {up}, back {}", lv.table.mobys[f].position[2]);
    assert!((up - (rest + rising_float::RISE)).abs() < 0.05);
    assert!((lv.table.mobys[f].position[2] - rest).abs() < 1e-4);
    // A piece set going slides out to its distance along row 0.
    let pc = lv.of_class(647)[0];
    lv.run(&hero, pc, unit("U499"));
    let home = lv.table.mobys[pc].position;
    // The distance is written by the moby that starts it (0 in the data): 2 here.
    p::set_ff(&mut lv.table.mobys[pc].pvars, 0x14, 2.0);
    let dist = 2.0f32;
    lv.table.mobys[pc].state = 1;
    for _ in 0..600 { if lv.table.mobys[pc].state == 1 { lv.run(&hero, pc, unit("U499")); } }
    let q = lv.table.mobys[pc].position;
    let moved = ((q[0] - home[0]).powi(2) + (q[1] - home[1]).powi(2) + (q[2] - home[2]).powi(2)).sqrt();
    eprintln!("piece {pc}: distance {dist}, moved {moved}");
    assert_eq!(lv.table.mobys[pc].state, 2);
    assert!((moved - dist.abs()).abs() < 1e-3);
    let _ = extending_piece::CLASSES;

    let Some(mut lv) = load(18) else { return };
    lv.load_pass(&hero);
    // A carrier reports its move to its block and, told to, makes its attachment 1892 that follows it.
    let cr = lv.of_class(1584)[0];
    lv.run(&hero, cr, unit("U563"));
    lv.run(&hero, cr, unit("U563"));
    lv.table.mobys[cr].position[0] += 0.5;
    lv.table.mobys[cr].cmd = 1;
    p::set_i32(&mut lv.table.mobys[cr].pvars, veldin_carrier::WANTS, 1);
    lv.run(&hero, cr, unit("U563"));
    let d = rc_game::moby_update::triggers::platform_delta(&lv.table.mobys[cr]);
    eprintln!("carrier {cr}: block {d:?}, attachment {:?}", lv.of_class(veldin_carrier::ATTACHMENT));
    assert!(d.is_some_and(|d| (d.displacement[0] - 0.5).abs() < 1e-6));
    let att = lv.of_class(veldin_carrier::ATTACHMENT);
    assert_eq!(att.len(), 1);
    assert_eq!(lv.table.mobys[att[0]].position, lv.table.mobys[cr].position);
}

/// The frame cameras: an instance of each unit and an eye 8 units off it (`RC_CAM` strings for the engine shots).
#[test]
#[ignore]
fn frame_cameras() {
    for (name, level, class) in [("U408", 13, 212), ("U303", 9, 1181), ("U553", 18, 891), ("U294", 9, 1184), ("U417", 13, 1261), ("U533", 17, 1359), ("U477", 15, 937), ("U500", 16, 650), ("U563", 18, 1584), ("U479", 15, 1250), ("U499", 16, 647)] {
        let Some(lv) = load(level) else { return };
        let Some(&m) = lv.of_class(class).first() else { continue };
        let p = lv.table.mobys[m].position;
        println!("{name} L{level:02} class {class} moby {m}: RC_CAM={},{},{},{},{},{}", p[0] + 6.0, p[1] - 6.0, p[2] + 4.0, p[0], p[1], p[2]);
    }
}

/// U559 hides its props, U493 deletes its mobys, U484 blows bubbles, U456 initialises its fields.
#[test]
fn tiny_units_on_their_levels() {
    let hero = hero_at([1.0, 1.0, 1.0]);
    let Some(mut lv) = load(18) else { eprintln!("skipped"); return };
    let h = lv.of_class(1432);
    lv.load_pass(&hero);
    for &m in &h { let q = &lv.table.mobys[m]; assert_eq!((q.state, q.mode & 3, q.visible), (1, 3, 0)); }
    let Some(mut lv) = load(16) else { return };
    let d = lv.of_class(482);
    lv.load_pass(&hero);
    assert!(d.iter().all(|&m| lv.table.mobys[m].state >= 0x80), "482 deletes itself");
    let Some(mut lv) = load(15) else { return };
    let v = lv.of_class(1425)[0];
    let n0 = lv.svc.fx.part_spawns.get(&34).copied().unwrap_or(0);
    for _ in 0..100 { lv.run(&hero, v, unit("U484")); }
    let n = lv.svc.fx.part_spawns.get(&34).copied().unwrap_or(0) - n0;
    eprintln!("bubble vent {v}: {n} bubbles in 100 ticks");
    assert!((25..=75).contains(&n), "one tick in two");
    let Some(mut lv) = load(14) else { return };
    let s = lv.of_class(1397)[0];
    lv.run(&hero, s, unit("U456"));
    assert_eq!((lv.table.mobys[s].state, lv.table.mobys[s].pvars[0x28]), (1, 4));
    lv.table.mobys[s].state = 2;
    lv.run(&hero, s, unit("U456"));
    assert_eq!((lv.table.mobys[s].state, p::i16(&lv.table.mobys[s].pvars, 0x3e)), (3, 5));
}

/// Diagnostic: what each level's class table runs for a class number (`CLASS=n`), and the port it resolves to.
#[test]
#[ignore]
fn class_table_survey() {
    let oc: i16 = std::env::var("CLASS").ok().and_then(|v| v.parse().ok()).unwrap_or(360);
    for level in 0..19u32 {
        let Some(p) = ports(level) else { return };
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let placed = gameplay::parse_moby_instances(&gp).unwrap().iter().filter(|m| m.o_class as i16 == oc).count();
        println!("L{level:02} class {oc}: in table {}, update {:x?}, port {:?}, placed {placed}", p.in_table(oc), p.level_update(oc), p.get(oc));
    }
}
