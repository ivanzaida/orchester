//! A run's services as the task page's Services tab reads and controls them (ORC-265, `docs/architecture/
//! agent-testing.md`, "Screens", `cjvRr`; protocol 42): the services the run's steps at work started, each with its
//! state, port, URL and times; a service's latest log lines, their secret values replaced where the runner kept them;
//! and the person's Stop and Restart, which the runner carries out as `stop_service` and `start_service` do.

use orchester_core::{id::RunId, time::Timestamp};
use serde::{Deserialize, Serialize};

/// The most log lines one [`ServiceCommand::Log`] answers.
pub const MAX_SHOWN_LINES: u32 = 500;

/// The Services tab's requests to the project's runner, each answered with a [`ServiceReply`] or a rejection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServiceCommand {
  /// The services the steps of `run` at work started, also those that stopped since: [`ServiceReply::Listed`].
  List { run: RunId },
  /// The kept lines of `service` of `run` after line `since`, at most the last `tail` of them (at most
  /// [`MAX_SHOWN_LINES`]): [`ServiceReply::Log`].
  Log {
    run: RunId,
    service: String,
    since: u64,
    tail: u32,
  },
  /// The person stops `service` of `run` with everything it started: [`ServiceReply::Done`] once it stopped.
  Stop { run: RunId, service: String },
  /// The person starts `service` of `run` again, as published, with what it needs: one that runs is stopped first.
  /// [`ServiceReply::Done`] once it is ready, [`crate::Rejection::Failed`] with why it did not start.
  Restart { run: RunId, service: String },
}

/// Where a service of a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunServiceState {
  /// Started, and not ready yet.
  Starting,
  Ready,
  /// Its program exited on its own; `code` is `None` when a signal ended it.
  Crashed {
    code: Option<i32>,
  },
  /// Orchester stopped it.
  Stopped,
}

/// Who started a service of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStarter {
  /// The agent of the step, with `start_service`, `start_stack` or a test suite's needs.
  Agent,
  /// A person, with the Services tab's Restart or Start.
  Person,
}

/// One service a step of a run started. It changes only when the service does: how long it has run follows from
/// `started_at`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunService {
  pub name: String,
  pub state: RunServiceState,
  pub port: u16,
  pub url: String,
  /// When its program started.
  pub started_at: Timestamp,
  /// When it crashed or was stopped; `None` while it runs.
  pub ended_at: Option<Timestamp>,
  /// The stack whose start brought it up; `None` for a start of the service itself or of one that needs it.
  pub stack: Option<String>,
  /// Who started it last.
  pub started_by: ServiceStarter,
  /// The service whose crash stopped it, as it needs that one.
  pub stopped_for: Option<String>,
}

/// One kept line of a service's output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceLogLine {
  /// Its place in the service's output, from 1.
  pub number: u64,
  pub at: Timestamp,
  /// It came from standard error.
  pub stderr: bool,
  /// What it says, its secret values replaced.
  pub text: String,
}

/// The runner's answers to a [`ServiceCommand`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServiceReply {
  /// The services of `run`, in the order they started.
  Listed { run: RunId, services: Vec<RunService> },
  /// The lines asked for, oldest first, and the number of the latest line the service wrote, kept or not.
  Log { lines: Vec<ServiceLogLine>, last: u64 },
  /// The stop or restart was carried out.
  Done,
}
