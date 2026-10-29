//! Local persistence, as described in `docs/architecture/storage.md`: one SQLite database per
//! project in `<repo>/.orchester/`, and one application database next to the executable (`location`).

mod app_db;
mod busy;
mod error;
mod git_exclude;
pub mod location;
mod lock_watch;
mod migrate;
mod project_db;
mod rows;

pub use app_db::{
  AcceptedInvite, AppDb, CloudSignInEnd, EndedCloudSignIn, JoinedWith, PreviousSession, RegisteredProject,
  ServerAccount, SessionEnd, SessionId, StoredCloudAccount, StoredCloudAddress, StoredServer,
};
pub use busy::pause_lock_waits_with;
#[cfg(feature = "test-support")]
pub use busy::{fail_next_writes_for_test, fail_next_writes_of_for_test, set_lock_budget_for_test};
pub use error::{RunStoreError, StoreError};
pub use git_exclude::is_repository_root;
#[cfg(feature = "test-support")]
pub use lock_watch::worst::{LockHold, LockHolds, lock_holds, reset_lock_holds};
#[cfg(feature = "test-support")]
pub use project_db::ProjectMigration;
pub use project_db::{
  AttachmentOwner, AutoModeSave, AutomaticStatus, AutomationPoints, Bound, BoundBy, BoundReference, CachedClaim,
  CachedModule, CachedModuleChange, CachedRunStatus, CachedTask, CachedVersion, ChatListing, ChatTurn, DecisionCounts,
  EndedRun, JoinedKey, KeptAction, KeptReport, KeptWait, NotSent, ProjectDb, ReadOnlyProjectDb, ReferenceBinding,
  ReferenceTarget, ResolutionStart, ResourceSharing, ResourceUpdate, ReturnedTask, RunBranch, SharedMember,
  SharedPerson, SharedTaskInfo, SharingLink, StartedQueued, TaskDeletionFacts, UnsentStatus, VersionReference,
  attachment_file, exclude_project_data,
};
