//! The project's tasks as the UI needs them: the new task form, the task list and its tabs,
//! and reading and saving through the project database. Task rules themselves are
//! `orchester_core::task`; this module only maps between them and what is on screen.

pub mod data;
pub mod detail;
pub mod form;
pub mod list;
pub mod overview;
pub mod progress;
pub mod run;
pub mod transcript;
pub mod usage;
