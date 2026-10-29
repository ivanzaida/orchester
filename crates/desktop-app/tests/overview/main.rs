//! The project's Overview on the QA seed's projects (`examples/seed`), in the app's real window: its
//! sections and states (`states`) and where its links lead (`actions`); and its sections as data on states made
//! here (`data`). No runner serves the project, so a run recorded as running reads as interrupted: *At work* keeps
//! it as read. Needs `git`, as the seed does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod actions;
mod data;
mod harness;
mod states;
