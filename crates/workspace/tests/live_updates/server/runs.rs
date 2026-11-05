//! The fake team server's `PUT …/tasks/{task}/run`: a run's progress, which completes the task's claim when the run
//! merged. A test can have the next reports refused, as a server that fails for a moment does.

use std::net::TcpStream;

use orchester_api::{ClaimAction, Event, RunReport, RunStatus, RunStep, TaskClaim, TaskRun, UserRef};
use orchester_core::{id::TaskId, run::TaskPhase};
use serde_json::json;

use super::{Shared, TeamServer};
use crate::http::{Request, refusal, respond};

impl TeamServer {
  /// Answers the next `count` run progress reports `503`.
  pub fn refuse_run_reports(&self, count: usize) {
    self.state().refuse_runs = count;
  }
}

/// `PUT …/tasks/{task}/run`: the run's progress; a merged run's completes the task's claim.
pub(super) fn run(shared: &Shared, user: &UserRef, id: TaskId, request: &Request, stream: &mut TcpStream) {
  let report: RunReport = request.json();
  let mut state = shared.state.lock().expect("state");
  if state.refuse_runs > 0 {
    state.refuse_runs -= 1;
    drop(state);
    return refusal(stream, 503, "unavailable");
  }
  let at = state.tick();
  let task = state.task(id);
  task.has_run = true;
  task.run = Some(RunStatus {
    holder: user.clone(),
    number: report.number,
    phase: report.phase,
    step: report.step.clone(),
    started_at: report.started_at,
    phase_since: report.phase_since,
    updated_at: at,
  });
  let completed = report.phase == TaskPhase::Merged && task.claim.take().is_some();
  let confirmed = task.clone();
  let delta = TaskRun {
    task: id,
    number: report.number,
    phase: report.phase,
    step: report.step.map(|step| RunStep {
      key: step.to_lowercase(),
      name: step,
    }),
    phase_since: report.phase_since,
    updated_at: at,
  };
  state.append(user, delta, Event::TaskRun);
  if completed {
    let delta = TaskClaim {
      task: id,
      claim: None,
      action: ClaimAction::Completed,
    };
    state.append(user, delta, Event::TaskClaim);
  }
  drop(state);
  shared.appended.notify_all();
  respond(stream, 200, &json!({ "task": confirmed }));
}
