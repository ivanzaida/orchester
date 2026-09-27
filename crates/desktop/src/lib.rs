//! Orchester desktop UI. `main.rs` only wires the window; everything else lives here so
//! it can be tested from `tests/`.

pub mod activity;
pub mod agents;
pub mod app_paths;
mod background;
pub mod budget;
pub mod close;
pub mod connections;
pub mod display;
pub mod drafts;
mod external;
pub mod i18n;
pub mod messages;
pub mod navigation;
pub mod pipelines;
pub mod programs;
pub mod project_setup;
pub mod pull_requests;
pub mod recovery;
pub mod repository;
pub mod root;
pub mod runner;
mod screens;
pub use screens::{close_dialog, recovery_dialog};
pub mod session;
pub mod tasks;
pub mod theme;
mod ui;
pub use ui::{pressable, unit_input::FieldStroke};
pub mod workspace;
