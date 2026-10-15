//! Ratchet's posed shadow list (`rc_game::shadows::volume::pose_prims`, *BuildShadowList* 0x29b948) against the
//! game's, distilled from PCSX2 RAM at the Novalis idle savestate (`fixtures/shadow_ratchet_novalis_idle.tsv`:
//! his animation keys, rows, position, scale and size word as inputs; the 21 posed records as the expected
//! output). The pose comes from the port's joint evaluator (`moby_anim::evaluate_chains`, the joints' frames in
//! model space) on his class and the level's `ratchet_seq` table. Settles docs/plan/shadows.md §7 items 2 and 3:
//! the radius travels in w and is scaled by `scale·size/(1024·4096)`, and the shadow uses the posed joint matrices
//! (rotation included), not the skinning palette. Skipped without `extracted/`.
//!
//! The fixture also carries Ratchet's runtime joint-modifier list (moby +0x64) at the savestate: the eyelid nodes of
//! his blink and the head-look / idle joint records. The port rebuilds that list from its own sources
//! (`rc_game::hero::idle`: the records' Euler angles through `FUN_0026ee30`, the blink tables, the node order) and
//! poses with it (`moby_anim::evaluate_chains_posed`), so every record matches, head, ears and arms included.

use rc_formats::moby_anim::{self, AnimState, JointModifier, MobyAnimClass};
use rc_formats::moby_shadow::ShadowBlock;
use rc_game::shadows::volume::{self, CasterPose};

/// One fixture primitive: kind, segments, end A, end B.
type Prim = (u8, [i32; 2], [f32; 4], [f32; 4]);

struct Fixture {
    anim: (u8, u8, u8, u8, f32),
    rows: [[f32; 3]; 3],
    position: [f32; 3],
    scale: f32,
    size: u16,
    slab: [f32; 2],
    dir: [f32; 3],
    dir0: [f32; 3],
    pitch: f32,
    prims: Vec<Prim>,
    mods: Vec<ModRow>,
}

/// One node of the modifier list: blink (true) or joint record, its index (eyelid k / record r), joint list, target
/// joint, mode, weight, blink frame or Euler angles, and the node as RAM holds it.
struct ModRow {
    blink: bool,
    index: usize,
    list: usize,
    target: u8,
    node: JointModifier,
    frame: i32,
    euler: [f32; 3],
}

fn fixture() -> Fixture {
    let text = include_str!("../fixtures/shadow_ratchet_novalis_idle.tsv");
    let mut fx = Fixture { anim: (0, 0, 0, 0, 0.0), rows: [[0.0; 3]; 3], position: [0.0; 3], scale: 0.0, size: 0, slab: [0.0; 2], dir: [0.0; 3], dir0: [0.0; 3], pitch: 0.0, prims: Vec::new(), mods: Vec::new() };
    let mut row = 0;
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let v: Vec<&str> = line.split('\t').collect();
        let f = |k: usize| v[k].parse::<f32>().unwrap();
        let f3 = |k: usize| [f(k), f(k + 1), f(k + 2)];
        let f4 = |k: usize| [f(k), f(k + 1), f(k + 2), f(k + 3)];
        match v[0] {
            "anim" => fx.anim = (v[1].parse().unwrap(), v[2].parse().unwrap(), v[3].parse().unwrap(), v[4].parse().unwrap(), f(5)),
            "row" => {
                fx.rows[row] = f3(1);
                row += 1;
            }
            "position" => fx.position = f3(1),
            "scale" => fx.scale = f(1),
            "size" => fx.size = v[1].parse().unwrap(),
            "slab" => fx.slab = [f(1), f(2)],
            "dir" => fx.dir = f3(1),
            "dir0" => fx.dir0 = f3(1),
            "pitch" => fx.pitch = f(1),
            "prim" => fx.prims.push((v[1].parse().unwrap(), [v[2].parse().unwrap(), v[3].parse().unwrap()], f4(4), f4(8))),
            "mod" => {
                let i = |k: usize| v[k].parse::<i32>().unwrap();
                let target = i(4) as u8;
                let node = JointModifier { joint: target, mode: i(5) as u8, weight: f(6), quat: f4(10), scale: f3(14), trans: f3(17) };
                let blink = v[1] == "blink";
                fx.mods.push(ModRow { blink, index: i(2) as usize, list: i(3) as usize, target, node, frame: if blink { i(7) } else { 0 }, euler: if blink { [0.0; 3] } else { f3(7) } });
            }
            k => panic!("fixture key {k}"),
        }
    }
    fx
}

/// Ratchet's class 0 and his animation class with the level's `ratchet_seq` sequences (as the engine builds it), and
/// the second byte lists of his joint lists (the modifier targets).
fn ratchet() -> Option<(ShadowBlock, MobyAnimClass, Vec<Vec<u8>>)> {
    let c = rc_formats::test_data::core(1)?;
    let blob = c.block("moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(blob).unwrap();
    let block = ShadowBlock::of_class(blob).unwrap().unwrap();
    let seqs = (0..c.core.ratchet_seqs.len())
        .map(|i| c.block(&format!("ratchet_seq/{i:03}")).and_then(|b| moby_anim::parse_sequence(b, 0).ok()))
        .collect();
    let seconds = (0..64).map(|l| rc_formats::gadget::joint_list(blob, &class.header, l).map(|(_, b)| b).unwrap_or_default()).collect();
    Some((block, MobyAnimClass::new(&class, seqs), seconds))
}

/// Ratchet's modifier list as the port builds it (`rc_game::hero::idle`): the fixture's nodes by kind and order, the
/// records' angles and scales, the blink frame; checked node by node against RAM on the way.
fn port_modifiers(fx: &Fixture, seconds: &[Vec<u8>]) -> Vec<JointModifier> {
    use rc_game::hero::idle::{Idle, Manip, BLINK_NODES};
    let mut idle = Idle::new();
    for m in &fx.mods {
        // AttachManipulator's target: the list's second byte list, first entry.
        assert_eq!(moby_anim::list_target(&seconds[m.list]), Some(m.target), "list {} target", m.list);
        if m.blink {
            idle.blink = m.frame;
            assert_eq!(BLINK_NODES[m.index].0 as usize, m.list);
            idle.manips.push(Manip::Blink(m.index as u8));
        } else {
            let k = idle.joints.iter().position(|r| r.rec as usize == m.index).unwrap_or_else(|| panic!("record {} not modelled", m.index));
            let r = &mut idle.joints[k];
            assert_eq!(r.joint as usize, m.list);
            r.cur = m.euler;
            r.node_scale = m.node.scale[0];
            idle.manips.push(Manip::Rec(k as u8));
        }
    }
    let targets: Vec<u8> = seconds.iter().map(|b| moby_anim::list_target(b).unwrap_or(0xff)).collect();
    let mods = idle.modifiers(&targets);
    assert_eq!(mods.len(), fx.mods.len());
    for (p, m) in mods.iter().zip(&fx.mods) {
        let d = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max);
        let n = &m.node;
        assert_eq!((p.joint, p.mode), (n.joint, n.mode));
        assert!((p.weight - n.weight).abs() < 1e-7 || n.mode == 0, "weight {} vs {}", p.weight, n.weight);
        // FUN_0026ee30 from the record's angles (VU0 sin / cos vs std: a few 1e-7).
        assert!(d(&p.quat, &n.quat) < 2e-6, "{} {}: quat {:?} vs RAM {:?}", if m.blink { "eyelid" } else { "record" }, m.index, p.quat, n.quat);
        assert!(d(&p.scale, &n.scale) < 1e-7 && d(&p.trans, &n.trans) < 1e-4, "node {}: {:?} vs {:?}", m.index, p, n);
    }
    mods
}

#[test]
fn ratchet_posed_list_matches_the_game() {
    let Some((block, anim, seconds)) = ratchet() else {
        eprintln!("skipped: no extracted/");
        return;
    };
    let fx = fixture();
    assert_eq!(block.prims.len(), fx.prims.len());
    let mods = port_modifiers(&fx, &seconds);
    let (seq_a, frame_a, seq_b, frame_b, t) = fx.anim;
    let state = AnimState { seq_a, frame_a, seq_b, frame_b, t, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
    let chains: Vec<Vec<u8>> = (0..anim.joint_count).map(|j| (0..=j as u8).collect()).collect();
    let refs: Vec<&[u8]> = chains.iter().map(Vec::as_slice).collect();
    let joints = moby_anim::evaluate_chains_posed(&anim, &state, None, &refs, &[], &mods);
    let pose = CasterPose { joints: &joints, rows: fx.rows, position: fx.position, scale: fx.scale, size: fx.size };
    let got = volume::pose_prims(&block, &pose);
    // Without the modifier list (the port before it): the head, ear and arm records are off.
    let plain = moby_anim::evaluate_chains(&anim, &state, None, &refs);
    let before = volume::pose_prims(&block, &CasterPose { joints: &plain, ..pose });
    // The joints the modifier list acts on (the eyelids 18, 21..24, 49, 50; the head look / idle records 2, 7, 8, 18)
    // and every joint below them.
    let modified: Vec<usize> = mods.iter().map(|m| m.joint as usize).collect();
    let under = |mut j: usize| loop {
        if modified.contains(&j) { break true; }
        match anim.parent(j) { Some(p) if p != j => j = p, _ => break false }
    };
    let (mut worst, mut worst_r, mut worst_layer, mut worst_before, mut layered) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0);
    for (k, ((g, b), (ty, seg, a, bb))) in got.iter().zip(&before).zip(&fx.prims).enumerate() {
        assert_eq!((g.capsule as u8, g.segments), (*ty, *seg), "record {k}: type / segments (negative counts become 0)");
        let under_list = block.prims[k].joints().iter().any(|&j| under(j as usize));
        if under_list { layered += 1; }
        for (p, pb, q) in [(g.a, b.a, a), (g.b, b.b, bb)] {
            let d = (0..3).map(|c| (p[c] - q[c]).abs()).fold(0.0, f32::max);
            let db = (0..3).map(|c| (pb[c] - q[c]).abs()).fold(0.0, f32::max);
            worst = worst.max(d);
            if under_list { worst_layer = worst_layer.max(d); worst_before = worst_before.max(db); }
            worst_r = worst_r.max((p[3] - q[3]).abs());
        }
    }
    eprintln!(
        "{} records: largest point difference {worst:.2e} units; the {layered} under the modifier list {worst_layer:.2e} (without the list: {worst_before:.3}); radius {worst_r:.2e}",
        got.len()
    );
    // Radii are exact up to float rounding: w only goes through ×k.
    assert!(worst_r < 1e-6, "radius {worst_r}");
    assert!(layered >= 10, "head, ears and arms: {layered}");
    // Points, all 21 records: the port's evaluator and the game's agree (hardware_fidelity_layers.md "Tolerances").
    assert!(worst < TOLERANCE, "largest point difference {worst}");
    // Without the list the head, ears and arms were off by up to 0.18 units.
    assert!(worst_before > 0.1, "{worst_before}");
    // The slab and direction 1 the fixture carries are the ones the unit tests of rc_game::shadows reproduce.
    assert!((fx.slab[0] - 59.88).abs() < 1e-5 && (fx.slab[1] - 60.24).abs() < 1e-5);
    assert!((fx.dir.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-5);
}

/// Accepted point difference (world units): hardware_fidelity_layers.md "Tolerances".
const TOLERANCE: f32 = 1e-4;

/// Directions 0 and 1 from Ratchet's light (light word 0: set 0's light A, the level's gameplay light bank) and the
/// settled pitch 0.97, against RAM 0x1af000 / 0x1af010; his slab on the flat spawn ground (59.88 / 60.24).
#[test]
fn directions_and_slab_match_the_game() {
    use rc_game::moby_runtime::Moby;
    use rc_game::shadows::{self, HeroShadowIn, ShadowDirs};
    let Some(g) = rc_formats::test_data::gameplay(1) else {
        eprintln!("skipped: no extracted/");
        return;
    };
    let fx = fixture();
    let bank = rc_formats::tfrag_light::parse_light_bank(&g).unwrap();
    let dir_a: Vec<[f32; 4]> = bank.sets.iter().map(|s| s.dir_a).collect();
    let light = shadows::light_dir(&dir_a, 0);
    let mut dirs = ShadowDirs::default();
    shadows::update_shadow_dir(&mut dirs, light);
    let mut m = Moby::zeroed();
    m.position = [fx.position[0], fx.position[1], fx.position[2], 0.0];
    m.bsphere = [fx.position[0] * 1024.0, fx.position[1] * 1024.0, (fx.position[2] + 0.6) * 1024.0, 0.85 * 1024.0];
    let mut pitch = fx.pitch;
    let h = HeroShadowIn { ground_z: fx.position[2], ..Default::default() };
    shadows::update_hero_shadow(&mut m, &mut pitch, &mut dirs, &h, light, |_, _| Some(60.0));
    let close = |a: [f32; 4], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 2e-6);
    assert!(close(dirs.0[0], fx.dir0), "direction 0 {:?} vs RAM {:?}", dirs.0[0], fx.dir0);
    assert!(close(dirs.0[1], fx.dir), "direction 1 {:?} vs RAM {:?}", dirs.0[1], fx.dir);
    assert!((m.shadow_lo - fx.slab[0]).abs() < 1e-5 && (m.shadow_hi - fx.slab[1]).abs() < 1e-5);
}
