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
pub use lock_watch::statements::statements_run;
#[cfg(feature = "test-support")]
pub use lock_watch::worst::{LockHold, LockHolds, lock_holds, reset_lock_holds};
#[cfg(feature = "test-support")]
pub use project_db::ProjectMigration;
pub use project_db::{
  AcceptedInputs, AcceptedWork, ActivityRecord, AlwaysAllowAnswer, AlwaysAllowRecord, ApprovalRecord, ApprovedPlan,
  ArchiveStop, AttachmentOwner, AutoModeSave, AutomaticStatus, AutomationPoints, Bound, BoundBy, BoundReference,
  CachedClaim, CachedModule, CachedModuleChange, CachedRunStatus, CachedTask, CachedVersion, ChatDeletion, ChatFork,
  ChatListing, ChatRewind, ChatTurn, ConfirmedVersion, DecisionCounts, DeliveryMerge, DeliveryReport, EndedRun,
  InvocationRecord, InvocationStart, JoinedKey, KeptAction, KeptReport, KeptRunWait, KeptSubscription, KeptWait,
  MemoryAnswer, MemoryRecord, MessageRecord, ModelRequirement, NamingVersion, NewTeamOperation, NewWorkReport, NotSent,
  NoticeRecord, ProjectDb, PublishState, QuestionRecord, ReadOnlyProjectDb, RecordedDecisionReview, ReferenceBinding,
  ReferenceTarget, RemovalRecord, ResolutionStart, ResourceSharing, ResourceUpdate, ReturnedTask, RuleAnswer,
  RuleRecord, RunBranch, RunLogRecord, RunWaitEnd, RunWaitEnded, ServerAdmin, SharedEvent, SharedMember, SharedPerson,
  SharedSnapshot, SharedStanding, SharedTaskInfo, SharingLink, StartedQueued, StepRecord, TaskDeletionFacts,
  TeamDelivery, TeamDeliveryStage, TeamFence, TeamOperation, TeamOperationState, TeamRun, TeamSettlement, TeamStart,
  TeamStartStage, UnsentStatus, VersionReference, WaitingPublish, WaitingVersion, WarmWorkspaceRecord, WatchStart,
  WorkReportEntry, WorkReportState, WorkspaceWritten, attachment_file, exclude_project_data,
};
