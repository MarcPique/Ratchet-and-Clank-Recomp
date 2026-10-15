//! Level-load shrub lighting: runs the game's `LightShrubs` pass (bit-exact port in
//! `rc_formats::shrub_light`, spec docs/plan/shrub_lighting.md) once per shrub instance.
//!
//! The level loader (`FUN_00255958`) lists every instance and runs the pass once; afterwards `ShrubProc`
//! relists an instance each frame only when its dirty byte is set or it has point lights, and the pass
//! restarts from the ambient, so with no point lights (the port has none yet) the load-time palette is
//! the one every frame draws with.
//!
//! `RC_NO_LIGHT=1` skips the pass: every palette entry gets 0x80 (the texture colour unchanged).

use anyhow::{Context, Result};
use rc_formats::shrub::{LevelShrubClass, ShrubInstance};
use rc_formats::shrub_light::{light_shrub_instance, PALETTE};
use rc_formats::tfrag_light;

/// The 24 lit palette colours of every instance (`None` class: the instance draws nothing, white palette).
pub fn light_instances(gameplay: &[u8], classes: &[LevelShrubClass], instances: &[ShrubInstance], class_of: &[Option<usize>]) -> Result<Vec<[[u8; 4]; PALETTE]>> {
    if std::env::var("RC_NO_LIGHT").is_ok_and(|v| v != "0") {
        return Ok(vec![[[0x80; 4]; PALETTE]; instances.len()]);
    }
    let bank = tfrag_light::parse_light_bank(gameplay).context("parsing directional lights")?;
    Ok(instances
        .iter()
        .zip(class_of)
        .map(|(inst, c)| match c {
            Some(ci) => light_shrub_instance(&classes[*ci].class, inst, &bank),
            None => [[0x80; 4]; PALETTE],
        })
        .collect())
}
