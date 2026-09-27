//! The project's pipelines as the UI needs them: the library, creating a pipeline, the
//! editor's working state, the canvas layout and zoom, the version history, and reading,
//! saving and publishing through the project database. Pipeline rules themselves are
//! `orchester_core::resource`; this module only maps between them and what is on screen.

pub mod data;
pub mod describe;
pub mod editor;
pub mod fields;
pub mod history;
pub mod layout;
pub mod library;
pub mod problems;
pub mod zoom;
