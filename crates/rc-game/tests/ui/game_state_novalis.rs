//! New game → Veldin start → Veldin→Novalis transition → Novalis start, against the disc's own tables and
//! save template (`docs/plan/game_state.md` §4). Skipped when `extracted/` (the extracted game data, never
//! shipped with the repo) is absent. The disc reader's `save_game` lump against `extracted/global/save_game.bin`
//! needs the disc image, so it is a dev command, not a test: `cargo run --release -p rc-trace -- disc-check`.

use rc_formats::save_game::{crc16, ChunkTables, ItemTables, SaveFile, SaveGameLump};
use rc_game::game_state::{item, GameState, SessionState, FLAG_VELDIN_CLANK};
use std::path::PathBuf;

fn root() -> PathBuf { rc_formats::test_data::root() }

struct Data { elf: Vec<u8>, lump: SaveGameLump, overlay: [Vec<u8>; 2] }

fn data() -> Option<Data> {
    let r = root();
    let elf = std::fs::read(r.join("boot/SCUS_971.99")).ok()?;
    let lump = SaveGameLump::parse(&std::fs::read(r.join("global/save_game.bin")).ok()?).unwrap();
    let ov = |l: u32| std::fs::read(r.join(format!("levels/{l:02}/overlay.bin"))).ok();
    Some(Data { elf, lump, overlay: [ov(0)?, ov(1)?] })
}

#[test]
fn first_novalis_arrival() {
    let Some(d) = data() else { eprintln!("skipped: no extracted/"); return; };
    let tables = ChunkTables::from_boot_elf(&d.elf).unwrap();
    let veldin_items = ItemTables::load(&d.elf, &d.overlay[0]).unwrap();
    let novalis_items = ItemTables::load(&d.elf, &d.overlay[1]).unwrap();

    // The template's checksums, and the new game (template restored, level 0).
    let t = &d.lump.template;
    assert_eq!(crc16(&t[16..8 + 0x1530]), 0x9ad4);
    assert_eq!(crc16(&t[8 + 0x1530 + 8..8 + 0x1530 + 0xaa4]), 0xdce3);
    let mut s = GameState::new_game(tables.clone(), t).unwrap();
    let g = &s.global;
    assert_eq!((g.level, g.max_hp, g.wrench_held, g.bolts), (0, 4, 1, 0));
    assert_eq!((g.cam_yaw_normal, g.cam_pitch_normal, g.cam_speed, g.helpdesk_voice, g.helpdesk_text), (1, 1, 1, 1, 1));
    assert_eq!((g.stereo, g.music_volume, g.effects_volume, g.help_log_pos), (1, 0x2cc, 0x400, 1));
    assert_eq!(g.vendor, [0xff; 12]);
    let o = s.options();
    assert!(o.yaw_normal && o.pitch_normal && o.helpdesk_text && o.stereo && !o.subtitles && !o.mirror);
    assert_eq!(o.rotation_speed, 1);
    assert_eq!(o.yaw_rate().to_f32().to_degrees(), 1.3f32.to_radians().to_degrees());

    // Veldin: level start, Clank's first update, leave for Novalis, Novalis start.
    let mut session = SessionState::default();
    s.apply_level_start(0, &veldin_items, &mut session);
    assert_eq!(s.global.quick_select[0], item::BOMB_GLOVE as i32);
    assert_eq!(s.global.planet_unlocked, [0; 20], "Veldin (level 0) unlocks nothing");
    s.on_veldin_clank_init(&mut session);
    s.apply_transition(1);
    assert_eq!((s.global.level, s.levels[0].visited), (1, 2));
    s.apply_level_start(1, &novalis_items, &mut session);

    let g = &s.global;
    assert_eq!((session.hp, g.max_hp), (4, 4));
    assert_eq!(session.clank_hidden, 0, "the hero-block clear shows Clank again on Novalis");
    assert_eq!(g.wrench_held, 1);
    assert_eq!((g.owned[item::BOMB_GLOVE], g.acquired[item::BOMB_GLOVE], g.ammo[item::BOMB_GLOVE]), (1, 1, 10));
    assert_eq!(g.owned.iter().filter(|&&o| o != 0).count(), 1, "only the bomb glove is an owned item slot");
    assert_eq!(g.equipped[0], item::BOMB_GLOVE as i32);
    assert_eq!(g.quick_select, [10, 0, 0, 0, 0, 0, 0, 0]);
    let mut vendor = [0xff; 12];
    vendor[0] = 0x4a;
    vendor[1] = item::PYROCITOR as u8;
    assert_eq!(g.vendor, vendor, "bomb glove owned (0x40|10), pyrocitor for sale");
    let mut unlocked = [0u8; 20];
    unlocked[1] = 1;
    assert_eq!(g.planet_unlocked, unlocked);
    assert_eq!(&g.map_order[..2], &[1, 0]);
    assert_eq!(s.levels.iter().map(|l| l.visited).collect::<Vec<_>>()[..3], [2, 1, 0]);
    assert_eq!(g.flags[FLAG_VELDIN_CLANK], 1);
    assert_eq!(g.elapsed, 360, "two level loads");
    assert_eq!((s.levels[0].entry.count, s.levels[1].entry.count), (1, 1));
    assert_eq!((s.levels[0].entry.mask, s.levels[1].entry.mask), (0x8000_0001, 0x8000_0002));

    // Whole save → parse → state: byte-identical and field-identical.
    let file = s.to_save_file();
    let bytes = file.to_bytes();
    assert_eq!(bytes.len(), tables.file_size());
    let parsed = SaveFile::parse(&bytes).unwrap();
    assert!(parsed.all_crcs_ok());
    let (back, report) = GameState::from_save_file(tables.clone(), &parsed);
    assert_eq!(report.total(), 0);
    assert_eq!(back, s);
    assert_eq!(back.to_save_file().to_bytes(), bytes);
    let mut via_card = GameState::zeroed(tables.clone());
    assert_eq!(via_card.load_card_file(&bytes).unwrap().total(), 0);
    assert_eq!(via_card, s);

    // Incremental save onto a fresh slot (the template): global + level 1 only.
    let mut card = t.clone();
    let mut s2 = s.clone();
    s2.save_incremental(&mut card, None, [1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    let (gs, ls) = (tables.global_size(), tables.level_size());
    let expect = s2.to_save_file().to_bytes();
    assert_eq!(&card[..8], &t[..8]);
    assert_eq!(&card[8..8 + gs], &expect[8..8 + gs]);
    for l in 0..20 {
        let r = 8 + gs + l * ls..8 + gs + (l + 1) * ls;
        let want = if l == 1 { &expect[r.clone()] } else { &t[r.clone()] };
        assert_eq!(&card[r], want, "level section {l}");
    }
    let (from_card, rep) = GameState::from_save_file(tables.clone(), &SaveFile::parse(&card).unwrap());
    assert_eq!(rep.total(), 0);
    assert_eq!(from_card.levels[0].visited, 0, "Veldin's section was not written: the template's stays");
    assert_eq!(from_card.global, s2.global);

    // Round trip of the untouched template through GameState (level −1 kept).
    let (tmpl, rep) = GameState::from_save_file(tables, &SaveFile::parse(t).unwrap());
    assert_eq!((rep.total(), tmpl.global.level), (0, -1));
    assert_eq!(tmpl.to_save_file().to_bytes(), *t);
}
