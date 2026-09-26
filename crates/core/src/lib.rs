//! Orchester domain model.
//!
//! Must stay free of UI and transport dependencies: the desktop app uses it
//! directly today, and a future server will share it.

pub mod command;
pub mod connection;
pub mod github;
pub mod id;
pub mod limits;
pub mod name;
pub mod project;
pub mod pull_request;
pub mod repository;
pub mod resource;
pub mod revision;
pub mod run;
pub mod store;
pub mod task;
pub mod time;
pub mod tokens;
