//! Local persistence, as described in `docs/architecture/storage.md`: one SQLite database per
//! project in `<repo>/.orchester/`, and one application database next to the executable.

mod app_db;
mod error;
mod git_exclude;
mod migrate;
mod project_db;
mod rows;

pub use app_db::{AppDb, PreviousSession, RegisteredProject, ServerAccount, SessionEnd, SessionId, StoredServer};
pub use error::{RunStoreError, StoreError};
pub use git_exclude::is_repository_root;
pub use project_db::{
  AutomaticStatus, CachedClaim, CachedRunStatus, CachedTask, DecisionCounts, JoinedKey, NotSent, ProjectDb,
  ReadOnlyProjectDb, ReturnedTask, SharedPerson, SharedTaskInfo, SharingLink, UnsentStatus,
};
