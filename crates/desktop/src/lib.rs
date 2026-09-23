//! Orchester desktop UI. `main.rs` only wires the window; everything else lives here so
//! it can be tested from `tests/`.

pub mod app_paths;
mod background;
pub mod close;
pub mod display;
pub mod i18n;
pub mod messages;
mod navigation;
pub mod project_setup;
pub mod root;
pub mod runner;
mod screens;
pub mod session;
pub mod theme;
mod ui;
pub mod workspace;
