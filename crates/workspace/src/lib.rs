//! Orchester's local application layer, shared by the desktop apps (`orchester-desktop`, being
//! replaced, and `orchester-app`): the application session and its projects, the runners' client
//! side and the session's background activity, project data as screens and automation read and
//! change it, and the tools of Orchester's own MCP server. Results that a person reads carry
//! [`messages::Message`] keys of this crate's locale files ([`i18n`]); no UI toolkit is used here.

pub mod accounts;
pub mod activity;
pub mod agents;
pub mod app_mcp;
pub mod app_paths;
pub mod background;
pub mod budget;
pub mod build;
pub mod clock;
pub mod close;
pub mod code_index;
pub mod connections;
pub mod display;
pub mod drafts;
pub mod environment;
pub mod highlight;
pub mod i18n;
pub mod import;
#[cfg(any(feature = "devtools-inspect", feature = "devtools-inspect-legacy"))]
mod inspect;
pub mod instructions;
pub mod messages;
pub mod orchestrator;
pub mod pipelines;
pub mod plan_limits;
pub mod project_list;
pub mod project_settings;
pub mod project_setup;
pub mod pull_requests;
pub mod recovery;
pub mod repository;
pub mod runner;
pub mod secrets;
pub mod session;
pub mod sharing;
pub mod skills;
pub mod tasks;
pub mod tools;
pub mod workspace;
