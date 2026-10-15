//! Committed loader snapshots (`crates/rc-formats/data/loader_snapshots.tsv`): what the golden tests compare the
//! Rust loaders' output against since the C++ reference extractor was retired (docs/plan/decisions.md,
//! 2026-09-27). The table holds hashes, sizes and counts only, never disc bytes.
//!
//! A test feeds every byte string it used to compare with the C++ dumps into a [`Snap`] under a (scope, key)
//! row: scope is the level (`00`..`18`) or `global`, key names the section (`tfrag.positions`, ...). Per row the
//! table keeps the number of strings, their total size and the SHA-1 of the strings in order, each prefixed by
//! its length as a little-endian u64 (so moving a byte across an item boundary changes the hash).
//! The table was generated on 2026-09-27 while the C++ dumps were still present, by the same test code with the
//! byte comparison against them still in place: every hashed string was then byte-identical to the C++ output.
//!
//! `RC_SNAPSHOT_WRITE=1` rewrites the calling test's rows instead of checking them (after an intended loader
//! change, with the reason in the commit).

#![allow(dead_code)]

use rc_formats::sha1::{hex, Sha1};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

const HEADER: &str = "\
# Ratchet & Clank Recomp loader snapshots: SCUS_971.99 v1.00 (NTSC-U), all 19 levels. Hashes, sizes and counts only (no disc bytes).
# Columns: test<TAB>scope<TAB>key<TAB>items<TAB>bytes<TAB>sha1. scope = level (00..18) or global; items = byte
# strings hashed; bytes = their total size; sha1 over the strings in order, each prefixed by its length (u64 LE).
# Generated 2026-09-27 by crates/rc-formats/tests/formats/golden.rs while the Rust output was byte-identical to the retired
# C++ reference extractor's dumps (git 2230812). Rewrite rows: RC_SNAPSHOT_WRITE=1 cargo xtask test-job --test formats --filter golden::
";

/// The table's path.
pub fn table_path() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/loader_snapshots.tsv") }

/// Serialises table rewrites between tests running in parallel.
static WRITE: Mutex<()> = Mutex::new(());

type Key = (String, String);
type Row = (u64, u64, String);

fn read_table() -> BTreeMap<(String, Key), Row> {
    let text = std::fs::read_to_string(table_path()).unwrap_or_default();
    let mut out = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() { continue; }
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 6, "{}:{}: expected 6 columns", table_path().display(), n + 1);
        let num = |s: &str| s.parse::<u64>().unwrap_or_else(|_| panic!("{}:{}: bad number {s}", table_path().display(), n + 1));
        out.insert((f[0].to_string(), (f[1].to_string(), f[2].to_string())), (num(f[3]), num(f[4]), f[5].to_string()));
    }
    out
}

/// The SHA-1 a row gets for these byte strings (for tests proving that an edited field changes the snapshot).
pub fn digest<B: AsRef<[u8]>>(items: &[B]) -> String {
    let mut h = Sha1::new();
    for b in items { h.update(&(b.as_ref().len() as u64).to_le_bytes()); h.update(b.as_ref()); }
    hex(&h.finish())
}

/// One test's rows.
pub struct Snap {
    test: &'static str,
    rows: BTreeMap<Key, (u64, u64, Sha1)>,
}

/// The scope of level `i`.
pub fn lv(i: usize) -> String { format!("{i:02}") }

impl Snap {
    pub fn new(test: &'static str) -> Snap { Snap { test, rows: BTreeMap::new() } }

    /// Appends one byte string to row (scope, key).
    pub fn add(&mut self, scope: &str, key: &str, bytes: &[u8]) {
        let e = self.rows.entry((scope.to_string(), key.to_string())).or_insert_with(|| (0, 0, Sha1::new()));
        e.0 += 1;
        e.1 += bytes.len() as u64;
        e.2.update(&(bytes.len() as u64).to_le_bytes());
        e.2.update(bytes);
    }

    /// Appends a list of little-endian u32 values as one byte string.
    pub fn add_u32s(&mut self, scope: &str, key: &str, values: &[u32]) {
        let b: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.add(scope, key, &b);
    }

    fn finished(self) -> BTreeMap<Key, Row> {
        self.rows.into_iter().map(|(k, (n, b, h))| (k, (n, b, hex(&h.finish())))).collect()
    }

    /// Checks the rows against the table (or rewrites them with `RC_SNAPSHOT_WRITE=1`). Returns the row count.
    pub fn finish(self) -> usize {
        let test = self.test;
        let ours = self.finished();
        assert!(!ours.is_empty(), "{test}: nothing recorded");
        if std::env::var_os("RC_SNAPSHOT_WRITE").is_some_and(|v| v == "1") {
            let _g = WRITE.lock().unwrap_or_else(|e| e.into_inner());
            let mut all = read_table();
            all.retain(|(t, _), _| t != test);
            for (k, r) in &ours { all.insert((test.to_string(), k.clone()), r.clone()); }
            let mut text = HEADER.to_string();
            for ((t, (scope, key)), (n, b, h)) in &all { text += &format!("{t}\t{scope}\t{key}\t{n}\t{b}\t{h}\n"); }
            std::fs::write(table_path(), text).unwrap();
            eprintln!("snapshot {test}: wrote {} rows to {}", ours.len(), table_path().display());
            return ours.len();
        }
        let want: BTreeMap<Key, Row> = read_table().into_iter().filter(|((t, _), _)| t == test).map(|((_, k), r)| (k, r)).collect();
        assert!(!want.is_empty(), "{test}: no rows in {} (generate with RC_SNAPSHOT_WRITE=1)", table_path().display());
        let mut diffs = Vec::new();
        for (k, w) in &want {
            match ours.get(k) {
                None => diffs.push(format!("  {}/{}: missing (table: {} items, {} bytes)", k.0, k.1, w.0, w.1)),
                Some(o) if o != w => diffs.push(format!("  {}/{}: {} items, {} bytes, sha1 {} (table: {} items, {} bytes, {})", k.0, k.1, o.0, o.1, o.2, w.0, w.1, w.2)),
                _ => {}
            }
        }
        for k in ours.keys().filter(|k| !want.contains_key(*k)) { diffs.push(format!("  {}/{}: not in the table", k.0, k.1)); }
        assert!(diffs.is_empty(), "{test}: {} of {} snapshot rows differ from {}:\n{}", diffs.len(), want.len(), table_path().display(),
                diffs.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
        eprintln!("snapshot {test}: {} rows match", ours.len());
        ours.len()
    }
}
