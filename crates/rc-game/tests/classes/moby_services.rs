//! W3 lane 2, the moby services (triggers.md §5b, creatures.md §12): the units of the platform system and of
//! `BreakFxB` resolve to their ports on exactly their census levels with the census's created counts (their behaviour
//! and side effects are the module tests of `units::carriers`, `units::falling_platform`, `units::light_fixture`,
//! `triggers`, `bolt`, `crate_` and `infobot`). Skips when `extracted/` is absent.
//!
//! `cargo xtask test-job --test classes --filter moby_services:: --nocapture` prints the counts.

use rc_formats::{gameplay, moby_spawn};
use rc_game::moby_update::classes::units;
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use std::collections::HashMap;

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap();
    ClassUpdate::Unit(i as u16)
}

/// (unit rows that may run it, class, the levels whose table runs it, created instances on them).
const EXPECTED: &[(&[&str], i16, &[u32], usize)] = &[
    (&["U102 707"], 707, &[2], 0),
    (&["U102 734"], 734, &[2], 0),
    (&["U126 1210"], 1210, &[3], 5),
    (&["U179 812"], 812, &[5], 25),
    (&["U565 1381"], 1381, &[18], 8),
    (&["U207 1511", "U472 1511"], 1511, &[5, 14], 48),
];

#[test]
fn moby_service_units_resolve_on_their_levels() {
    let Some(_) = crate::common::overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut created: HashMap<i16, usize> = HashMap::new();
    let mut bad = Vec::new();
    for level in 0..19u32 {
        let p = ports(level).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        for &(names, oc, levels, _) in EXPECTED {
            let ours: Vec<ClassUpdate> = names.iter().map(|n| unit(n)).collect();
            let is_ours = p.get(oc).is_some_and(|u| ours.contains(&u));
            if levels.contains(&level) {
                if !is_ours { bad.push(format!("level {level:02} class {oc}: not the unit ({:?})", p.get(oc))); }
                let n = inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == oc && t.spawn).count();
                *created.entry(oc).or_default() += n;
                if let Some(m) = inst.iter().find(|m| m.o_class as i16 == oc) { eprintln!("level {level:02} class {oc}: first at {:?}", m.position); }
            } else if p.in_table(oc) && is_ours {
                bad.push(format!("level {level:02} class {oc}: also the unit"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    eprintln!("created instances now ported: {created:?}");
    for &(_, oc, _, n) in EXPECTED { if n > 0 { assert_eq!(created[&oc], n, "class {oc}"); } }
    assert_eq!(created[&707] + created[&734], 4, "the turntables");
}
