//! Messages between the desktop and a runner. The desktop sends [`Request`]s; the runner answers
//! each with an [`Event::Reply`] carrying the request's id, and on its own sends change
//! notifications. Notifications only say *that* something changed: the project database is the
//! source of truth, and the desktop reads the changed run from it (`docs/architecture/storage.md`,
//! "Write ownership").

use std::fmt;

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  run::PreflightProblem,
};
use serde::{Deserialize, Serialize};

/// Identifies a request within one connection, so its reply can be matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(u64);

impl RequestId {
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  pub const fn get(self) -> u64 {
    self.0
  }
}

impl fmt::Display for RequestId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "request {}", self.0)
  }
}

/// Desktop to runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
  pub id: RequestId,
  pub command: Command,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Command {
  /// The first request of a connection: the desktop starts listening to notifications. Answered
  /// with [`Reply::Attached`].
  Attach,
  /// Liveness check, answered with [`Reply::Pong`] carrying the same nonce.
  Ping { nonce: u64 },
  /// Starts a run of the queued task `task`. Idempotent by `key`: repeating it (a retry after
  /// a lost reply) answers with the run the first request created. Answered with
  /// [`Reply::RunStarted`] or [`Reply::Rejected`].
  StartRun { key: IdempotencyKey, task: TaskId },
  /// Stop the runner. It answers with [`Event::Stopped`] and exits once the desktop closes
  /// the connection.
  Shutdown { mode: ShutdownMode },
}

/// How the runner stops, matching the close confirmation in `docs/architecture/storage.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownMode {
  /// "Wait for agents to save": stop scheduling and let active steps reach a checkpoint.
  FinishAtCheckpoints,
  /// "Quit now": terminate agents without a checkpoint.
  Immediate,
}

/// Runner to desktop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Event {
  /// The answer to request `id`.
  Reply { id: RequestId, reply: Reply },
  /// The last event of a connection: the runner has stopped all work and exits.
  Stopped { mode: ShutdownMode },
  /// Run `run` of task `task` changed state (status, step, attempt); read it from the database.
  RunChanged { run: RunId, task: TaskId },
  /// Run `run`'s transcript grew to `sequence` entries. Coalesced: one notification can stand
  /// for many entries.
  AgentActivity { run: RunId, sequence: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Reply {
  /// Runs this runner is executing right now.
  Attached {
    active_runs: Vec<RunId>,
  },
  Pong {
    nonce: u64,
  },
  /// The run of a [`Command::StartRun`], newly created or created by an earlier request with
  /// the same key.
  RunStarted {
    run: RunId,
  },
  Rejected(Rejection),
}

/// Why the runner did not do what a request asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Rejection {
  TaskNotFound,
  /// Only queued tasks start a run.
  TaskNotQueued,
  /// The run cannot start until these are resolved.
  Preflight {
    problems: Vec<PreflightProblem>,
  },
  /// Starting failed in the runner (database, git worktree); its log has the details.
  Failed {
    detail: String,
  },
}
