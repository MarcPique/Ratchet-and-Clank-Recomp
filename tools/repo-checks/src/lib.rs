//! Repository-layout guards (dev only, never ships; tests in `tests/guards.rs`, doc `tools/repo-checks/README.md`
//! and `docs/plan/repo_reorg.md`). Plain text reads, no dependencies.
//!
//! 1. No product crate (`crates/*/Cargo.toml`) depends on anything under `tools/`.
//! 2. No test and no product file names personal material or dev output ([`PERSONAL_PATTERNS`]; test files also
//!    [`TEST_ONLY_PATTERNS`]). No allow-list: tests never rely on the user's personal files (docs/workflows/testing.md §10).
//! 3. The top level holds only the agreed folders ([`TOP_LEVEL_DIRS`]), dotfiles and the Cargo / README / CHANGELOG / LICENSE files.
//! 4. Tests run only through `cargo xtask test-*` (docs/workflows/testing.md §1): no Cargo alias runs tests, and no doc
//!    or source comment recommends the retired alias or a by-hand `cargo test` ([`stale_test_command_hits`]).

use std::path::{Component, Path, PathBuf};

/// The repository root (this crate lives at `tools/repo-checks`).
pub fn repo_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..") }

/// Allowed top-level folders.
pub const TOP_LEVEL_DIRS: &[&str] = &["crates", "tools", "docs", "extracted", "work", "target", "dist"];
/// Allowed top-level files (besides dotfiles).
pub const TOP_LEVEL_FILES: &[&str] = &["Cargo.toml", "Cargo.lock", "README.md", "CHANGELOG.md", "LICENSE"];

/// Text no product or test file may contain: savestates, the personal folder, dev output, recordings, the personal
/// env var. `work/`, `/traces/` and `.mov` count only as a path segment / extension ([`personal_hits`]).
pub const PERSONAL_PATTERNS: &[&str] = &[".p2s", "~/PS2", "PS2/ratchet1", "work/", "/traces/", "RC_PERSONAL", ".mov", "Screen Recording"];

/// Also forbidden in test files (a `tests/` folder): reading the disc image or anything under the home folder. The
/// product reads them on purpose (the extractor's `RC_ISO`, the port settings file under `HOME`); a test never does.
/// `env_remove("RC_ISO")` (a test making sure the user's image is not picked up) is not a read and does not match.
pub const TEST_ONLY_PATTERNS: &[&str] = &["var(\"RC_ISO\")", "var_os(\"RC_ISO\")", "RC_ISO=", "var(\"HOME\")", "var_os(\"HOME\")", "home_dir", "Application Support"];

/// The folders the personal-path guard scans, repo-relative: all of `crates/` (product sources and their tests) and
/// every tool's `tests/` folder. Tool sources (`tools/*/src`) are deliberately not scanned: the dev tools read personal
/// material by design (`rc-trace`: savestates and recordings in the personal folder, output under `work/`; `xtask
/// regen-data`: the disc image), and they never ship.
pub fn guarded_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.join("crates")];
    if let Ok(rd) = std::fs::read_dir(root.join("tools")) {
        let mut tools: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("tests")).filter(|p| p.is_dir()).collect();
        tools.sort();
        out.extend(tools);
    }
    out
}

/// Whether a repo-relative path is test code (under a `tests/` folder).
pub fn is_test_file(rel: &str) -> bool { rel.split('/').any(|c| c == "tests") }

/// Lexical normalisation (`a/b/../c` → `a/c`), relative to the repo root.
pub fn normalise(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => { out.pop(); }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// Every `path = "..."` value in a manifest's text (dependency tables, `[lib]`, `[[bin]]`).
pub fn manifest_paths(toml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in toml.lines() {
        let line = line.split('#').next().unwrap_or("");
        let mut rest = line;
        while let Some(i) = rest.find("path") {
            let before_ok = i == 0 || !rest.as_bytes()[i - 1].is_ascii_alphanumeric() && rest.as_bytes()[i - 1] != b'_' && rest.as_bytes()[i - 1] != b'-';
            let after = rest[i + 4..].trim_start();
            if before_ok && after.starts_with('=') {
                let v = after[1..].trim_start();
                if let Some(v) = v.strip_prefix('"') {
                    if let Some(end) = v.find('"') { out.push(v[..end].to_string()); }
                }
            }
            rest = &rest[i + 4..];
        }
    }
    out
}

/// Paths in the manifest of the crate at `crate_dir` (repo-relative) that resolve under `tools/`.
pub fn tool_dependencies(crate_dir: &Path, toml: &str) -> Vec<String> {
    manifest_paths(toml).into_iter().filter(|p| normalise(&crate_dir.join(p)).starts_with("tools")).collect()
}

/// The patterns of [`PERSONAL_PATTERNS`] on one line. `work/` and `/traces/` count only as a path segment (not inside
/// `framework/`), `.mov` only as an extension (not `self.moved`).
pub fn personal_hits(line: &str) -> Vec<&'static str> {
    let b = line.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-';
    PERSONAL_PATTERNS.iter().copied().filter(|pat| {
        line.match_indices(pat).any(|(i, _)| match *pat {
            "work/" => i == 0 || !word(b[i - 1]),
            ".mov" => b.get(i + pat.len()).is_none_or(|&c| !word(c)),
            _ => true,
        })
    }).collect()
}

/// The patterns of [`TEST_ONLY_PATTERNS`] on one line.
pub fn test_only_hits(line: &str) -> Vec<&'static str> { TEST_ONLY_PATTERNS.iter().copied().filter(|p| line.contains(p)).collect() }

/// Every file under `dir`, recursively (skips `target/`).
pub fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { if e.file_name() != "target" { stack.push(p); } } else { out.push(p); }
        }
    }
    out.sort();
    out
}

/// The retired workspace test alias as a command (assembled, so this file holds no hit).
pub const RETIRED_TEST_ALIAS: &str = concat!("cargo test", "-all");
/// By-hand test commands a workflow doc must not recommend.
pub const BY_HAND_TEST_COMMANDS: &[&str] = &["cargo test --workspace", "cargo test -p"];
/// Words that mark a line as explaining why not (or naming the xtask fallback's flags), not recommending.
pub const EXPLAINING_WORDS: &[&str] = &["plain", "never", "Never", "by-hand", "rc-engine/dev"];

/// The workflow docs: where commands are recommended (README.md, docs/workflows/, the orchestration handbook, the tool
/// READMEs). Plan docs elsewhere keep their historical records.
pub fn is_workflow_doc(rel: &str) -> bool {
    rel == "README.md" || rel.starts_with("docs/workflows/") || rel == "docs/plan/orchestration.md" || (rel.starts_with("tools/") && rel.ends_with("/README.md"))
}

/// Stale test commands on one line of `rel` (rule 4): the retired alias anywhere (a line about the alias itself, naming
/// it an alias, is fine); in a workflow doc also a by-hand `cargo test --workspace` / `-p` unless the line explains
/// ([`EXPLAINING_WORDS`]); in `.cargo/config.toml` any alias that runs `test` or `nextest`.
pub fn stale_test_command_hits(rel: &str, line: &str) -> Vec<&'static str> {
    let mut v = Vec::new();
    if line.contains(RETIRED_TEST_ALIAS) && !line.contains("alias") { v.push(RETIRED_TEST_ALIAS); }
    if is_workflow_doc(rel) && !EXPLAINING_WORDS.iter().any(|w| line.contains(w)) {
        v.extend(BY_HAND_TEST_COMMANDS.iter().copied().filter(|c| line.contains(c)));
    }
    if rel == ".cargo/config.toml" {
        let t = line.trim_start();
        if !t.starts_with('#') && (t.contains("= \"test") || t.contains("= \"nextest")) { v.push("a Cargo alias that runs tests"); }
    }
    v
}
