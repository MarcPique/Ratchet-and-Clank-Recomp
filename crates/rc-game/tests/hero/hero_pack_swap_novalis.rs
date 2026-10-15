//! Clank's packs on Novalis (docs/plan/gadgets.md §1 "Swaps", hero_states.md "After-images"): the back slot's swap
//! sequence and its timings into and out of the lake (the game's `UpdateWrenchSelected(3)` 0x2307e0, the slot loop
//! 0x231088, the deletion 0x2305e8, `HeroItemsCreate` 0x22f3c0), and Ratchet's after-images in the Thruster-Pack long
//! jump. Skipped when `extracted/` is absent. Scripts: the engine's `RC_PLAY_SCRIPT` ticks.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level};
use rc_game::hero::anim::RatchetAnim;
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::tick::{Game, GameOptions, TickHooks};

struct Novalis {
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    spawn: [f32; 3],
    yaw: f32,
}

fn novalis() -> Option<Novalis> {
    let dir = rc_formats::test_data::root().join("levels/01");
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let mobys = gameplay::parse_moby_instances(&gp).unwrap();
    let r = mobys.iter().find(|m| m.o_class == 0)?;
    let blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    Some(Novalis { mesh, ratchet: MobyAnimClass::new(&class, seqs), spawn: r.position, yaw: r.rotation[2] })
}

fn level_class(o_class: u32) -> Option<MobyAnimClass> {
    let blob = rc_formats::test_data::core_block(1, &format!("moby_class/{o_class:04}"))?;
    let c = rc_formats::moby::parse_moby_class(&blob).ok()?;
    Some(MobyAnimClass::new(&c, parse_sequences(&blob, &c).ok()?))
}

/// One tick of the back slot: hero state, slot +0x24 state, the pack moby's class (−1: none), its key A / key B
/// sequences; Ratchet's after-images (alpha, placed) and trail activity.
#[derive(Clone, Debug, PartialEq)]
struct Rec {
    state: i32,
    slot: i32,
    pack: i16,
    seq_a: u8,
    seq_b: u8,
    trail: bool,
    ghosts: Vec<(u8, bool)>,
}

fn run(n: &Novalis, input: impl Fn(u32) -> PadInput, ticks: u32, items: &[usize], back: i32) -> Vec<Rec> {
    let ci = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&ci));
    m.position = [n.spawn[0], n.spawn[1], n.spawn[2], 1.0];
    m.rotation = [0.0, 0.0, n.yaw, 0.0];
    let mut g = Game::new(&n.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), -100.0);
    g.finish_load();
    g.hero.grant_items(items);
    g.hero.equip_back(back);
    let packs: Vec<(i32, i16, MobyAnimClass)> = [(2, 607), (3, 608), (4, 609)].into_iter().filter_map(|(i, o)| Some((i, o as i16, level_class(o)?))).collect();
    g.hero.set_back_packs(packs, level_class(601).unwrap());
    let mut anim = RatchetAnim::new(&n.ratchet);
    let mut mf = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut pf = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mf, particles: &mut pf, world: None };
    let mut out = Vec::new();
    for t in 0..ticks {
        g.hero.idle.counter = g.counter as i32;
        g.tick(Some(&input(t).bytes()), &n.mesh, &mut anim.ctl(&n.ratchet), &mut hooks);
        let h = &g.hero;
        let (pack, seq_a, seq_b) = h.back.as_ref().map_or((-1, 0, 0), |b| (if b.state == 0 { -1 } else { b.pack_o_class }, b.pack.anim.seq_a, b.pack.anim.seq_b));
        let tr = &h.fx.trails.hero;
        out.push(Rec { state: h.state, slot: h.back_slot.slot.state, pack, seq_a, seq_b, trail: tr.active, ghosts: tr.ghosts.iter().map(|g| (g.alpha, g.drawn.is_some())).collect() });
    }
    out
}

/// Runs of equal values: `(value, first tick, length)`.
fn runs<T: PartialEq + Copy>(v: &[T]) -> Vec<(T, usize, usize)> {
    let mut o: Vec<(T, usize, usize)> = Vec::new();
    for (i, &x) in v.iter().enumerate() {
        match o.last_mut() {
            Some(r) if r.0 == x => r.2 += 1,
            _ => o.push((x, i, 1)),
        }
    }
    o
}

/// Into the lake and back out with the Heli-Pack on the back and the Hydro-Pack owned. Every back change is one
/// sequence: the slot goes to 3 (put away: the pack blends to its sequence 2 over 2 ticks), the pack is deleted when
/// sequence 2 wraps, the slot is empty (0) for one tick, the next hero update creates the new pack on its sequence 0
/// (the take-out), and a wrapped sequence 0 blends to 1 over 2 ticks. The Heli-Pack's sequences 0 and 2 are single
/// frames (the put-away takes 2 ticks); the Hydro-Pack's take-out has 7 keys and its put-away 3 (2 + 2·2 ticks).
#[test]
fn back_swap_sequence_into_and_out_of_the_lake() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let input = |t: u32| {
        let (x, y) = match t { 0..=214 => (0.0, -1.0), 215..=299 => (0.5, -0.85), 300..=399 => (0.2, -0.98), 400..=560 => (0.0, -1.0), _ => (0.0, 1.0) };
        PadInput::neutral().stick(x, y)
    };
    let rows = run(&n, input, 800, &[2, 4], 2);
    let slot = runs(&rows.iter().map(|r| r.slot).collect::<Vec<_>>());
    eprintln!("slot runs {slot:?}");
    // In: 2 → 3 (2 ticks) → 0 (1 tick) → 2 with the Hydro-Pack.
    let i = slot.iter().position(|r| r.0 == 3).expect("a swap");
    let (put, empty, back) = (slot[i], slot[i + 1], slot[i + 2]);
    assert_eq!((put.2, empty.0, empty.2, back.0), (2, 0, 1, 2), "{slot:?}");
    let t = put.1;
    assert!(matches!(rows[t].state, 0x37 | 0x36), "the swap starts in the water: {:#x}", rows[t].state);
    assert_eq!((rows[t].pack, rows[t].seq_b), (607, 2), "the Heli-Pack blends to its put-away");
    assert_eq!(rows[back.1].pack, 609);
    assert_eq!((rows[back.1].seq_a, rows[back.1].seq_b), (0, 0), "the Hydro-Pack starts on its take-out");
    // Out (the shallow edge, state 0x73): the Hydro-Pack folds (sequence 2, 6 ticks), 1 tick empty, the Heli-Pack.
    let j = slot.iter().enumerate().skip(i + 3).find(|(_, r)| r.0 == 3).map(|(k, _)| k).expect("the swap back");
    let (put, empty, back) = (slot[j], slot[j + 1], slot[j + 2]);
    assert_eq!((put.2, empty.0, empty.2, back.0), (6, 0, 1, 2), "{slot:?}");
    assert_eq!((rows[put.1].pack, rows[put.1].seq_b), (609, 2));
    assert_eq!(rows[back.1].pack, 607);
    // Two identical runs give identical records.
    assert_eq!(rows, run(&n, input, 800, &[2, 4], 2));
}

/// The Thruster-Pack long jump's after-images (Ratchet's record 0x1409c0): three ghosts (0x28 / 0x14 / 0x0a at 2 / 4 / 6
/// ticks back) from the SetState of 0x10, each placed once the history is long enough, faded by 2 a tick after tick
/// 12 (the brightest is empty 20 ticks later and the trail ends), and ended by the next SetState.
#[test]
fn thruster_long_jump_after_images() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let input = |t: u32| {
        let p = PadInput::neutral().stick(0.0, -1.0);
        match t {
            60..=69 => p.press(button::R1),
            70 => p.press(button::R1 | button::CROSS),
            _ if t <= 100 => p,
            _ => PadInput::neutral(),
        }
    };
    let rows = run(&n, input, 200, &[2, 3], 3);
    let s = rows.iter().position(|r| r.state == 0x10).expect("the long jump");
    assert!(rows[s].trail);
    assert_eq!(rows[s].ghosts.iter().map(|g| g.0).collect::<Vec<_>>(), vec![0x28, 0x14, 0x0a]);
    // Placed by the physics of the jump's ticks (the SetState tick has none): back 2 after 2 updates, back 6 after 6.
    let placed = |k: usize| rows[s..].iter().position(|r| r.ghosts.get(k).is_some_and(|g| g.1)).unwrap();
    assert_eq!((placed(0), placed(1), placed(2)), (2, 4, 6), "{:?}", &rows[s..s + 8]);
    // The fade: the brightest ghost drops by 2 a tick from the 13th physics tick on.
    let a0: Vec<u8> = rows[s..].iter().take_while(|r| r.state == 0x10 && r.trail).map(|r| r.ghosts[0].0).collect();
    eprintln!("ghost 0 alpha {a0:?}");
    assert!(a0.windows(2).all(|w| w[1] == w[0] || w[1] + 2 == w[0] || w[1] == 0));
    let full = a0.iter().filter(|&&a| a == 0x28).count();
    assert!((12..=14).contains(&full), "{full} ticks unfaded: {a0:?}");
    let end = s + a0.len();
    assert!(!rows[end].trail, "ended: faded out ({}) or the landing's SetState ({:#x})", a0.last().copied().unwrap_or(0), rows[end].state);
    assert!(rows[end].ghosts.is_empty());
}
