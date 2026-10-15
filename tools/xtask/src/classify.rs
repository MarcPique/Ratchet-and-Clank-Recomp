//! Sorts the files of a data folder into what `ratchet_clank-extract` writes, the retired C++ extractor's known leftovers,
//! and unknown files (which `regen-data` refuses to delete without `--force`). Names only; nothing is read or written.

use std::collections::HashSet;
use std::path::Path;

/// What the extractor writes besides the Tier 0 table's files: the manifest, the Tier 1 cache, Tier 2 exports.
pub const INFO_FILE: &str = "extract-info.json";
pub const EXTRACTOR_DIRS: &[&str] = &["cache/", "exports/"];

/// The known stale leftovers, in match order (first match wins). All are rebuildable or were copied to `work/` on
/// 2026-09-27 (docs/workflows/game-data.md), so `regen-data` removes them without asking.
pub const LEFTOVER_KINDS: &[&str] = &[
    "*.partial (an interrupted extract)",
    ".dec decompressed copies",
    "*_dump.bin dumps",
    "core/ gameplay/ textures/ splits",
    "overlay.elf / overlay.txt",
    "vu/ listings",
    "PNG / OBJ previews",
    ".DS_Store (Finder)",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A Tier 0 file, `extract-info.json`, or under `cache/` / `exports/`.
    Extractor,
    /// A known C++-era leftover: index into [`LEFTOVER_KINDS`].
    Leftover(usize),
    Unknown,
}

/// `levels/<digits>/rest` → `rest`.
fn level_rest(rel: &str) -> Option<&str> {
    let r = rel.strip_prefix("levels/")?;
    let (nn, rest) = r.split_once('/')?;
    (!nn.is_empty() && nn.bytes().all(|b| b.is_ascii_digit())).then_some(rest)
}

/// Classifies one file by its path relative to the data folder (`/`-separated). `tier0` holds the Tier 0 paths.
pub fn classify(rel: &str, tier0: &HashSet<String>) -> Class {
    if tier0.contains(rel) || rel == INFO_FILE || EXTRACTOR_DIRS.iter().any(|d| rel.starts_with(d)) {
        return Class::Extractor;
    }
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let lvl = level_rest(rel);
    let kind = if name.ends_with(".partial") {
        0
    } else if name.ends_with(".dec") {
        1
    } else if name.ends_with("_dump.bin") {
        2
    } else if lvl.is_some_and(|r| ["core/", "gameplay/", "textures/"].iter().any(|d| r.starts_with(d))) {
        3
    } else if lvl.is_some_and(|r| r == "overlay.elf" || r == "overlay.txt") {
        4
    } else if rel.starts_with("vu/") {
        5
    } else if lvl.is_some_and(|r| !r.contains('/') && (r.ends_with(".png") || r.ends_with(".obj"))) {
        6
    } else if name == ".DS_Store" {
        7
    } else {
        return Class::Unknown;
    };
    Class::Leftover(kind)
}

/// Tier 0 paths from every `*.tsv` table in `data_dir` (`crates/rc-extract/data`): the first column of each row.
pub fn load_tier0(data_dir: &Path) -> std::io::Result<HashSet<String>> {
    let mut set = HashSet::new();
    for e in std::fs::read_dir(data_dir)? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "tsv") {
            for line in std::fs::read_to_string(&p)?.lines() {
                if line.starts_with('#') || line.trim().is_empty() { continue; }
                if let Some(path) = line.split('\t').next() { set.insert(path.to_string()); }
            }
        }
    }
    Ok(set)
}

#[derive(Default, Debug)]
pub struct Tally {
    pub files: u64,
    pub bytes: u64,
}

impl Tally {
    fn add(&mut self, bytes: u64) { self.files += 1; self.bytes += bytes; }
}

/// A data folder's contents by class.
#[derive(Default, Debug)]
pub struct Report {
    pub extractor: Tally,
    pub leftovers: Vec<Tally>,
    /// Unknown files (relative paths) and their total.
    pub unknown: Vec<String>,
    pub unknown_tally: Tally,
}

impl Report {
    pub fn leftover_total(&self) -> Tally {
        self.leftovers.iter().fold(Tally::default(), |mut t, l| { t.files += l.files; t.bytes += l.bytes; t })
    }
}

/// Walks `root` (symlinks are not followed and count as unknown files). Empty folders are ignored.
pub fn scan(root: &Path, tier0: &HashSet<String>) -> std::io::Result<Report> {
    let mut r = Report { leftovers: LEFTOVER_KINDS.iter().map(|_| Tally::default()).collect(), ..Report::default() };
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)? {
            let e = e?;
            let ft = e.file_type()?;
            let path = e.path();
            if ft.is_dir() { stack.push(path); continue; }
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            match if ft.is_file() { classify(&rel, tier0) } else { Class::Unknown } {
                Class::Extractor => r.extractor.add(bytes),
                Class::Leftover(k) => r.leftovers[k].add(bytes),
                Class::Unknown => { r.unknown_tally.add(bytes); r.unknown.push(rel); }
            }
        }
    }
    r.unknown.sort();
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> HashSet<String> {
        ["toc.bin", "boot/SCUS_971.99", "levels/01/core_data.bin", "levels/01/overlay.bin", "global/vendor.bin"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn extractor_output_is_recognised() {
        let t = t0();
        for p in ["toc.bin", "levels/01/core_data.bin", "extract-info.json", "cache/v1/stamp.toml", "cache/v1/01/core.lump", "exports/textures/a.png"] {
            assert_eq!(classify(p, &t), Class::Extractor, "{p}");
        }
    }

    #[test]
    fn cpp_leftovers_are_known() {
        let t = t0();
        let cases = [
            ("levels/01/core_data.bin.partial", 0),
            ("global/vendor.dec", 1),
            ("levels/05/hud_bank_3.dec", 1),
            ("global/mission_ss/004.dec", 1),
            ("levels/01/tfrag_dump.bin", 2),
            ("global/sound_dump.bin", 2),
            ("levels/01/core/moby_class/0123.bin", 3),
            ("levels/01/core/index.txt", 3),
            ("levels/12/gameplay/level_settings.bin", 3),
            ("levels/01/textures/moby/0001.png", 3),
            ("levels/01/textures/rgba.bin", 3),
            ("levels/01/overlay.elf", 4),
            ("levels/01/overlay.txt", 4),
            ("vu/104691.err", 5),
            ("levels/01/tfrag_topdown.png", 6),
            ("levels/01/moby_7_front.png", 6),
            ("levels/01/ties_lod2.obj", 6),
            ("levels/01/.DS_Store", 7),
            (".DS_Store", 7),
        ];
        for (p, k) in cases { assert_eq!(classify(p, &t), Class::Leftover(k), "{p}"); }
    }

    #[test]
    fn anything_else_is_unknown() {
        let t = t0();
        for p in ["notes.txt", "levels/01/my_edit.bin", "global/foo.png", "levels/xx/core/a.bin", "savestates/a.p2s", "levels/01/sub/x.obj", "vuX/a.txt", "Cargo.toml"] {
            assert_eq!(classify(p, &t), Class::Unknown, "{p}");
        }
    }

    #[test]
    fn the_committed_table_loads() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/rc-extract/data");
        let t = load_tier0(&dir).unwrap();
        assert!(t.len() > 2000 && t.contains("toc.bin") && t.contains("boot/SCUS_971.99"), "{}", t.len());
    }
}
