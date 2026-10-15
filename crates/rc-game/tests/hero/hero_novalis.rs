//! The on-foot hero, pad and follow camera on Novalis (level 1) collision. Skipped when `extracted/` (the
//! extracted game data, never shipped with the repo) is absent.

use rc_formats::{collision, gameplay, level};
use rc_game::collision_query::{coll_line, QueryFlags};
use rc_formats::moby_anim::{parse_sequence, MobyAnimClass, MobySequence};
use rc_game::hero::anim::RatchetAnim;
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::tick::{Game, GameOptions, TickHooks};

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
    let mobys = gameplay::parse_moby_instances(&gp).unwrap();
    let r = mobys.iter().find(|m| m.o_class == 0)?;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    Some(Novalis { mesh, ratchet, spawn: r.position, yaw: r.rotation[2], death_z })
}

fn ground_below(mesh: &collision::Collision, p: [f32; 3]) -> Option<f32> {
    coll_line(mesh, [p[0], p[1], p[2] + 1.0], [p[0], p[1], (p[2] - 64.0).max(0.001)], QueryFlags(2)).map(|h| h.point[2])
}

/// The game with Ratchet at his instance (hero init 0x226b70 snaps the moby to the ground).
fn game(n: &Novalis) -> (Game, f32) {
    let ground = ground_below(&n.mesh, n.spawn).expect("ground under the spawn");
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [n.spawn[0], n.spawn[1], n.spawn[2], 1.0];
    m.rotation = [0.0, 0.0, n.yaw, 0.0];
    let table = MobyTable::new(vec![m], 16);
    (Game::new(&n.mesh, table, 0, GameOptions::default(), n.death_z), ground)
}

struct Driver {
    anim: RatchetAnim,
    states: Vec<i32>,
    seqs: Vec<u8>,
    zs: Vec<f32>,
    resets: u32,
}

impl Driver {
    fn new(class: &MobyAnimClass) -> Self { Driver { anim: RatchetAnim::new(class), states: Vec::new(), seqs: Vec::new(), zs: Vec::new(), resets: 0 } }
    fn run(&mut self, g: &mut Game, class: &MobyAnimClass, mesh: &collision::Collision, input: PadInput, n: usize) {
        let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
        let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
        for _ in 0..n {
            let r = g.tick(Some(&input.bytes()), mesh, &mut self.anim.ctl(class), &mut hooks);
            assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x}", g.hero.state);
            if r.camera_reset { self.resets += 1; }
            self.states.push(g.hero.state);
            self.seqs.push(self.anim.state.seq_b);
            self.zs.push(g.hero.position()[2]);
        }
    }
}

fn dedup(v: &[i32]) -> Vec<i32> {
    let mut o: Vec<i32> = Vec::new();
    for &s in v { if o.last() != Some(&s) { o.push(s); } }
    o
}

#[test]
fn novalis_idle_run_jump_and_camera() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let (mut g, ground) = game(&n);
    eprintln!("spawn {:?} yaw {} ground {ground} death z {}", n.spawn, n.yaw, n.death_z);
    let mut d = Driver::new(&n.ratchet);

    // 1. 120 idle ticks: grounded on the ground, state 0.
    d.run(&mut g, &n.ratchet, &n.mesh, PadInput::neutral(), 120);
    let p = g.hero.position();
    eprintln!("after idle: {p:?}, air ticks {}, state {}", g.hero.air_ticks, g.hero.state);
    assert_eq!(g.hero.state, 0);
    assert_eq!(g.hero.air_ticks, 0);
    assert!((p[2] - ground).abs() < 1e-4, "z {} vs ground {ground}", p[2]);

    // Camera settled behind the hero at its spring distance, with a clear line from the pivot.
    let cam = g.camera.out.pos_f32();
    let fwd = g.camera.out.rows_f32()[0];
    let to_hero = [p[0] - cam[0], p[1] - cam[1]];
    let hd = (to_hero[0] * to_hero[0] + to_hero[1] * to_hero[1]).sqrt();
    let pivot = rc_game::hero::physics::to_f32x3(g.camera.cam.pivot);
    eprintln!("camera {cam:?} fwd {fwd:?}, horizontal distance {hd}, pivot {pivot:?}, dist {} red {}",
        g.camera.cam.dist.to_f32(), g.camera.cam.red.to_f32());
    assert!(hd > 1.4 && hd < 6.1, "camera distance {hd}");
    assert!(fwd[0] * to_hero[0] + fwd[1] * to_hero[1] > 0.0, "camera looks away from the hero");
    assert!(coll_line(&n.mesh, pivot, cam, QueryFlags(0xb4)).is_none(), "camera inside the collision mesh");
    let cam_yaw = g.camera.out.yaw().to_f32();

    // 2. Stick forward for 120 ticks: moves along the camera's forward, stays on the terrain.
    let start = g.hero.position();
    d.run(&mut g, &n.ratchet, &n.mesh, PadInput::neutral().stick(0.0, -1.0), 120);
    let end = g.hero.position();
    let moved = [end[0] - start[0], end[1] - start[1]];
    let dist = (moved[0] * moved[0] + moved[1] * moved[1]).sqrt();
    let dir = moved[1].atan2(moved[0]);
    let gz = ground_below(&n.mesh, end).expect("ground under the run end");
    eprintln!("run: {dist} units, heading {dir} (camera yaw {cam_yaw}), end {end:?}, ground {gz}, states {:?}",
        dedup(&d.states[120..]));
    eprintln!("run anim seqs {:?}", dedup(&d.seqs[120..].iter().map(|&s| s as i32).collect::<Vec<_>>()));
    assert!(d.seqs[120..].contains(&4), "run cycle (seq 4) never selected");
    let expect = 5.7 * (120.0 - 46.0) / 60.0 + 0.5 * 5.7 * 46.0 / 60.0;
    assert!(dist > 0.6 * expect && dist < 1.1 * expect, "ran {dist}, model {expect}");
    let dyaw = (dir - cam_yaw + std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
    assert!(dyaw.abs() < 0.35, "heading {dir} vs camera yaw {cam_yaw}");
    assert!((end[2] - gz).abs() < 0.3, "z {} vs ground {gz}", end[2]);
    assert!(d.zs.iter().all(|&z| z > ground - 20.0), "fell through the terrain");

    // 3. Stop, then jump: 7 → landing → 0.
    d.run(&mut g, &n.ratchet, &n.mesh, PadInput::neutral(), 60);
    let before = d.states.len();
    let z0 = g.hero.position()[2];
    d.run(&mut g, &n.ratchet, &n.mesh, PadInput::neutral().press(button::CROSS), 1);
    d.run(&mut g, &n.ratchet, &n.mesh, PadInput::neutral(), 90);
    let seq = dedup(&d.states[before..]);
    let apex = d.zs[before..].iter().copied().fold(f32::MIN, f32::max) - z0;
    eprintln!("jump: states {seq:?}, apex {apex}, end z {} (from {z0}), camera resets {}", g.hero.position()[2], d.resets);
    eprintln!("jump anim seqs {:?}", dedup(&d.seqs[before..].iter().map(|&s| s as i32).collect::<Vec<_>>()));
    assert_eq!(seq.first(), Some(&7));
    assert_eq!(seq.last(), Some(&0));
    assert!(apex > 1.0 && apex < 1.5, "apex {apex}");
    assert_eq!(g.hero.air_ticks, 0);

    // Camera still clear after all that.
    let cam = g.camera.out.pos_f32();
    let pivot = rc_game::hero::physics::to_f32x3(g.camera.cam.pivot);
    assert!(coll_line(&n.mesh, pivot, cam, QueryFlags(0xb4)).is_none(), "camera inside the collision mesh");
}

/// Class `o_class` of level 1 (core block `moby_class/NNNN`) as an anim class.
fn level_class(o_class: u32) -> Option<MobyAnimClass> {
    let blob = rc_formats::test_data::core_block(1, &format!("moby_class/{o_class:04}"))?;
    let c = rc_formats::moby::parse_moby_class(&blob).ok()?;
    let seqs = rc_formats::moby_anim::parse_sequences(&blob, &c).ok()?;
    Some(MobyAnimClass::new(&c, seqs))
}

/// The two idle fidgets' timing on Ratchet's own sequences against the savestates
/// (docs/plan/trace_results_novalis.md "Second savestate"): slot 1 shows fidget record 0 (sequence 1)
/// started 202 ticks earlier (its cooldown 330 − 202 = 128) and the blend back to the idle sequence 14 ticks
/// earlier (sequence timer 14, t = 0.97 = curve −2 step 14), so sequence 1 wrapped 188 ticks after
/// `SetAnim(12, 1, 0)`; slot 2 shows sequence 2 100 ticks after its start on frames 32 → 33 with t = 0.
#[test]
fn novalis_fidget_sequences_match_the_savestates() {
    use rc_game::hero::AnimCtl;
    use rc_game::ps2v::Pf;
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let mut a = RatchetAnim::new(&n.ratchet);
    let mut c = a.ctl(&n.ratchet);
    c.set_anim(Pf::from_i32(12), 1, 0);
    let mut wrap = None;
    for k in 1..=400 {
        c.advance(Pf::ONE);
        if c.view().flags & 2 != 0 { wrap = Some(k); break; }
    }
    assert_eq!(wrap, Some(188), "sequence 1 wraps {wrap:?} advances after the fidget's SetAnim");
    // The blend back (curve −2): 14 advances later t = 0.97.
    c.set_anim(Pf::b(0xc000_0000), 0, 0);
    for _ in 0..14 { c.advance(Pf::ONE); }
    assert_eq!(c.view().t, f32::from_bits(0x3f78_51ec));
    let mut a = RatchetAnim::new(&n.ratchet);
    let mut c = a.ctl(&n.ratchet);
    c.set_anim(Pf::from_i32(12), 2, 0);
    for _ in 0..100 { c.advance(Pf::ONE); }
    let v = c.view();
    assert_eq!((v.seq_a, v.seq_b, v.frame_a, v.frame_b, v.t), (2, 2, 32, 33, 0.0), "sequence 2 after 100 ticks");
}

/// Idle from the load pass on Novalis with the back items (pack 607, Clank 601): Ratchet fidgets (sequences 1
/// and 2, 188 ticks for sequence 1), at least 120 ticks apart, Clank fidgets on his own table rows and returns to
/// sequence 1, and the hero's draws per tick stay within the idle consumers' bounds.
#[test]
fn novalis_idle_fidgets_and_draws() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let (Some(pack), Some(clank)) = (level_class(607), level_class(601)) else { eprintln!("skipped: no classes 601/607"); return; };
    let (mut g, _) = game(&n);
    g.finish_load();
    g.hero.set_back_classes(pack, clank);
    g.hero.idle.level = 1;
    let mut anim = RatchetAnim::new(&n.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let ticks = 3600usize;
    let (mut seqs, mut packs, mut draws, mut coolds) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut starts: Vec<(usize, u8)> = Vec::new();
    for t in 0..ticks {
        g.hero.idle.counter = g.counter as i32;
        let before = g.rng;
        let prev = anim.state.seq_b;
        let r = g.tick(Some(&PadInput::neutral().bytes()), &n.mesh, &mut anim.ctl(&n.ratchet), &mut hooks);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran);
        assert_eq!(g.hero.state, 0, "idle left state 0 at tick {t}");
        let mut probe = before;
        let mut d = 0;
        while probe.state != g.rng.state { probe.rand(); d += 1; assert!(d < 1000); }
        let s = anim.state.seq_b;
        if prev == 0 && (s == 1 || s == 2) { starts.push((t, s)); }
        seqs.push(s);
        packs.push(g.hero.back.as_ref().map(|b| b.pack.anim.seq_b).unwrap_or(0xff));
        draws.push(d);
        coolds.push(g.hero.idle.cooldown);
    }
    let total: usize = draws.iter().sum();
    let window: usize = draws[2439.min(ticks)..3511.min(ticks)].iter().sum();
    eprintln!("{ticks} idle ticks: {total} draws ({:.3}/tick); ticks 2439..3511: {window} ({:.3}/tick)", total as f64 / ticks as f64, window as f64 / 1072.0);
    eprintln!("fidget starts {starts:?}");
    let pack_seqs: std::collections::BTreeSet<u8> = packs.iter().copied().collect();
    eprintln!("pack sequences seen {pack_seqs:?}, Clank blinks / fidget timer {} / {}", g.hero.idle.clank_blink_timer, g.hero.idle.clank_fidget_timer);
    assert!(!starts.is_empty(), "no fidget in {ticks} ticks");
    for w in starts.windows(2) { assert!(w[1].0 - w[0].0 >= 120, "fidgets {w:?} closer than the cooldown"); }
    for &(t, s) in &starts {
        let len = seqs[t..].iter().position(|&q| q == 0).unwrap_or(usize::MAX);
        eprintln!("fidget seq {s} at tick {t}: {len} ticks");
        if s == 1 && len != usize::MAX { assert_eq!(len, 188, "sequence 1 fidget length"); }
    }
    assert!(pack_seqs.iter().any(|s| [3, 4, 7].contains(s)), "Clank never fidgeted: {pack_seqs:?}");
    assert!(pack_seqs.contains(&1));
    // Per tick: the back table draws once in the idle sequence; the most a tick can draw here is the table,
    // the chance, a SetAnim, Clank's fidget (2), the look (4 or 1), the secondaries (9), a blink (2), Clank's blink.
    for t in 2..ticks {
        if seqs[t - 1] == 0 && seqs[t] == 0 { assert!(draws[t] >= 1, "tick {t}: no back-table draw"); }
        assert!(draws[t] <= 1 + 1 + 1 + 2 + 4 + 9 + 2 + 1, "tick {t}: {} draws", draws[t]);
        if seqs[t - 1] != 0 && seqs[t] == seqs[t - 1] { assert!(draws[t] >= 1, "tick {t}: no look re-arm in a fidget"); }
    }
    let _ = coolds;
}

/// The engine's `RC_PLAY_SCRIPT` syntax (`a-b:stick x y`, `a-b:press B+B`; ranges inclusive, items compose).
fn script_input(script: &str, t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    for item in script.split(',') {
        let (range, action) = item.split_once(':').unwrap();
        let (a, b) = range.split_once('-').unwrap_or((range, range));
        let (a, b): (u32, u32) = (a.parse().unwrap(), b.parse().unwrap());
        if t < a || t > b { continue; }
        let w: Vec<&str> = action.split_whitespace().collect();
        p = match w[..] {
            ["stick", x, y] => p.stick(x.parse().unwrap(), y.parse().unwrap()),
            ["press", names] => p.press(names.split('+').map(|n| match n {
                "X" => button::CROSS,
                "SQUARE" => button::SQUARE,
                "R1" => button::R1,
                _ => panic!("button {n}"),
            }).fold(0, |m, b| m | b)),
            _ => panic!("action {action}"),
        };
    }
    p
}

/// Novalis: from the spawn, run off the ledge to the lower walkway and walk into the lake (the level's surface-0
/// faces at z ≈ 39); swim on the surface, stop, then dive: the Hydro-Pack (R1, then R1+□ and the stick) takes
/// Ratchet down fast; without it R1 does nothing and □ is the slow dive. The engine checks run these scripts
/// (docs/plan/player_controller.md §14).
pub const LAKE_SCRIPT: &str = "0-214:stick 0 -1,215-299:stick 0.5 -0.85,300-399:stick 0.2 -0.98,400-700:stick 0 -1";
pub const DIVE_SCRIPT: &str = ",760-760:press R1,761-820:press R1+SQUARE,761-840:stick 0 -1,821-900:press R1";

#[test]
fn novalis_walk_into_the_lake_swim_and_dive() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    for pack in [false, true] {
        let (mut g, _) = game(&n);
        g.finish_load();
        g.hero.owned.set(rc_game::hero::swim::ITEM_HYDRO_PACK, pack);
        let script = format!("{LAKE_SCRIPT}{DIVE_SCRIPT}");
        let mut d = Driver::new(&n.ratchet);
        let mut first_water = None;
        let mut at_760 = None;
        let mut min_z = f32::MAX;
        for t in 0..900u32 {
            d.run(&mut g, &n.ratchet, &n.mesh, script_input(&script, t), 1);
            if first_water.is_none() && matches!(g.hero.state, 0x36 | 0x37) { first_water = Some((t, g.hero.position())); }
            if t == 759 { at_760 = Some((g.hero.state, g.hero.position(), g.hero.water_level.to_f32())); }
            if t > 760 { min_z = min_z.min(g.hero.position()[2]); }
        }
        let states = dedup(&d.states);
        eprintln!("pack {pack}: water at {first_water:?}, before the dive {at_760:?}, states {states:x?}, lowest z after {min_z}, end {:?} state {:#x}",
            g.hero.position(), g.hero.state);
        let (t, p) = first_water.expect("never reached the water");
        assert!(t > 400 && t < 700 && p[2] < 40.0, "water at {t} {p:?}");
        let (s, p, w) = at_760.unwrap();
        assert_eq!(s, 0x37, "treading water before the dive");
        assert!((p[2] - (w - 0.12)).abs() < 0.2, "floats: z {} level {w}", p[2]);
        assert!(states.contains(&0x36), "surface swim");
        if pack {
            assert!(states.contains(&0x35), "Hydro-Pack dive: {states:x?}");
            assert!(min_z < w - 3.0, "dove to {min_z} under {w}");
        } else {
            assert!(!states.contains(&0x35));
            assert!(states.contains(&0x33), "□ dives without the pack: {states:x?}");
        }
    }
}

/// Refactor guard (docs/plan/hero_states.md "Restructure"): with `RC_HERO_DIGEST=<file>` set, runs fixed pad scripts
/// on Novalis (the lake swim / dive with and without the Hydro-Pack, and a move script: jumps, double jump,
/// running jump, crouch, flip, fall off the ledge, L1 into the unported look stance) and writes one line per tick:
/// state, timer, and a hash of the whole hero block, Ratchet's anim state, the camera, the pad and the RNG. A
/// behaviour-neutral change of the hero code must leave the file byte-identical. Without the variable it only runs.
#[test]
fn novalis_hero_digest() {
    use std::hash::{Hash, Hasher};
    use std::fmt::Write as _;
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    const MOVES: &str = "30-31:press X,50-51:press X,120-180:stick 0 -1,150-151:press X,168-169:press X,\
        240-255:press R1,256-257:press R1+X,256-258:stick 0 1,330-345:press R1,346-352:press R1+X,420-470:stick 1 0,\
        480-481:press X,600-900:stick 0 -1,700-701:press X,980-990:press L1";
    let runs: [(&str, String, bool, u32); 3] = [
        ("lake", format!("{LAKE_SCRIPT}{DIVE_SCRIPT}"), false, 900),
        ("lake+pack", format!("{LAKE_SCRIPT}{DIVE_SCRIPT}"), true, 900),
        ("moves", MOVES.to_string(), false, 1000),
    ];
    let mut out = String::new();
    for (name, script, pack, ticks) in runs {
        let (mut g, _) = game(&n);
        g.finish_load();
        g.hero.owned.set(rc_game::hero::swim::ITEM_HYDRO_PACK, pack);
        let mut anim = RatchetAnim::new(&n.ratchet);
        let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
        let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
        for t in 0..ticks {
            let input = script_input_ext(&script, t);
            let r = g.tick(Some(&input.bytes()), &n.mesh, &mut anim.ctl(&n.ratchet), &mut hooks);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            // The fields after `surf` (package P4: item ownership, the back slot, the pack block, the wall-ahead
            // probe) are cut, and the Hydro-Pack / O2 mask flags the swim block printed before the owned mirror
            // are put back.
            let mut hero = format!("{:?}", g.hero);
            if let Some(i) = hero.find(", owned: ") { hero.truncate(i); hero.push_str(" }"); }
            let hero = hero.replacen("swim: Swim { ", &format!("swim: Swim {{ hydro_pack: {pack}, o2_mask: false, "), 1);
            let mut text = format!("{hero}|{:?}|{:?}|{:?}|{:?}|{:?}", anim.state, g.camera, g.pad, g.rng, r.hero);
            for f in DIGEST_NEW_FIELDS { text = text.replace(&format!(", {f}: 0"), ""); }
            // The ledge block (package P3) while it holds its defaults.
            text = text.replace(&format!(", ledge_blk: {:?}", rc_game::hero::LedgeBlock::default()), "");
            // The platform carry and surface fields (package P1) print 0 while untouched.
            for f in ["carry", "surf"] { text = text.replace(&format!(", {f}: 0"), ""); }
            // The damage and walk-to-point blocks (package P2) while they hold their defaults.
            text = text.replace(&format!(", damage: {:?}", rc_game::hero::damage::Damage::default()), "");
            text = text.replace(&format!(", walk_to: {:?}", rc_game::hero::stance::WalkTo::default()), "");
            // The camera's shake records (hero polish) while idle.
            text = text.replace(&format!(", shake: {:?}", [rc_game::follow_camera::Shake::default(); 2]), "");
            // The first-person camera and the switch blend (weapons + first person) while idle.
            text = text.replace(&format!(", first_person: {:?}", rc_game::follow_camera::FirstPerson::default()), "");
            text = text.replace(&format!(", blend: {:?}", rc_game::follow_camera::CamBlend::default()), "");
            // The blend's orbit kind (the Swingshot camera's blend) while unused, inside a running blend too.
            text = text.replace(&format!(", kind: 0, running: 0, orbit_len: 0, orbit: {:?}", rc_game::follow_camera::OrbitBlend::default()), "");
            // The script camera (cinematics) while idle.
            text = text.replace(&format!(", script: {:?}", rc_game::follow_camera::script::ScriptCamera::default()), "");
            // The type-6 camera (the Visibomb's missile view) while idle.
            text = text.replace(&format!(", type6: {:?}", rc_game::follow_camera::type6::Type6::default()), "");
            // The level's camera slots (the level camera system) while no records are loaded.
            text = text.replace(&format!(", level_cams: {:?}", rc_game::follow_camera::level::LevelCameras::default()), "");
            // The Swingshot camera and the Swingshot targets' look-up hint (camera classes) while idle.
            text = text.replace(&format!(", swing: {:?}", rc_game::follow_camera::swing::SwingCamera::default()), "");
            text = text.replace(&format!(", hint: {:?}", rc_game::follow_camera::swing::LookHint::default()), "");
            text = text.replace(&format!(", world: {:?}", rc_game::follow_camera::CamWorld::default()), "");
            // The hand slot's hand point 0x1403c0 (the thrown wrench's target; no hand item in these runs).
            text = text.replace(", hand_point: [0.0, 0.0, 0.0]", "");
            // The water effects (hero water pass): the effect counters 0x13fc54..58 and the breath timer 0x13fc40 while
            // 0, and the effects channel's moby queue while empty and its copies of Ratchet's moby / joint lists.
            text = text.replace(", fx_counters: [0, 0, 0], breath: 0, jets: 0", "");
            text = text.replace(", mobys: [], frame: .., joints: .., back: ..", "");
            // RC_HERO_DIGEST_NO_IDLE=1: without the idle block (the cosmetic joint records, blink and back items),
            // to show that a change to Ratchet's joint modifiers leaves everything else as it was.
            if std::env::var("RC_HERO_DIGEST_NO_IDLE").is_ok_and(|v| v == "1") { text = cut_block(&text, ", idle: Idle {"); }
            text.hash(&mut h);
            let _ = writeln!(out, "{name} {t} state {:#x} timer {} pos {:?} {:016x}", g.hero.state, g.hero.timer, g.hero.position(), h.finish());
        }
    }
    if let Ok(path) = std::env::var("RC_HERO_DIGEST") {
        std::fs::write(&path, &out).unwrap();
        eprintln!("hero digest: {} lines -> {path}", out.lines().count());
    }
}

/// `text` without the Debug block that starts with `start` (up to its matching closing brace).
fn cut_block(text: &str, start: &str) -> String {
    let Some(i) = text.find(start) else { return text.to_string() };
    let mut depth = 0;
    for (k, c) in text[i..].char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => {
                depth -= 1;
                if depth == 0 { return format!("{}{}", &text[..i], &text[i + k + 1..]); }
            }
            _ => {}
        }
    }
    text.to_string()
}

/// Hero-block fields added after a digest baseline, with their default 0 (dropped from the hashed text so an old
/// baseline still compares): the package timers of the 2026-09-26 restructure.
const DIGEST_NEW_FIELDS: [&str; 13] = ["f500", "f502", "f508", "f510", "f518", "f51a", "f520", "f524", "f530", "f534", "f536", "f53e", "f546"];

/// [`script_input`] plus L1.
fn script_input_ext(script: &str, t: u32) -> PadInput {
    let l1: Vec<&str> = script.split(',').filter(|i| i.ends_with("press L1")).collect();
    let rest: Vec<&str> = script.split(',').filter(|i| !i.ends_with("press L1")).collect();
    let mut p = script_input(&rest.join(","), t);
    for item in l1 {
        let (range, _) = item.split_once(':').unwrap();
        let (a, b) = range.split_once('-').unwrap_or((range, range));
        let (a, b): (u32, u32) = (a.parse().unwrap(), b.parse().unwrap());
        if a <= t && t <= b { p = p.press(button::L1); }
    }
    p
}
