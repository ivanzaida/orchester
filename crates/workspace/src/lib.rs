//! Orchester's local application layer of the desktop app (`orchester-app`): the application
//! session and its projects, the runners' client side and the session's background activity,
//! project data as screens and automation read and change it, and the tools of Orchester's own MCP
//! server. Results that a person reads carry [`messages::Message`] keys of this crate's locale files
//! ([`i18n`]); no UI toolkit is used here.

pub mod accounts;
pub mod activity;
pub mod agents;
pub mod app_mcp;
pub mod app_paths;
pub mod attachments;
pub mod background;
pub mod base_moved;
pub mod browser;
pub mod budget;
pub mod build;
pub mod clock;
pub mod close;
pub mod code_index;
pub mod connections;
pub mod decisions;
pub mod delivery;
pub mod display;
pub mod drafts;
pub mod environment;
pub mod files;
pub mod highlight;
pub mod i18n;
pub mod import;
#[cfg(feature = "devtools-inspect")]
mod inspect;
pub mod instructions;
pub mod links;
pub mod memory;
pub mod merge_queue;
pub mod messages;
pub mod modules;
pub mod notifications;
pub mod orchestrator;
pub mod overview;
pub mod pipelines;
pub mod plan_limits;
pub mod project_list;
pub mod project_settings;
pub mod project_setup;
pub mod prompt_changes;
pub mod pull_requests;
pub mod recovery;
pub mod removal;
pub mod repository;
pub mod runner;
pub mod secrets;
pub mod session;
pub mod sharing;
pub mod skills;
pub mod sources;
pub mod tasks;
pub mod tools;
pub mod usage;
pub mod warm_workspaces;
pub mod workspace;
