//! Orchester desktop UI. `main.rs` only wires the window; everything else lives here so
//! it can be tested from `tests/`. The application layer under the screens is
//! `orchester-workspace`, re-exported here under its old module names.

pub mod activity;
pub mod agents;
pub mod app_icon;
pub mod app_log;
pub mod app_mcp;
pub mod close;
pub mod drafts;
pub mod embedded_mcp;
pub mod i18n;
pub mod navigation;
pub mod pipelines;
pub mod root;
mod screens;
pub mod sharing;
pub mod tabs;
pub mod tasks;
pub mod theme;
mod ui;

use orchester_exec::external;
pub use orchester_exec::programs;
use orchester_workspace::background;
pub use orchester_workspace::{
  app_paths, budget, clock, code_index, connections, display, environment, highlight, import, instructions, messages,
  plan_limits, project_setup, pull_requests, recovery, repository, runner, secrets, session, skills, tools, workspace,
};
pub use screens::{close_dialog, recovery_dialog, status_bar};
pub use ui::{command_row, loader, markdown, pressable, unit_input::FieldStroke};
