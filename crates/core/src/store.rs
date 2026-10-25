//! Persistence boundaries. The local SQLite store implements them today; a server client
//! will implement the same traits later, so domain code depends only on these.

use std::collections::BTreeMap;

use crate::{
  agent_files::{
    RepositoryAgentFiles,
    import::{RepositoryDraft, RepositoryOrigin},
  },
  blob::ContentHash,
  code_index::{CodeIndexSetting, CodeIndexUse},
  connection::{
    Connection, LastCheck,
    mcp::{McpServer, UnreadableMcpServer},
    stream::AgentEvent,
  },
  conversation::Message,
  diff::RevisionDiff,
  id::{ConnectionId, IdempotencyKey, InvocationId, ResourceId, RunId, SecretId, StepExecutionId, TaskId},
  import::{ImportBatch, ImportRecord},
  integration::{RemovalStart, RunMerge},
  limits::ProjectLimits,
  name::Name,
  permission::LaunchPermissions,
  pull_request::{PullRequest, SyncAttempt, SyncedPullRequests},
  repository::{GitHubRepo, RepositoryLink},
  resource::{
    CommandEnvironment, Draft, DraftSaved, ModelId, Resource, ResourceBody, ResourceKind, Slug, Version, VersionNumber,
  },
  revision::Revision,
  run::{
    ActivityEntry, Checkpoint, EditProgress, Invocation, InvocationEngine, InvocationPrompt, ListedRun, NewRun, Plan,
    Run, RunEvent, RunNumber, StepOutput,
  },
  secret::Secret,
  sharing::TaskRevision,
  task::{Task, TaskDetails, TaskUpdate},
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

  /// Persists the details and status of an existing task, read at `task.revision()`. A task changed
  /// since is not overwritten: the result is a conflict with the current task.
  fn save_task(&mut self, task: &Task) -> Result<TaskUpdate, Self::Error>;

  /// Replaces the details of `id`, read at revision `read`, in place; a task changed since is a
  /// conflict, as for [`TaskStore::save_task`]. A queued task with no run is locked; existing runs
  /// keep their original task content for their prompts.
  fn update_task(&mut self, id: TaskId, read: TaskRevision, details: TaskDetails) -> Result<TaskUpdate, Self::Error>;
}

/// Project settings. Written by the desktop; the runner reads them.
pub trait SettingsStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// The project's limits: the defaults until they were changed.
  fn project_limits(&self) -> Result<ProjectLimits, Self::Error>;

  fn save_project_limits(&mut self, limits: &ProjectLimits) -> Result<(), Self::Error>;

  /// Variables the project adds to the environment of every agent CLI and command tool its runs
  /// start (Project settings › Environment); none until some are added.
  fn project_environment(&self) -> Result<CommandEnvironment, Self::Error>;

  /// Replaces the project's environment additions with `environment`.
  fn save_project_environment(&mut self, environment: &CommandEnvironment) -> Result<(), Self::Error>;

  /// Whether the project's agents may load the repository's own agent files (Project settings ›
  /// General); on until it was changed.
  fn repository_agent_files(&self) -> Result<RepositoryAgentFiles, Self::Error>;

  fn save_repository_agent_files(&mut self, setting: RepositoryAgentFiles) -> Result<(), Self::Error>;

  /// Whether the project's agents get the code index (Project settings › Code index); off until a
  /// person turned it on.
  fn code_index(&self) -> Result<CodeIndexSetting, Self::Error>;

  fn save_code_index(&mut self, setting: CodeIndexSetting) -> Result<(), Self::Error>;
}

/// Content-addressed project blobs (`docs/architecture/storage.md`, "Blobs"), e.g. a skill's
/// reference files. Equal content is stored once; blobs are never deleted.
pub trait BlobStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores `content` of `media_type` unless it is stored already, and returns its hash.
  fn put_blob(&mut self, media_type: &str, content: &[u8]) -> Result<ContentHash, Self::Error>;

  /// The content of blob `hash`, checked against it.
  fn blob(&self, hash: &ContentHash) -> Result<Vec<u8>, Self::Error>;
}

/// Drafts made from the repository's own agent files (Import from repository). Written by the
/// desktop; the repository is only read.
pub trait RepositoryImportStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates a resource for each of `drafts` whose only content is its draft, with where it came
  /// from, atomically; each slug must be free among its kind. Repeating the call with the same
  /// `key` returns the resources created the first time instead of new ones.
  fn import_repository_drafts(
    &mut self,
    key: IdempotencyKey,
    drafts: &[RepositoryDraft],
    now: Timestamp,
  ) -> Result<Vec<Resource>, Self::Error>;

  /// Where `resource` was imported from, if it was made from the repository.
  fn repository_origin(&self, resource: ResourceId) -> Result<Option<RepositoryOrigin>, Self::Error>;
}

/// Runs whose work was merged into their base branch. Written by the desktop, which merges; a run
/// is merged at most once.
pub trait MergeStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Records `merge`. Recording a merge of a run that has one already fails.
  fn save_run_merge(&mut self, merge: &RunMerge) -> Result<(), Self::Error>;

  fn run_merge(&self, run: RunId) -> Result<Option<RunMerge>, Self::Error>;
}

/// When runs' worktrees were removed (ORC-63), by *Remove worktree*, a merge or Run again, or found
/// gone. A run whose removal is recorded never continues: an interrupted one no longer holds its
/// task, and recording a continuation of it ([`RunStore::append_events`]) is refused.
pub trait WorktreeStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Records that the worktree of `run` was found removed at `at`. A removal recorded before stays.
  fn save_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<(), Self::Error>;

  /// Records, before git removes anything, that the worktrees of `run` are removed at `at`, unless
  /// the run has live work, a reserved local integration or an unfinished local Git process.
  /// Removal and continuation or local reservation acquisition share one write transaction gate.
  fn begin_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<RemovalStart, Self::Error>;

  /// Takes back the removal [`WorktreeStore::begin_worktree_removal`] recorded at `at`, once git
  /// removed nothing: the run is as it was.
  fn undo_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<(), Self::Error>;

  fn worktree_removal(&self, run: RunId) -> Result<Option<Timestamp>, Self::Error>;

  /// The runs of `task` whose worktree was removed, with when, ordered by run number.
  fn worktree_removals(&self, task: TaskId) -> Result<Vec<(RunId, Timestamp)>, Self::Error>;
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

/// Copies of another project's resources (Import resources). Written by the desktop.
pub trait ImportStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates every copy of `batch` as a resource whose only content is a draft, with where it
  /// came from and when (the copy's creation), atomically, after checking that the project still has every resource,
  /// connection and secret the copies rely on and that each copy's slug is free. Also records what the import
  /// used of this project's and left unresolved. Repeating the call with the same key returns the resources
  /// created the first time instead of new ones.
  fn import(&mut self, batch: &ImportBatch) -> Result<Vec<Resource>, Self::Error>;

  /// The import that created `resource`; `None` when no import did.
  fn import_record(&self, resource: ResourceId) -> Result<Option<ImportRecord>, Self::Error>;
}

/// Project connections. Written by the desktop; the runner only reads them.
pub trait ConnectionStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores a new connection. Names are unique within the project.
  fn create_connection(&mut self, connection: &Connection) -> Result<(), Self::Error>;

  /// Stores the current name, executable and last check of an existing connection. Model
  /// requirements stored meanwhile are kept: they are merged with the connection's, never dropped.
  fn save_connection(&mut self, connection: &Connection) -> Result<(), Self::Error>;

  /// Gives the stored connection `id` the name `name`, in one transaction that changes nothing else of it: a check
  /// running meanwhile keeps its result, and storing that result keeps the name. Names are unique within the project.
  fn rename_connection(&mut self, id: ConnectionId, name: &Name) -> Result<(), Self::Error>;

  /// Records `check` as the last check of the stored connection `id`, in one transaction that changes nothing else
  /// of it: a check of a CLI takes long, and the person may rename the connection meanwhile.
  fn record_connection_check(&mut self, id: ConnectionId, check: LastCheck) -> Result<(), Self::Error>;

  /// Records on the stored agent CLI connection `id` that `model` needs version `required` of the
  /// CLI or newer ([`crate::connection::AgentConnection::record_model_requirement`]), in one
  /// transaction that changes nothing else of it: the runner records it while a person may be
  /// editing the connection. A connection that no longer exists is an error, not created again.
  fn record_model_requirement(&mut self, id: ConnectionId, model: &ModelId, required: &str) -> Result<(), Self::Error>;

  fn connection(&self, id: ConnectionId) -> Result<Option<Connection>, Self::Error>;

  /// All connections of the project, ordered by name.
  fn connections(&self) -> Result<Vec<Connection>, Self::Error>;
}

/// Who still uses an MCP server, asked of `S` as it stands inside a removal's transaction.
pub type ServerUsers<S> = dyn Fn(&S) -> Result<Vec<std::sync::Arc<str>>, <S as McpServerStore>::Error>;

/// The project's MCP servers. Written by the desktop; the runner reads them when an agent starts.
pub trait McpServerStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores a new server. Names are unique among the project's MCP servers.
  fn create_mcp_server(&mut self, server: &McpServer) -> Result<(), Self::Error>;

  /// Stores the current name, transport, variables and enabled flag of an existing server.
  fn save_mcp_server(&mut self, server: &McpServer) -> Result<(), Self::Error>;

  /// Removes server `id` unless `users`, asked inside the same write transaction, names who still uses it; then
  /// nothing is removed and their names come back. Nothing else can start using the server in between.
  fn delete_unused_mcp_server(
    &mut self,
    id: ConnectionId,
    users: &ServerUsers<Self>,
  ) -> Result<Vec<std::sync::Arc<str>>, Self::Error>;

  fn mcp_server(&self, id: ConnectionId) -> Result<Option<McpServer>, Self::Error>;

  /// The MCP servers of the project that can be read back, ordered by name; [`Self::unreadable_mcp_servers`]
  /// lists the others.
  fn mcp_servers(&self) -> Result<Vec<McpServer>, Self::Error>;

  /// The stored MCP servers that cannot be read back, ordered by name, and why.
  fn unreadable_mcp_servers(&self) -> Result<Vec<UnreadableMcpServer>, Self::Error>;
}

/// Project secrets' metadata. Values are never stored here: they are in the OS keychain
/// ([`crate::secret::SecretVault`]). Written by the desktop; the runner reads it.
pub trait SecretStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores a new secret. No other active secret of the project has its name in its environment.
  fn create_secret(&mut self, secret: &Secret) -> Result<(), Self::Error>;

  /// Stores when an existing secret's value was last replaced and whether it is archived.
  fn save_secret(&mut self, secret: &Secret) -> Result<(), Self::Error>;

  /// The secret, archived or not.
  fn secret(&self, id: SecretId) -> Result<Option<Secret>, Self::Error>;

  /// The project's secrets that are not archived, ordered by name, then environment.
  fn secrets(&self) -> Result<Vec<Secret>, Self::Error>;
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

  /// Records how the latest sync of `repo` went, in place of the one before. A failure's details that
  /// only a log needs (an error's text, a program's output) are not kept.
  fn record_pull_request_sync(&mut self, repo: &GitHubRepo, attempt: &SyncAttempt) -> Result<(), Self::Error>;

  /// How the latest recorded sync of `repo` went; `None` before the first.
  fn pull_request_sync(&self, repo: &GitHubRepo) -> Result<Option<SyncAttempt>, Self::Error>;
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

  /// The run [`RunStore::create_run`] created with `key`, if one did.
  fn run_with_key(&self, key: IdempotencyKey) -> Result<Option<Run>, Self::Error>;

  /// The number the project's next run gets.
  fn next_run_number(&self) -> Result<RunNumber, Self::Error>;

  /// Appends `events`, which must not be empty, and stores `run`, the state after them,
  /// atomically. The first event must directly follow the stored log; otherwise another writer
  /// appended first and the call fails with a sequence conflict, changing nothing.
  fn append_events(&mut self, run: &Run, events: &[RunEvent]) -> Result<(), Self::Error>;

  /// [`RunStore::append_events`], storing `plan` in the same transaction: the plan the events
  /// propose ([`Run::propose_plan`]), or the decision on a stored one ([`Run::decide_plan`]). A
  /// plan's text never changes, and it is decided once.
  fn append_events_with_plan(&mut self, run: &Run, events: &[RunEvent], plan: &Plan) -> Result<(), Self::Error>;

  /// The plans of a run's gated steps, ordered by step and version.
  fn plans(&self, run: RunId) -> Result<Vec<Plan>, Self::Error>;

  /// The run with id `id`. A stored run that cannot be read back is an error, the same one a
  /// listing reports for it ([`crate::run::UnreadableRun::error`]).
  fn run(&self, id: RunId) -> Result<Option<Run>, Self::Error>;

  /// Runs of a task, ordered by number. The listings of runs read each run on its own: one whose
  /// stored record cannot be read back is listed as unreadable beside the others; only a listing
  /// that cannot be read at all fails.
  fn runs_of_task(&self, task: TaskId) -> Result<Vec<ListedRun<Self::Error>>, Self::Error>;

  /// The complete event log of a run, in sequence order.
  fn events(&self, run: RunId) -> Result<Vec<RunEvent>, Self::Error>;

  fn latest_checkpoint(&self, run: RunId) -> Result<Option<Checkpoint>, Self::Error>;

  /// Interrupted runs of the project that can continue, ordered by number, for recovery: a run
  /// whose worktree was removed ([`WorktreeStore`]) never continues and is left out.
  fn interrupted_runs(&self) -> Result<Vec<ListedRun<Self::Error>>, Self::Error>;

  /// Runs recorded as running, ordered by number: a runner executes them, or the runner or the
  /// session ended before it could record their interruption.
  fn running_runs(&self) -> Result<Vec<ListedRun<Self::Error>>, Self::Error>;

  /// The numbers of the runs that pinned each published version of pipeline `pipeline`, in run
  /// order. A version no run pinned is absent.
  fn runs_by_pipeline_version(
    &self,
    pipeline: ResourceId,
  ) -> Result<BTreeMap<VersionNumber, Vec<RunNumber>>, Self::Error>;
}

/// Task conversations. A person's messages are written by the desktop; their delivery states and
/// agents' messages by the runner. A message's text never changes and it is never removed.
pub trait ConversationStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores a person's new message. Repeating the call with the same `key` returns the message
  /// stored the first time instead of a second one.
  fn send_message(&mut self, key: IdempotencyKey, message: &Message) -> Result<Message, Self::Error>;

  /// Stores an agent's message.
  fn add_agent_message(&mut self, message: &Message) -> Result<(), Self::Error>;

  /// Stores how far people's `messages` got (delivered, acknowledged) and the run a delivered one
  /// went to, atomically. A delivery state only moves forward.
  fn save_deliveries(&mut self, messages: &[Message]) -> Result<(), Self::Error>;

  /// Stores how far a person's message got, `changed`, only while it still stands as `before` (its
  /// delivery and timing as stored): `false`, with nothing stored, when another writer (the desktop,
  /// the runner) moved it on first. For changes that two writers can race to make.
  fn change_delivery(&mut self, before: &Message, changed: &Message) -> Result<bool, Self::Error>;

  /// The conversation of `task`, oldest first.
  fn messages(&self, task: TaskId) -> Result<Vec<Message>, Self::Error>;

  /// The runs people's queued messages were written for, each once, by their oldest queued
  /// message. A message written before its task had a run names none and is not counted.
  fn runs_with_queued_messages(&self) -> Result<Vec<RunId>, Self::Error>;
}

/// Agent CLI and command tool invocations and their transcripts, written by the runner. Transcript
/// entries are append-only.
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

  /// Records a new run of a tool step's command tool for `execution` of `run`, numbered after the
  /// execution's earlier invocations.
  fn start_command_invocation(
    &mut self,
    run: RunId,
    execution: StepExecutionId,
    now: Timestamp,
  ) -> Result<Invocation, Self::Error>;

  /// Records the exact enforced policy used to prepare an open agent invocation, once, before
  /// its process starts. Existing snapshots cannot be replaced, including by save_invocation.
  /// Older invocations remain unknown; this method must not infer or backfill their policy.
  fn save_invocation_permissions(
    &mut self,
    invocation: InvocationId,
    permissions: &LaunchPermissions,
  ) -> Result<(), Self::Error>;

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

  /// Stores what `invocation` got of the code index, once, before its CLI starts.
  fn save_invocation_code_index(&mut self, invocation: InvocationId, used: &CodeIndexUse) -> Result<(), Self::Error>;

  /// What `invocation` got of the code index; `None` for one recorded before it was kept.
  fn invocation_code_index(&self, invocation: InvocationId) -> Result<Option<CodeIndexUse>, Self::Error>;

  /// Stores the endpoint, vendor and effort a native agent's `invocation` uses, once, before it starts.
  fn save_invocation_engine(&mut self, invocation: InvocationId, engine: &InvocationEngine) -> Result<(), Self::Error>;

  /// What `invocation` reached its model through; `None` for a CLI's invocation.
  fn invocation_engine(&self, invocation: InvocationId) -> Result<Option<InvocationEngine>, Self::Error>;

  /// Stores what `invocation` used before its agent first changed a file, replacing what was
  /// stored.
  fn save_edit_progress(&mut self, invocation: InvocationId, progress: &EditProgress) -> Result<(), Self::Error>;

  /// What `invocation` used before its first edit; `None` for one recorded before it was kept.
  fn edit_progress(&self, invocation: InvocationId) -> Result<Option<EditProgress>, Self::Error>;

  /// Appends `events` of `invocation` to its run's transcript, atomically, and returns the
  /// sequence of the last one (of the transcript so far when `events` is empty).
  fn append_activity(
    &mut self,
    invocation: &Invocation,
    events: &[(Timestamp, AgentEvent)],
  ) -> Result<u64, Self::Error>;

  /// Transcript entries of a run after sequence `after` (0: from the start), in order.
  fn activity(&self, run: RunId, after: u64) -> Result<Vec<ActivityEntry>, Self::Error>;

  /// The last message the agent of `execution` wrote in `run`'s transcript, with its sequence: its
  /// final message once its latest invocation ended.
  fn last_agent_message(&self, run: RunId, execution: StepExecutionId) -> Result<Option<(u64, String)>, Self::Error>;

  /// How the last command tool run of `execution` in `run` ended, as its transcript recorded it: an
  /// [`AgentEvent::ToolCommandFinished`] with what the program printed.
  fn last_command_finished(&self, run: RunId, execution: StepExecutionId) -> Result<Option<AgentEvent>, Self::Error>;

  /// Tokens that counted toward budgets in every run of the project since `since`, from the
  /// usage each completed turn and each review of auto mode's reviewer reported.
  fn tokens_used_since(&self, since: Timestamp) -> Result<TokensUsed, Self::Error>;

  /// Stores what a step execution left for the steps after it; an execution has at most one
  /// output, and storing it again changes nothing.
  fn save_step_output(&mut self, output: &StepOutput) -> Result<(), Self::Error>;

  /// Outputs of a run's step executions, in the order they were stored.
  fn step_outputs(&self, run: RunId) -> Result<Vec<StepOutput>, Self::Error>;
}
