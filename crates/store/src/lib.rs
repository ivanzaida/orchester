//! Local persistence, as described in `docs/architecture/storage.md`: one SQLite database per
//! project in `<repo>/.orchester/`, and one application database next to the executable.

mod app_db;
mod error;
mod git_exclude;
mod migrate;
mod project_db;
mod rows;

pub use app_db::{AppDb, PreviousSession, RegisteredProject, SessionEnd, SessionId};
pub use error::StoreError;
pub use project_db::ProjectDb;
