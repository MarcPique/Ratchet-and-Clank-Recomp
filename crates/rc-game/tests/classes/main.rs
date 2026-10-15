//! Integration tests: the moby class ports: common and cheap classes, creatures, breakables, bolts and path classes (docs/workflows/testing.md §3).
//! One module per former test file; run one with `cargo xtask test-job --test classes --filter <module>::`.

#[path = "../common/mod.rs"]
mod common;

mod bolt_crank_novalis;
mod breakables_levels;
mod cheap_classes_a;
mod cheap_classes_b;
mod cheap_classes_c;
mod cheap_classes_d;
mod cheap_classes_e;
mod cheap_classes_f;
mod common_classes_levels;
mod creature_classes;
mod creatures_w3;
mod creatures_enemies_novalis;
mod creatures_novalis;
mod enemy_units;
mod explosion_survey;
mod gold_bolt_infobot_novalis;
mod help_directors;
mod manipulators;
mod moby_services;
mod moby_update_novalis;
mod orxon_gemlik_enemies;
mod particle_consumers;
mod path_classes;
mod quick_wins;
mod scripted_consumers;
