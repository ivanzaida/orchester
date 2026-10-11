//! The project's pipelines as the UI needs them: the library, creating a pipeline, the version
//! history, the steps' dependency levels, and reading, saving and publishing through the project
//! database. Pipeline rules themselves are `orchester_core::resource`; this module only maps between
//! them and what is on screen. The editor's working state and the canvas are each app's.

pub mod create;
pub mod data;
pub mod describe;
pub mod fields;
pub mod history;
pub mod levels;
pub mod library;
pub mod narrowing;
pub mod problems;
pub mod widening;
pub mod write;
