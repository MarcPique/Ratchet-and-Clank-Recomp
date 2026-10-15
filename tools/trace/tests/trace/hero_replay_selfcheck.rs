//! End-to-end check of the hero feel-pass pipeline on the level data (skipped without `extracted/`): a "recording"
//! (pad bytes only, Heli-Pack on the back) → `replay-hero` → the port's trace in the file format → parsed back →
//! replayed again from its own first sample → the per-tick diff must be empty (the placement reproduces the run and
//! the run is deterministic), and the Heli-Pack long jumps must be segmented.

use rc_game::pad::{button, PadInput};
use rc_trace::hero_analysis::{diff, jump_stats, segment};
use rc_trace::hero_replay::{replay, HeroLevel, ReplayOptions};
use rc_trace::hero_trace::{Hex, Sample, Trace};

/// Stand 20 ticks, then run forward; from tick 60 a crouch + ✕ long jump every 70 ticks (R1 10 ticks, ✕ on the
/// 10th), five times (on Novalis the terrain turns some of them into falls, glides and a high jump).
fn pad(t: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if t >= 20 { p = p.stick(0.0, -1.0); }
    if (60..60 + 5 * 70).contains(&t) {
        let k = (t - 60) % 70;
        if k < 11 { p = p.press(button::R1); }
        if k == 10 { p = p.press(button::CROSS); }
    }
    p
}

#[test]
fn replay_of_the_ports_own_trace_has_no_divergence() {
    let extracted = rc_trace::default_extracted();
    let Ok(lv) = HeroLevel::load(&extracted, 1) else { eprintln!("skipped: no extracted/levels/01"); return };
    let (p, yaw) = lv.spawn;
    let mut owned = vec![0u8; 37];
    owned[2] = 1;
    let mut rec = Trace::default();
    for t in 0..480u32 {
        let mut s = Sample { tick: 1000 + t, level: 1, pad: Hex(pad(t).bytes().to_vec()), ..Default::default() };
        if t == 0 {
            // Ratchet's Novalis instance stands 0.5 above the floor (the hero init snaps it down).
            (s.pos_x, s.pos_y, s.pos_z, s.yaw, s.target_yaw) = (p[0], p[1], p[2] - 0.5, yaw, yaw);
            (s.owned, s.back_state, s.back_id, s.health) = (Hex(owned.clone()), 2, 2, 4);
        }
        rec.samples.push(s);
    }
    let r = replay(&lv, &rec, &ReplayOptions::default()).unwrap();
    eprintln!("notes: {:?}", r.notes);
    assert_eq!(r.samples.len(), rec.samples.len());
    assert_eq!(r.filled, 0);
    let st: Vec<i32> = r.samples.iter().map(|s| s.state).fold(Vec::new(), |mut v, s| { if v.last() != Some(&s) { v.push(s); } v });
    eprintln!("port states: {st:x?}");
    assert!(st.contains(&0xa), "Heli-Pack long jump reached");

    // Through the file format and back, then replayed from its own first sample.
    let port = Trace { meta: vec![("source".into(), "port".into())], samples: r.samples.clone() };
    let port = Trace::parse(&port.to_text()).unwrap();
    assert_eq!(port.samples, r.samples, "format round trip");
    let again = replay(&lv, &port, &ReplayOptions::default()).unwrap();
    let d = diff(&port.samples, &again.samples, 0.0);
    assert!(d.first.is_none() && d.first_cam.is_none(), "self replay diverged: {:?} / {:?}", d.first, d.first_cam);
    assert_eq!(d.paired, port.samples.len());

    let jumps = segment(&port.samples);
    let stats = jump_stats(&port.samples, &jumps);
    for j in &stats { eprintln!("{} takeoff {} air {:?} dist {:?} apex {:.3} land {:?} next {:?}", j.kind, j.takeoff_tick, j.airtime, j.hdist, j.apex_h, j.land_state, j.to_next_takeoff); }
    let long: Vec<_> = stats.iter().filter(|j| j.kind.contains("0xa")).collect();
    assert!(long.len() >= 2, "{} long jumps", long.len());
    assert!(long.iter().all(|j| j.hdist.unwrap_or(0.0) > 5.0));
}
