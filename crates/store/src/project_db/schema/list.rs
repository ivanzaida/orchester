//! The project database's migrations in the order they apply: the first ones written out here, the later ones in
//! modules of their own beside this one. Append only: a new migration goes at the end.

pub(in super::super) const MIGRATIONS: &[&str] = &[
  r"
CREATE TABLE project (
  id TEXT PRIMARY KEY,
  key TEXT NOT NULL,
  name TEXT NOT NULL
) STRICT;

CREATE TABLE counters (
  name TEXT PRIMARY KEY,
  next INTEGER NOT NULL CHECK (next > 0)
) STRICT;
INSERT INTO counters (name, next) VALUES ('task', 1);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  batch_key TEXT NOT NULL,
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  pipeline_id TEXT,
  budget_micros INTEGER,
  budget_currency TEXT,
  max_duration_ms INTEGER,
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  CHECK ((budget_micros IS NULL) = (budget_currency IS NULL)),
  CHECK ((budget_micros IS NULL) = (max_duration_ms IS NULL))
) STRICT;
CREATE INDEX tasks_by_batch ON tasks (batch_key);

CREATE TABLE task_criteria (
  task_id TEXT NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  text TEXT NOT NULL,
  PRIMARY KEY (task_id, position)
) STRICT;
",
  r"
CREATE TABLE resources (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('agent_profile', 'instruction', 'pipeline', 'tool')),
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (kind, slug)
) STRICT;

CREATE TABLE resource_drafts (
  resource_id TEXT PRIMARY KEY REFERENCES resources (id),
  body TEXT NOT NULL,
  based_on INTEGER CHECK (based_on > 0),
  updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE resource_versions (
  id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES resources (id),
  number INTEGER NOT NULL CHECK (number > 0),
  body TEXT NOT NULL,
  published_at INTEGER NOT NULL,
  UNIQUE (resource_id, number)
) STRICT;
CREATE TRIGGER resource_versions_no_update BEFORE UPDATE ON resource_versions
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
CREATE TRIGGER resource_versions_no_delete BEFORE DELETE ON resource_versions
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
",
  r"
INSERT INTO counters (name, next) VALUES ('run', 1);

CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  idempotency_key TEXT NOT NULL UNIQUE,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  pipeline_version_id TEXT NOT NULL,
  snapshot TEXT NOT NULL,
  branch TEXT NOT NULL,
  base_revision TEXT NOT NULL CHECK (length(base_revision) = 40),
  currency TEXT NOT NULL,
  budget_micros INTEGER NOT NULL CHECK (budget_micros > 0),
  spent_micros INTEGER NOT NULL CHECK (spent_micros >= 0),
  status TEXT NOT NULL
    CHECK (status IN ('pending', 'running', 'paused', 'interrupted', 'succeeded', 'failed', 'cancelled')),
  status_reason TEXT CHECK ((status IN ('interrupted', 'failed')) = (status_reason IS NOT NULL)),
  failed_step TEXT CHECK ((status = 'failed') = (failed_step IS NOT NULL)),
  last_sequence INTEGER NOT NULL CHECK (last_sequence > 0),
  created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX runs_by_task ON runs (task_id, number);
CREATE INDEX runs_by_status ON runs (status, number);

CREATE TABLE attempts (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  number INTEGER NOT NULL CHECK (number > 0),
  reason TEXT NOT NULL CHECK (reason IN ('initial', 'retry')),
  failed_step TEXT,
  return_to TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'routed_back', 'succeeded', 'failed', 'cancelled')),
  UNIQUE (run_id, number),
  CHECK ((reason = 'retry') = (failed_step IS NOT NULL)),
  CHECK ((reason = 'retry') = (return_to IS NOT NULL))
) STRICT;

CREATE TABLE step_executions (
  id TEXT PRIMARY KEY,
  attempt_id TEXT NOT NULL REFERENCES attempts (id),
  step_key TEXT NOT NULL,
  profile_version_id TEXT,
  model TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'waiting', 'passed', 'failed', 'skipped')),
  wait_reason TEXT CHECK (wait_reason IN ('approval', 'plan', 'message')),
  UNIQUE (attempt_id, step_key),
  CHECK ((status = 'waiting') = (wait_reason IS NOT NULL)),
  CHECK ((profile_version_id IS NULL) = (model IS NULL))
) STRICT;

CREATE TABLE checkpoints (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  revision TEXT NOT NULL CHECK (length(revision) = 40),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  created_at INTEGER NOT NULL,
  UNIQUE (run_id, sequence)
) STRICT;
CREATE TRIGGER checkpoints_no_update BEFORE UPDATE ON checkpoints
BEGIN SELECT RAISE(ABORT, 'checkpoints are immutable'); END;
CREATE TRIGGER checkpoints_no_delete BEFORE DELETE ON checkpoints
BEGIN SELECT RAISE(ABORT, 'checkpoints are immutable'); END;

CREATE TABLE run_events (
  run_id TEXT NOT NULL REFERENCES runs (id),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  payload TEXT NOT NULL,
  PRIMARY KEY (run_id, sequence)
) STRICT;
CREATE TRIGGER run_events_no_update BEFORE UPDATE ON run_events
BEGIN SELECT RAISE(ABORT, 'run events are append-only'); END;
CREATE TRIGGER run_events_no_delete BEFORE DELETE ON run_events
BEGIN SELECT RAISE(ABORT, 'run events are append-only'); END;
",
  r"
CREATE TABLE connections (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  provider TEXT NOT NULL,
  name TEXT NOT NULL UNIQUE,
  settings TEXT NOT NULL,
  status TEXT NOT NULL,
  checked_at INTEGER,
  last_check TEXT,
  created_at INTEGER NOT NULL,
  CHECK ((checked_at IS NULL) = (last_check IS NULL))
) STRICT;
",
  r"
-- A project links at most one GitHub repository, so the table has at most one row.
CREATE TABLE repository_link (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  connection_id TEXT NOT NULL REFERENCES connections (id),
  remote TEXT NOT NULL,
  host TEXT NOT NULL,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  default_branch TEXT NOT NULL,
  permission TEXT NOT NULL CHECK (permission IN ('admin', 'maintain', 'write', 'triage', 'read')),
  expected_login TEXT NOT NULL,
  checked_at INTEGER NOT NULL
) STRICT;
",
  r"
-- Open pull requests of the linked repository as the last sync read them: a cache of GitHub,
-- replaced by every sync. A repository has a sync row once it was read, even with no pull requests.
CREATE TABLE pull_request_syncs (
  host TEXT NOT NULL,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  synced_at INTEGER NOT NULL,
  PRIMARY KEY (host, owner, name)
) STRICT;

CREATE TABLE pull_requests (
  host TEXT NOT NULL,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  number INTEGER NOT NULL CHECK (number > 0),
  title TEXT NOT NULL,
  url TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('open', 'draft', 'merged', 'closed')),
  head_branch TEXT NOT NULL,
  base_branch TEXT NOT NULL,
  head_sha TEXT NOT NULL,
  author TEXT,
  changed_files INTEGER NOT NULL CHECK (changed_files >= 0),
  additions INTEGER NOT NULL CHECK (additions >= 0),
  deletions INTEGER NOT NULL CHECK (deletions >= 0),
  checks_passed INTEGER NOT NULL CHECK (checks_passed >= 0),
  checks_failed INTEGER NOT NULL CHECK (checks_failed >= 0),
  checks_pending INTEGER NOT NULL CHECK (checks_pending >= 0),
  checks_skipped INTEGER NOT NULL CHECK (checks_skipped >= 0),
  review_decision TEXT CHECK (review_decision IN ('review_required', 'approved', 'changes_requested')),
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (host, owner, name, number),
  FOREIGN KEY (host, owner, name) REFERENCES pull_request_syncs (host, owner, name) ON DELETE CASCADE
) STRICT;
",
  r"
-- Content-addressed blobs: zstd files at blobs/<aa>/<sha256>, never deleted.
CREATE TABLE blobs (
  sha256 TEXT PRIMARY KEY,
  size INTEGER NOT NULL CHECK (size >= 0),
  stored_size INTEGER NOT NULL CHECK (stored_size >= 0),
  media_type TEXT NOT NULL,
  created_at INTEGER NOT NULL
) STRICT;

-- A pull request's diff per head revision, read from GitHub (a cache, desktop-written).
CREATE TABLE pr_revisions (
  host TEXT NOT NULL,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  number INTEGER NOT NULL CHECK (number > 0),
  head_sha TEXT NOT NULL,
  base_sha TEXT NOT NULL,
  diff_blob TEXT NOT NULL REFERENCES blobs (sha256),
  synced_at INTEGER NOT NULL,
  PRIMARY KEY (host, owner, name, number, head_sha)
) STRICT;
CREATE INDEX pr_revisions_by_sync ON pr_revisions (host, owner, name, number, synced_at);
",
  r"
-- Launches of agent CLIs for step executions, with the session and usage each reported.
CREATE TABLE invocations (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  number INTEGER NOT NULL CHECK (number > 0),
  model TEXT NOT NULL,
  session_id TEXT,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  input_tokens INTEGER NOT NULL CHECK (input_tokens >= 0),
  cached_input_tokens INTEGER NOT NULL CHECK (cached_input_tokens >= 0),
  output_tokens INTEGER NOT NULL CHECK (output_tokens >= 0),
  outcome TEXT,
  UNIQUE (step_execution_id, number),
  CHECK ((ended_at IS NULL) = (outcome IS NULL))
) STRICT;
CREATE INDEX invocations_by_run ON invocations (run_id, started_at);

-- The agent transcript of a run: what its CLIs reported, in order. Append-only.
CREATE TABLE agent_events (
  run_id TEXT NOT NULL REFERENCES runs (id),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  invocation_id TEXT NOT NULL REFERENCES invocations (id),
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  payload TEXT NOT NULL,
  PRIMARY KEY (run_id, sequence)
) STRICT;
CREATE TRIGGER agent_events_no_update BEFORE UPDATE ON agent_events
BEGIN SELECT RAISE(ABORT, 'agent events are append-only'); END;
CREATE TRIGGER agent_events_no_delete BEFORE DELETE ON agent_events
BEGIN SELECT RAISE(ABORT, 'agent events are append-only'); END;
",
  super::token_budgets::MIGRATION,
  r"
-- Parallel step graphs (M4). Each step execution may leave an output for the steps that run after
-- it: the summary of its structured result, else its final message. Append-only.
CREATE TABLE step_outputs (
  step_execution_id TEXT PRIMARY KEY REFERENCES step_executions (id),
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_key TEXT NOT NULL,
  outcome TEXT CHECK (outcome IN ('done', 'failed')),
  text TEXT NOT NULL,
  created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX step_outputs_by_run ON step_outputs (run_id, created_at);
CREATE TRIGGER step_outputs_no_update BEFORE UPDATE ON step_outputs
BEGIN SELECT RAISE(ABORT, 'step outputs are append-only'); END;
CREATE TRIGGER step_outputs_no_delete BEFORE DELETE ON step_outputs
BEGIN SELECT RAISE(ABORT, 'step outputs are append-only'); END;

-- Why a failed execution failed; executions that failed before reasons were recorded have none.
ALTER TABLE step_executions ADD COLUMN failure TEXT
  CHECK (failure IN ('agent_failed', 'reported_failed', 'not_started', 'step_budget', 'merge_conflict',
    'checkpoint_failed'));

-- Step budgets are tokens (user decision 2026-09-25). Published pipeline versions are immutable,
-- so their bodies are not rewritten: a step budget stored as money reads as no step budget (the
-- run budget still bounds the step). Every such amount, from versions and drafts, is kept here.
CREATE TABLE money_step_budgets_before_tokens (
  owner TEXT NOT NULL CHECK (owner IN ('version', 'draft')),
  owner_id TEXT NOT NULL,
  step_key TEXT NOT NULL,
  micros INTEGER NOT NULL,
  currency TEXT NOT NULL,
  PRIMARY KEY (owner, owner_id, step_key)
) STRICT;
INSERT INTO money_step_budgets_before_tokens (owner, owner_id, step_key, micros, currency)
  SELECT 'version', versions.id, json_extract(step.value, '$.key'), json_extract(step.value, '$.budget.micros'),
    json_extract(step.value, '$.budget.currency')
  FROM resource_versions AS versions
    JOIN resources ON resources.id = versions.resource_id AND resources.kind = 'pipeline',
    json_each(versions.body, '$.body.steps') AS step
  WHERE json_type(step.value, '$.budget') = 'object';
INSERT INTO money_step_budgets_before_tokens (owner, owner_id, step_key, micros, currency)
  SELECT 'draft', drafts.resource_id, json_extract(step.value, '$.key'), json_extract(step.value, '$.budget.micros'),
    json_extract(step.value, '$.budget.currency')
  FROM resource_drafts AS drafts
    JOIN resources ON resources.id = drafts.resource_id AND resources.kind = 'pipeline',
    json_each(drafts.body, '$.body.steps') AS step
  WHERE json_type(step.value, '$.budget') = 'object';
",
  r"
-- A step that uses its budget stops the run at its limit (user decision 2026-09-26) instead of
-- failing: the run is paused with reason `step_budget` and names the execution that used it.
-- Raising that step's budget applies to the run only; published pipeline versions stay as they are.
ALTER TABLE runs ADD COLUMN limit_execution TEXT
  CHECK ((status_reason IS 'step_budget') = (limit_execution IS NOT NULL));
CREATE TABLE run_step_budgets (
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_key TEXT NOT NULL,
  budget_tokens INTEGER NOT NULL CHECK (budget_tokens > 0),
  PRIMARY KEY (run_id, step_key)
) STRICT;
",
  r"
-- Permissions changed for one run (02.J): per run and agent profile, the permissions the run uses
-- instead of the profile's, the agents that restart with them at their next checkpoint and the last
-- one that did. Current state; the run's events hold every change.
CREATE TABLE run_permissions (
  run_id TEXT NOT NULL REFERENCES runs (id),
  profile_id TEXT NOT NULL,
  body TEXT NOT NULL,
  PRIMARY KEY (run_id, profile_id)
) STRICT;
",
  r"
-- The diff between two commits, read with git: a run's changes from its base to a checkpoint (the
-- task page's Changes tab). The diff of two commits does not change (a cache, desktop-written).
CREATE TABLE revision_diffs (
  base_sha TEXT NOT NULL,
  head_sha TEXT NOT NULL,
  diff_blob TEXT NOT NULL REFERENCES blobs (sha256),
  read_at INTEGER NOT NULL,
  PRIMARY KEY (base_sha, head_sha)
) STRICT;
",
  r"
-- What each agent CLI launch was told (Instructions): the injection event and the
-- sources of its prompt in order, as JSON. Written once by the runner before the CLI starts.
CREATE TABLE invocation_prompts (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  body TEXT NOT NULL
) STRICT;
",
  r"
-- Project secrets (07.01): what the user sees of a secret, never its value. The value is in the OS
-- keychain under the project and secret ids (`orchester_core::secret::VaultKey`). An archived
-- secret keeps its row, so references to it still resolve, and frees its name in its environment.
CREATE TABLE secrets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL
    CHECK (kind IN ('access_token', 'api_key', 'database_connection', 'deployment_token', 'password')),
  environment TEXT NOT NULL CHECK (environment IN ('development', 'staging', 'production')),
  created_at INTEGER NOT NULL,
  rotated_at INTEGER,
  archived_at INTEGER
) STRICT;
CREATE UNIQUE INDEX secrets_active_names ON secrets (name, environment) WHERE archived_at IS NULL;
",
  r"
-- Command tool steps (M10a). A command tool's run is an invocation of its step execution, so its
-- time counts toward the run's time limit and its output joins the run's transcript. It runs no
-- model: its `model` is empty.
ALTER TABLE invocations ADD COLUMN kind TEXT NOT NULL DEFAULT 'agent'
  CHECK (kind IN ('agent', 'command') AND (kind = 'command') = (model = ''));

-- A tool step fails with `command_failed` when its program exits with a code the tool does not
-- expect or does not finish in time. SQLite cannot change a CHECK constraint, so the table is
-- rebuilt, its rows keeping their `rowid` (executions are read in that order).
CREATE TABLE step_executions_new (
  id TEXT PRIMARY KEY,
  attempt_id TEXT NOT NULL REFERENCES attempts (id),
  step_key TEXT NOT NULL,
  profile_version_id TEXT,
  model TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'waiting', 'passed', 'failed', 'skipped')),
  wait_reason TEXT CHECK (wait_reason IN ('approval', 'plan', 'message')),
  failure TEXT CHECK (failure IN ('agent_failed', 'reported_failed', 'not_started', 'step_budget', 'merge_conflict',
    'checkpoint_failed', 'command_failed')),
  UNIQUE (attempt_id, step_key),
  CHECK ((status = 'waiting') = (wait_reason IS NOT NULL)),
  CHECK ((profile_version_id IS NULL) = (model IS NULL))
) STRICT;
INSERT INTO step_executions_new
  (rowid, id, attempt_id, step_key, profile_version_id, model, status, wait_reason, failure)
  SELECT rowid, id, attempt_id, step_key, profile_version_id, model, status, wait_reason, failure
  FROM step_executions;
DROP TABLE step_executions;
ALTER TABLE step_executions_new RENAME TO step_executions;
",
  r"
-- Approval gates (M7): each result of a gated step is a plan, numbered per run and step, and waits
-- for a person's decision; requested changes carry the reviewer's note. The text never changes and
-- a plan is decided once; the run's events hold every decision.
CREATE TABLE plans (
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_key TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version > 0),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  text TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  decision TEXT CHECK (decision IN ('approved', 'changes_requested')),
  note TEXT CHECK ((decision IS 'changes_requested') = (note IS NOT NULL)),
  decided_at INTEGER CHECK ((decision IS NULL) = (decided_at IS NULL)),
  PRIMARY KEY (run_id, step_key, version)
) STRICT;
CREATE TRIGGER plans_decided_once BEFORE UPDATE ON plans
WHEN OLD.decision IS NOT NULL OR NEW.text IS NOT OLD.text OR NEW.step_execution_id IS NOT OLD.step_execution_id
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'a plan keeps its text and is decided once'); END;
CREATE TRIGGER plans_no_delete BEFORE DELETE ON plans
BEGIN SELECT RAISE(ABORT, 'plans are never deleted'); END;
",
  r"
-- Project settings › Environment: variables added to the environment of every agent CLI and command
-- tool the project's runs start. Plain values, never secrets; names are distinct without regard to
-- case, as Windows compares them.
CREATE TABLE project_environment (
  name TEXT PRIMARY KEY COLLATE NOCASE,
  value TEXT NOT NULL
) STRICT;
",
  r"
-- Integration: the branch the user's checkout had when a run started, which merging the run brings
-- its work into (NULL on a detached HEAD, and for runs recorded before it was kept).
ALTER TABLE runs ADD COLUMN base_branch TEXT;

-- Runs whose work was merged into their base branch: the commit that branch was at afterwards and
-- how it got there. Desktop-written, once per run.
CREATE TABLE run_merges (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  into_branch TEXT NOT NULL,
  revision TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('fast_forward', 'merge_commit', 'already_merged')),
  merged_at INTEGER NOT NULL
) STRICT;
",
  super::conversation::MIGRATION,
  super::import::MIGRATION,
  super::mcp::MIGRATION,
  super::agent_files::MIGRATION,
  r"
-- New queued tasks pin the selected published pipeline version. Existing queued rows have no
-- recoverable selection and remain NULL until the user explicitly resolves them.
ALTER TABLE tasks ADD COLUMN pipeline_version INTEGER CHECK (pipeline_version > 0);
",
  super::task_edits::MIGRATION,
  super::tool_approvals::ACTIVE_TIME_MIGRATION,
  super::tool_approvals::MIGRATION,
  super::recovery::MIGRATION,
  super::command_approvals::MIGRATION,
  super::invocation_permissions::MIGRATION,
  super::code_index::MIGRATION,
  super::memory::MIGRATION,
  super::native_commands::MIGRATION,
  super::task_groups::MIGRATION,
  super::task_groups::FINISHED_MIGRATION,
  super::stuck_runs::MIGRATION,
  super::steering::MIGRATION,
  super::approval_choices::MIGRATION,
  super::organization::MIGRATION,
  super::worktree_removals::MIGRATION,
  super::sharing::MIGRATION,
  super::joining::MIGRATION,
  super::permitted_decisions::MIGRATION,
  super::automation_answers::MIGRATION,
  super::auto_mode::MIGRATION,
  super::auto_mode::REVIEWER_MIGRATION,
  super::auto_mode::ORIGIN_MIGRATION,
  super::auto_mode::REVIEWING_MIGRATION,
  super::native_conversations::MIGRATION,
  super::agent_slots::MIGRATION,
  super::plan_limits::MIGRATION,
  super::reviewer_usage::MIGRATION,
  super::questions::MIGRATION,
  super::plan_decided_by::MIGRATION,
  super::sources::MIGRATION,
  super::step_progress::MIGRATION,
  super::optional_limits::MIGRATION,
  super::optional_daily_budget::MIGRATION,
  super::agent_accounts::MIGRATION,
  super::orchestrator::MIGRATION,
  super::queue_holds::MIGRATION,
  super::unverified_steps::MIGRATION,
  super::local_integration::MIGRATION,
  super::editor_drafts::MIGRATION,
  super::sharing::ADDRESS_MIGRATION,
  super::pull_request_syncs::MIGRATION,
  super::worktree_not_created::MIGRATION,
  super::shared_members::MIGRATION,
  super::shared_members::READ_AS_MIGRATION,
  super::orchestrator_waits::MIGRATION,
  super::run_rebases::MIGRATION,
  super::creation_snapshots::MIGRATION,
  super::api_providers::MIGRATION,
  super::project_approval_rules::MIGRATION,
  super::decision_authors::MIGRATION,
  super::returned_after_end::MIGRATION,
  super::run_resolutions::MIGRATION,
  super::orchestrator_run_ends::MIGRATION,
  super::base_notices::MIGRATION,
  super::orchestrator_sent::MIGRATION,
  super::qa_slots::MIGRATION,
  super::orchestrator_reports::MIGRATION,
  super::story_points::MIGRATION,
];
