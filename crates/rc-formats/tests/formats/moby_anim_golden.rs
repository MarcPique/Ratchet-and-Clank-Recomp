//! Moby animation against the retail data (docs/plan/moby_animation.md §1, §7). Skipped when
//! `extracted/` is absent. Uses the level core blocks `moby_class/NNNN` and `ratchet_seq/NNN`
//! (`rc_formats::test_data::core`).

use rc_formats::moby::parse_moby_class;
use rc_formats::moby_anim::{self, evaluate, parse_sequence, parse_sequences, AnimState, MobyAnimClass};
use std::path::PathBuf;

fn extracted() -> Option<PathBuf> {
    let root = rc_formats::test_data::root();
    root.join("toc.bin").exists().then_some(root)
}

fn load(_root: &std::path::Path, level: u32, class: u32) -> MobyAnimClass {
    let blob = rc_formats::test_data::core_block(level, &format!("moby_class/{class:04}")).unwrap();
    let c = parse_moby_class(&blob).unwrap();
    let seqs = parse_sequences(&blob, &c).unwrap();
    MobyAnimClass::new(&c, seqs)
}

fn at(seq: u8, a: u8, b: u8, t: f32) -> AnimState {
    AnimState { seq_a: seq, frame_a: a, seq_b: seq, frame_b: b, t, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false }
}

#[test]
fn every_sequence_parses_with_consistent_headers() {
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let (mut classes, mut seqs, mut frames, mut pairs, mut ratchet, mut same_time) = (0, 0, 0, 0, 0, 0);
    let _ = root;
    for level in 0..19 {
        let Some(core) = rc_formats::test_data::core(level) else { continue };
        for b in core.core.blocks.iter().filter(|b| b.name.starts_with("moby_class/")) {
            let blob = core.block(&b.name).unwrap();
            let c = parse_moby_class(blob).unwrap();
            let s = parse_sequences(blob, &c).unwrap_or_else(|err| panic!("level {level} {}: {err}", b.name));
            let a = MobyAnimClass::new(&c, s);
            classes += 1;
            for j in 0..a.joint_count {
                // Parents come first (the chain runs in index order) or the word is 0 (root).
                if a.parent_word[j] != 0 { assert!(a.parent(j).is_some_and(|p| p < j), "level {level}: parent word {:#x} of joint {j}", a.parent_word[j]); }
            }
            for q in a.sequences.iter().flatten() {
                seqs += 1;
                frames += q.frames.len();
                assert_eq!(q.frames.len(), q.header.frame_count as usize);
                // rate · Δtime = 8 on every consecutive key pair.
                for w in q.frames.windows(2) {
                    if w[1].header.time == w[0].header.time { same_time += 1; continue; }
                    let dt = w[1].header.time as f32 - w[0].header.time as f32;
                    assert!((w[0].header.rate * dt - 8.0).abs() < 1e-3, "level {level}: rate {} dt {dt}", w[0].header.rate);
                    pairs += 1;
                }
            }
        }
        for b in core.core.blocks.iter().filter(|b| b.name.starts_with("ratchet_seq/")) {
            let q = parse_sequence(core.block(&b.name).unwrap(), 0).unwrap();
            assert!(q.frames.iter().all(|f| f.quats.len() == 111), "Ratchet sequences carry 111 joints");
            ratchet += 1;
        }
    }
    eprintln!("{classes} classes, {seqs} sequences, {frames} frames, {pairs} key pairs with rate·Δtime = 8, {same_time} pairs with Δtime = 0, {ratchet} ratchet_seq blobs");
    assert!(classes > 0 && frames > 0 && ratchet > 0);
}

#[test]
fn class66_spinner_matches_the_worked_example() {
    let Some(root) = extracted() else { return; };
    let c = load(&root, 1, 66);
    assert_eq!(c.joint_count, 1);
    let q = c.sequence(0).unwrap();
    assert_eq!((q.header.frame_count, q.header.loop_sound, q.header.rate_override), (15, 0xff, 0.125));
    assert_eq!(c.frame(0, 1).unwrap().quats[0], [0, -6812, 0, 32051]);
    // Frame 0: identity pose → F_0 = S_0. Frame 1: F_0 = P·S with the §6.7 formula gives rows
    // (0.104594, 0, −0.994509), (0,1,0), (0.994509, 0, 0.104594); the doc's §7 quotes the r0.z / r2.x
    // signs the other way round, which the formula (0x20ed18) and its own P and S rows do not give.
    let f = evaluate(&c, &at(0, 0, 0, 0.0));
    assert!((f[0][0][0] - 0.5).abs() < 1e-4 && (f[0][0][2] + 0.866).abs() < 1e-3, "{:?}", f[0]);
    let f = evaluate(&c, &at(0, 1, 1, 0.0));
    assert!((f[0][0][0] - 0.104594).abs() < 2e-5 && (f[0][0][2] + 0.994509).abs() < 2e-5, "{:?}", f[0]);
    assert!((f[0][2][0] - 0.994509).abs() < 2e-5 && (f[0][2][2] - 0.104594).abs() < 2e-5, "{:?}", f[0]);
    // One turn = 120 ticks: advance from spawn and come back to key 0 → 1 at t = 0.
    let mut s = AnimState::spawn(&c);
    moby_anim::advance(&mut s, &c);
    let start = (s.frame_a, s.frame_b, s.t);
    for _ in 0..120 { moby_anim::advance(&mut s, &c); }
    assert_eq!((s.frame_a, s.frame_b, s.t), start);
}

#[test]
fn class747_frame1_matches_the_worked_example() {
    let Some(root) = extracted() else { return; };
    let c = load(&root, 1, 747);
    assert_eq!(c.joint_count, 2);
    assert_eq!(c.parent_word, [0, 0x7000_0000]);
    let f = evaluate(&c, &at(0, 1, 1, 0.0));
    assert!((f[0][3][2] - 164.41).abs() < 1e-2, "F0 z = {}", f[0][3][2]);
    assert!((f[1][3][2] + 5187.94).abs() < 1e-2, "F1 z = {}", f[1][3][2]);
    assert!((f[0][2][2] - 0.80225).abs() < 1e-4);
}

#[test]
fn bind_pose_palette_is_identity() {
    let Some(root) = extracted() else { return; };
    for class in [11, 608, 724, 754, 1365] {
        let c = load(&root, 1, class);
        let f = evaluate(&c, &at(0, 0, 0, 0.0));
        let (mut rot, mut tr) = (0f32, 0f32);
        for m in &f {
            for (i, row) in m.iter().take(3).enumerate() {
                for (k, v) in row.iter().take(3).enumerate() { rot = rot.max((v - if i == k { 1.0 } else { 0.0 }).abs()); }
            }
            tr = m[3][..3].iter().fold(tr, |a, v| a.max(v.abs()));
        }
        eprintln!("class {class}: {} joints, max |P·S − I| rotation {rot:.2e}, translation {tr:.3}", c.joint_count);
        // The doc rounds to 2e-4; f64 on the same data gives 2.349e-4 for class 754 (this port: 2.352e-4).
        assert!(rot <= 2.5e-4, "class {class}: rotation error {rot}");
        assert!(tr < 1.0, "class {class}: translation error {tr}");
    }
}

#[test]
fn post_scale_quirk_on_level00_class608() {
    let Some(root) = extracted() else { return; };
    let c = load(&root, 0, 608);
    let (a, b) = (c.frame(8, 1).unwrap(), c.frame(8, 2).unwrap());
    let post = |f: &rc_formats::moby_anim::MobyFrame| f.scales.iter().filter(|s| !s.inherited()).map(|s| s.joint).collect::<Vec<u8>>();
    let (pa, pb) = (post(a), post(b));
    let list = moby_anim::post_scale_list(&pa, &pb);
    let mut union: Vec<u8> = pa.iter().chain(&pb).copied().collect();
    union.sort();
    union.dedup();
    eprintln!("class 608 seq 8 keys 1→2: A {pa:?}, B {pb:?}, linked {list:?}");
    assert_eq!(list, [0], "only joint 0 keeps its post-scale");
    assert!(union.len() > 1);
}
