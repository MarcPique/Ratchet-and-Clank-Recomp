//! The inventory and the Gadgets page on Novalis (docs/plan/gadgets.md), headless, from the disc's data: the game
//! state of a first arrival without debug grants, the Gadgets page driven by pad presses (the overlay's page,
//! widget, grid and item records, the frame mobys of class 1138), the close's requests carried out by the hero's
//! back slot, and the pack moves they make available. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::save_game::{ChunkTables, ItemTables, SaveGameLump};
use rc_formats::{collision, gameplay, level};
use rc_game::game_state::{GameState, SessionState};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hud::HudAssets;
use rc_game::inventory::{self, item, ItemInfos, Slot};
use rc_game::menus::pause::frame::{FrameClass, FrameMobys, CORNER_LISTS_ADDR};
use rc_game::menus::pause::{Data, MenuEnv, PageMenu};
use rc_game::menus::{MenuAssets, MenuDraw, MenuInput, MenuSound, Overlay};
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

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

/// The engine's direct boot into Novalis: new game → Veldin's level start and Clank → transition → Novalis' start.
fn first_arrival(d: &Disc) -> (GameState, SessionState, ItemTables) {
    let t1 = ItemTables::load(&d.elf, &d.ov1).unwrap();
    let mut gs = GameState::new_game(ChunkTables::from_boot_elf(&d.elf).unwrap(), &d.template).unwrap();
    let mut s = SessionState::default();
    gs.apply_level_start(0, &ItemTables::load(&d.elf, &d.ov0).unwrap(), &mut s);
    gs.on_veldin_clank_init(&mut s);
    gs.apply_transition(1);
    gs.apply_level_start(1, &t1, &mut s);
    (gs, s, t1)
}

struct Novalis {
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    spawn: [f32; 3],
    yaw: f32,
    death_z: f32,
}

fn novalis() -> Option<Novalis> {
    let dir = rc_formats::test_data::root().join("levels/01");
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let settings = rc_formats::test_data::gameplay_section(1, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let r = gameplay::parse_moby_instances(&gp).unwrap().into_iter().find(|m| m.o_class == 0)?;
    let blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256).map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok())).collect();
    Some(Novalis { mesh, ratchet: MobyAnimClass::new(&class, seqs), spawn: r.position, yaw: r.rotation[2], death_z: f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap()) })
}

fn level_class(o: u32) -> Option<MobyAnimClass> {
    let blob = rc_formats::test_data::core_block(1, &format!("moby_class/{o:04}"))?;
    let c = rc_formats::moby::parse_moby_class(&blob).ok()?;
    Some(MobyAnimClass::new(&c, parse_sequences(&blob, &c).ok()?))
}

/// `a-b:press B+B` / `a-b:stick x y` (the engine's `RC_PLAY_SCRIPT` syntax).
fn script_input(script: &str, t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    for it in script.split(',').filter(|s| !s.is_empty()) {
        let (range, action) = it.split_once(':').unwrap();
        let (a, b) = range.split_once('-').unwrap_or((range, range));
        let (a, b): (u32, u32) = (a.trim().parse().unwrap(), b.parse().unwrap());
        if t < a || t > b { continue; }
        let w: Vec<&str> = action.split_whitespace().collect();
        p = match w[..] {
            ["stick", x, y] => p.stick(x.parse().unwrap(), y.parse().unwrap()),
            ["press", n] => p.press(n.split('+').map(|n| match n { "X" => button::CROSS, "R1" => button::R1, _ => panic!("{n}") }).fold(0, |m, b| m | b)),
            _ => panic!("{action}"),
        };
    }
    p
}

/// The hero at the spawn with the game state's items (the engine's per-tick sync of the owned table and the four
/// slots' saved items / requests), the back items modelled; runs `script`, returns the states and back modules.
fn play(n: &Novalis, gs: &mut GameState, s: &mut SessionState, script: &str, ticks: u32) -> Vec<(i32, i32)> {
    let mut m = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
    m.position = [n.spawn[0], n.spawn[1], n.spawn[2], 1.0];
    m.rotation = [0.0, 0.0, n.yaw, 0.0];
    let mut g = Game::new(&n.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), n.death_z);
    g.finish_load();
    let packs = [(2, 607), (3, 608), (4, 609)].into_iter().filter_map(|(i, o)| Some((i, o as i16, level_class(o)?))).collect();
    g.hero.set_back_packs(packs, level_class(601).unwrap());
    let mut anim = RatchetAnim::new(&n.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let mut out = Vec::new();
    for t in 0..ticks {
        let h = &mut g.hero;
        h.owned.0 = gs.global.owned;
        (h.feet_slot.saved, h.head_slot.saved, h.back_slot.slot.saved) = (gs.global.equipped[1], gs.global.equipped[2], gs.global.equipped[3]);
        (h.feet_slot.request, h.head_slot.request, h.back_slot.slot.request) = (s.temp_feet, s.temp_head, s.temp_back);
        g.hero.idle.counter = g.counter as i32;
        g.tick(Some(&script_input(script, t).bytes()), &n.mesh, &mut anim.ctl(&n.ratchet), &mut hooks);
        let h = &g.hero;
        (gs.global.equipped[1], gs.global.equipped[2], gs.global.equipped[3]) = (h.feet_slot.saved, h.head_slot.saved, h.back_slot.slot.saved);
        (s.temp_feet, s.temp_head, s.temp_back) = (h.feet_slot.request, h.head_slot.request, h.back_slot.slot.request);
        out.push((h.state, h.back_module()));
    }
    out
}

/// The Novalis GLIDE / STOMP scripts of `hero_packs_novalis.rs`.
const GLIDE: &str = "0-214:stick 0 -1,150-151:press X,152-260:press X";
const STOMP: &str = "20-40:press X,50-50:press R1";

/// A first arrival owns only the Bomb Glove (given by Novalis' level start with its quick-select slot), has it saved
/// in the hand, nothing on feet or head, and no saved back item: `HeroItemsCreate` then makes back item 2 (the
/// Heli-Pack's model 607 folded, the game's own look of Clank on Ratchet's back), but no pack move works without
/// the Heli-Pack owned (0x13d4c2) or the Thruster as the back item.
#[test]
fn first_arrival_owns_only_the_bomb_glove() {
    let (Some(d), Some(n)) = (disc(), novalis()) else { eprintln!("skipped: no extracted/"); return; };
    let (mut gs, mut s, _) = first_arrival(&d);
    let owned: Vec<usize> = (0..gs.global.owned.len()).filter(|&i| gs.global.owned[i] != 0).collect();
    assert_eq!(owned, vec![item::BOMB_GLOVE as usize]);
    assert_eq!(&gs.global.equipped[..4], &[item::BOMB_GLOVE, 0, 0, 0]);
    assert_eq!((gs.global.wrench_held, gs.global.thruster_last), (1, 0));
    assert_eq!((s.temp_hand, s.temp_feet, s.temp_head, s.temp_back, s.clank_hidden), (0, 0, 0, 0, 0));
    assert_eq!(gs.global.quick_select[0], item::BOMB_GLOVE);
    let rows = play(&n, &mut gs, &mut s, GLIDE, 300);
    assert!(rows.iter().all(|r| r.1 == item::HELI_PACK), "back item 2 all along");
    assert!(!rows.iter().any(|r| r.0 == 8), "no glide");
    let rows = play(&n, &mut gs, &mut s, STOMP, 150);
    assert!(!rows.iter().any(|r| r.0 == 0x22), "no stomp");
    assert_eq!(&gs.global.equipped[..4], &[item::BOMB_GLOVE, 0, 0, 0], "nothing changed by playing");
}

struct Menu {
    menu: PageMenu,
    assets: MenuAssets,
}

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
    let gp = rc_formats::test_data::gameplay(1)?;
    let messages = rc_formats::strings::parse_strings(&gp, 0).ok()?;
    let (glyphs, _) = rc_formats::font::parse_glyph_tables(&d.ov1).ok()?;
    Some(Menu { menu, assets: MenuAssets::new(HudAssets { icons: Vec::new(), frame_sizes: Vec::new(), glyphs, messages }, ov) })
}

/// The frame counter, the sounds and the last close's requests of a menu run.
#[derive(Default)]
struct Drv {
    f: u32,
    sounds: Vec<MenuSound>,
    equip: Option<[Option<i32>; 4]>,
}

/// `(button, frames)`: the button pressed on the first of `frames` menu frames. The last frame's draws.
fn run(d: &mut Drv, m: &mut Menu, gs: &mut GameState, keys: &[(u32, u32)]) -> Vec<MenuDraw> {
    let mut last = Vec::new();
    for &(k, n) in keys {
        for i in 0..n { last = frame(m, gs, if i == 0 { k } else { 0 }, &mut d.f, &mut d.sounds, &mut d.equip); }
    }
    last
}

/// One menu frame with `pressed` (edge) buttons; the draws.
fn frame(m: &mut Menu, gs: &mut GameState, pressed: u32, f: &mut u32, sounds: &mut Vec<MenuSound>, equip: &mut Option<[Option<i32>; 4]>) -> Vec<MenuDraw> {
    let inp = MenuInput { pressed, pressed_u: pressed, raw_pressed: pressed, connected: true, ..Default::default() };
    let env = MenuEnv { vsync: *f, b13f4: 0, pal: false };
    *f += 1;
    let out = m.menu.tick(&inp, gs, &env);
    sounds.extend(out.sounds);
    if out.equip.is_some() { *equip = out.equip; }
    let mut draws = Vec::new();
    if out.exit.is_none() { m.menu.draw(&m.assets, gs, &env, &mut rc_game::rng::Rng::new(), &mut draws); }
    draws
}

/// The Gadgets page from the pause menu with both packs owned: Start, Down to "Gadgets", ✕; the focus is on the
/// back packs (Heli-Pack under the cursor, equipped icon variant); Right, ✕ equips the Thruster-Pack; Start closes:
/// the back request 3 (0x141414), which the hero's back slot carries out (put away, created), after which the
/// Thruster moves work (the stomp; the glide becomes the Thruster's). Then the Heli-Pack the same way (no stomp). Selecting an unowned or already-equipped item changes nothing.
#[test]
fn gadgets_page_equips_the_packs() {
    let (Some(d), Some(n)) = (disc(), novalis()) else { eprintln!("skipped: no extracted/"); return; };
    let (mut gs, mut s, t) = first_arrival(&d);
    let Some(mut m) = menu(&d, &t) else { eprintln!("skipped: no menu data"); return; };
    gs.global.owned[item::HELI_PACK as usize] = 1;
    gs.global.owned[item::THRUSTER_PACK as usize] = 1;
    gs.global.equipped[3] = item::HELI_PACK;
    let mut dr = Drv::default();
    m.menu.enter(0, &gs);
    run(&mut dr, &mut m, &mut gs, &[(0, 20), (button::DOWN, 5), (button::CROSS, 20)]);
    let page = m.menu.current;
    assert_eq!(page, rc_game::menus::pause::gadgets::PAGE, "the Gadgets page");
    let focus = m.menu.pages[&page].focus;
    let Data::Grid(g) = &m.menu.widgets[&focus].data else { panic!("focus on a grid") };
    assert_eq!((g.rows, g.cols, g.item()), (1, 3, item::HELI_PACK), "the back packs grid");
    assert_eq!(m.menu.equip, [item::BOMB_GLOVE, 0, 0, item::HELI_PACK]);
    // Its draws: the grids' owned icons (Bomb Glove is not on the Gadgets page; the two packs are), the model and
    // preview views, the name label.
    let draws = run(&mut dr, &mut m, &mut gs, &[(0, 2)]);
    let sprites = draws.iter().filter(|d| matches!(d, MenuDraw::SpriteUv { .. })).count();
    assert_eq!(sprites, 2, "two owned items on the page");
    // `fun_00223e28` last, over every on-screen panel (its navy rect: x0 = x + 1, x1 = x + w − 1): the vignette, the
    // scan lines and the glass (and a noise burst only now and then).
    use rc_game::menus::screen_static::{StaticTex, BAR_FX};
    let navy: Vec<[i32; 4]> = draws
        .iter()
        .take_while(|d| !matches!(d, MenuDraw::PanelBegin { .. }))
        .filter_map(|d| match d { MenuDraw::Rect { x0, y0, x1, y1, rgba } if *rgba == rc_game::menus::pause::NAVY => Some([x0 - 1, y0 - 1, x1 - x0 + 2, y1 - y0 + 2]), _ => None })
        .filter(|[x, y, w, h]| *x < 0x200 && x + w >= 0 && *y < 0x1a1 && y + h >= 0)
        .collect();
    let lines: Vec<[i32; 4]> = draws.iter().filter_map(|d| match d { MenuDraw::Static(s) if s.tex == StaticTex::Fx(BAR_FX) => Some([s.x, s.y, s.w, s.h]), _ => None }).collect();
    assert!(!navy.is_empty());
    assert_eq!(lines, navy, "scan lines on every on-screen panel, in slot order");
    let first = draws.iter().position(|d| matches!(d, MenuDraw::Static(_))).unwrap();
    assert!(draws[first..].iter().all(|d| matches!(d, MenuDraw::Static(_))), "the effect is drawn after everything else");
    let v = m.menu.view;
    assert_eq!(v.model.map(|mv| mv.equip), Some([item::BOMB_GLOVE, 0, 0, item::HELI_PACK]));
    let pv = v.preview.expect("the item preview");
    assert_eq!((pv.item, pv.o_class, pv.clank, pv.seq), (item::HELI_PACK, 607, true, 6));
    let name = String::from_utf8_lossy(m.assets.msg(20078)).to_string();
    let texts: Vec<String> = draws.iter().filter_map(|d| match d { MenuDraw::Hud(rc_game::hud::Draw::TextWindow { text, .. }) => Some(String::from_utf8_lossy(text).to_string()), _ => None }).collect();
    assert!(texts.contains(&name), "the name label shows {name:?}: {texts:?}");
    // Right → the Thruster-Pack; ✕ equips it; Right → the Hydro-Pack (not owned): ✕ is denied.
    dr.sounds.clear();
    run(&mut dr, &mut m, &mut gs, &[(button::RIGHT, 2), (button::CROSS, 2)]);
    assert_eq!(m.menu.equip[3], item::THRUSTER_PACK);
    assert_eq!(dr.sounds, vec![MenuSound::Cursor, MenuSound::Confirm]);
    dr.sounds.clear();
    run(&mut dr, &mut m, &mut gs, &[(button::RIGHT, 2), (button::CROSS, 2)]);
    assert_eq!((m.menu.equip[3], dr.sounds.clone()), (item::THRUSTER_PACK, vec![MenuSound::Cursor, MenuSound::Denied]));
    // Close: the back request.
    run(&mut dr, &mut m, &mut gs, &[(button::START, 20)]);
    let req = dr.equip.take().expect("the close's requests");
    assert_eq!(req, [None, None, None, Some(item::THRUSTER_PACK)]);
    inventory::apply_close_requests(&mut s, &req);
    assert_eq!(inventory::request(&s, Slot::Back), item::THRUSTER_PACK);
    // The swap in play: the Heli-Pack put away (sequence 2), then the Thruster-Pack created.
    let rows = play(&n, &mut gs, &mut s, "", 120);
    let swap: Vec<i32> = rows.iter().map(|r| r.1).fold(Vec::new(), |mut v, x| { if v.last() != Some(&x) { v.push(x); } v });
    // (The Heli-Pack is created and its put-away starts in the same `HeroItemsUpdate` of the first tick.)
    assert_eq!(swap, vec![-1, item::THRUSTER_PACK], "put away → ready 3");
    let away = rows.iter().take_while(|r| r.1 == -1).count();
    assert!((2..60).contains(&away), "the put-away animation plays ({away} ticks)");
    assert_eq!((gs.global.equipped[3], s.temp_back), (item::THRUSTER_PACK, 0), "saved, request consumed");
    let rows = play(&n, &mut gs, &mut s, STOMP, 150);
    assert!(rows.iter().any(|r| r.0 == 0x22), "the Thruster stomp");
    // The glide 8 only needs the Heli-Pack owned and no Hydro-Pack on the back (0x242930): with the Thruster on it
    // is the Thruster's glide.
    let rows = play(&n, &mut gs, &mut s, GLIDE, 300);
    assert!(rows.iter().any(|r| r.0 == 8 && r.1 == item::THRUSTER_PACK), "the Thruster glide");
    // The Heli-Pack back through the page. The records keep their run-time fields as the game's structs do: the root
    // page's focus is still on "Gadgets" and the grid's cursor on the Hydro-Pack.
    m.menu.enter(0, &gs);
    run(&mut dr, &mut m, &mut gs, &[(0, 20), (button::CROSS, 20)]);
    assert_eq!(m.menu.current, rc_game::menus::pause::gadgets::PAGE);
    let focus = m.menu.pages[&m.menu.current].focus;
    let Data::Grid(g) = &m.menu.widgets[&focus].data else { panic!("focus on a grid") };
    assert_eq!(g.cursor, 2, "the grid keeps its cursor");
    run(&mut dr, &mut m, &mut gs, &[(button::LEFT, 2), (button::LEFT, 2), (button::CROSS, 2), (button::START, 20)]);
    let req = dr.equip.take().unwrap();
    assert_eq!(req, [None, None, None, Some(item::HELI_PACK)]);
    inventory::apply_close_requests(&mut s, &req);
    play(&n, &mut gs, &mut s, "", 120);
    assert_eq!(gs.global.equipped[3], item::HELI_PACK);
    let rows = play(&n, &mut gs, &mut s, GLIDE, 300);
    assert!(rows.iter().any(|r| r.0 == 8 && r.1 == item::HELI_PACK), "the Heli glide");
    let rows = play(&n, &mut gs, &mut s, STOMP, 150);
    assert!(!rows.iter().any(|r| r.0 == 0x22), "no stomp with the Heli-Pack");
}

/// Feet and head items through the page: the Grindboots on, then off again (the feet toggle), the O2 Mask on; the
/// close requests them (0x14140c / 0x141410) and the hero wears them; a second close without changes requests nothing.
#[test]
fn gadgets_page_feet_and_head() {
    let (Some(d), Some(n)) = (disc(), novalis()) else { eprintln!("skipped: no extracted/"); return; };
    let (mut gs, mut s, t) = first_arrival(&d);
    let Some(mut m) = menu(&d, &t) else { return };
    for i in [item::GRINDBOOTS, item::MAGNEBOOTS, item::O2_MASK] { gs.global.owned[i as usize] = 1; }
    let mut dr = Drv::default();
    m.menu.enter(0, &gs);
    // Gadgets; Down: head (O2 Mask first) ✕; Down: feet (Grindboots) ✕ ✕ ✕ (on, off, on).
    run(&mut dr, &mut m, &mut gs, &[(0, 20), (button::DOWN, 5), (button::CROSS, 20), (button::DOWN, 3), (button::CROSS, 2), (button::DOWN, 3)]);
    run(&mut dr, &mut m, &mut gs, &[(button::CROSS, 2)]);
    assert_eq!(m.menu.equip[1], item::GRINDBOOTS);
    run(&mut dr, &mut m, &mut gs, &[(button::CROSS, 2)]);
    assert_eq!(m.menu.equip[1], 0, "selected again: off");
    run(&mut dr, &mut m, &mut gs, &[(button::CROSS, 2), (button::START, 20)]);
    let req = dr.equip.take().unwrap();
    assert_eq!(req, [None, Some(item::GRINDBOOTS), Some(item::O2_MASK), None]);
    inventory::apply_close_requests(&mut s, &req);
    play(&n, &mut gs, &mut s, "", 10);
    assert_eq!(&gs.global.equipped[1..3], &[item::GRINDBOOTS, item::O2_MASK]);
    m.menu.enter(0, &gs);
    run(&mut dr, &mut m, &mut gs, &[(0, 20), (button::START, 20)]);
    assert_eq!(dr.equip.take().unwrap(), [None; 4], "nothing changed");
}
