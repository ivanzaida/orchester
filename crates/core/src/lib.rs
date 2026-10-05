//! Orchester domain model.
//!
//! Must stay free of UI and transport dependencies: the desktop app uses it
//! directly today, and a future server will share it.

pub mod agent_files;
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
pub mod name;
pub mod native;
pub mod orchester_tools;
pub mod permission;
pub mod project;
pub mod pull_request;
pub mod repository;
pub mod resource;
pub mod revision;
pub mod run;
pub mod secret;
pub mod sharing;
pub mod store;
pub mod task;
pub mod task_group;
pub mod task_selection;
pub mod time;
pub mod tokens;
