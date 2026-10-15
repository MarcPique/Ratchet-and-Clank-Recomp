//! Where tests and dev tools find the development data tree: `RC_EXTRACTED`, else `<workspace>/extracted`
//! (a Tier 0 archive as `ratchet_clank-extract extract` writes it; git-ignored, never shipped), plus the decompressed
//! lumps and the named core blocks / gameplay sections the tests use, produced by the Rust loaders.
//!
//! Tests skip when a file they need is absent. The game runtime resolves its own data folder (`rc-engine`
//! `disc_source`: `--data-dir`, `RC_DATA_DIR`, then this tree) and reads decompressed lumps through `rc-data`.
//! Nothing here writes to the tree.

use crate::level::{parse_level_core, LevelCore};
use crate::{gameplay, wad};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// The development data root: `RC_EXTRACTED` (when set and non-empty), else `<workspace>/extracted`.
pub fn root() -> PathBuf {
    std::env::var_os("RC_EXTRACTED")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted"))
}

/// `levels/NN` under [`root`].
pub fn level_dir(level: u32) -> PathBuf { root().join(format!("levels/{level:02}")) }

fn memo<T: Send + Sync + 'static>(cache: &'static OnceLock<Mutex<HashMap<String, Arc<T>>>>, key: String, make: impl FnOnce() -> Option<T>) -> Option<Arc<T>> {
    let map = cache.get_or_init(Default::default);
    if let Some(v) = map.lock().unwrap_or_else(|e| e.into_inner()).get(&key) { return Some(v.clone()); }
    let v = Arc::new(make()?);
    Some(map.lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_insert(v).clone())
}

/// `wad::decompress` of the Tier 0 file `rel` under [`root`] (e.g. `levels/01/core_data.bin`), once per process.
/// None when the file is absent; panics when it is not a valid WAD.
pub fn decompressed(rel: &str) -> Option<Arc<Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Vec<u8>>>>> = OnceLock::new();
    memo(&CACHE, rel.to_string(), || {
        let raw = std::fs::read(root().join(rel)).ok()?;
        Some(wad::decompress(&raw).unwrap_or_else(|e| panic!("{rel}: {e}")))
    })
}

/// A level's decompressed core data (`levels/NN/core_data.bin`).
pub fn core_data(level: u32) -> Option<Arc<Vec<u8>>> { decompressed(&format!("levels/{level:02}/core_data.bin")) }

/// A level's decompressed NTSC gameplay file (`levels/NN/gameplay_ntsc.bin`).
pub fn gameplay(level: u32) -> Option<Arc<Vec<u8>>> { decompressed(&format!("levels/{level:02}/gameplay_ntsc.bin")) }

/// One level's core: the index, the parsed [`LevelCore`] and the decompressed data.
pub struct Core {
    pub index: Vec<u8>,
    pub core: LevelCore,
    pub data: Arc<Vec<u8>>,
}

impl Core {
    /// The named block (see [`LevelCore::block`]).
    pub fn block(&self, name: &str) -> Option<&[u8]> { self.core.block(&self.data, name) }
}

/// A level's core, once per process. None when the level's files are absent.
pub fn core(level: u32) -> Option<Arc<Core>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Core>>>> = OnceLock::new();
    memo(&CACHE, format!("{level:02}"), || {
        let index = std::fs::read(level_dir(level).join("core_index.bin")).ok()?;
        let data = core_data(level)?;
        let core = parse_level_core(&index, data.len()).unwrap_or_else(|e| panic!("level {level:02} core index: {e}"));
        Some(Core { index, core, data })
    })
}

/// The named block of a level's core data (`moby_class/0042`, `ratchet_seq/007`, `tfrags`, …).
pub fn core_block(level: u32, name: &str) -> Option<Vec<u8>> { core(level)?.block(name).map(<[u8]>::to_vec) }

/// The named section of a level's NTSC gameplay file (`level_settings`, …; [`gameplay::sections`]).
pub fn gameplay_section(level: u32, name: &str) -> Option<Vec<u8>> {
    let g = gameplay(level)?;
    gameplay::section(&g, name).unwrap_or_else(|e| panic!("level {level:02} gameplay sections: {e}")).map(<[u8]>::to_vec)
}
