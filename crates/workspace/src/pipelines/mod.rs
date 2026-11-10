//! The project's pipelines as the UI needs them: the library, creating a pipeline, the version
//! history, the steps' dependency levels, and reading, saving and publishing through the project
//! database. Pipeline rules themselves are `orchester_core::resource`; this module only maps between
//! them and what is on screen. The editor's working state and the canvas are each app's.

pub mod create;
pub mod data;
pub mod default;
pub mod deliver;
pub mod describe;
pub mod fields;
pub mod flow;
pub mod history;
pub mod hub;
pub mod levels;
pub mod library;
pub mod local_edit;
pub mod narrowing;
pub mod problems;
pub mod rename;
pub mod sources;
pub mod task_use;
pub mod widening;
pub mod write;
