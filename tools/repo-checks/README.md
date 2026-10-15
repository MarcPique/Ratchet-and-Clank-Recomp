# tools/repo-checks: layout guard tests

Dev only, never ships. No dependencies; it exists for its tests. Rules: `docs/plan/repo_reorg.md`.

* **Command:** `cargo xtask test-job repo` (the `guards` binary; also part of `cargo xtask test-full`).
* **Inputs:** the repository tree, read as text. **Outputs:** none.
* **Checks:**
  1. No product crate (`crates/*/Cargo.toml`) depends on anything under `tools/` (every `path = "..."` is resolved,
     and no manifest names a tool package: `rc-trace`, `repo-checks`, `xtask`).
  2. No test relies on personal files (docs/workflows/testing.md §10). Every file under `crates/` and every tool's
     `tests/` folder is free of `.p2s`, `~/PS2`, `PS2/ratchet1`, `work/`, `/traces/`, `RC_PERSONAL`, `.mov` and
     `Screen Recording`; files under a `tests/` folder also never read `RC_ISO` or `HOME` (`home_dir`,
     `Application Support`). There is no allow-list. Tool sources (`tools/*/src`) are not scanned: the dev tools read
     personal material on purpose (`src/lib.rs` `guarded_dirs`).
  3. The top level holds only `crates tools docs extracted work target dist`, dotfiles, `Cargo.toml`, `Cargo.lock`,
     `README.md`, `CHANGELOG.md` and `LICENSE`.
  4. Tests run only through `cargo xtask test-*` (docs/workflows/testing.md §1): no `.cargo/config.toml` alias runs
     `test` or `nextest`; no file under `crates/`, `tools/`, `docs/` (or `README.md`, `.config/nextest.toml`) names
     the retired `cargo test-all` alias except as the alias; and the workflow docs (`README.md`, `docs/workflows/`,
     `docs/plan/orchestration.md`, `tools/*/README.md`) recommend no by-hand `cargo test --workspace` / `-p` (a line
     that explains why not, with "plain", "never" or "by-hand", or names the xtask fallback's `rc-engine/dev` flags,
     is fine). Plan docs keep their historical records.
