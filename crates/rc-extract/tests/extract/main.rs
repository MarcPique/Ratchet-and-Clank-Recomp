//! Integration tests: the extractor on synthetic discs (docs/workflows/testing.md §3). The checks against the real
//! disc image are a dev command, not tests (no test reads personal files): `cargo run --release -p rc-trace --
//! disc-check`. One module per former test file; run one with `cargo xtask test-job --test extract --filter <module>::`.

mod synthetic;
