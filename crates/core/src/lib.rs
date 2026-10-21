//! Orchester domain model.
//!
//! Must stay free of UI and transport dependencies: the desktop app uses it
//! directly today, and a future server will share it.

pub mod account;
pub mod agent_files;
pub mod app_mcp;
pub mod appearance;
pub mod attachment;
pub mod base_move;
pub mod blob;
pub mod code_index;
pub mod command;
pub mod connection;
pub mod conversation;
pub mod diff;
pub mod github;
pub mod id;
pub mod import;
pub mod integration;
pub mod limits;
pub mod memory;
pub mod module;
pub mod name;
pub mod native;
pub mod notification;
pub mod orchester_tools;
pub mod orchestrator;
pub mod permission;
pub mod private_file;
pub mod project;
pub mod pull_request;
pub mod question;
pub mod repository;
pub mod resource;
pub mod revision;
pub mod run;
pub mod secret;
pub mod sharing;
pub mod source;
pub mod store;
pub mod task;
pub mod task_group;
pub mod task_selection;
pub mod time;
pub mod tokens;
pub mod usage;
