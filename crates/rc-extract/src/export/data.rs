//! Reading the Tier 0 archive for the exports: raw and WAD lumps by archive path, and a level's core.

use crate::{Code, Error};
use rc_formats::level::{self, LevelCore};
use rc_formats::FormatError;
use std::path::Path;

/// A Tier 0 file by archive path (forward slashes).
pub(crate) fn read(data: &Path, rel: &str) -> Result<Vec<u8>, Error> {
    std::fs::read(data.join(rel)).map_err(|e| Error::new(Code::CannotRead, format!("cannot read {rel} in {}: {e}", data.display())))
}

/// A Tier 0 WAD lump, decompressed exactly as the engine does (`rc_formats::wad::decompress`).
pub(crate) fn wad(data: &Path, rel: &str) -> Result<Vec<u8>, Error> {
    let raw = read(data, rel)?;
    rc_formats::wad::decompress(&raw).map_err(|e| damaged(rel, e))
}

/// A Tier 0 lump that the golden-tested loaders reject: the archive is damaged (40), as for `prepare`.
pub(crate) fn damaged(rel: &str, e: FormatError) -> Error {
    Error::new(Code::VerifyFailed, format!("{rel}: {e}; run `ratchet_clank-extract verify` and re-extract"))
}

pub(crate) fn lvl(id: u32, file: &str) -> String { format!("levels/{id:02}/{file}") }

/// The lumps most level exports read.
pub(crate) struct LevelData {
    pub id: u32,
    pub core_index: Vec<u8>,
    pub core_data: Vec<u8>,
    pub gs_ram: Vec<u8>,
    pub core: LevelCore,
}

impl LevelData {
    pub(crate) fn load(data: &Path, id: u32) -> Result<LevelData, Error> {
        let core_index = read(data, &lvl(id, "core_index.bin"))?;
        let core_data = wad(data, &lvl(id, "core_data.bin"))?;
        let gs_ram = read(data, &lvl(id, "gs_ram.bin"))?;
        let core = level::parse_level_core(&core_index, core_data.len()).map_err(|e| damaged(&lvl(id, "core_index.bin"), e))?;
        Ok(LevelData { id, core_index, core_data, gs_ram, core })
    }

    /// `levels/NN/<file>` for error messages and sidecar `source` fields.
    pub(crate) fn rel(&self, file: &str) -> String { lvl(self.id, file) }
}

/// The level's decompressed `gameplay_ntsc` (what the NTSC game reads).
pub(crate) fn gameplay(data: &Path, id: u32) -> Result<Vec<u8>, Error> { wad(data, &lvl(id, "gameplay_ntsc.bin")) }

/// Relative URI from a file in `from_dir` (archive-style, forward slashes, under the export root) to `to`.
pub(crate) fn rel_uri(from_dir: &str, to: &str) -> String {
    let depth = from_dir.split('/').filter(|s| !s.is_empty()).count();
    format!("{}{to}", "../".repeat(depth))
}

#[cfg(test)]
mod tests {
    #[test]
    fn relative_uris_climb_to_the_export_root() {
        assert_eq!(super::rel_uri("levels/01", "textures/levels/01/tfrag/000.png"), "../../textures/levels/01/tfrag/000.png");
        assert_eq!(super::rel_uri("", "a.png"), "a.png");
    }
}
