//! Orchester's desktop app in the orchester2 design (`data/orchester2.pen`, Kontur's design language), the program
//! `orchester`. It replaced the deprecated `orchester-desktop` (`crates/desktop`, no longer built).
//!
//! The window is a shell (`chrome`, `shell`) around routed screens (`routes`, `screens`), moved around
//! through tabs with their own histories (`navigation`), styled only through lurq theme roles (`theme`),
//! worded only through i18n keys (`i18n`), and open to agents through one MCP server (`mcp`). Under it is
//! `orchester-workspace`, the application layer: the app starts a session of it (`startup`) or says why it
//! cannot, puts its services together (`services`), and follows the projects its tabs show (`project`).

pub mod app_log;
pub mod appearance;
pub mod brand;
pub mod chrome;
pub mod clipboard;
pub mod i18n;
pub mod lifecycle;
pub mod mcp;
pub mod navigation;
pub mod notifications;
pub mod project;
pub mod root;
pub mod routes;
mod screens;
pub mod services;
pub mod shell;
pub mod startup;
pub mod system_icon;
#[cfg(feature = "test-support")]
pub mod test_support;
pub mod theme;
pub mod ui;
pub mod window;
