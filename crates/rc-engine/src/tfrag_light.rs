//! Level-load terrain lighting: runs the game's `LightTfrags` pass (bit-exact port in
//! `rc_formats::tfrag_light`, spec docs/plan/tfrag_lighting.md) over every tfrag and writes the
//! result over `Tfrag::rgba`, exactly as the game overwrites the tfrag RGBA block in RAM that VU1
//! then copies to the GS. The game runs it once for all tfrags at level load and again each frame
//! for visible tfrags; with no point lights (the port has none yet) the per-frame result is
//! identical to the load-time one, so doing it once here is equivalent.
//!
//! Inputs from `extracted/`: `levels/NN/gameplay_ntsc.bin` (WAD-compressed; directional lights at
//! gameplay pointer 0x04) and `boot/SCUS_971.99` (the normal-decode `(cos, sin)` table).
//!
//! `RC_NO_LIGHT=1` skips the pass and keeps the stored RGBA, for comparison.

use anyhow::{Context, Result};
use rc_formats::tfrag::Tfrag;
use rc_formats::tfrag_light::{self, NormalTable, BOOT_NORMAL_TABLE_VADDR};
use std::path::Path;

/// Relights `tfrags` in place. Returns the number of light sets used, or `None` if disabled.
pub fn light_level_tfrags(root: &Path, index: u32, tfrags: &mut [Tfrag]) -> Result<Option<usize>> {
    if std::env::var("RC_NO_LIGHT").is_ok_and(|v| v != "0") { return Ok(None); }
    let read = |p: &Path| crate::disc_source::read_path(root, p);
    let gameplay = rc_data::level_gameplay(root, index).context("decompressing gameplay_ntsc")?;
    let bank = tfrag_light::parse_light_bank(&gameplay).context("parsing directional lights")?;
    let table = NormalTable::from_elf(&read(&root.join("boot/SCUS_971.99"))?, BOOT_NORMAL_TABLE_VADDR)
        .context("reading the normal table from the boot ELF")?;
    for t in tfrags.iter_mut() {
        t.rgba = tfrag_light::light_tfrag(t, &bank, &table, None);
    }
    Ok(Some(bank.count))
}
