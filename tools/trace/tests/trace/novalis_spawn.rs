//! Regression on the first real savestate (Novalis spawn, 2026-09-26; docs/plan/trace_results_novalis.md),
//! through its distilled facts: `tests/fixtures/novalis_spawn.tsv` (numbers only, committed; regenerate with
//! `cargo run -p rc-trace -- distill-spawn --state novalis_spawn`, docs/workflows/pcsx2.md). The port side needs
//! only `extracted/` game data; skipped when that is absent. No personal file (savestate, EE dump) is read.
//!
//! Same thresholds as the savestate version of this test: shrubs all equal, ties all but one instance (one entry
//! off by ±1 on two channels), game state all but at most 4 chunks, and the timeline / load-pass / fog checks exact.

use rc_trace::spawn_facts::{self as sf, SpawnFacts};

#[test]
fn novalis_spawn_state_checks() {
    let root = rc_trace::default_extracted();
    if !root.join("levels/01/core_data.bin").exists() { eprintln!("skipped: no extracted/levels/01"); return; }
    let facts = SpawnFacts::read(&rc_trace::spawn_fixture()).unwrap();
    let out = sf::check_all(&facts, &root).unwrap();
    let t = |name: &str| out.tallies.iter().find(|t| t.0 == name).map(|t| (t.1, t.2)).unwrap_or_else(|| panic!("no tally {name}"));
    // Tie / shrub palettes: shrubs all equal, ties all but one instance.
    assert_eq!(t("g. shrub instances"), (1208, 1208));
    let (ok, located) = t("g. tie instances");
    assert!(located == 1508 && ok >= 1507, "ties {ok}/{located}");
    // Game state: every chunk equal except flags 15/16 (scene 5) and the entry-record times.
    let (ok, total) = t("a. game state (chunks, excl. play-dependent)");
    assert!(total - ok <= 4, "{ok}/{total}");
    // The port run on the savestate's timeline (trace_results_novalis.md "Fixes applied"): the spawn test and
    // the index remap, the load pass pinned on the stream, the tick counter, 751's underwater colours.
    for name in ["e. tick counter", "f. moby slots paired by index", "f. rejected instances absent from RAM", "e. load pass: bolt spin bits",
        "e. load pass: crate 500 turns", "d. underwater look", "d. fog globals"] {
        let (ok, total) = t(name);
        assert!(total > 0 && ok == total, "{name}: {ok}/{total}");
    }
    assert_eq!(t("f. moby slots paired by index"), (929, 929));
    assert_eq!(t("f. rejected instances absent from RAM"), (54, 54));
}

/// The fixture parses and writes back to the same facts (no personal data needed).
#[test]
fn fixture_round_trips() {
    let text = std::fs::read_to_string(rc_trace::spawn_fixture()).unwrap();
    let facts = SpawnFacts::parse_tsv(&text).unwrap();
    assert_eq!(facts.level, 1);
    assert_eq!(SpawnFacts::parse_tsv(&facts.to_tsv("x")).unwrap(), facts);
}
