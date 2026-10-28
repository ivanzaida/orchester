//! The project's Usage page on the QA seed's `usage` and `usage-empty` scenarios (`examples/seed`), in the
//! app's real window: its states (`states`) and what its controls do (`actions`). Needs `git`, as the seed
//! does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod actions;
mod harness;
mod providers;
mod states;
mod unreported;
