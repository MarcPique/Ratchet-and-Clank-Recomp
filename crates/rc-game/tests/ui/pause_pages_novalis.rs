//! The pause pages beyond Gadgets on Novalis (docs/plan/menus.md §12), headless, from the disc's data: the page tree
//! of the overlay driven by pad presses, each page's own widgets (`rc_game::menus::pause::pages`) and their side
//! effects: Quick Select's slot ring (R1 / L1, ✕ assigns, move record 21, the leave writes 0x141ea0), the Weapons
//! page's ammo text and ammo model, the Items page's gold-bolt panel, the Help pages (the streamed image's buffer
//! states, the Help Log list newest first with its text swap and help-message label, Controls, Moves by the Heli-Pack,
//! the Weapons / Gadgets icon lists), Goodies (Skill Points, In-Level Movies), and the class-0x472 sound each action
//! plays. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::save_game::{ChunkTables, ItemTables, SaveGameLump};
use rc_game::game_state::{GameState, SessionState};
use rc_game::hud::{Draw, HudAssets};
use rc_game::inventory::{item, ItemInfos};
use rc_game::menus::pause::frame::{FrameClass, FrameMobys, CORNER_LISTS_ADDR};
use rc_game::menus::pause::pages::{self, func};
use rc_game::menus::pause::{Data, MenuEnv, PageMenu};
use rc_game::menus::{ImageSrc, MenuAssets, MenuDraw, MenuInput, MenuSound, Overlay};
use rc_game::pad::button;
use std::sync::Arc;

const WEAPONS: u32 = 0x1b3238;
const QUICK_SELECT: u32 = 0x1b3738;
const ITEMS: u32 = 0x1b6138;
const HELP: u32 = 0x1b3e18;
const HELP_LOG: u32 = 0x1b40e0;
const CONTROLS: u32 = 0x1b4980;
const MOVES: u32 = 0x1b4ac8;
const HELP_WEAPONS: u32 = 0x1b51a0;
const GOODIES: u32 = 0x1b6ca0;
const SKILL: u32 = 0x1b71c0;
const MOVIES: u32 = 0x1b8560;
/// `help_ss`' TOC field (the Help page's picture, widget +0x30 = 0x138008).
const HELP_SS: u32 = 0x138008 - pages::TOC_BASE;

struct Disc {
    elf: Vec<u8>,
    ov0: Vec<u8>,
    ov1: Vec<u8>,
    template: Vec<u8>,
}

fn disc() -> Option<Disc> {
    let root = rc_formats::test_data::root();
    Some(Disc {
        elf: std::fs::read(root.join("boot/SCUS_971.99")).ok()?,
        ov0: std::fs::read(root.join("levels/00/overlay.bin")).ok()?,
        ov1: std::fs::read(root.join("levels/01/overlay.bin")).ok()?,
        template: SaveGameLump::parse(&std::fs::read(root.join("global/save_game.bin")).ok()?).ok()?.template,
    })
}

/// The engine's direct boot into Novalis (as `gadgets_novalis`).
fn first_arrival(d: &Disc) -> (GameState, ItemTables) {
    let t1 = ItemTables::load(&d.elf, &d.ov1).unwrap();
    let mut gs = GameState::new_game(ChunkTables::from_boot_elf(&d.elf).unwrap(), &d.template).unwrap();
    let mut s = SessionState::default();
    gs.apply_level_start(0, &ItemTables::load(&d.elf, &d.ov0).unwrap(), &mut s);
    gs.on_veldin_clank_init(&mut s);
    gs.apply_transition(1);
    gs.apply_level_start(1, &t1, &mut s);
    (gs, t1)
}

struct Menu {
    menu: PageMenu,
    assets: MenuAssets,
}

/// The menu as the engine sets it up (`rc-engine/src/menu_render.rs`): the overlay's tree, the item definitions, the
/// price records' ammo fields, the frame mobys, the level text and the global `all_text` (English).
fn menu(d: &Disc, t: &ItemTables) -> Option<Menu> {
    let ov = Overlay::parse(&d.ov1).unwrap();
    let mut menu = PageMenu::load(&ov)?;
    let blob = rc_formats::test_data::core_block(1, "moby_class/1138")?;
    let class = rc_formats::moby::parse_moby_class(&blob).ok()?;
    let anim = MobyAnimClass::new(&class, parse_sequences(&blob, &class).ok()?);
    let chains = std::array::from_fn(|k| rc_formats::gadget::joint_list(&blob, &class.header, ov.i32(CORNER_LISTS_ADDR + 4 * k as u32).unwrap() as usize).unwrap().0);
    menu.frames = Some(FrameMobys::new(FrameClass { anim, chains, scale: class.header.scale }, false));
    let n = rc_formats::save_game::ITEM_COUNT * rc_formats::save_game::ITEM_DEF_SIZE;
    menu.items = Some(Arc::new(ItemInfos::from_raw(ov.bytes(t.item_defs_addr, n)?)));
    let u = |r: &rc_formats::save_game::ItemRecord, o: usize| u16::from_le_bytes([r.0[o], r.0[o + 1]]);
    menu.ammo_records = t.records.iter().map(|r| (u(r, 8), u(r, 0xe))).collect();
    let gp = rc_formats::test_data::gameplay(1)?;
    let messages = rc_formats::strings::parse_strings(&gp, 0).ok()?;
    let (glyphs, _) = rc_formats::font::parse_glyph_tables(&d.ov1).ok()?;
    let mut assets = MenuAssets::new(HudAssets { icons: Vec::new(), frame_sizes: Vec::new(), glyphs, messages }, ov);
    let raw = std::fs::read(rc_formats::test_data::root().join("global/all_text.bin")).ok()?;
    let bytes = if rc_formats::wad::is_wad(&raw) { rc_formats::wad::decompress(&raw).ok()? } else { raw };
    let off = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    assets.all_text = rc_formats::strings::parse_text_block(&bytes[off..]).ok()?;
    Some(Menu { menu, assets })
}

#[derive(Default)]
struct Drv {
    f: u32,
    sounds: Vec<MenuSound>,
}

/// `(button, frames)`: the button pressed on the first of `frames` menu frames. The last frame's draws.
fn run(d: &mut Drv, m: &mut Menu, gs: &mut GameState, keys: &[(u32, u32)]) -> Vec<MenuDraw> {
    let mut last = Vec::new();
    for &(k, n) in keys {
        for i in 0..n {
            let pressed = if i == 0 { k } else { 0 };
            let inp = MenuInput { pressed, pressed_u: pressed, raw_pressed: pressed, connected: true, ..Default::default() };
            let env = MenuEnv { vsync: d.f, b13f4: 0, pal: false };
            d.f += 1;
            let out = m.menu.tick(&inp, gs, &env);
            d.sounds.extend(out.sounds);
            last.clear();
            if out.exit.is_none() { m.menu.draw(&m.assets, gs, &env, &mut rc_game::rng::Rng::new(), &mut last); }
        }
    }
    last
}

/// Opens the pause menu and goes to the root entry `row` (0 Weapons … 6 Goodies) with ✕.
fn open_root(d: &mut Drv, m: &mut Menu, gs: &mut GameState, row: u32) {
    m.menu.enter(0, gs);
    run(d, m, gs, &[(0, 20)]);
    for _ in 0..row { run(d, m, gs, &[(button::DOWN, 3)]); }
    run(d, m, gs, &[(button::CROSS, 20)]);
}

fn texts(draws: &[MenuDraw]) -> Vec<String> {
    draws
        .iter()
        .filter_map(|d| match d {
            MenuDraw::Hud(Draw::Text { text, .. }) | MenuDraw::Hud(Draw::TextWindow { text, .. }) => Some(String::from_utf8_lossy(text).to_string()),
            _ => None,
        })
        .collect()
}

fn images(draws: &[MenuDraw]) -> Vec<(u32, u32)> {
    draws
        .iter()
        .filter_map(|d| match d {
            MenuDraw::Image { src: ImageSrc::Lump { field, index }, .. } => Some((*field, *index)),
            _ => None,
        })
        .collect()
}

/// The first widget of the current page matching `f`.
fn widget_of(m: &PageMenu, f: impl Fn(&Data) -> bool) -> Option<u32> {
    m.pages[&m.current].widgets.iter().copied().find(|w| *w != 0 && m.widgets.get(w).is_some_and(|x| f(&x.data)))
}

fn setup() -> Option<(GameState, Menu)> {
    let d = disc()?;
    let (gs, t) = first_arrival(&d);
    Some((gs, menu(&d, &t)?))
}

/// Quick Select (0x2901a8 / 0x2903d0 / 0x290468): the enter copies 0x141ea0 and puts the cursor on the first empty
/// slot; R1 / L1 step it (sound 1 each); ✕ on the focused grid's owned item moves it into the cursor's slot (out of
/// its old one), steps the cursor and bumps move record 21; the leave writes the ring back to 0x141ea0.
#[test]
fn quick_select_assigns_slots_and_writes_them_back() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(gs.global.quick_select[0], item::BOMB_GLOVE);
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 2);
    assert_eq!(m.menu.current, QUICK_SELECT);
    let w = widget_of(&m.menu, |x| matches!(x, Data::Slots { .. })).expect("the slot ring");
    let slots = |m: &Menu| match m.menu.widgets[&w].data { Data::Slots { slots, cursor } => (slots, cursor), _ => unreachable!() };
    assert_eq!(slots(&m), ([item::BOMB_GLOVE, 0, 0, 0, 0, 0, 0, 0], 1), "the first empty slot");
    d.sounds.clear();
    run(&mut d, &mut m, &mut gs, &[(button::R1, 2)]);
    assert_eq!((slots(&m).1, d.sounds.clone()), (2, vec![MenuSound::Cursor]));
    d.sounds.clear();
    run(&mut d, &mut m, &mut gs, &[(button::L1, 2), (button::L1, 2), (button::L1, 2)]);
    assert_eq!((slots(&m).1, d.sounds.len()), (7, 3), "L1 wraps 0 → 7");
    // ✕ on the Bomb Glove (the focused grid's first cell): out of slot 0, into slot 7; the cursor steps to 0.
    assert_eq!(rc_game::menus::pause::gadgets::focused_item(&m.menu), Some(item::BOMB_GLOVE));
    let before = gs.global.move_help[21];
    run(&mut d, &mut m, &mut gs, &[(button::CROSS, 2)]);
    assert_eq!(slots(&m), ([0, 0, 0, 0, 0, 0, 0, item::BOMB_GLOVE], 0));
    assert_eq!(gs.global.move_help[21].count, before.count + 1, "move record 21 bumped");
    assert_ne!(gs.global.move_help[21].mask & 2, 0, "Novalis' bit");
    assert_eq!(gs.global.quick_select[0], item::BOMB_GLOVE, "0x141ea0 unchanged until the leave");
    // Its draw: the pulsing frame around the cursor's slot (slot 0, straight up), the ring's empty squares.
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    let empties = draws.iter().filter(|x| matches!(x, MenuDraw::Rect { rgba: 0x4040_4040, .. })).count();
    assert_eq!(empties, 7);
    // The close (Start): the leave writes 0x141ea0.
    run(&mut d, &mut m, &mut gs, &[(button::START, 20)]);
    assert!(!m.menu.active);
    assert_eq!(gs.global.quick_select, [0, 0, 0, 0, 0, 0, 0, item::BOMB_GLOVE]);
}

/// The Weapons page: the ammo text 0x292b60 ("ammo/max" from the price record, "(no ammo)" for a weapon without) and
/// the ammo model 0x2919a0 (the Bomb Glove's pickup class, turned 0.01 rad a tick from π).
#[test]
fn weapons_page_ammo_text_and_model() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    gs.global.ammo[item::BOMB_GLOVE as usize] = 7;
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 0);
    assert_eq!(m.menu.current, WEAPONS);
    let max = m.menu.ammo_records[item::BOMB_GLOVE as usize].1;
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    let t = texts(&draws);
    assert!(t.contains(&format!("7/{max}")), "{t:?}");
    let w = widget_of(&m.menu, |x| matches!(x, Data::AmmoModel(_))).expect("the ammo model");
    let Data::AmmoModel(am) = m.menu.widgets[&w].data else { unreachable!() };
    let class = m.menu.items.as_ref().unwrap().get(item::BOMB_GLOVE).unwrap().ammo_class as i32;
    assert_eq!(am.class, class);
    assert!(class > 0);
    let a0 = am.angle;
    run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    let Data::AmmoModel(am) = m.menu.widgets[&w].data else { unreachable!() };
    assert!((rc_game::moby_update::creature::add_rot(a0, 0.01) - am.angle).abs() < 1e-6);
    let view = m.menu.view.ammo.expect("the ammo moby placed");
    assert_eq!((view.o_class, view.offset), (class, pages::AMMO_OFFSET));
}

/// The Items page: the gold-bolt panel (Found / Used / Remain from the level gold bolts and the gold weapons, the gold
/// bolt moby spun 0.02 rad a tick while the page is up and freed by its leave) and the item picture of the focused cell.
#[test]
fn items_page_gold_bolts_and_picture() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    gs.levels[1].gold_bolts = [1, 1, 0, 0];
    gs.levels[4].gold_bolts = [1, 1, 1, 1];
    gs.global.gold_weapons[item::BOMB_GLOVE as usize] = 1;
    assert_eq!(pages::gold_counts(&gs), (6, 4, 2));
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 3);
    assert_eq!(m.menu.current, ITEMS);
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 4)]);
    let t = texts(&draws);
    for s in ["6", "4", "2"] { assert!(t.iter().any(|x| x == s), "{s} in {t:?}"); }
    let title = String::from_utf8_lossy(m.assets.msg(0x4f4e)).to_string();
    assert!(t.contains(&title), "{title:?} in {t:?}");
    let s0 = m.menu.gold_spin.expect("the gold bolt spawned");
    run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    assert!((rc_game::moby_update::creature::add_rot(s0, 0.02) - m.menu.gold_spin.unwrap()).abs() < 1e-6);
    assert_eq!(m.menu.view.gold_bolt.map(|v| v.o_class), Some(pages::GOLD_BOLT_CLASS));
    // △: the page's leave frees the moby.
    run(&mut d, &mut m, &mut gs, &[(button::TRIANGLE, 20)]);
    assert_eq!(m.menu.gold_spin, None);
}

/// The Help page's streamed picture (0x2937d0 / 0x293d50) through its buffer states: 0 → 1 (reading A) → 2 (A shown);
/// a new index → B (3 → 4); back to A's index → 2 at once; a third index → B again. The list's cursor moves play sound 1.
#[test]
fn help_page_streamed_image_states() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 4);
    assert_eq!(m.menu.current, HELP);
    let w = widget_of(&m.menu, |x| matches!(x, Data::Image(_))).expect("the picture");
    let st = |m: &Menu| match m.menu.widgets[&w].data { Data::Image(i) => (i.state, i.a, i.b), _ => unreachable!() };
    assert_eq!(st(&m), (2, 0, -1));
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    assert_eq!(images(&draws), vec![(HELP_SS, 0)]);
    // The picture (W2) updates before the list (W3): it sees the new cursor on the next frame.
    d.sounds.clear();
    run(&mut d, &mut m, &mut gs, &[(button::DOWN, 1)]);
    assert_eq!((d.sounds.clone(), st(&m)), (vec![MenuSound::Cursor], (2, 0, -1)));
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    assert_eq!((st(&m), images(&draws)), ((3, 0, 1), vec![(HELP_SS, 0)]), "B reading: A still shown");
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    assert_eq!((st(&m), images(&draws)), ((4, 0, 1), vec![(HELP_SS, 1)]));
    let draws = run(&mut d, &mut m, &mut gs, &[(button::UP, 1), (0, 1)]);
    assert_eq!((st(&m), images(&draws)), ((2, 0, 1), vec![(HELP_SS, 0)]), "A holds index 0: shown at once");
    // Down twice: index 1 is B's (→ 4), then index 2 is new while B is shown → A reads it (5 → 6 → 2).
    run(&mut d, &mut m, &mut gs, &[(button::DOWN, 1), (button::DOWN, 1)]);
    assert_eq!(st(&m), (4, 0, 1));
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    assert_eq!((st(&m), images(&draws)), ((5, 2, 1), vec![(HELP_SS, 1)]));
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    assert_eq!((st(&m), images(&draws)), ((6, 2, 1), vec![(HELP_SS, 1)]), "states ≥ 4 show B");
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 1)]);
    assert_eq!((st(&m), images(&draws)), ((2, 2, 1), vec![(HELP_SS, 2)]));
    // The leave: state −1.
    run(&mut d, &mut m, &mut gs, &[(button::TRIANGLE, 20)]);
    assert_eq!(st(&m), (-1, -1, -1));
}

/// Help / Help Log: the list newest first with the log table's titles (0x290b70), `all_text` swapped in while the page
/// is up (0x290c00 / 0x290d40 / 0x290cd0) and the label (source 0x1000) showing the focused entry's help message.
#[test]
fn help_log_page_lists_the_log_and_shows_the_message() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    let ov = Overlay::parse(&disc().unwrap().ov1).unwrap();
    m.menu.log_ids = rc_game::help::read_log_ids(&ov);
    let ids = m.menu.log_ids.clone();
    // Logged: the welcome (index 0), the Infobot hint 1000 (4), the look-around hint 1004 (0x40).
    gs.global.help_log[..3].copy_from_slice(&[0, 4, 0x40]);
    gs.global.help_log_pos = 3;
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 4);
    run(&mut d, &mut m, &mut gs, &[(button::CROSS, 20)]);
    assert_eq!(m.menu.current, HELP_LOG);
    let lw = widget_of(&m.menu, |x| matches!(x, Data::List(_))).expect("the log list");
    let Data::List(l) = &m.menu.widgets[&lw].data else { unreachable!() };
    let labels: Vec<i16> = l.items.iter().map(|i| i.label).collect();
    assert_eq!(labels, vec![ids[0x40].1, ids[4].1, ids[0].1], "newest first");
    assert_eq!(m.menu.text_swap, 2, "all_text in");
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 30)]);
    let msg = String::from_utf8_lossy(m.assets.msg_in(ids[0x40].0 as i32, true)).to_string();
    assert!(!msg.is_empty());
    let t = texts(&draws);
    assert!(t.contains(&msg), "the 1004 message {msg:?} in {t:?}");
    // △ back to Help: the level's table again.
    run(&mut d, &mut m, &mut gs, &[(button::TRIANGLE, 20)]);
    assert_eq!((m.menu.current, m.menu.text_swap), (HELP, 0));
}

/// Help / Controls (0x294050 / 0x294198): `help_controls[lang]` then `[6 + lang]`, side by side.
#[test]
fn help_controls_page_two_pictures() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 4);
    run(&mut d, &mut m, &mut gs, &[(button::DOWN, 3), (button::CROSS, 20)]);
    assert_eq!(m.menu.current, CONTROLS);
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 2)]);
    assert_eq!(images(&draws), vec![(pages::HELP_CONTROLS, 0), (pages::HELP_CONTROLS, 6)]);
}

/// Help / Moves (0x2904a0): the description table by the Heli-Pack.
#[test]
fn help_moves_table_follows_the_heli_pack() {
    let Some((mut gs, m0)) = setup() else { eprintln!("skipped: no extracted/"); return };
    let tables = m0.menu.addrs.moves_tables;
    let label_id = |m: &Menu| {
        let w = m.menu.pages[&MOVES].widgets.iter().copied().find(|w| m.menu.widgets.get(w).is_some_and(|x| x.enter == func::MOVES_LABEL_ENTER)).unwrap();
        match &m.menu.widgets[&w].data { Data::Label(l) => l.id, _ => unreachable!() }
    };
    for (heli, want) in [(0u8, tables[1]), (1, tables[0])] {
        gs.global.owned[item::HELI_PACK as usize] = heli;
        let (_, mut m) = setup().unwrap();
        let mut d = Drv::default();
        open_root(&mut d, &mut m, &mut gs, 4);
        run(&mut d, &mut m, &mut gs, &[(button::DOWN, 3), (button::DOWN, 3), (button::CROSS, 20)]);
        assert_eq!(m.menu.current, MOVES);
        assert_eq!(label_id(&m), want, "Heli-Pack owned {heli}");
    }
}

/// Help / Weapons (0x295000, icon list 0x28d818): the owned weapons of the Weapons grid, their help texts; Down steps
/// (sound 1) and stops at the end (no sound).
#[test]
fn help_weapons_icon_list() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    let ws = m.menu.help_items.weapons.clone();
    let own: Vec<i16> = ws.iter().copied().filter(|&i| i > 0).take(3).collect();
    for &i in &own { gs.global.owned[i as usize] = 1; }
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 4);
    for _ in 0..3 { run(&mut d, &mut m, &mut gs, &[(button::DOWN, 3)]); }
    run(&mut d, &mut m, &mut gs, &[(button::CROSS, 20)]);
    assert_eq!(m.menu.current, HELP_WEAPONS);
    let w = widget_of(&m.menu, |x| matches!(x, Data::IconList(_))).expect("the icon list");
    let cells = |m: &Menu| match &m.menu.widgets[&w].data { Data::IconList(l) => (l.cells.iter().map(|c| c.item).collect::<Vec<_>>(), l.cursor), _ => unreachable!() };
    let mut owned_in_grid: Vec<i16> = ws.iter().copied().filter(|&i| i > 0 && gs.global.owned[i as usize] != 0).collect();
    owned_in_grid.dedup();
    assert_eq!(cells(&m).0, owned_in_grid);
    let texts_t = &m.menu.label_tables[&m.menu.addrs.help_texts[0]];
    let defs = m.menu.items.clone().unwrap();
    assert_eq!(texts_t, &owned_in_grid.iter().map(|&i| defs.get(i as i32).unwrap().help[0] as u16 as u32).collect::<Vec<_>>());
    // Focus the icon list, then Down to the end.
    m.menu.pages.get_mut(&HELP_WEAPONS).unwrap().focus = w;
    d.sounds.clear();
    let n = owned_in_grid.len() as u32;
    for _ in 0..n + 1 { run(&mut d, &mut m, &mut gs, &[(button::DOWN, 1)]); }
    assert_eq!(cells(&m).1, n as i32 - 1);
    assert_eq!(d.sounds, vec![MenuSound::Cursor; n as usize - 1], "no sound at the end");
}

/// Goodies: Skill Points' level text (0x295af8) and In-Level Movies' list (0x295730, `0x1b8aa8[level]`); the locked
/// entries (action −1) do nothing.
#[test]
fn goodies_skill_points_and_movies() {
    let Some((mut gs, mut m)) = setup() else { eprintln!("skipped: no extracted/"); return };
    // Goodies is on the root once the game is beaten (0x2917d8).
    gs.global.game_beaten = 1;
    let mut d = Drv::default();
    open_root(&mut d, &mut m, &mut gs, 6);
    assert_eq!(m.menu.current, GOODIES);
    run(&mut d, &mut m, &mut gs, &[(button::CROSS, 20)]);
    assert_eq!(m.menu.current, SKILL);
    let draws = run(&mut d, &mut m, &mut gs, &[(0, 2)]);
    let lw = widget_of(&m.menu, |x| matches!(x, Data::List(_))).expect("the skill list");
    let Data::List(l) = &m.menu.widgets[&lw].data else { unreachable!() };
    let lvl = l.items[l.cursor as usize].arg as i32;
    let t = texts(&draws);
    let want = if lvl == -1 { vec![m.assets.msg(0x5019).to_vec()] } else {
        let (loc, planet) = m.menu.level_names[lvl as usize];
        vec![m.assets.msg(loc as i32).to_vec(), m.assets.msg(planet as i32).to_vec()]
    };
    for s in want { let s = String::from_utf8_lossy(&s).to_string(); assert!(t.contains(&s), "{s:?} in {t:?}"); }
    run(&mut d, &mut m, &mut gs, &[(button::TRIANGLE, 20)]);
    assert_eq!(m.menu.current, GOODIES);
    for _ in 0..4 { run(&mut d, &mut m, &mut gs, &[(button::DOWN, 3)]); }
    run(&mut d, &mut m, &mut gs, &[(button::CROSS, 20)]);
    assert_eq!(m.menu.current, MOVIES);
    let lw = widget_of(&m.menu, |x| matches!(x, Data::List(_))).expect("the movies list");
    let Data::List(l) = &m.menu.widgets[&lw].data else { unreachable!() };
    let key = |v: &[rc_game::menus::pause::Item]| v.iter().map(|i| (i.label, i.action, i.arg)).collect::<Vec<_>>();
    assert_eq!(key(&l.items), key(&m.menu.movie_lists[1]));
    assert_eq!(l.items.len(), 6);
    // A locked entry (Sketchbook: the Goodies enter `0x29aa60` makes it action 2 below 15 skill points): ✕ stays and
    // plays the Denied sound.
    run(&mut d, &mut m, &mut gs, &[(button::TRIANGLE, 20)]);
    for _ in 0..1 { run(&mut d, &mut m, &mut gs, &[(button::DOWN, 3)]); }
    d.sounds.clear();
    run(&mut d, &mut m, &mut gs, &[(button::CROSS, 20)]);
    assert_eq!((m.menu.current, d.sounds.clone()), (GOODIES, vec![MenuSound::Denied]));
}
