//! Local persistence, as described in `docs/architecture/storage.md`: one SQLite database per
//! project in `<repo>/.orchester/`, and one application database next to the executable (`location`).

mod app_db;
mod error;
mod git_exclude;
pub mod location;
mod migrate;
mod project_db;
mod rows;

pub use app_db::{
  AcceptedInvite, AppDb, CloudSignInEnd, EndedCloudSignIn, JoinedWith, PreviousSession, RegisteredProject,
  ServerAccount, SessionEnd, SessionId, StoredCloudAccount, StoredServer,
};
pub use error::{RunStoreError, StoreError};
pub use git_exclude::is_repository_root;
#[cfg(feature = "test-support")]
pub use project_db::ProjectMigration;
pub use project_db::{
  AutoModeSave, AutomaticStatus, CachedClaim, CachedRunStatus, CachedTask, ChatListing, ChatTurn, DecisionCounts,
  JoinedKey, KeptAction, KeptWait, NotSent, ProjectDb, ReadOnlyProjectDb, ResourceUpdate, ReturnedTask, RunBranch,
  SharedMember, SharedPerson, SharedTaskInfo, SharingLink, UnsentStatus,
};
