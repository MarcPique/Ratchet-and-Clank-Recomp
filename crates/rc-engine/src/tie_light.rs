//! Level-load tie lighting: runs the game's `LightTies` pass (bit-exact port in
//! `rc_formats::tie_light`, spec docs/plan/tie_lighting.md) once per tie instance.
//!
//! The game runs it for every instance at level load (`FUN_00255958`, list 0..n) and each frame for
//! the visible list `TieProc` builds (0x1c7780); the pass always restarts from the instance's
//! ambient colours, so with no point lights (the port has none yet) the per-frame result equals the
//! load-time one and computing it once here is equivalent.
//!
//! `RC_NO_LIGHT=1` skips the pass: every slot gets 0x80 (the texture colour unchanged).

use anyhow::{Context, Result};
use rc_formats::tfrag_light;
use rc_formats::tie::{LevelTieClass, TieInstance};
use rc_formats::tie_light::{light_tie_instance, SLOTS};

/// The 64 lit slot colours of every instance (`None` class: the instance draws nothing, white table).
pub fn light_instances(gameplay: &[u8], classes: &[LevelTieClass], instances: &[TieInstance], class_of: &[Option<usize>]) -> Result<Vec<[[u8; 4]; SLOTS]>> {
    if std::env::var("RC_NO_LIGHT").is_ok_and(|v| v != "0") {
        return Ok(vec![[[0x80; 4]; SLOTS]; instances.len()]);
    }
    let bank = tfrag_light::parse_light_bank(gameplay).context("parsing directional lights")?;
    Ok(instances
        .iter()
        .zip(class_of)
        .map(|(inst, c)| match c {
            Some(ci) => light_tie_instance(&classes[*ci].class, inst, &bank, None),
            None => [[0x80; 4]; SLOTS],
        })
        .collect())
}
