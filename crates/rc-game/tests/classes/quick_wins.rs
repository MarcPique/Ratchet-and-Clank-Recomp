//! The W2 lane-2 quick wins on the disc's data (docs/plan/level_scripting.md §8, gaps G-AUD-007, G-AUD-010,
//! G-LVL-008, G-REN-020): skipped when `extracted/` is absent.
//! * G-AUD-007: level04's `0x27eca0` is a copy of `PlayLevelSoundAtMoby` (level01 0x2a1770), not a new system;
//! * G-LVL-008: the height grid (core +0xa4) on its three levels, read as `0x278020` reads it;
//! * G-AUD-010: the laser fences 838 on Rilgar, one looping voice per group handed along by the slot rewrite.

use rc_formats::level::HeightGrid;
use rc_formats::level_overlay::Relocation;
use rc_formats::{gameplay, moby_spawn};
use rc_game::moby_runtime::{MobyId, MobyTable};
use rc_game::moby_update::classes::units::{self, laser_fence};
use rc_game::moby_update::classes::ClassUpdate;
use rc_game::moby_update::services::{pvar as p, SoundEvent, SoundSink, World};
use rc_game::moby_update::scheduler::load_level_mobys;
use rc_game::moby_update::{ClassTable, Services};
use rc_game::rng::Rng;

/// G-AUD-007: the census's "level sound on a slot" (level04 `0x27eca0`, the 466 family's skill point) is the level's
/// own copy of `PlayLevelSoundAtMoby` 0x2a1770 (same masked code): ported (`World::play_level_sound`).
#[test]
fn eudora_level_sound_is_play_level_sound_at_moby() {
    let (Some(r), Some(t)) = (crate::common::overlay(1), crate::common::overlay(4)) else { eprintln!("skipped"); return };
    let copies = Relocation::new(&r, &t).copies(0x2a_1770);
    assert!(copies.contains(&0x27_eca0), "copies {copies:x?}");
}

/// G-LVL-008: the grid is on 08 / 12 / 14 only; every cell reads between its two heights, and the corners of the
/// grid read as the game's formula gives them.
#[test]
fn height_grid_on_its_three_levels() {
    for level in 0..19 {
        let Some(core) = rc_formats::test_data::core(level) else { eprintln!("skipped"); return };
        let g = HeightGrid::parse(&core.core.header, &core.data);
        assert_eq!(g.is_some(), matches!(level, 8 | 12 | 14), "level {level:02}");
        let Some(g) = g else { continue };
        assert!(g.width > 0 && g.rows > 0 && g.low < g.high, "level {level:02}: {} × {} {}..{}", g.width, g.rows, g.low, g.high);
        let c0 = g.cells[0] as f32;
        assert_eq!(g.height(0.5, 0.9), Some((g.high - g.low) * (1.0 - c0 / 255.0) + g.low));
        let (x, y) = (g.width - 1, g.rows - 1);
        let h = g.height(x as f32 + 0.99, y as f32).unwrap();
        assert!((g.low - 1e-4..=g.high + 1e-4).contains(&h), "{h}");
        assert_eq!(g.height(0.0, g.rows as f32), None, "past the last row");
        eprintln!("level {level:02}: {} × {}, heights {}..{}", g.width, g.rows, g.low, g.high);
    }
}

/// One voice slot as the audio port keeps it (owner, position), recording the plays.
#[derive(Default)]
struct Slot {
    owner: Option<MobyId>,
    pos: [f32; 3],
    plays: Vec<(i32, u32, MobyId)>,
    handoffs: usize,
}

impl SoundSink for Slot {
    fn play_class_sound(&mut self, ev: &SoundEvent, _rng: &mut Rng) -> i32 {
        self.plays.push((ev.index, ev.flags, ev.moby));
        self.owner = Some(ev.moby);
        self.pos = ev.pos;
        0
    }
    fn slot_owner(&self, slot: i32) -> Option<(MobyId, u16)> { (slot == 0).then_some(self.owner?).map(|o| (o, 0)) }
    fn hand_over(&mut self, _slot: i32, moby: MobyId, pos: [f32; 3]) {
        self.owner = Some(moby);
        self.pos = pos;
        self.handoffs += 1;
    }
}

/// G-AUD-010 on Rilgar: the 26 fences resolve to U183; the fences of a group share one slot word; with the camera at
/// one fence of a group only that fence starts the loop, and when the camera moves to another the slot's owner and
/// position move with it (no second play).
#[test]
fn laser_fences_rilgar_hand_one_voice_along() {
    let (Some(ports), Some(gp)) = (crate::common::ports(5, &[]), rc_formats::test_data::gameplay(5)) else { eprintln!("skipped"); return };
    let i = units::PORTS.iter().position(|u| u.unit == "U183").unwrap() as u16;
    assert!(matches!(ports.get(838), Some(ClassUpdate::Unit(k)) if k == i));
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let mut classes = ClassTable::default();
    let statics = load_level_mobys(&instances, &mut classes, &pvars, &tests);
    let mut table = MobyTable::new(statics.mobys.clone(), 64);
    let mut svc = Services::new();
    svc.pvar_shared = gameplay::parse_pvar_shared_data(&gp).unwrap();
    svc.groups = statics.groups(&gp);
    let fences: Vec<MobyId> = table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == 838).map(|(k, _)| k).collect();
    assert_eq!(fences.len(), 26);
    // The fences of a group share their slot word.
    let word = |t: &MobyTable, k: MobyId| p::i32(&t.mobys[k].pvars, 4);
    let g0 = table.mobys[fences[0]].group;
    let group: Vec<MobyId> = fences.iter().copied().filter(|&k| table.mobys[k].group == g0).collect();
    assert!(group.len() >= 2, "a group of fences");
    assert!(group.iter().all(|&k| word(&table, k) == word(&table, group[0])));
    let hero = rc_game::hero::Hero::new();
    let mut rng = Rng::new();
    let mut sink = Slot::default();
    let mut run = |table: &mut MobyTable, svc: &mut Services, sink: &mut Slot, cam: [f32; 4], tick: u64| {
        let mut w = World::new(table, &hero, &mut rng, &classes, svc, tick);
        w.camera = rc_game::hero::physics::v4(cam[0], cam[1], cam[2] + 1.0);
        w.sound = Some(sink);
        for &k in &group { units::update(&mut w, k, i); }
    };
    let at = |k: MobyId, t: &MobyTable| t.mobys[k].position;
    let c0 = at(group[0], &table);
    run(&mut table, &mut svc, &mut sink, c0, 1);
    assert_eq!(svc.shared_i32(word(&table, group[0])), -1);
    run(&mut table, &mut svc, &mut sink, c0, 2);
    assert_eq!(sink.plays, vec![(0, 4, group[0])], "the nearest fence starts the loop");
    let far = *group.iter().max_by(|&&a, &&b| {
        let d = |k: MobyId| { let q = at(k, &table); let o = at(group[0], &table); (q[0] - o[0]).hypot(q[1] - o[1]) };
        d(a).total_cmp(&d(b))
    }).unwrap();
    let cf = at(far, &table);
    run(&mut table, &mut svc, &mut sink, cf, 3);
    let q = at(far, &table);
    assert_eq!((sink.owner, sink.pos, sink.plays.len()), (Some(far), [q[0], q[1], q[2]], 1), "handed over, not replayed");
    assert!(sink.handoffs >= 1);
    // One callback per tick; its quads: 10 per lit fence of the group.
    let regs: Vec<_> = svc.draw_callbacks.list1.iter().filter(|(c, _)| matches!(c, rc_game::moby_update::classes::draw_callbacks::Callback::UnitQuads(_))).collect();
    assert_eq!(regs.len(), 2, "ticks 2 and 3 (tick 1 only arms)");
    let quads = units::fx_quads(&table, &svc, i, regs[0].1).unwrap();
    let lit = svc.groups.lists[g0 as u8 as usize].as_ref().unwrap().iter().filter(|&&e| { let m = &table.mobys[(e & 0x7fff) as usize]; m.o_class == 838 && m.state == 1 }).count();
    assert_eq!(quads.quads.len(), 10 * lit);
    assert_eq!((quads.fx, quads.additive), (laser_fence::FX, true));
    eprintln!("Rilgar fences: {} in group {g0} of {}; {} lit", group.len(), fences.len(), lit);
}
