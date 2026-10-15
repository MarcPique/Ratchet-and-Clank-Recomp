//! Ledges on Novalis (package P3, `hero/ledge.rs`): the game's ledge probes on the level's own collision find a
//! ledge a pad script from the spawn can jump at; Ratchet grabs it, hangs, and climbs up. Skipped when `extracted/`
//! (the extracted game data) is absent. The scripts use the engine's `RC_PLAY_SCRIPT` syntax (tick = gameplay tick
//! after the load), so the same string drives the engine run.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level};
use rc_game::hero::anim::{AnimCtl, RatchetAnim};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, ClassInfo, Moby, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{self, class_info, load_static_mobys, Groups, Scheduler};
use rc_game::moby_update::services::{SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

struct Novalis {
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    spawn: [f32; 3],
    yaw: f32,
    death_z: f32,
    // The level's mobys for the world runs (as `hero_platform_novalis.rs` loads them).
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    spawnable: usize,
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
}

fn novalis() -> Option<Novalis> {
    let dir = rc_formats::test_data::root().join("levels/01");
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let settings = rc_formats::test_data::gameplay_section(1, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let r = *instances.iter().find(|m| m.o_class == 0)?;
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let splines = gameplay::parse_splines(&gp).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let Some(blob) = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)) else { continue };
        let Ok(c) = rc_formats::moby::parse_moby_class(&blob) else { continue };
        let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
        let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
        info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
        classes.classes.insert(oc, (info, Some(anim)));
    }
    let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    Some(Novalis { mesh, ratchet, spawn: r.position, yaw: r.rotation[2], death_z, instances, pvars, splines, gp: gp.to_vec(), classes, spawnable, coll_blobs })
}

/// The engine's `RC_PLAY_SCRIPT` syntax (`a-b:stick x y`, `a-b:press B+B`; ranges inclusive).
fn script_input(script: &str, t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    for item in script.split(',').filter(|s| !s.is_empty()) {
        let (range, action) = item.split_once(':').unwrap();
        let (a, b) = range.split_once('-').unwrap_or((range, range));
        let (a, b): (u32, u32) = (a.parse().unwrap(), b.parse().unwrap());
        if t < a || t > b { continue; }
        let w: Vec<&str> = action.split_whitespace().collect();
        p = match w[..] {
            ["stick", x, y] => p.stick(x.parse().unwrap(), y.parse().unwrap()),
            ["press", names] => p.press(names.split('+').map(|n| match n {
                "X" => button::CROSS,
                "R1" => button::R1,
                "L1" => button::L1,
                "SQUARE" => button::SQUARE,
                _ => panic!("button {n}"),
            }).fold(0, |m, b| m | b)),
            _ => panic!("action {action}"),
        };
    }
    p
}

/// One tick of the run: state, position, key time, anim sequence.
type Row = (i32, [f32; 3], f32, u8);

fn run(n: &Novalis, script: &str, ticks: u32) -> Vec<Row> { run_world(n, script, ticks) }

/// The level with its mobys, as the engine ticks it (the loader's static mobys, the scheduler's load pass and
/// moby loop, their collision for the hero's and the camera's queries: `hero_platform_novalis.rs`'s harness; no
/// particles, sounds or water tables). The spawn walk crosses the lift 726 at the plateau's edge, so the world
/// runs, not [`run_with`], match the engine's `RC_PLAY_SCRIPT` runs.
fn run_world(n: &Novalis, script: &str, ticks: u32) -> Vec<Row> {
    let classes = Arc::new(ClassTable { classes: n.classes.classes.clone() });
    let mut ct = ClassTable { classes: n.classes.classes.clone() };
    let statics = load_static_mobys(&n.instances, &mut ct, &n.pvars);
    let hero_idx = statics.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let mut table = MobyTable::new(statics, n.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&n.mesh, table, hero_idx, GameOptions::default(), n.death_z);
    game.hero.idle.level = 1;
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&n.splines);
    svc.groups = Groups::parse(&n.gp, &|i| (i < n.instances.len()).then_some(i));
    svc.set_moby_collision(n.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    let mut sched = Scheduler::new();
    {
        let hero: Hero = game.hero.clone();
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&n.mesh);
        sched.load_pass(&mut w);
    }
    game.finish_load();
    let mut anim = RatchetAnim::new(&n.ratchet);
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let mut rows = Vec::new();
    for t in 0..ticks {
        let classes_ref: &ClassTable = &classes;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            sched.tick(&mut w);
        };
        let mut parts = |_: &Hero, _: &rc_game::follow_camera::CameraView, _: &mut Rng, _: u64| {};
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        let r = game.tick(Some(&script_input(script, t).bytes()), &n.mesh, &mut anim.ctl(&n.ratchet), &mut hooks);
        if r.hero != rc_game::hero::HeroTick::Ran { break; }
        let v = anim.ctl(&n.ratchet).view();
        rows.push((game.hero.state, game.hero.position(), v.frame, v.seq_b));
    }
    rows
}

fn run_with(n: &Novalis, script: &str, ticks: u32, hydro: bool, each: &mut dyn FnMut(&rc_game::hero::Hero)) -> Vec<Row> {
    run_from(n, n.spawn, n.yaw, script, ticks, hydro, each)
}

fn run_from(n: &Novalis, at: [f32; 3], yaw: f32, script: &str, ticks: u32, hydro: bool, each: &mut dyn FnMut(&rc_game::hero::Hero)) -> Vec<Row> {
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [at[0], at[1], at[2], 1.0];
    m.rotation = [0.0, 0.0, yaw, 0.0];
    let mut g = Game::new(&n.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), n.death_z);
    g.finish_load();
    g.hero.owned.set(rc_game::hero::swim::ITEM_HYDRO_PACK, hydro);
    let mut anim = RatchetAnim::new(&n.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let mut rows = Vec::new();
    for t in 0..ticks {
        let r = g.tick(Some(&script_input(script, t).bytes()), &n.mesh, &mut anim.ctl(&n.ratchet), &mut hooks);
        if r.hero != rc_game::hero::HeroTick::Ran { break; }
        each(&g.hero);
        let v = anim.ctl(&n.ratchet).view();
        rows.push((g.hero.state, g.hero.position(), v.frame, v.seq_b));
    }
    rows
}

fn dedup(v: impl Iterator<Item = i32>) -> Vec<i32> {
    let mut o: Vec<i32> = Vec::new();
    for s in v { if o.last() != Some(&s) { o.push(s); } }
    o
}

/// Prints the per-tick trace of the script in `LEDGE_SCRIPT_TRY` (a manual helper, `--ignored --nocapture`).
#[test]
#[ignore]
fn novalis_try_script() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let Ok(s) = std::env::var("LEDGE_SCRIPT_TRY") else { return };
    for (t, r) in run(&n, &s, 700).iter().enumerate() { println!("tick {t:5}: state {:#04x} pos {:?} key {} seq {:#x}", r.0, r.1, r.2, r.3); }
}

/// Walk from the spawn with the stick at `(x, y)` for `walk` ticks (into a wall), stand for 20, then a held jump
/// (✕ for 20 ticks) with the same stick.
fn jump_script(x: f32, y: f32, walk: u32) -> String {
    let j = walk + 21;
    format!("0-{walk}:stick {x:.3} {y:.3},{j}-{}:press X,{j}-{}:stick {x:.3} {y:.3}", j + 19, j + 80)
}

/// Search (prints the scripts that reach a hang; run with `--ignored --nocapture`): 32 stick directions × walk
/// lengths.
#[test]
#[ignore]
fn novalis_find_a_ledge() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let dirs: u32 = std::env::var("LEDGE_DIRS").ok().and_then(|v| v.parse().ok()).unwrap_or(16);
    for k in 0..dirs {
        let a = k as f32 * std::f32::consts::TAU / dirs as f32;
        let (x, y) = (a.sin(), -a.cos());
        for walk in (20..=300).step_by(10) {
            let s = jump_script(x, y, walk);
            let s = format!("{s},{}-{}:press X", walk + 140, walk + 141);
            let rows = run(&n, &s, walk + 260);
            if let Some(t) = rows.iter().position(|r| r.0 == 0x19) {
                let st = dedup(rows.iter().map(|r| r.0));
                eprintln!("{s:?}: hang at tick {t} {:?}, states {st:x?}, end {:?}", rows[t].1, rows.last().map(|r| (r.0, r.1)));
                break;
            }
        }
    }
}

/// Two-leg search (`--ignored --nocapture`): the spawn walk down to the lower walkway (`LEG1`, as the engine's first
/// check run), then 32 stick directions × walk lengths toward a wall, stop, held jump.
#[test]
#[ignore]
fn novalis_find_a_ledge_below() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    const LEG1: &str = "0-230:stick 0.290 -0.957";
    for k in 0..32 {
        let a = k as f32 * std::f32::consts::TAU / 32.0;
        let (x, y) = (a.sin(), -a.cos());
        for walk in (10..=160).step_by(10) {
            let (b, e) = (250, 250 + walk);
            let j = e + 21;
            let s = format!("{LEG1},{b}-{e}:stick {x:.3} {y:.3},{j}-{}:press X,{j}-{}:stick {x:.3} {y:.3},{}-{}:press X", j + 19, j + 80, j + 140, j + 141);
            let rows = run(&n, &s, j + 260);
            if let Some(t) = rows.iter().position(|r| r.0 == 0x19) {
                let st = dedup(rows.iter().map(|r| r.0));
                eprintln!("{s:?}: hang at tick {t} {:?}, states {st:x?}, end {:?}", rows[t].1, rows.last().map(|r| (r.0, r.1)));
                break;
            }
        }
    }
}

/// The ledge the search found (`novalis_find_a_ledge_below`): from the spawn, forward-right off the plateau (over
/// the lift 726's edge) down to the lower walkway (z 40), then right into the low wall at (178, 157) (its top ≈
/// 42.7), stop, and a held jump at it with the stick still pushed: grab 0x18 at the wall, hang 0x19; ✕ at 531:
/// climb 0x1c onto the top and stand. The engine's `RC_PLAY_SCRIPT` runs the same string.
pub const LEDGE_SCRIPT: &str = "0-230:stick 0.290 -0.957,250-370:stick 0.831 -0.556,391-410:press X,391-471:stick 0.831 -0.556,531-532:press X";

#[test]
fn novalis_ledge_grab_and_climb() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let rows = run(&n, LEDGE_SCRIPT, 650);
    let st = dedup(rows.iter().map(|r| r.0));
    let first = |s: i32| rows.iter().position(|r| r.0 == s);
    if std::env::var("LEDGE_TRACE").is_ok() {
        for (t, r) in rows.iter().enumerate() { eprintln!("{t} {:#x} {:?} key {} seq {:#x}", r.0, r.1, r.2, r.3); }
    }
    eprintln!("states {st:x?}; grab {:?} hang {:?} climb {:?}; end {:?}", first(0x18), first(0x19), first(0x1c), rows.last().map(|r| r.1));
    assert!(st.windows(3).any(|w| w == [7, 0x18, 0x19]), "{st:x?}");
    assert!(st.windows(2).any(|w| w == [0x19, 0x1c]), "{st:x?}");
    let hang = &rows[first(0x19).unwrap() + 30];
    let end = rows.last().unwrap();
    assert!(end.1[2] > hang.1[2] + 1.2, "climbed from {:?} to {:?}", hang.1, end.1);
    assert_eq!(end.0, 0, "standing at the end");
}

/// The scripts of `hero_novalis.rs::novalis_hero_digest` (the refactor guard) with the ledges ported: they enter no
/// ledge state; reports which probe fields they write (those make the guard's hash differ from a pre-P3 baseline
/// on the ticks listed, with identical states and positions).
#[test]
fn novalis_digest_scripts_enter_no_ledge_state() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    const LAKE: &str = "0-214:stick 0 -1,215-299:stick 0.5 -0.85,300-399:stick 0.2 -0.98,400-700:stick 0 -1,\
        760-760:press R1,761-820:press R1+SQUARE,761-840:stick 0 -1,821-900:press R1";
    const MOVES: &str = "30-31:press X,50-51:press X,120-180:stick 0 -1,150-151:press X,168-169:press X,\
        240-255:press R1,256-257:press R1+X,256-258:stick 0 1,330-345:press R1,346-352:press R1+X,420-470:stick 1 0,\
        480-481:press X,600-900:stick 0 -1,700-701:press X,980-990:press L1";
    for (name, script, hydro, ticks) in [("lake", LAKE, false, 900), ("lake+pack", LAKE, true, 900), ("moves", MOVES, false, 1000)] {
        let mut t = 0;
        let mut touched = Vec::new();
        let rows = run_with(&n, script, ticks, hydro, &mut |h| {
            if h.ledge_blk != rc_game::hero::LedgeBlock::default() || h.ledge != 0 || h.f540 != 0 || h.f838 != 0 { touched.push(t); }
            t += 1;
        });
        let st = dedup(rows.iter().map(|r| r.0));
        eprintln!("{name}: states {st:x?}; ledge fields non-default on {} ticks (first {:?})", touched.len(), touched.first());
        assert!(!st.iter().any(|s| matches!(s, 0x11 | 0x18..=0x1c)), "{name}: {st:x?}");
    }
}

/// Geometric prefilter + simulation (prints ledges within 60 units of the spawn; `--ignored --nocapture`): standing
/// at a grid point facing one of 8 yaws, a held jump straight ahead (the camera starts behind the hero, so stick
/// forward is the facing).
#[test]
#[ignore]
fn novalis_scan_ledges() {
    use rc_game::collision_query::{coll_line, QueryFlags};
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let mut found = 0;
    for gx in -30..=30 {
        for gy in -30..=30 {
            let (x, y) = (n.spawn[0] + gx as f32 * 2.0, n.spawn[1] + gy as f32 * 2.0);
            // Every floor under the point between z 0 and spawn + 10.
            let mut top = n.spawn[2] + 10.0;
            while top > 1.0 {
                let Some(h) = coll_line(&n.mesh, [x, y, top], [x, y, (top - 60.0).max(0.5)], QueryFlags(2)) else { break };
                let z0 = h.point[2];
                top = z0 - 0.5;
                if h.normal[2] <= 0.0 { continue; }
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::TAU / 8.0;
                    let (c, s) = (a.cos(), a.sin());
                    // A flat top 1.9..3.8 above, 0.9..1.4 ahead, and a wall in front of it.
                    let q = [x + c * 1.2, y + s * 1.2];
                    let Some(t) = coll_line(&n.mesh, [q[0], q[1], z0 + 3.9], [q[0], q[1], z0 + 1.9], QueryFlags(4)) else { continue };
                    let tz = t.point[2];
                    if coll_line(&n.mesh, [x, y, z0 + 1.0], [q[0], q[1], z0 + 1.0], QueryFlags(2)).is_none() { continue; }
                    let _ = tz;
                    let rows = run_from(&n, [x, y, z0 + 0.3], a, "0-20:press X,0-80:stick 0 -1", 90, false, &mut |_| {});
                    if let Some(i) = rows.iter().position(|r| r.0 == 0x19) {
                        found += 1;
                        eprintln!("ledge: from {:?} yaw {a:.3} (floor {z0}): hang at {:?}, top {tz} surface {}", [x, y], rows[i].1, t.surface_id());
                    }
                }
            }
        }
    }
    eprintln!("{found} spots");
}
