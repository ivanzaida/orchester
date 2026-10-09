//! Orchester's desktop app in the orchester2 design (`data/orchester2.pen`, Kontur's design language). It
//! replaces `orchester-desktop`, which stays as it is until this app takes over its screens.
//!
//! The window is a shell (`chrome`, `shell`) around routed screens (`routes`, `screens`), moved around
//! through tabs with their own histories (`navigation`), styled only through lurq theme roles (`theme`),
//! worded only through i18n keys (`i18n`), and open to agents through one MCP server (`mcp`).

pub mod app_log;
pub mod appearance;
pub mod brand;
mod chrome;
pub mod i18n;
pub mod mcp;
pub mod navigation;
pub mod root;
pub mod routes;
mod screens;
mod shell;
pub mod theme;
mod ui;
