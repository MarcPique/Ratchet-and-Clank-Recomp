//! The help system on the disc's data (docs/plan/hud_text.md "Help system"), headless:
//! * the help log's id table 0x1798d0 (`fun_001fecc8`) found through `Relocation` on all 19 overlays, the same 148 +
//!   2 entries everywhere, and the record indices the callers pass match it (1000 → 4, 1004 → 0x40, 20014 → 0x78 …);
//! * the Novalis help director 1341 (census unit U82) with its placed pvars and the level's cuboids: the look-around
//!   hint 1004 (record 0x40) when Ratchet stands in cuboid +0x38, not once three looks were counted; the map hint 1007
//!   (0x43) in cuboid +0x3c after an hour of play; each request logged, gated by the box and the records;
//! * a whole box on the level text: the Infobot hint 1000 sized in the small font, its voice line 30004 requested,
//!   the record bumped once when it closes.
//!
//! Skipped when `extracted/` is absent.

use rc_formats::{gameplay, strings, volumes};
use rc_game::game_state::HelpRec;
use rc_game::help::{self, Help, HelpInputs, HelpText, VoiceCmd};
use rc_game::moby_runtime::{Moby, MobyTable};
use rc_game::moby_update::classes::units::help_director as director;
use rc_game::moby_update::services::{pvar as p, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::ps2v::Pf;
use std::sync::Arc;

fn overlay(level: u32) -> Option<Vec<u8>> { std::fs::read(rc_formats::test_data::root().join(format!("levels/{level:02}/overlay.bin"))).ok() }

#[test]
fn log_table_on_every_level() {
    let Some(reference) = overlay(1) else { return };
    let l01 = help::load_log_ids(&reference, None);
    assert_eq!(l01.len(), 150);
    assert_eq!(&l01[..6], &[(3, 21106), (0, 21103), (1, 21104), (2, 21105), (1000, 21107), (1001, 21108)]);
    assert_eq!(&l01[148..], &[(0, 0), (0, 0)]);
    // The record index each ported caller passes is the message's log index (except the director's nanotech pair,
    // which passes 1 / 2 for messages 1 / 2: the game's own numbers).
    for (msg, rec) in [(1000, 4), (0x3e9, 5), (0x3ea, 6), (0x3ec, 0x40), (0x3ed, 0x41), (0x3ee, 0x42), (0x3ef, 0x43), (0x3f0, 0x44), (0x4e24, 0x4e), (0x4e26, 0x50), (0x4e27, 0x51), (0x4e28, 0x72), (0x4e2e, 0x78), (0x4e30, 0x81), (9000, 0x34)] {
        assert_eq!(l01.iter().position(|&(m, _)| m as i32 == msg), Some(rec), "message {msg}");
    }
    for level in 0..19 {
        let Some(t) = overlay(level) else { continue };
        assert_eq!(help::load_log_ids(&t, Some(&reference)), l01, "level {level:02}");
    }
}

struct Novalis {
    director: Moby,
    vol: volumes::Volumes,
    messages: Vec<strings::Message>,
    log_ids: Vec<(i16, i16)>,
}

fn novalis() -> Option<Novalis> {
    let gp = rc_formats::test_data::gameplay(1)?;
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let k = instances.iter().position(|i| i.o_class == 1341).expect("one 1341 on Novalis");
    let director = Moby { o_class: 1341, pvars: pvars[instances[k].pvar_index as usize].clone().expect("its pvars"), ..Default::default() };
    Some(Novalis {
        director,
        vol: volumes::parse_volumes(&gp).unwrap(),
        messages: strings::parse_strings(&gp, 0).unwrap(),
        log_ids: help::load_log_ids(&overlay(1)?, None),
    })
}

fn services(n: &Novalis) -> Services {
    let mut svc = Services::new();
    svc.volumes = Arc::new(n.vol.clone());
    svc.help.bx.enabled = true;
    svc.help.log_ids = Arc::new(n.log_ids.clone());
    svc.help.text = HelpText { messages: Arc::new(n.messages.clone()), small: None, lang: 0 };
    svc.interact.game.planet_unlocked = vec![0; 20];
    svc.help.level = 1;
    svc
}

/// One update of the director at `pos` (after its first update, which only arms it); the request it left.
fn run(n: &Novalis, svc: &mut Services, pos: [f32; 3], looks: i32, counter: u64) -> (i32, i32) {
    let mut t = MobyTable::new(vec![n.director.clone()], 4);
    let mut hero = rc_game::hero::Hero::new();
    hero.pos = [Pf::f(pos[0]), Pf::f(pos[1]), Pf::f(pos[2]), Pf::ONE];
    hero.help.looks = looks;
    let mut rng = rc_game::rng::Rng::new();
    let classes = ClassTable::default();
    let mut w = World::new(&mut t, &hero, &mut rng, &classes, svc, counter);
    director::update(&mut w, 0);
    assert_eq!(w.m(0).state, 1);
    director::update(&mut w, 0);
    (w.svc.help.request, w.svc.help.rec)
}

fn cuboid_centre(n: &Novalis, off: usize) -> [f32; 3] {
    let i = p::i32(&n.director.pvars, off);
    n.vol.shape(volumes::ShapeKind::Cuboid, i).expect("the director's cuboid").centre()
}

#[test]
fn director_look_and_map_hints_in_its_cuboids() {
    let Some(n) = novalis() else { return };
    let look = cuboid_centre(&n, director::pv_::LOOK);
    let mut svc = services(&n);
    assert_eq!(run(&n, &mut svc, look, 0, 1000), (0x3ec, 0x40));
    // Logged: the log holds the welcome (index 0) and 1004's index 64.
    assert_eq!((&svc.help.log[..2], svc.help.log_pos), (&[0u8, 64][..], 2));
    // Three looks counted: nothing.
    let mut svc = services(&n);
    assert_eq!(run(&n, &mut svc, look, 3, 1000), (-1, 0));
    // The record shown once already: nothing.
    let mut svc = services(&n);
    svc.help.records.help[0x40].count = 1;
    assert_eq!(run(&n, &mut svc, look, 0, 1000).0, -1);
    // The map cuboid after an hour of play (move record 9 never used), this level's bit clear.
    let map = cuboid_centre(&n, director::pv_::MAP);
    let mut svc = services(&n);
    svc.help.play_time = 0x34bc1;
    assert_eq!(run(&n, &mut svc, map, 0, 1000), (0x3ef, 0x43));
    let mut svc = services(&n);
    svc.help.play_time = 0x34bc1;
    svc.help.records.help[0x43] = HelpRec { count: 1, time: 0, mask: 0x8000_0002 };
    assert_eq!(run(&n, &mut svc, map, 0, 1000).0, -1);
    // Far from every cuboid: nothing.
    let mut svc = services(&n);
    assert_eq!(run(&n, &mut svc, [2.0, 2.0, -100.0], 0, 1000).0, -1);
}

#[test]
fn infobot_hint_box_on_the_level_text() {
    let Some(n) = novalis() else { return };
    let mut h = Help { text: HelpText { messages: Arc::new(n.messages.clone()), small: None, lang: 0 }, log_ids: Arc::new(n.log_ids.clone()), ..Help::default() };
    // The first-input gate: help stays off until 118 ticks after the first direction.
    assert!(h.request(1000, 4));
    let inp = |held| HelpInputs { held, play_time: 36_000, level: 1, text_on: true, voice_on: true, ..Default::default() };
    h.update(&inp(0));
    assert_eq!((h.bx.state, h.request), (0, -1));
    h.update(&inp(0x2000));
    for _ in 0..118 { h.update(&inp(0)); }
    assert!(h.bx.enabled);
    // Accepted, then opened on the next update, with the opening sound; the voice line requested in state 1 and started
    // by the dialogue player's next frame.
    assert!(h.request(1000, 4));
    let mut voice = Vec::new();
    let mut sounds = Vec::new();
    for _ in 0..700 {
        h.voice_frame();
        h.update(&inp(0));
        if h.out.voice.contains(&VoiceCmd::Load { id: 30004 }) { h.voice_loaded(Some(240)); }
        voice.append(&mut h.out.voice);
        sounds.append(&mut h.out.sounds);
    }
    assert_eq!(sounds, vec![help::OPEN_SOUND]);
    assert_eq!(voice, vec![VoiceCmd::Load { id: 30004 }, VoiceCmd::Play { audible: true }]);
    assert_eq!(h.bx.state, 0);
    assert_eq!(h.records.help[4], HelpRec { count: 1, time: 60, mask: 0x8000_0002 });
    // The message's help audio is 4 (help_audio 004 "L1_infobot1").
    assert_eq!(strings::find_index(&n.messages, 1000).map(|i| n.messages[i].help_audio), Some(4));
}

/// Prints the director's cuboid centres (`cargo xtask test-job --test ui --filter help_novalis::print_cuboids --ignored --nocapture`).
#[test]
#[ignore]
fn print_cuboids() {
    let Some(n) = novalis() else { return };
    for (name, off) in [("swim A", director::pv_::SWIM_A), ("swim B", director::pv_::SWIM_B), ("crank", director::pv_::CRANK_CUBOID), ("look", director::pv_::LOOK), ("map", director::pv_::MAP)] {
        let i = p::i32(&n.director.pvars, off);
        let s = n.vol.shape(volumes::ShapeKind::Cuboid, i);
        println!("{name}: cuboid {i} centre {:?} rows {:?}", s.map(|s| s.centre()), s.map(|s| s.matrix));
    }
}
