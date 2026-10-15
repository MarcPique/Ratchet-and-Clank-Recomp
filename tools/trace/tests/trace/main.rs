//! Integration tests: the trace harness: synthetic states, the hero replay self-check and the Novalis spawn facts (docs/workflows/testing.md §3).
//! One module per former test file; run one with `cargo xtask test-job --test trace --filter <module>::`.

mod hero_replay_selfcheck;
mod novalis_spawn;
mod synthetic;
