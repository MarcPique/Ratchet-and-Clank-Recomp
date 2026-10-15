//! Repository-layout guards (docs/plan/repo_reorg.md "Rules"). Read-only; no data needed.

use repo_checks::*;
use std::path::Path;

/// Rule: `crates/` ships and never depends on anything under `tools/`.
#[test]
fn product_crates_do_not_depend_on_tools() {
    let root = repo_root();
    let mut checked = 0;
    let mut bad = Vec::new();
    for e in std::fs::read_dir(root.join("crates")).unwrap().flatten() {
        let manifest = e.path().join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else { continue };
        checked += 1;
        let dir = Path::new("crates").join(e.file_name());
        for p in tool_dependencies(&dir, &text) { bad.push(format!("{}: path = {p:?}", manifest.display())); }
        // A workspace-inherited or git dependency naming a tool package would also be a dependency on tools/.
        for tool in ["rc-trace", "repo-checks", "xtask"] {
            if text.lines().any(|l| l.trim_start().starts_with(&format!("{tool} ")) || l.trim_start().starts_with(&format!("{tool}="))) {
                bad.push(format!("{}: depends on the tool package {tool}", manifest.display()));
            }
        }
    }
    assert!(checked >= 5, "found only {checked} product manifests under crates/");
    assert!(bad.is_empty(), "product crates depend on tools/:\n{}", bad.join("\n"));
}

/// The manifest scan itself: a path under tools/ is caught, product paths are not.
#[test]
fn manifest_scan_catches_a_tool_path() {
    let toml = "[dependencies]\nrc-formats = { path = \"../rc-formats\" }\nrc-trace = { path = \"../../tools/trace\" } # dev\n[lib]\npath = \"src/lib.rs\"\n";
    assert_eq!(manifest_paths(toml), ["../rc-formats", "../../tools/trace", "src/lib.rs"]);
    assert_eq!(tool_dependencies(Path::new("crates/rc-game"), toml), ["../../tools/trace"]);
}

/// Rule: no test relies on the user's personal files, and no product file names them (docs/workflows/testing.md §10):
/// every file under `crates/` and every tool's `tests/` folder is free of [`PERSONAL_PATTERNS`], and test files also
/// of [`TEST_ONLY_PATTERNS`] (reading the disc image or the home folder). No exceptions.
#[test]
fn no_test_or_product_file_references_personal_paths() {
    let root = repo_root();
    let (mut hits, mut scanned) = (Vec::new(), 0);
    for dir in guarded_dirs(&root) {
        for f in files_under(&dir) {
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            scanned += 1;
            let rel = f.strip_prefix(&root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
            let rel = normalise(Path::new(&rel)).to_string_lossy().into_owned();
            let test = is_test_file(&rel);
            for (n, line) in text.lines().enumerate() {
                let mut pats = personal_hits(line);
                if test { pats.extend(test_only_hits(line)); }
                if !pats.is_empty() { hits.push(format!("{rel}:{}: {pats:?}: {}", n + 1, line.trim())); }
            }
        }
    }
    assert!(scanned > 100, "scanned only {scanned} files");
    assert!(hits.is_empty(), "test / product files reference personal paths (docs/workflows/testing.md §10):\n{}", hits.join("\n"));
}

/// The scanned folders: all of crates/ and the tools' tests/, never a tool's src/.
#[test]
fn guard_scans_crates_and_tool_tests() {
    let root = repo_root();
    let dirs: Vec<String> = guarded_dirs(&root).iter().map(|d| d.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/")).collect();
    assert_eq!(dirs[0], "crates");
    for t in ["tools/repo-checks/tests", "tools/trace/tests"] { assert!(dirs.iter().any(|d| d == t), "{t} not scanned: {dirs:?}"); }
    assert!(dirs.iter().all(|d| d == "crates" || d.ends_with("/tests")), "{dirs:?}");
    assert!(is_test_file("crates/rc-game/tests/hero/hero_novalis.rs") && is_test_file("tools/trace/tests/trace/main.rs"));
    assert!(!is_test_file("crates/rc-extract/src/main.rs"));
}

/// The pattern matchers. The examples are assembled at run time so this file itself holds no personal path.
#[test]
fn personal_patterns_match_segments_only() {
    let (w, ps2, p2s) = ("work", "PS2", "p2s");
    assert_eq!(personal_hits(&format!("see {w}/decomp/level01.elf")), [PERSONAL_PATTERNS[3]]);
    assert!(personal_hits("the framework/ folder and network/io").is_empty());
    assert_eq!(personal_hits(&format!("~/{ps2}/ratchet1/savestates/x.{p2s}")), PERSONAL_PATTERNS[..3]);
    assert_eq!(personal_hits(&format!("{} {} 2026-09-29.{}", "Screen", "Recording", "mov")), PERSONAL_PATTERNS[6..]);
    assert!(personal_hits("self.moved[i] = true; m.movie").is_empty());
    let (iso, home) = ("RC_ISO", "HOME");
    assert_eq!(test_only_hits(&format!("std::env::var_os(\"{iso}\")")), [TEST_ONLY_PATTERNS[1]]);
    assert_eq!(test_only_hits(&format!("std::env::var(\"{home}\")")).len(), 1);
    assert!(test_only_hits(&format!(".env_remove(\"{iso}\")")).is_empty());
    assert!(test_only_hits("o::HOME, std::env::temp_dir()").is_empty());
}

/// Rule: the top level holds only crates tools docs extracted work target dist, dotfiles, and the Cargo /
/// README / CHANGELOG / LICENSE files.
#[test]
fn top_level_holds_only_the_agreed_folders() {
    let root = repo_root();
    let mut bad = Vec::new();
    for e in std::fs::read_dir(&root).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') { continue; }
        let ok = if e.path().is_dir() { TOP_LEVEL_DIRS.contains(&name.as_str()) } else { TOP_LEVEL_FILES.contains(&name.as_str()) };
        if !ok { bad.push(name); }
    }
    bad.sort();
    assert!(bad.is_empty(), "unexpected top-level entries (product goes in crates/, dev tools in tools/, output in the work folder): {bad:?}");
}

/// Rule 4: tests run only through `cargo xtask test-*` (docs/workflows/testing.md §1). No Cargo alias runs tests, and
/// no doc, README or source comment recommends the retired alias or (in a workflow doc) a by-hand `cargo test`.
#[test]
fn tests_run_only_through_xtask() {
    let root = repo_root();
    let mut files: Vec<std::path::PathBuf> = ["README.md", ".cargo/config.toml", ".config/nextest.toml"].iter().map(|f| root.join(f)).collect();
    for d in ["crates", "tools", "docs"] { files.extend(files_under(&root.join(d))); }
    let (mut hits, mut scanned) = (Vec::new(), 0);
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        scanned += 1;
        let rel = f.strip_prefix(&root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let rel = normalise(Path::new(&rel)).to_string_lossy().into_owned();
        for (n, line) in text.lines().enumerate() {
            let pats = stale_test_command_hits(&rel, line);
            if !pats.is_empty() { hits.push(format!("{rel}:{}: {pats:?}: {}", n + 1, line.trim())); }
        }
    }
    assert!(scanned > 100, "scanned only {scanned} files");
    assert!(hits.is_empty(), "stale test commands (use cargo xtask test-quick | test-job | test-full, testing.md §1):\n{}", hits.join("\n"));
}

/// The stale-command matcher. The examples are assembled at run time so this file itself holds no hit.
#[test]
fn stale_test_command_matcher() {
    let alias = format!("cargo test{}", "-all");
    assert_eq!(stale_test_command_hits("crates/x/tests/a/main.rs", &format!("//! run one with `{alias} --test a`")), [RETIRED_TEST_ALIAS]);
    assert!(stale_test_command_hits("docs/workflows/testing.md", &format!("the old `{alias}` alias was removed")).is_empty());
    let ws = format!("cargo test --{}", "workspace");
    assert_eq!(stale_test_command_hits("README.md", &format!("Run `{ws}`.")).len(), 1);
    assert!(stale_test_command_hits("README.md", &format!("plain `{ws}` rebuilds Bevy")).is_empty());
    assert!(stale_test_command_hits("docs/plan/particles.md", &format!("`{ws}` green")).is_empty(), "plan records are history");
    assert_eq!(stale_test_command_hits(".cargo/config.toml", &format!("t{} = \"test --workspace\"", "a")).len(), 1);
    assert!(stale_test_command_hits(".cargo/config.toml", "check-all = \"check --workspace --all-targets\"").is_empty());
    assert!(is_workflow_doc("tools/trace/README.md") && !is_workflow_doc("docs/plan/particles.md"));
}
