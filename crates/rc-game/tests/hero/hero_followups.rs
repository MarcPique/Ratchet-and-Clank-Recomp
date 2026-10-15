//! Hero follow-ups (docs/plan/hero_states.md "Hero follow-ups"), headless on the levels' own data: the flow class
//! 679 carries Ratchet down the sinking-floor chutes through the hero-block write channel
//! (`moby_update::services::HeroFields`), the follow camera swings behind a hanging Ratchet, the moby-ledge flag and
//! `HeroOnMoby`'s ledge branch. The level runs as the engine ticks it (the loader's static mobys with the headerless
//! classes, the scheduler's load pass and moby loop, the moby collision; `hero_platform_novalis.rs`'s harness).
//! Skipped when `extracted/` (the extracted game data) is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level};
use rc_game::collision_query::{coll_line, QueryFlags};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::platform::{Carriers, HeroWorld};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, ClassInfo, Moby, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{self, class_info, load_static_mobys, Groups, Scheduler};
use rc_game::moby_update::services::{pvar, SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

struct Lv {
    n: u32,
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    spawnable: usize,
    death_z: f32,
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    ratchet: MobyAnimClass,
}

fn load(n: u32) -> Option<Lv> {
    let dir = rc_formats::test_data::root().join(format!("levels/{n:02}"));
    let data = rc_formats::test_data::core_data(n)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(n)?;
    let settings = rc_formats::test_data::gameplay_section(n, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let splines = gameplay::parse_splines(&gp).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(n, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            // A slot without a class blob (the flow 679 has no geometry): no header, the slot's update (as the
            // engine's `class_table`).
            let info = ClassInfo { slot: slot as u8, no_header: true, update_fn: scheduler::port_update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    let blob = rc_formats::test_data::core_block(n, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(n, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    Some(Lv { n, mesh, instances, pvars, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet })
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
                _ => panic!("button {n}"),
            }).fold(0, |m, b| m | b)),
            _ => panic!("action {action}"),
        };
    }
    p
}

/// Runs level `lv` with its mobys for `ticks` ticks of `script`, Ratchet at `at` (position, yaw) instead of his
/// spawn when given; `each(tick, game)` after every tick.
fn run(lv: &Lv, at: Option<([f32; 3], f32)>, script: &str, ticks: u32, each: &mut dyn FnMut(u32, &Game)) {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let mut statics = load_static_mobys(&lv.instances, &mut ct, &lv.pvars);
    let hero_idx = statics.iter().position(|m| m.o_class == 0).expect("Ratchet");
    if let Some((p, yaw)) = at {
        statics[hero_idx].position = [p[0], p[1], p[2], 1.0];
        statics[hero_idx].rotation = [0.0, 0.0, yaw, 0.0];
    }
    let mut table = MobyTable::new(statics, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = lv.n as i32;
    let mut svc = Services::new();
    svc.level = lv.n;
    svc.set_splines(&lv.splines);
    svc.groups = Groups::parse(&lv.gp, &|i| (i < lv.instances.len()).then_some(i));
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    let mut sched = Scheduler::new();
    {
        let hero: Hero = game.hero.clone();
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&lv.mesh);
        sched.load_pass(&mut w);
    }
    game.finish_load();
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let svc_cell = std::cell::RefCell::new(&mut svc);
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
        game.hero.idle.counter = game.counter as i32;
        let r = game.tick(Some(&script_input(script, t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        if r.hero == rc_game::hero::HeroTick::OutOfBounds { break; }
        each(t, &game);
    }
}

/// The surface id of the ground straight below `p` (a line from 2 above to 3 below), if any.
fn surface_below(mesh: &collision::Collision, p: [f32; 3]) -> Option<(i32, f32)> {
    coll_line(mesh, [p[0], p[1], p[2] + 2.0], [p[0], p[1], p[2] - 3.0], QueryFlags(2)).map(|h| (h.surface_id(), h.point[2]))
}

/// The 679 instances' splines of a level: (instance, spline indices).
fn flow_splines(lv: &Lv) -> Vec<(usize, Vec<usize>)> {
    lv.instances.iter().enumerate().filter(|(_, m)| m.o_class == 679).filter_map(|(k, m)| {
        let pv = lv.pvars.get(usize::try_from(m.pvar_index).ok()?)?.as_ref()?;
        let n = pvar::i32(pv, 0x70).clamp(0, 24) as usize;
        let s: Vec<usize> = (0..n).filter_map(|j| usize::try_from(pvar::i32(pv, 4 * j)).ok()).collect();
        (!s.is_empty()).then_some((k, s))
    }).collect()
}

/// The first point of spline `s` (from `from` on) with the sinking floor (surface 4) right below it.
fn chute_start(lv: &Lv, s: usize, from: usize) -> Option<(usize, [f32; 3])> {
    lv.splines[s].iter().enumerate().skip(from).find_map(|(k, q)| {
        let (id, z) = surface_below(&lv.mesh, [q[0], q[1], q[2]])?;
        (id == 4).then_some((k, [q[0], q[1], z]))
    })
}

/// One tick's record of a flow run.
#[derive(Clone, Copy, Debug)]
struct Rec {
    state: i32,
    pos: [f32; 3],
    hold: i16,
    on4: u8,
}

fn flow_run(lv: &Lv, at: [f32; 3], yaw: f32, ticks: u32) -> Vec<Rec> {
    let mut out = Vec::new();
    run(lv, Some((at, yaw)), "", ticks, &mut |_, g| {
        let h = &g.hero;
        out.push(Rec { state: h.state, pos: h.position(), hold: h.f530, on4: h.surf.f0633 });
    });
    out
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }

fn dedup(v: impl Iterator<Item = i32>) -> Vec<i32> {
    let mut o: Vec<i32> = Vec::new();
    for s in v { if o.last() != Some(&s) { o.push(s); } }
    o
}

/// Prints where the flow splines of levels 1, 5, 8, 15 run over the sinking floor (`--ignored --nocapture`).
#[test]
#[ignore]
fn flow_chutes_explore() {
    for n in [1u32, 5, 8, 15] {
        let Some(lv) = load(n) else { continue };
        for (k, ss) in flow_splines(&lv) {
            for s in ss {
                let pts = &lv.splines[s];
                let on: Vec<usize> = (0..pts.len()).filter(|&j| surface_below(&lv.mesh, [pts[j][0], pts[j][1], pts[j][2]]).is_some_and(|x| x.0 == 4)).collect();
                eprintln!("level {n} instance {k} spline {s}: {} points, {} over surface 4 (first {:?}, last {:?}); ends {:?} → {:?}", pts.len(), on.len(), on.first(), on.last(), &pts[0][..3], &pts[pts.len() - 1][..3]);
            }
        }
    }
}

/// Novalis' chute (spline 43 of the flow 679): dropped onto its sinking floor upstream, Ratchet sinks (0x31) and
/// is carried along the spline down to the landing plateau, never frozen, and stands up (idle) once off the chute
/// and 30 ticks after the flow's last push (its hold 0x13f530). Before the flow class was ported he stayed in 0x31
/// where he landed (`hero_surfaces.rs::novalis_flow_surface_is_the_sinking_floor`).
#[test]
fn novalis_flow_carries_ratchet_down_the_chute() {
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/levels/01"); return };
    let s = &lv.splines[43];
    let end = [s[s.len() - 1][0], s[s.len() - 1][1], s[s.len() - 1][2]];
    let (k, p) = chute_start(&lv, 43, 120).expect("the chute of spline 43 over surface 4");
    let recs = flow_run(&lv, [p[0], p[1], p[2] + 0.5], 0.0, 1500);
    let st = dedup(recs.iter().map(|r| r.state));
    let first = recs.iter().position(|r| r.state == 0x31).expect("sinking floor");
    let last = recs.iter().rposition(|r| r.state == 0x31).unwrap();
    eprintln!("start point {k} {p:?}; states {st:x?}; 0x31 ticks {first}..={last}; at {:?} → {:?}; end {:?}", recs[first].pos, recs[last].pos, recs.last().map(|r| (r.state, r.pos)));
    for t in (first..recs.len()).step_by(60) { eprintln!("  tick {t}: {:?}", recs[t]); }
    // Carried: from the start to the chute's end, far along the spline.
    let d0 = dist2(recs[first].pos, end);
    let d1 = dist2(recs[last].pos, end);
    assert!(d0 > 15.0 && d1 < 0.25 * d0, "carried from {d0} to {d1} of the end {end:?}");
    // Never frozen while held: every 20-tick window of 0x31 with the hold on moves him.
    for t in first..last.saturating_sub(20) {
        if recs[t..t + 20].iter().all(|r| r.state == 0x31 && r.hold > 0) {
            assert!(dist2(recs[t].pos, recs[t + 20].pos) > 0.05, "frozen at tick {t}: {:?}", recs[t]);
        }
    }
    // The hold: 30 on every pushing tick, and 0x31 ends exactly when it has run out off the sinking floor.
    assert!(recs[first + 5..last - 40].iter().all(|r| r.hold >= 29), "held while the flow pushes");
    let exit = &recs[last + 1];
    assert_eq!(exit.state, 0, "stands up after the chute: {st:x?}");
    // (Recorded after each tick: the post-move timers count 0x13f530 down before the transitions read it, so the last
    // 0x31 tick shows 1 left and the exit tick 0.)
    assert_eq!((recs[last].hold, recs[last].on4), (1, 0), "0x31 ends when off the floor with the hold run out: {:?}", recs[last]);
    assert_eq!((exit.hold, exit.on4), (0, 0));
    assert!(dist2(exit.pos, end) < 8.0, "ends near the chute's foot {end:?}: {:?}", exit.pos);
}

/// The flow on the other levels that have it (Rilgar 5, Blarg 8, Orxon 15; the same function, clusters.tsv): each
/// chute moves Ratchet along its spline. Printed per level; asserted where the spline runs over surface 4.
#[test]
fn flow_chutes_other_levels() {
    for n in [5u32, 8, 15] {
        let Some(lv) = load(n) else { eprintln!("skipped: no extracted/levels/{n:02}"); continue };
        let mut tried = 0;
        for (inst, ss) in flow_splines(&lv) {
            for s in ss {
                let pts = &lv.splines[s];
                let end = [pts[pts.len() - 1][0], pts[pts.len() - 1][1], pts[pts.len() - 1][2]];
                // The first spline point over surface 4 where a dropped Ratchet lands on the sinking floor.
                let mut found = None;
                let mut from = 0;
                while let Some((k, p)) = chute_start(&lv, s, from) {
                    let recs = flow_run(&lv, [p[0], p[1], p[2] + 0.5], 0.0, 600);
                    if let Some(first) = recs.iter().position(|r| r.state == 0x31) {
                        found = Some((k, p, recs, first));
                        break;
                    }
                    from = k + 3;
                }
                let Some((k, p, recs, first)) = found else {
                    eprintln!("level {n} instance {inst} spline {s}: no drop onto its sinking floor");
                    continue;
                };
                let st = dedup(recs.iter().map(|r| r.state));
                let moved = dist2(recs[first].pos, recs.last().unwrap().pos);
                eprintln!("level {n} instance {inst} spline {s} point {k} {p:?}: states {st:x?}, moved {moved:.2}, end {:?} (spline end {end:?})", recs.last().map(|r| (r.state, r.pos)));
                assert!(moved > 2.0, "level {n}: the flow did not carry him");
                tried += 1;
            }
        }
        eprintln!("level {n}: {tried} chutes run");
    }
}

/// The ledge script of `hero_ledge_novalis.rs` (grab at ~431, hang from ~448, ✕ at 531 climbs).
const LEDGE_SCRIPT: &str = "0-230:stick 0.290 -0.957,250-370:stick 0.831 -0.556,391-410:press X,391-471:stick 0.831 -0.556,531-532:press X";

fn wrap(a: f32) -> f32 { (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI }

/// The camera swings behind a hanging Ratchet (`CamType0HeroStateTweaks` 0x3111d8's ledge branch): while he hangs the
/// camera's yaw rate is 12°/tick with the scripted yaw input, so the angle between the camera's forward and his
/// facing (into the wall, ledge yaw + π) shrinks to a few degrees.
#[test]
fn novalis_ledge_camera_swings_behind() {
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/levels/01"); return };
    let mut rows = Vec::new();
    run(&lv, None, LEDGE_SCRIPT, 540, &mut |t, g| {
        let facing = wrap(g.hero.ledge_blk.yaw + std::f32::consts::PI);
        let f = g.camera.out.rows[0];
        let cam = f[1].to_f32().atan2(f[0].to_f32());
        rows.push((t, g.hero.state, g.hero.f15d4, wrap(cam - facing), g.camera.cam.script_yaw.to_f32(), g.camera.cam.look_from_s));
    });
    let st = dedup(rows.iter().map(|r| r.1));
    let hang = rows.iter().position(|r| r.1 == 0x19).unwrap_or_else(|| panic!("no hang: {st:x?}"));
    for r in rows[hang.saturating_sub(20)..].iter().step_by(8) { eprintln!("tick {} state {:#x} 15d4 {:#x} cam − facing {:.1}° script yaw {:.3} look {}", r.0, r.1, r.2, r.3.to_degrees(), r.4, r.5); }
    let grab = rows.iter().position(|r| r.1 == 0x18).unwrap();
    let a0 = rows[grab].3.abs();
    // The ledge states set 0x1415d4 = 0xd, the tweak's condition (the camera's own reset clears its fields at the
    // end of each update, so they read 0 here).
    let ledge: Vec<_> = rows.iter().filter(|r| r.2 == 0xd).collect();
    assert!(!ledge.is_empty());
    let a1 = rows[hang + 40].3.abs();
    eprintln!("grab tick {grab} ({:.1}°), hang tick {hang}, 40 ticks later {:.1}°; states {st:x?}", a0.to_degrees(), a1.to_degrees());
    assert!(a1 < 10f32.to_radians(), "the camera is behind him: {:.1}°", a1.to_degrees());
    assert!(a0 > a1 + 10f32.to_radians(), "it swung ({:.1}° → {:.1}°)", a0.to_degrees(), a1.to_degrees());
}

/// Probe B's moby ledge flag through `HeroWorld`: a mode-0x20 moby whose pvar record (pvar+0x00, block-relative) has
/// bit 0 of its u16 at +0x1e; not without mode 0x20 or without the bit.
#[test]
fn moby_ledge_flag_from_the_pvar_record() {
    let rec = |mode_bits: u16, flag: u16| {
        let mut m = Moby::zeroed();
        m.mode = mode_bits;
        m.pvars = vec![0; 0x60];
        pvar::set_i32(&mut m.pvars, 0, 0x20);
        m.pvars[0x3e..0x40].copy_from_slice(&flag.to_le_bytes());
        m
    };
    let table = MobyTable::new(vec![Moby::zeroed(), rec(0x20, 1), rec(0x20, 2), rec(0, 1), rec(0x20, 0x8001)], 0);
    let w = Carriers::collect(&table, 0);
    assert_eq!((1..5).map(|i| w.moby_ledge_flag(i)).collect::<Vec<_>>(), vec![true, false, false, true]);
    assert!(!w.moby_ledge_flag(0));
}

/// `HeroOnMoby` 0x277fb8: in the ledge group (3) or the climb (0x1c) he is on the ledge moby 0x13f848; otherwise on
/// his ground moby while grounded.
#[test]
fn hero_on_moby_ledge_branch() {
    let mut table = MobyTable::new(vec![Moby::zeroed(), Moby::zeroed(), Moby::zeroed()], 0);
    let classes = ClassTable::default();
    let mut svc = Services::new();
    let mut rng = Rng::new();
    let mut h = Hero::spawn([10.0, 10.0, 10.0], 0.0);
    h.ground_moby = Some(2);
    h.air_ticks = 0;
    h.ledge_blk.moby = Some(1);
    h.group = 3;
    {
        let w = World::new(&mut table, &h, &mut rng, &classes, &mut svc, 1);
        assert!(w.hero_on_moby(1) && !w.hero_on_moby(2));
    }
    h.group = 4;
    h.state = 0x1c;
    {
        let w = World::new(&mut table, &h, &mut rng, &classes, &mut svc, 1);
        assert!(w.hero_on_moby(1) && !w.hero_on_moby(2));
    }
    h.group = 0;
    h.state = 0;
    let w = World::new(&mut table, &h, &mut rng, &classes, &mut svc, 1);
    assert!(!w.hero_on_moby(1) && w.hero_on_moby(2));
}

/// The flow's registration: class 679 maps to the port on every level (the registry is by class; levels 1, 5, 8,
/// 15 compile the same function) and the four levels have instances with splines.
#[test]
fn flow_registered_for_its_levels() {
    assert_eq!(scheduler::port_update_fn(679), Some(rc_game::moby_update::classes::flow::UPDATE_FN));
    for (n, _) in rc_game::moby_update::classes::flow::LEVEL_FNS {
        let Some(lv) = load(n) else { continue };
        assert!(!flow_splines(&lv).is_empty(), "level {n} has a 679 with splines");
    }
}
