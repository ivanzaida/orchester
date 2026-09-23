//! Orchester domain model.
//!
//! Must stay free of UI and transport dependencies: the desktop app uses it
//! directly today, and a future server will share it.

pub mod id;
pub mod money;
pub mod name;
pub mod project;
pub mod resource;
pub mod task;
pub mod time;
