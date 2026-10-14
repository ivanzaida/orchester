//! The project's Overview on the QA seed's projects (`examples/seed`), in the app's real window: its
//! sections and states (`states`) and where its actions lead (`actions`). No runner serves the project, so a
//! run recorded as running reads as interrupted. Needs `git`, as the seed does.

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
mod states;
