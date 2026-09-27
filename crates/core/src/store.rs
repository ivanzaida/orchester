//! Persistence boundaries. The local SQLite store implements them today; a server client
//! will implement the same traits later, so domain code depends only on these.

use std::collections::BTreeMap;

use crate::{
  connection::{Connection, stream::AgentEvent},
  diff::RevisionDiff,
  id::{ConnectionId, IdempotencyKey, InvocationId, ResourceId, RunId, StepExecutionId, TaskId},
  limits::ProjectLimits,
  name::Name,
  pull_request::{PullRequest, SyncedPullRequests},
  repository::{GitHubRepo, RepositoryLink},
  resource::{Draft, DraftSaved, ModelId, Resource, ResourceBody, ResourceKind, Slug, Version, VersionNumber},
  revision::Revision,
  run::{ActivityEntry, Checkpoint, Invocation, InvocationPrompt, NewRun, Run, RunEvent, RunNumber, StepOutput},
  task::{Task, TaskDetails},
  time::Timestamp,
  tokens::TokensUsed,
};

pub trait TaskStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates `details.len()` draft tasks with consecutive numbers, atomically. Repeating the
  /// call with the same `batch` returns the tasks created the first time instead of new ones.
  fn create_tasks(
    &mut self,
    batch: IdempotencyKey,
    details: Vec<TaskDetails>,
    now: Timestamp,
  ) -> Result<Vec<Task>, Self::Error>;

  fn task(&self, id: TaskId) -> Result<Option<Task>, Self::Error>;

  /// All tasks of the project, ordered by number.
  fn tasks(&self) -> Result<Vec<Task>, Self::Error>;

  /// Persists the details and status of an existing task.
  fn save_task(&mut self, task: &Task) -> Result<(), Self::Error>;
}

/// Project settings. Written by the desktop; the runner reads them.
pub trait SettingsStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// The project's limits: the defaults until they were changed.
  fn project_limits(&self) -> Result<ProjectLimits, Self::Error>;

  fn save_project_limits(&mut self, limits: &ProjectLimits) -> Result<(), Self::Error>;
}

/// Versioned configuration resources. Reading a body of the wrong kind is an error, not `None`.
pub trait ResourceStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates a resource of kind `B::KIND` whose first draft is `body`, atomically. The slug is
  /// unique among resources of the same kind.
  fn create_resource<B: ResourceBody>(
    &mut self,
    slug: Slug,
    name: Name,
    body: &B,
    now: Timestamp,
  ) -> Result<Resource, Self::Error>;

  fn resource(&self, id: ResourceId) -> Result<Option<Resource>, Self::Error>;

  /// Resources of one kind, ordered by slug.
  fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, Self::Error>;

  /// Stores `draft` as the resource's only draft, replacing an existing one. A draft that
  /// repeats the version it is based on ([`Draft::repeats`]) removes the stored draft instead.
  fn save_draft<B: ResourceBody>(&mut self, resource: ResourceId, draft: &Draft<B>) -> Result<DraftSaved, Self::Error>;

  fn draft<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Draft<B>>, Self::Error>;

  /// Publishes the stored draft as the next version and removes the draft, atomically. A draft
  /// not based on the latest version is rejected, see [`crate::resource::publish`], and so is
  /// a body that fails [`ResourceBody::check_publishable`].
  fn publish_draft<B: ResourceBody>(&mut self, resource: ResourceId, now: Timestamp)
  -> Result<Version<B>, Self::Error>;

  fn latest_version<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Version<B>>, Self::Error>;

  /// Every published version of a resource, oldest first.
  fn versions<B: ResourceBody>(&self, resource: ResourceId) -> Result<Vec<Version<B>>, Self::Error>;

  fn version<B: ResourceBody>(
    &self,
    resource: ResourceId,
    number: VersionNumber,
  ) -> Result<Option<Version<B>>, Self::Error>;
}

/// Project connections. Written by the desktop; the runner only reads them.
pub trait ConnectionStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores a new connection. Names are unique within the project.
  fn create_connection(&mut self, connection: &Connection) -> Result<(), Self::Error>;

  /// Stores the current name, executable and last check of an existing connection.
  fn save_connection(&mut self, connection: &Connection) -> Result<(), Self::Error>;

  fn connection(&self, id: ConnectionId) -> Result<Option<Connection>, Self::Error>;

  /// All connections of the project, ordered by name.
  fn connections(&self) -> Result<Vec<Connection>, Self::Error>;
}

/// The project's link to its GitHub repository; a project has at most one. Written by the
/// desktop.
pub trait RepositoryStore {
  type Error: std::error::Error + Send + Sync + 'static;

  fn repository_link(&self) -> Result<Option<RepositoryLink>, Self::Error>;

  /// Stores `link` as the project's link, replacing an existing one. Its connection must be a
  /// GitHub connection of the project.
  fn save_repository_link(&mut self, link: &RepositoryLink) -> Result<(), Self::Error>;

  /// Unlinks the project from GitHub; nothing happens if it is not linked.
  fn remove_repository_link(&mut self) -> Result<(), Self::Error>;
}

/// The last list of a repository's open pull requests read from GitHub, kept so the list shows
/// while GitHub cannot be reached. GitHub is the source; this is a cache. Written by the desktop.
pub trait PullRequestStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Replaces the stored pull requests of `repo` with `pull_requests`, read at `synced_at`,
  /// atomically.
  fn replace_pull_requests(
    &mut self,
    repo: &GitHubRepo,
    pull_requests: &[PullRequest],
    synced_at: Timestamp,
  ) -> Result<(), Self::Error>;

  /// The pull requests of `repo` as last read, most recently updated first; `None` before the
  /// first sync of `repo`.
  fn pull_requests(&self, repo: &GitHubRepo) -> Result<Option<SyncedPullRequests>, Self::Error>;
}

/// Pull request diffs, one per head revision (`pr_revisions` with content-addressed blobs). GitHub
/// is the source; this is a cache that is never pruned. Written by the desktop.
pub trait PullRequestDiffStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores `diff` as the diff of pull request `number` of `repo` at `diff.head`, replacing the
  /// one stored for that head. The text is stored once however many heads share it.
  fn save_pull_request_diff(&mut self, repo: &GitHubRepo, number: u64, diff: &RevisionDiff) -> Result<(), Self::Error>;

  /// The stored diff of pull request `number` of `repo` at `head`.
  fn pull_request_diff(
    &self,
    repo: &GitHubRepo,
    number: u64,
    head: &Revision,
  ) -> Result<Option<RevisionDiff>, Self::Error>;

  /// The most recently synced diff of pull request `number` of `repo`, whatever its head.
  fn latest_pull_request_diff(&self, repo: &GitHubRepo, number: u64) -> Result<Option<RevisionDiff>, Self::Error>;
}

/// Diffs between two commits of the project's repository, read with git: one per pair of commits
/// (`revision_diffs` with content-addressed blobs), since the diff of two commits does not
/// change. A cache that is never pruned, written by whoever reads the diff (the desktop).
pub trait RevisionDiffStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores `diff` as the diff from `diff.base` to `diff.head`, replacing a stored one.
  fn save_revision_diff(&mut self, diff: &RevisionDiff) -> Result<(), Self::Error>;

  fn revision_diff(&self, base: &Revision, head: &Revision) -> Result<Option<RevisionDiff>, Self::Error>;
}

/// Execution data, written by the runner. A run changes only by appending the events a
/// transition returned; the resulting state is stored alongside in the same transaction.
pub trait RunStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates a pending run of `new.task` with the project's next run number and stores its
  /// creation event, atomically. Repeating the call with the same `key` returns the run
  /// created the first time instead of a new one.
  fn create_run(&mut self, key: IdempotencyKey, new: NewRun, now: Timestamp) -> Result<Run, Self::Error>;

  /// Appends `events`, which must not be empty, and stores `run`, the state after them,
  /// atomically. The first event must directly follow the stored log; otherwise another writer
  /// appended first and the call fails with a sequence conflict, changing nothing.
  fn append_events(&mut self, run: &Run, events: &[RunEvent]) -> Result<(), Self::Error>;

  fn run(&self, id: RunId) -> Result<Option<Run>, Self::Error>;

  /// Runs of a task, ordered by number.
  fn runs_of_task(&self, task: TaskId) -> Result<Vec<Run>, Self::Error>;

  /// The complete event log of a run, in sequence order.
  fn events(&self, run: RunId) -> Result<Vec<RunEvent>, Self::Error>;

  fn latest_checkpoint(&self, run: RunId) -> Result<Option<Checkpoint>, Self::Error>;

  /// Interrupted runs of the project, ordered by number, for recovery.
  fn interrupted_runs(&self) -> Result<Vec<Run>, Self::Error>;

  /// Runs recorded as running, ordered by number: a runner executes them, or the runner or the
  /// session ended before it could record their interruption.
  fn running_runs(&self) -> Result<Vec<Run>, Self::Error>;

  /// The numbers of the runs that pinned each published version of pipeline `pipeline`, in run
  /// order. A version no run pinned is absent.
  fn runs_by_pipeline_version(
    &self,
    pipeline: ResourceId,
  ) -> Result<BTreeMap<VersionNumber, Vec<RunNumber>>, Self::Error>;
}

/// Agent CLI invocations and their transcripts, written by the runner. Transcript entries are
/// append-only.
pub trait ActivityStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Records a new launch of the agent CLI for `execution` of `run`, numbered after the
  /// execution's earlier ones.
  fn start_invocation(
    &mut self,
    run: RunId,
    execution: StepExecutionId,
    model: &ModelId,
    now: Timestamp,
  ) -> Result<Invocation, Self::Error>;

  /// Stores the session, usage, end and outcome of an existing invocation.
  fn save_invocation(&mut self, invocation: &Invocation) -> Result<(), Self::Error>;

  /// Invocations of a run, in launch order.
  fn invocations(&self, run: RunId) -> Result<Vec<Invocation>, Self::Error>;

  /// Stores what `invocation` was told and where each part came from; an invocation has at most
  /// one prompt, written once before its CLI starts.
  fn save_invocation_prompt(&mut self, invocation: InvocationId, prompt: &InvocationPrompt) -> Result<(), Self::Error>;

  /// The prompt sources of `invocation`; `None` for one that never started or was recorded before
  /// prompts had sources.
  fn invocation_prompt(&self, invocation: InvocationId) -> Result<Option<InvocationPrompt>, Self::Error>;

  /// Appends `events` of `invocation` to its run's transcript, atomically, and returns the
  /// sequence of the last one (of the transcript so far when `events` is empty).
  fn append_activity(
    &mut self,
    invocation: &Invocation,
    events: &[(Timestamp, AgentEvent)],
  ) -> Result<u64, Self::Error>;

  /// Transcript entries of a run after sequence `after` (0: from the start), in order.
  fn activity(&self, run: RunId, after: u64) -> Result<Vec<ActivityEntry>, Self::Error>;

  /// Tokens that counted toward budgets in every run of the project since `since`, from the
  /// usage each completed turn reported.
  fn tokens_used_since(&self, since: Timestamp) -> Result<TokensUsed, Self::Error>;

  /// Stores what a step execution left for the steps after it; an execution has at most one
  /// output, and storing it again changes nothing.
  fn save_step_output(&mut self, output: &StepOutput) -> Result<(), Self::Error>;

  /// Outputs of a run's step executions, in the order they were stored.
  fn step_outputs(&self, run: RunId) -> Result<Vec<StepOutput>, Self::Error>;
}
