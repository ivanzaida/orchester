//! The project's tasks as the UI needs them: the new task form, the task list and its tabs,
//! and reading and saving through the project database. Task rules themselves are
//! `orchester_core::task`; this module only maps between them and what is on screen.

pub mod changes;
pub mod conversation;
pub mod data;
pub mod detail;
pub mod form;
pub mod integration;
pub mod integration_jobs;
pub mod list;
pub mod organization;
pub mod overview;
pub mod permissions;
pub mod plan;
pub mod progress;
pub mod run;
pub mod run_again;
pub mod tag_chips;
pub mod transcript;
pub mod unreadable;
pub mod usage;
