//! The Heli-Pack and the Thruster-Pack on Novalis (package P4, `hero/packs.rs`), items granted: the glide off the
//! spawn's ledge, both long jumps, the stomp, and the back slot's pack models (Heli 607 / Thruster 608 / Hydro 609 in
//! the lake). Skipped when `extracted/` (the extracted game data) is absent. The scripts use the engine's
//! `RC_PLAY_SCRIPT` syntax (tick = gameplay tick after the load), so the same strings drive the engine runs
//! (`RC_GIVE_ITEMS=2,3` gives both packs, the Thruster-Pack on the back; `RC_GIVE_ITEMS=3,2` the Heli-Pack).

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level};
use rc_game::hero::anim::RatchetAnim;
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::path::PathBuf;

struct Novalis {
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    spawn: [f32; 3],
    yaw: f32,
    death_z: f32,
}

fn dir() -> PathBuf { rc_formats::test_data::root().join("levels/01") }

fn novalis() -> Option<Novalis> {
    let dir = dir();
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let settings = rc_formats::test_data::gameplay_section(1, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let mobys = gameplay::parse_moby_instances(&gp).unwrap();
    let r = mobys.iter().find(|m| m.o_class == 0)?;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    Some(Novalis { mesh, ratchet: MobyAnimClass::new(&class, seqs), spawn: r.position, yaw: r.rotation[2], death_z })
}

/// Class `o_class` of level 1 as an anim class.
fn level_class(o_class: u32) -> Option<MobyAnimClass> {
    let blob = rc_formats::test_data::core_block(1, &format!("moby_class/{o_class:04}"))?;
    let c = rc_formats::moby::parse_moby_class(&blob).ok()?;
    Some(MobyAnimClass::new(&c, parse_sequences(&blob, &c).ok()?))
}

/// The engine's `RC_PLAY_SCRIPT` syntax (`a-b:stick x y`, `a-b:press B+B`; ranges inclusive, items compose).
fn script_input(script: &str, t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    for item in script.split(',') {
        let (range, action) = item.split_once(':').unwrap();
        let (a, b) = range.split_once('-').unwrap_or((range, range));
        let (a, b): (u32, u32) = (a.trim().parse().unwrap(), b.parse().unwrap());
        if t < a || t > b { continue; }
        let w: Vec<&str> = action.split_whitespace().collect();
        p = match w[..] {
            ["stick", x, y] => p.stick(x.parse().unwrap(), y.parse().unwrap()),
            ["press", names] => p.press(names.split('+').map(|n| match n {
                "X" => button::CROSS,
                "SQUARE" => button::SQUARE,
                "R1" => button::R1,
                "L1" => button::L1,
                _ => panic!("button {n}"),
            }).fold(0, |m, b| m | b)),
            _ => panic!("action {action}"),
        };
    }
    p
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rec {
    state: i32,
    pos: [f32; 3],
    height: f32,
    seq: u8,
    back: i32,
    pack_class: i16,
}

/// Runs `script` for `ticks` from the spawn with `items` owned and `back` the saved back item, the back mobys
/// modelled (classes 607 / 608 / 609 and Clank 601); one record per tick.
fn run(n: &Novalis, script: &str, ticks: u32, items: &[usize], back: i32) -> Vec<Rec> {
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [n.spawn[0], n.spawn[1], n.spawn[2], 1.0];
    m.rotation = [0.0, 0.0, n.yaw, 0.0];
    let mut g = Game::new(&n.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), n.death_z);
    g.finish_load();
    g.hero.grant_items(items);
    g.hero.equip_back(back);
    let packs: Vec<(i32, i16, MobyAnimClass)> = [(2, 607), (3, 608), (4, 609)].into_iter().filter_map(|(i, o)| Some((i, o as i16, level_class(o)?))).collect();
    if let Some(clank) = level_class(601) { g.hero.set_back_packs(packs, clank); }
    let mut anim = RatchetAnim::new(&n.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let mut out = Vec::new();
    for t in 0..ticks {
        g.hero.idle.counter = g.counter as i32;
        let r = g.tick(Some(&script_input(script, t).bytes()), &n.mesh, &mut anim.ctl(&n.ratchet), &mut hooks);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", g.hero.state);
        let h = &g.hero;
        out.push(Rec {
            state: h.state,
            pos: h.position(),
            height: h.height.to_f32(),
            seq: anim.state.seq_b,
            back: h.back_module(),
            pack_class: h.back.as_ref().map_or(-1, |b| if b.state == 0 { -1 } else { b.pack_o_class }),
        });
    }
    out
}

fn dedup(v: impl IntoIterator<Item = i32>) -> Vec<i32> {
    let mut o: Vec<i32> = Vec::new();
    for s in v { if o.last() != Some(&s) { o.push(s); } }
    o
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }

/// Prints the states / heights of a script (`--ignored --nocapture`; `PACKS_SCRIPT`, `PACKS_ITEMS`, `PACKS_BACK`).
#[test]
#[ignore]
fn novalis_packs_explore() {
    let Some(n) = novalis() else { return };
    let script = std::env::var("PACKS_SCRIPT").unwrap_or_else(|_| GLIDE.to_string());
    let items: Vec<usize> = std::env::var("PACKS_ITEMS").unwrap_or_else(|_| "2,3".into()).split(',').filter_map(|s| s.parse().ok()).collect();
    let back: i32 = std::env::var("PACKS_BACK").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
    let rows = run(&n, &script, 700, &items, back);
    for (t, r) in rows.iter().enumerate() { eprintln!("{t} {:#x} h {:.2} {:?} seq {:#x} back {} {}", r.state, r.height, r.pos, r.seq, r.back, r.pack_class); }
}

/// From the spawn: run toward the lake (the walk of `hero_novalis.rs::LAKE_SCRIPT`), jump off the drop to the
/// lower walkway with ✕ held: the Heli-Pack glides down.
pub const GLIDE: &str = "0-214:stick 0 -1,150-151:press X,152-260:press X";
/// Run, crouch (R1) with the stick forward, ✕: the long jump of the pack on the back.
pub const LONG_JUMP: &str = "0-100:stick 0 -1,60-69:press R1,70-70:press R1+X";
/// A held jump, then R1 in the air: the Thruster stomp.
pub const STOMP: &str = "20-40:press X,50-50:press R1";

#[test]
fn novalis_heli_glide() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let rows = run(&n, GLIDE, 400, &[2], 2);
    let st = dedup(rows.iter().map(|r| r.state));
    eprintln!("glide: {st:x?}");
    let first = rows.iter().position(|r| r.state == 8).expect("glides");
    let last = rows.iter().rposition(|r| r.state == 8).unwrap();
    assert_eq!(rows[first].seq, 0x13, "the glide anim");
    // A steady 2.16 u/s sink in the middle of the glide.
    let (a, b) = (first + 10, (first + 40).min(last));
    let sink = (rows[a].pos[2] - rows[b].pos[2]) / ((b - a) as f32 / 60.0);
    eprintln!("glide ticks {first}..={last}: sink {sink} u/s, from {:?} to {:?}", rows[first].pos, rows[last].pos);
    assert!((sink - 2.16).abs() < 0.02, "sink {sink}");
    assert!(last - first > 30, "glided {} ticks", last - first);
    assert_eq!(rows.last().unwrap().state, 0, "landed: {st:x?}");
    // Without the Heli-Pack the same script falls.
    let rows = run(&n, GLIDE, 400, &[], 2);
    assert!(!rows.iter().any(|r| r.state == 8));
}

#[test]
fn novalis_long_jumps() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    for (back, id, class) in [(2, 10, 607), (3, 0x10, 608)] {
        let rows = run(&n, LONG_JUMP, 240, &[2, 3], back);
        let st = dedup(rows.iter().map(|r| r.state));
        let first = rows.iter().position(|r| r.state == id).unwrap_or_else(|| panic!("no {id:#x}: {st:x?}"));
        let last = rows.iter().rposition(|r| r.state == id).unwrap();
        let d = dist2(rows[first].pos, rows[last].pos);
        eprintln!("back {back}: {id:#x} ticks {first}..={last}, {d} units, {st:x?}, pack class {}", rows[first].pack_class);
        assert_eq!(rows[first].pack_class, class, "Clank wears the pack");
        assert!(d > 7.0, "long jump {d}");
        assert!(matches!(rows[last + 1].state, 3 | 0 | 2 | 6 | 0x7a), "{st:x?}");
    }
}

#[test]
fn novalis_thruster_stomp() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let rows = run(&n, STOMP, 200, &[3], 3);
    let st = dedup(rows.iter().map(|r| r.state));
    eprintln!("stomp: {st:x?}");
    let first = rows.iter().position(|r| r.state == 0x22).expect("stomps");
    assert_eq!(rows[first].seq, 0x2a);
    let apex = rows[first..].iter().map(|r| r.height).fold(0.0f32, f32::max);
    assert!(apex > 1.5, "rises first: {apex}");
    assert_eq!(rows.last().unwrap().state, 0, "back on the ground: {st:x?}");
    // The Heli-Pack on the back: no stomp.
    let rows = run(&n, STOMP, 200, &[2, 3], 2);
    assert!(!rows.iter().any(|r| r.state == 0x22));
}

/// The lake with the Hydro-Pack owned: in the water the back swaps to the Hydro-Pack (609, after the Heli-Pack's
/// put-away), the dive uses it.
#[test]
fn novalis_hydro_pack_on_the_back_in_the_lake() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let lake = "0-214:stick 0 -1,215-299:stick 0.5 -0.85,300-399:stick 0.2 -0.98,400-700:stick 0 -1";
    let rows = run(&n, lake, 760, &[2, 4], 2);
    let water = rows.iter().position(|r| matches!(r.state, 0x36 | 0x37)).expect("reaches the water");
    let classes = dedup(rows.iter().map(|r| r.pack_class as i32));
    eprintln!("water at {water}; pack classes {classes:?}; back modules {:?}", dedup(rows.iter().map(|r| r.back)));
    assert_eq!(rows[water - 1].pack_class, 607);
    assert_eq!(rows.last().unwrap().pack_class, 609, "{classes:?}");
    assert_eq!(rows.last().unwrap().back, 4);
    assert!(classes.windows(3).any(|w| w == [607, -1, 609]), "put away, then created: {classes:?}");
}

/// Two identical runs give identical records (no hidden state across runs).
#[test]
fn novalis_packs_deterministic() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    for (script, back) in [(GLIDE, 2), (LONG_JUMP, 3), (STOMP, 3)] {
        assert_eq!(run(&n, script, 300, &[2, 3], back), run(&n, script, 300, &[2, 3], back));
    }
}
