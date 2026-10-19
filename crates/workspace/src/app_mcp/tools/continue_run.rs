//! `continue_run`: continues a run stopped at a limit with one of the choices its task page offers for
//! how it stopped, read with the page's rules and sent as the page sends it. A run stopped at its token
//! budget or time limit continues with a new budget and time limit (02.L), one stopped at a step's
//! budget with a new budget for that step, one stopped by the project's daily budget resumes, and one
//! whose step used up its retries or broke again gets one more retry or returns to an earlier agent
//! step with a note (02.B); one whose step could not check everything, or was not run again on an
//! unchanged revision, also has its step accepted as it is. Every value is the caller's: nothing raises a limit by
//! itself. While unfinished blocking tasks hold the task, nothing continues, as on the page.

use std::path::Path;

use orchester_core::{
  id::{RunId, TaskId},
  resource::StepKey,
  run::{Answerer, PauseReason, RaisedLimits, ReviewNote, RunLimit, RunStatus, StuckDecision},
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{
  runs::{find_run, status_now},
  wire,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  sharing::project::WorkClaim,
  tasks::{
    detail::{RunView, TaskDetail},
    raise::{self, Raise},
    run::control_message,
  },
};

/// How whole tokens are grouped in the panel's text; the tool gives plain numbers.
const GROUP_SEPARATOR: &str = ",";

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ContinueRun {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  /// How the run continues: one of the choices its task page offers for how it stopped.
  pub choice: Choice,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub enum Choice {
  /// For a run stopped at its token budget or its time limit: continue with this token budget, above
  /// what the run used, and this time limit in whole minutes, above the active time the run used.
  /// Both are required; there is no maximum, and `null` is no limit.
  RaiseLimits {
    #[serde(deserialize_with = "wire::nullable")]
    #[schemars(required)]
    budget_tokens: Option<u64>,
    #[serde(deserialize_with = "wire::nullable")]
    #[schemars(required)]
    time_limit_minutes: Option<u64>,
  },
  /// For a run stopped at a step's budget: continue with this budget for that step, above what the
  /// step used. Required; there is no maximum, and `null` is no budget for the step in this run.
  RaiseStepBudget {
    #[serde(deserialize_with = "wire::nullable")]
    #[schemars(required)]
    budget_tokens: Option<u64>,
  },
  /// For a run stopped by the project's daily budget: resume, as the page's Resume does. The runner
  /// refuses while today's budget is still used up.
  Resume,
  /// For a run whose step used up its retries or broke again: Allow 1 retry & continue. For one whose
  /// step waits unverified or unchanged: run that step again.
  AllowRetry,
  /// For a run whose step used up its retries, broke again, or waits unverified or unchanged: return to
  /// an earlier agent step (its key or name) with a note for its agent.
  ReturnTo { step: String, note: String },
  /// For a run whose step could not check everything (unverified), or was not run again on an unchanged
  /// revision: accept the step as it is; the steps after it run. It passes the step in a person's place: refused
  /// as person_only unless a person turned on Let automation answer approvals, and recorded as automation's.
  Accept,
}

/// What the runner is asked.
enum Request {
  /// Resume after the daily budget, under the person's claim of a shared task.
  Resume,
  Command(Command),
}

pub async fn continue_run(services: &Services, args: ContinueRun) -> Result<Value, ToolError> {
  let reading = services.clone();
  let (project, run, request) = blocking(move || {
    // Accepting passes a step in a person's place, as approving does: only while a person lets automation answer.
    if matches!(args.choice, Choice::Accept) && !reading.workspace.automation_answers_approvals()? {
      return Err(ToolError::refused(
        "person_only",
        vec![Message::new("appMcp.errors.acceptPersonOnly")],
      ));
    }
    let project = reading.project(&args.project)?;
    let (run, request) = request(&project.repo_root, &args)?;
    Ok((project, run, request))
  })
  .await?;
  let (servers, runners) = (&services.servers, &services.runners);
  let (claim, command) = match request {
    Request::Resume => (
      servers
        .claim_for_work(&project.repo_root, run.task)
        .await
        .map_err(|refusal| ToolError::refused("claim_refused", vec![refusal.message()]))?,
      Command::ResumeRun { run: run.id },
    ),
    Request::Command(command) => (WorkClaim::default(), command),
  };
  let result = runners.control_run_in(&project, command).await;
  if result.is_err() {
    claim.release(servers).await;
  }
  result.map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))?;
  status_now(&project, run.id).await
}

/// The run the tool continues.
struct Named {
  id: RunId,
  task: TaskId,
}

/// Checks continuing the run `args` names in the project at `root` as the tool checks it, and changes nothing: the
/// run, and the step a return goes back to; a refusal when its task page would not offer the choice now. Blocks on
/// SQLite.
pub(crate) fn offered(root: &Path, args: &ContinueRun) -> Result<(RunId, Option<StepKey>), ToolError> {
  let (named, request) = request(root, args)?;
  let target = match request {
    Request::Command(Command::DecideStuckRun {
      decision: StuckDecision::ReturnTo { step, .. },
      ..
    }) => Some(step),
    Request::Command(_) | Request::Resume => None,
  };
  Ok((named.id, target))
}

/// The run `args` names in the project at `root` and what continuing it asks of the runner; a refusal when its task
/// page would not offer the choice now.
fn request(root: &Path, args: &ContinueRun) -> Result<(Named, Request), ToolError> {
  let db = ProjectDb::open(root)?;
  let run = find_run(&db, &args.run)?;
  drop(db);
  let number = run.number().get().to_string();
  let detail = TaskDetail::load_in(root, run.task())?;
  // Only a task's latest run can be stopped at a limit; its page offers the choices.
  let view = detail
    .run
    .as_ref()
    .filter(|view| view.id() == run.id())
    .ok_or_else(|| refused("not_stopped_at_limit", "appMcp.errors.notStoppedAtLimit", &number))?;
  if !detail.blocked_by.is_empty() {
    return Err(ToolError::refused(
      "task_blocked",
      vec![Message::new("tasks.blocked.runUnavailable")],
    ));
  }
  let named = Named {
    id: run.id(),
    task: run.task(),
  };
  let request = match (view.run.status(), &args.choice) {
    (RunStatus::Paused(PauseReason::LimitReached(limit)), choice) => at_limit(view, (*limit, choice), &number)?,
    (RunStatus::WaitingForPerson(_), choice) => Request::Command(Command::DecideStuckRun {
      run: run.id(),
      decision: decision(view, choice, &number)?,
    }),
    _ => {
      return Err(refused(
        "not_stopped_at_limit",
        "appMcp.errors.notStoppedAtLimit",
        &number,
      ));
    }
  };
  Ok((named, request))
}

/// What continuing a run stopped at `limit` with `choice` asks.
fn at_limit(view: &RunView, (limit, choice): (RunLimit, &Choice), number: &str) -> Result<Request, ToolError> {
  let not_offered = || refused("choice_not_offered", "appMcp.errors.choiceNotOffered", number);
  let used = raise::budget_use(view, limit);
  let raise = match (limit, choice) {
    (RunLimit::DailyBudget, Choice::Resume) => return Ok(Request::Resume),
    (
      RunLimit::Tokens | RunLimit::Time,
      Choice::RaiseLimits {
        budget_tokens,
        time_limit_minutes,
      },
    ) => {
      let (used, _) = used.ok_or_else(not_offered)?;
      let budget = raise::run_budget(&wire::limit_text(*budget_tokens), GROUP_SEPARATOR, used);
      let time = raise::time_limit(
        &wire::limit_text(*time_limit_minutes),
        view.active_time(Timestamp::now()),
      );
      match (budget, time) {
        (Ok(budget), Ok(max_duration)) => Raise::Limits(RaisedLimits { budget, max_duration }),
        (budget, time) => {
          let reasons = budget.err().into_iter().chain(time.err()).collect();
          return Err(ToolError::refused("invalid_limit", reasons));
        }
      }
    }
    (RunLimit::StepBudget { .. }, Choice::RaiseStepBudget { budget_tokens }) => {
      let (used, _) = used.ok_or_else(not_offered)?;
      let step = view.stopped_step().map(|step| &*step.name).unwrap_or_default();
      let budget = raise::step_budget(&wire::limit_text(*budget_tokens), GROUP_SEPARATOR, used, step)
        .map_err(|reason| ToolError::refused("invalid_limit", vec![reason]))?;
      Raise::StepBudget(budget)
    }
    (RunLimit::GroupBudget, _) => {
      return Err(ToolError::refused(
        "group_budget",
        vec![Message::new("tasks.bulk.stoppedGroupDetail")],
      ));
    }
    _ => return Err(not_offered()),
  };
  Ok(Request::Command(raise.command(view.id())))
}

/// The decision a run waiting after its step used up its retries or broke again takes with `choice`.
fn decision(view: &RunView, choice: &Choice, number: &str) -> Result<StuckDecision, ToolError> {
  match choice {
    Choice::AllowRetry => Ok(StuckDecision::Retry),
    Choice::Accept if matches!(view.run.status(), RunStatus::WaitingForPerson(stuck) if stuck.can_accept()) => {
      Ok(StuckDecision::Accept {
        by: Answerer::Automation,
      })
    }
    Choice::ReturnTo { step, note } => {
      let lowered = step.to_lowercase();
      let target = view
        .run
        .return_targets()
        .into_iter()
        .find(|key| {
          key.as_str() == step
            || view
              .steps
              .iter()
              .any(|shown| shown.key == *key && shown.name.to_lowercase() == lowered)
        })
        .ok_or_else(|| refused("step_not_offered", "appMcp.errors.returnStepNotOffered", number))?;
      let note = ReviewNote::new(note).map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
      Ok(StuckDecision::ReturnTo { step: target, note })
    }
    _ => Err(refused("choice_not_offered", "appMcp.errors.choiceNotOffered", number)),
  }
}

fn refused(code: &str, key: &'static str, run: &str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key).with("run", run.to_owned())])
}
