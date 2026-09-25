//! Orchester desktop UI. `main.rs` only wires the window; everything else lives here so
//! it can be tested from `tests/`.

pub mod activity;
pub mod agents;
pub mod app_paths;
mod background;
pub mod close;
pub mod connections;
pub mod display;
pub mod i18n;
pub mod messages;
mod navigation;
pub mod pipelines;
pub mod project_setup;
pub mod repository;
pub mod root;
pub mod runner;
mod screens;
pub use screens::close_dialog;
pub mod session;
pub mod tasks;
pub mod theme;
mod ui;
pub use ui::pressable;
pub mod workspace;
