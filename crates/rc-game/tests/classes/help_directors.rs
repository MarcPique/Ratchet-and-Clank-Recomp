//! The level help directors (docs/plan/level_scripting.md §8; census units U32, U119, U145, U165, U204, U232, U292,
//! U305, U341, U391, U419) on their own levels' data, headless: the placed director with its pvars, the level's
//! cuboids, text and log table. Per director: the trigger fires (the right message and record, logged), the gates keep
//! it from repeating, and the flags and stats it writes are written; the message is in the level's text with its voice
//! line, and a shown box bumps its record when it closes. Skipped when `extracted/` is absent.

use rc_formats::{gameplay, moby_spawn, strings, volumes};
use rc_game::game_state::HelpRec;
use rc_game::help::{self, HelpInputs, VoiceCmd};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{MobyId, MobyTable};
use rc_game::moby_update::classes::pickup::ItemTables;
use rc_game::moby_update::classes::units::{self, help_orxon};
use rc_game::moby_update::classes::ClassUpdate;
use rc_game::moby_update::interact::GameWrite;
use rc_game::moby_update::scheduler::load_level_mobys;
use rc_game::moby_update::services::{pvar as p, LevelMissions, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::rng::Rng;
use std::sync::Arc;

struct Lv {
    level: u32,
    table: MobyTable,
    classes: ClassTable,
    svc: Services,
    vol: Arc<volumes::Volumes>,
    messages: Vec<strings::Message>,
    director: MobyId,
    unit: u16,
    items: ItemTables,
    missions: LevelMissions,
    hero: Hero,
    counter: u64,
}

fn load(level: u32, class: i16) -> Option<Lv> {
    let gp = rc_formats::test_data::gameplay(level)?;
    let ports = crate::common::ports(level, &[])?;
    let (Some(ov), Some(r01)) = (crate::common::overlay(level), crate::common::overlay(1)) else { return None };
    let _ = (ov, r01);
    let raw = |l: u32| std::fs::read(rc_formats::test_data::level_dir(l).join("overlay.bin")).ok();
    let log_ids = help::load_log_ids(&raw(level)?, Some(&raw(1)?));
    let Some(ClassUpdate::Unit(unit)) = ports.get(class) else { panic!("class {class} resolves to no unit on level {level:02}") };
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let mut classes = ClassTable::default();
    let statics = load_level_mobys(&instances, &mut classes, &pvars, &tests);
    let table = MobyTable::new(statics.mobys.clone(), 64);
    let director = table.mobys.iter().position(|m| m.o_class == class).expect("the director is placed");
    let vol = Arc::new(volumes::parse_volumes(&gp).unwrap());
    let messages = strings::parse_strings(&gp, 0).unwrap();
    let mut svc = Services::new();
    svc.level = level;
    svc.volumes = vol.clone();
    svc.groups = statics.groups(&gp);
    svc.pvar_shared = gameplay::parse_pvar_shared_data(&gp).unwrap();
    svc.help.bx.enabled = true;
    svc.help.level = level as i32;
    svc.help.play_time = 36_000;
    svc.help.log_ids = Arc::new(log_ids);
    svc.help.text = help::HelpText { messages: Arc::new(messages.clone()), small: None, lang: 0 };
    svc.interact.game.planet_unlocked = vec![0; 20];
    svc.interact.game.flags = vec![0; 128];
    svc.interact.game.acquired = vec![0; 64];
    Some(Lv { level, table, classes, svc, vol, messages, director, unit, items: ItemTables::default(), missions: LevelMissions::fresh_load(level, [0; 16]), hero: Hero::new(), counter: 1000 })
}

impl Lv {
    /// One update of the director; the request it left (−1: none), then cleared for the next run.
    fn run(&mut self) -> (i32, i32) {
        self.counter += 1;
        let mut rng = Rng::new();
        {
            let mut w = World::new(&mut self.table, &self.hero, &mut rng, &self.classes, &mut self.svc, self.counter);
            w.inventory = &self.items;
            w.missions = &self.missions;
            units::update(&mut w, self.director, self.unit);
        }
        let out = (self.svc.help.request, self.svc.help.rec);
        self.svc.help.request = -1;
        out
    }
    /// The first update (a director with a state 0 only arms), then one more.
    fn arm(&mut self) { if self.table.mobys[self.director].state == 0 { self.run(); } self.svc.help.request = -1; }
    fn cuboid(&self, off: usize) -> [f32; 3] {
        let i = p::i32(&self.table.mobys[self.director].pvars, off);
        self.vol.shape(volumes::ShapeKind::Cuboid, i).unwrap_or_else(|| panic!("level {:02}: cuboid P+{off:#x} = {i}", self.level)).centre()
    }
    fn at(&mut self, off: usize) { let c = self.cuboid(off); self.hero.pos = rc_game::hero::physics::v4(c[0], c[1], c[2]); }
    fn away(&mut self) { self.hero.pos = rc_game::hero::physics::v4(-9999.0, -9999.0, -9999.0); }
    fn help(&mut self, r: usize) -> &mut HelpRec { &mut self.svc.help.records.help[r] }
    fn moves(&mut self, r: usize) -> &mut HelpRec { &mut self.svc.help.records.moves[r] }
    fn own(&mut self, item: usize) { self.items.owned[item] = true; }
    fn flag_writes(&self) -> Vec<usize> { self.svc.interact.writes.iter().filter_map(|w| match w { GameWrite::Flag(i, 1) => Some(*i), _ => None }).collect() }
    fn logged(&self, msg: i32) -> bool {
        let Some(i) = self.svc.help.log_index(msg) else { return false };
        self.svc.help.log[..self.svc.help.log_pos.max(0) as usize].contains(&(i as u8))
    }

    /// The message is in the level's text with a voice line; shown in a box, its voice stream is requested and its
    /// record bumped when the box closes.
    fn box_flow(&mut self, msg: i32, rec: usize) {
        let k = strings::find_index(&self.messages, msg).unwrap_or_else(|| panic!("level {:02}: message {msg} in the level text", self.level));
        let audio = self.messages[k].help_audio;
        let mut h = self.svc.help.clone();
        h.request = -1;
        let before = h.records.help[rec].count;
        assert!(h.request(msg, rec as i32), "level {:02}: {msg} accepted", self.level);
        let inp = HelpInputs { play_time: 40_000, level: self.level as i32, text_on: true, voice_on: true, ..Default::default() };
        let mut voice = Vec::new();
        for _ in 0..3000 {
            h.voice_frame();
            h.update(&inp);
            if let Some(VoiceCmd::Load { .. }) = h.out.voice.iter().find(|c| matches!(c, VoiceCmd::Load { .. })) { h.voice_loaded(Some(120)); }
            voice.append(&mut h.out.voice);
            if h.bx.state == 0 && h.request == -1 && !voice.is_empty() { break; }
        }
        if audio >= 0 { assert!(voice.contains(&VoiceCmd::Load { id: help::VOICE_BASE + audio }), "level {:02}: {msg}'s voice {audio}: {voice:?}", self.level); }
        assert_eq!(h.bx.state, 0, "level {:02}: {msg}'s box closed", self.level);
        assert_eq!(h.records.help[rec].count, before + 1, "level {:02}: record {rec:#x} bumped on close", self.level);
        assert!(h.records.help[rec].mask & (1 << self.level) != 0);
        eprintln!("level {:02}: {msg} (record {rec:#x}) voice {audio}", self.level);
    }
}

// ---------------------------------------------------------------------------------------------------

#[test]
fn veldin_1413_welcome_info_and_first_aid() {
    let Some(mut lv) = load(0, 1413) else { eprintln!("skipped"); return };
    lv.away();
    assert_eq!(lv.run(), (3, 3), "the welcome, anywhere");
    assert!(lv.logged(3));
    assert_eq!(lv.table.mobys[lv.director].update_dist, 0xff);
    lv.help(3).count = 1;
    assert_eq!(lv.run().0, -1, "welcome shown: nothing outside the cuboids");
    lv.at(0x10);
    assert_eq!(lv.run(), (0x4e2a, 0x74));
    lv.help(0x74).count = 1;
    lv.at(0x00);
    assert_eq!(lv.run(), (5, 9));
    lv.moves(1).count = 1;
    assert_eq!(lv.run().0, -1, "M[1] used: no 5");
    lv.at(0x08);
    assert_eq!(lv.run(), (0, 0));
    lv.help(0).count = 1;
    // A watched crate broken: below 4 health 1, else 2.
    let c = p::i32(&lv.table.mobys[lv.director].pvars, 0x14);
    assert!(c >= 0, "a watched crate");
    lv.table.mobys[c as usize].state = 0xfe;
    lv.away();
    lv.hero.health = 3;
    assert_eq!(lv.run(), (1, 1));
    lv.hero.health = 4;
    assert_eq!(lv.run(), (2, 2));
    lv.help(2).count = 1;
    assert_eq!(lv.run().0, -1, "record 2 used: no first-aid hints");
    lv.box_flow(3, 3);
}

#[test]
fn gaspar_1000_hint_touches_its_record() {
    let Some(mut lv) = load(9, 1000) else { eprintln!("skipped"); return };
    lv.at(0);
    assert_eq!(lv.run(), (9001, 0x5b));
    assert_eq!((lv.help(0x5b).count, lv.help(0x5b).mask), (0, 0x8000_0000 | 1 << 9), "time / mask refreshed, count not");
    lv.own(11);
    assert_eq!(lv.run(), (9001, 0x5b), "the tick the item is found: count 1, still asked");
    assert_eq!(lv.help(0x5b).count, 1);
    assert_eq!(lv.run().0, -1, "then never again");
    lv.help(0x5b).count = 0;
    lv.items.owned[11] = false;
    lv.box_flow(9001, 0x5b);
}

#[test]
fn gemlik_558_hint_and_its_stat() {
    let Some(mut lv) = load(13, 558) else { eprintln!("skipped"); return };
    lv.at(0);
    let link = p::i32(&lv.table.mobys[lv.director].pvars, 4);
    let alive = link >= 0 && lv.table.mobys.get(link as usize).is_some_and(|m| m.o_class == 170 && m.state < 0x80);
    lv.svc.game_mode = 0;
    if alive {
        assert_eq!(lv.run(), (13000, 0x6f));
        lv.help(0x6f).mask = 0x8000_0000;
        assert_eq!(lv.run().0, -1, "shown on some level: never again");
        lv.table.mobys[link as usize].state = 0xfe;
    }
    // The moby gone and item 11 owned: the record is bumped every tick (no request).
    lv.own(11);
    let c = lv.help(0x6f).count;
    assert_eq!(lv.run().0, -1);
    assert_eq!(lv.help(0x6f).count, c + 1);
    // Outside game mode 0: nothing.
    lv.svc.game_mode = 2;
    lv.run();
    assert_eq!(lv.help(0x6f).count, c + 1);
    lv.box_flow(13000, 0x6f);
}

#[test]
fn hoven_422_hints_and_hover_stat() {
    let Some(mut lv) = load(12, 422) else { eprintln!("skipped"); return };
    lv.at(0);
    assert_eq!(lv.run(), (12003, 0x6d));
    lv.hero.state = 0x81;
    lv.run();
    assert_eq!(lv.help(0x6d).count, 1, "state 0x81 marks the record");
    lv.away();
    lv.hero.state = 1;
    lv.own(4);
    assert_eq!(lv.run(), (12000, 0x3e));
    lv.help(0x3e).count = 1;
    assert_eq!(lv.run().0, -1);
    lv.box_flow(12003, 0x6d);
}

#[test]
fn blarg_1348_hints_and_switch_flag() {
    let Some(mut lv) = load(6, 1348) else { eprintln!("skipped"); return };
    for k in 0..6 {
        let m = p::i32(&lv.table.mobys[lv.director].pvars, 4 * k);
        if m >= 0 { lv.table.mobys[m as usize].state = 1; }
    }
    lv.at(0x24);
    assert_eq!(lv.run(), (0x1773, 0x2a));
    lv.help(0x2a).count = 1;
    assert_eq!(lv.run().0, -1);
    // The switch pressed: global flag 0x25 written once.
    let sw = p::i32(&lv.table.mobys[lv.director].pvars, 0x2c);
    if sw >= 0 {
        lv.table.mobys[sw as usize].cmd = 1;
        lv.run();
        lv.run();
        assert_eq!(lv.flag_writes(), vec![0x25]);
        assert_eq!(lv.svc.interact.game.flags[0x25], 1);
    }
    lv.box_flow(0x1773, 0x2a);
}

#[test]
fn batalia_1349_reminders_and_grind_flag() {
    let Some(mut lv) = load(8, 1349) else { eprintln!("skipped"); return };
    lv.at(0);
    assert_eq!(lv.run(), (8000, 0x30), "no Grindboots: the reminder (time 0)");
    // Shown a moment ago: refreshed, not asked; more than 18 s later: asked again.
    let now = lv.svc.help.play_time / 600;
    lv.help(0x30).time = now as u16;
    assert_eq!(lv.run().0, -1);
    lv.svc.help.play_time += 60 * 19;
    lv.help(0x30).time = now as u16;
    assert_eq!(lv.run(), (8000, 0x30));
    // Grinding: global flag 0x2f once.
    lv.away();
    lv.hero.group = 0xf;
    lv.run();
    lv.run();
    assert_eq!(lv.flag_writes(), vec![0x2f]);
    lv.box_flow(8000, 0x30);
}

#[test]
fn orxon_1344_hints_and_air_word() {
    let Some(mut lv) = load(10, 1344) else { eprintln!("skipped"); return };
    lv.arm();
    assert_eq!((lv.table.mobys[lv.director].state, lv.table.mobys[lv.director].update_dist), (1, 0xff));
    lv.at(0);
    assert_eq!(lv.run(), (10000, 0x35));
    lv.help(0x35).count = 1;
    assert_eq!(lv.run().0, -1);
    lv.hero.group = 5;
    lv.help(0x35).count = 0;
    assert_eq!(lv.run().0, -1, "not on foot: nothing");
    lv.hero.group = 0;
    lv.at(0x1c);
    lv.run();
    assert_eq!(lv.svc.units.word(help_orxon::AIR_WORD), 1);
    lv.away();
    lv.run();
    assert_eq!(lv.svc.units.word(help_orxon::AIR_WORD), 0);
    lv.box_flow(10000, 0x35);
}

#[test]
fn eudora_1343_entry_stat_hint_and_flags() {
    let Some(mut lv) = load(4, 1343) else { eprintln!("skipped"); return };
    lv.arm();
    lv.at(0);
    // The director's mission (2) open.
    assert_eq!(lv.table.mobys[lv.director].mission, 2);
    lv.run();
    assert_eq!(lv.moves(24).count, 1, "the first entry bumps M[24]");
    lv.run();
    assert_eq!(lv.moves(24).count, 1, "once");
    lv.moves(24).count = 3;
    assert_eq!(lv.run(), (0xfa1, 0x1d));
    lv.help(0x1d).count = 1;
    assert_eq!(lv.run().0, -1);
    // A watched Trespasser in its state 4: global flag 0x1c.
    let t = p::i32(&lv.table.mobys[lv.director].pvars, 0x48);
    if t >= 0 && lv.table.mobys[t as usize].o_class == 0x267 {
        lv.table.mobys[t as usize].state = 4;
        lv.run();
        assert!(lv.flag_writes().contains(&0x1c));
    }
    // The mission done: nothing in the cuboid.
    lv.help(0x1d).count = 0;
    lv.missions.done[2] = 0xff;
    assert_eq!(lv.run().0, -1);
    lv.box_flow(0xfa1, 0x1d);
}

#[test]
fn kerwan_1342_hints_stats_flags_and_cable() {
    let Some(mut lv) = load(3, 1342) else { eprintln!("skipped"); return };
    lv.arm();
    lv.at(0);
    assert_eq!(lv.run(), (3000, 0x14));
    lv.help(0x14).mask = 0x8000_0000;
    assert_eq!(lv.run().0, -1, "shown once: never again");
    // The cable: state 0x74 sets the "cable used" stat M[16]; before that, the cable help in cuboid +0x2c with the wrench.
    lv.at(0x2c);
    lv.hero.group = 6;
    lv.hero.items.slot.id = 8;
    assert_eq!(lv.run(), (0xbbf, 0x1b));
    lv.hero.state = 0x74;
    lv.run();
    assert_eq!(lv.moves(16).count, 1);
    lv.hero.state = 1;
    assert_eq!(lv.run().0, -1, "cable used: no cable help");
    // The flags: +0x70 → 0x18; +0x78 (on foot) → 0x1a, and the 3000 series stops.
    lv.hero.group = 0;
    lv.at(0x70);
    lv.run();
    lv.at(0x78);
    lv.run();
    let f = lv.flag_writes();
    assert!(f.contains(&0x18) && f.contains(&0x1a), "{f:?}");
    lv.help(0x14).mask = 0;
    lv.at(0);
    assert_eq!(lv.run().0, -1, "flag 0x1a set: no 3000");
    // A Swingshot miss is taken (the hero write) and counted.
    lv.hero.swing.help = 1;
    lv.away();
    lv.run();
    assert!(lv.svc.hero_writes.is_some_and(|(_, f)| f.swing_help_clear));
    assert_eq!(p::i32(&lv.table.mobys[lv.director].pvars, 0x54), 1);
    lv.box_flow(3000, 0x14);
}

#[test]
fn aridia_1324_reminder_and_flag() {
    let Some(mut lv) = load(2, 1324) else { eprintln!("skipped"); return };
    lv.at(0x14);
    assert_eq!(lv.run(), (0x7d7, 0x11));
    lv.own(12);
    lv.run();
    assert_eq!(lv.help(0x12).count, 1, "owned: the arm of record 0x12");
    lv.at(0x44);
    lv.run();
    assert_eq!(lv.flag_writes(), vec![0x14]);
    lv.box_flow(0x7d7, 0x11);
}

#[test]
fn rilgar_1347_reminder_and_swim_retire() {
    let Some(mut lv) = load(5, 1347) else { eprintln!("skipped"); return };
    lv.at(0x24);
    assert_eq!(lv.run(), (0x138a, 0x23));
    lv.own(22);
    lv.run();
    assert_eq!(lv.help(0x24).count, 1, "owned: the arm of record 0x24");
    lv.at(0x50);
    lv.hero.group = 0x11;
    lv.run();
    assert_eq!(lv.help(0x44).count, 0xffff, "under water in the swim cuboid: the swim hint retired");
    lv.box_flow(0x138a, 0x23);
}

/// The cuboid centres of the directors' first hints (for placing Ratchet with `RC_HERO_AT`):
/// `cargo xtask test-job --test classes --filter help_directors::print_hint_cuboids --ignored --nocapture`.
#[test]
#[ignore]
fn print_hint_cuboids() {
    for (level, class, off) in [(9u32, 1000i16, 0usize), (10, 1344, 0), (3, 1342, 0), (12, 422, 0), (8, 1349, 0)] {
        let Some(lv) = load(level, class) else { return };
        println!("level {level:02} class {class}: cuboid P+{off:#x} centre {:?}", lv.cuboid(off));
    }
}
