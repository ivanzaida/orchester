//! `continue_run`: continues a run stopped at a limit with one of the choices its task page offers for
//! how it stopped, read with the page's rules and sent as the page sends it. A run stopped at its token
//! budget or time limit continues with a new budget and time limit (02.L), one stopped at a step's
//! budget with a new budget for that step, one stopped by the project's daily budget resumes, and one
//! whose step used up its retries or broke again gets one more retry or returns to an earlier agent
//! step with a note (02.B); one whose step could not check everything, or was not run again on an
//! unchanged revision, also has its step accepted as it is. A run that ended, succeeded or failed, has its work
//! returned to one of its agent steps with a note (the project task ORC-44 · Return a finished run to an earlier step;
//! [`return_work`]); one whose last merge stopped at conflicts has them resolved by a writing step's agent, as the
//! task page's *Ask an agent to resolve* does ([`return_work::resolve_command`]). Every value is the caller's: nothing
//! raises a limit by itself. While unfinished blocking tasks hold the task, nothing continues, as on the page.

use std::path::Path;

use orchester_core::{
  id::{RunId, TaskId},
  resource::StepKey,
  run::{Answerer, MergeConflicts, PauseReason, RaisedLimits, ReviewNote, RunLimit, RunStatus, StuckDecision},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::{Command, WorkReturn};
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
  decisions::DecisionKind,
  messages::Message,
  sharing::project::WorkClaim,
  tasks::{
    detail::{RunView, TaskDetail},
    integration_jobs::{IntegrationJobs, JobState},
    raise::{self, Raise},
    resolve, return_work,
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
  /// Both are required; there is no maximum, and `null` is no limit. Refused as automation_not_allowed unless a
  /// person turned on Let automation answer approvals.
  RaiseLimits {
    #[serde(deserialize_with = "wire::nullable")]
    #[schemars(required)]
    budget_tokens: Option<u64>,
    #[serde(deserialize_with = "wire::nullable")]
    #[schemars(required)]
    time_limit_minutes: Option<u64>,
  },
  /// For a run stopped at a step's budget: continue with this budget for that step, above what the
  /// step used. Required; there is no maximum, and `null` is no budget for the step in this run. Refused as
  /// automation_not_allowed unless a person turned on Let automation answer approvals.
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
  /// an earlier agent step (its key or name) with a note for its agent. For a run that ended, succeeded or failed:
  /// return its work to any agent step it executed, which runs again with the note, then the steps after it.
  /// Refused as run_merged once its work was merged, worktree_removed once its worktree is gone, worktree_in_merge
  /// while a merge waits in its worktree for a person, and run_not_ended while it works.
  ReturnTo { step: String, note: String },
  /// For a run whose step could not check everything (unverified), or was not run again on an unchanged
  /// revision: accept the step as it is; the steps after it run. It passes the step in a person's place: refused
  /// as automation_not_allowed unless a person turned on Let automation answer approvals, and recorded as automation's.
  Accept,
  /// For a run whose last merge into its base branch stopped at conflicts (merge_run's merge_failed, the task page's
  /// Not merged into main): Ask an agent to resolve. Its work returns to `step` (its key or name; left out, the step
  /// that implemented what conflicts, else the last one), which must be a step whose agent may write in the worktree,
  /// with a note to merge the base branch in and resolve the conflicts, `note` after it, and the conflicting files as
  /// data. Refused as merge_not_conflicted unless the run's last merge in this app stopped at conflicts,
  /// step_cannot_write for a step that may not write, no_writing_step when none may, integration_running while git
  /// works on the run, and as return_to is refused (run_merged, worktree_removed, worktree_in_merge, run_not_ended).
  ResolveConflicts {
    #[serde(default)]
    step: Option<String>,
    #[serde(default)]
    note: Option<String>,
  },
}

/// What the runner is asked.
enum Request {
  /// Resume after the daily budget, under the person's claim of a shared task.
  Resume,
  Command(Command),
}

pub async fn continue_run(services: &Services, args: ContinueRun) -> Result<Value, ToolError> {
  continue_as(services, args, Answerer::Automation).await
}

/// `continue_run` decided by `by`: automation's decision on a waiting run, or its raise of a limit that stopped one,
/// needs the person's consent, while a person's (the Orchestrator chat's proposal a person applied) is recorded as
/// theirs and needs none. A resume after the daily budget raises nothing: the runner refuses it while today's budget
/// is still used up.
pub(crate) async fn continue_as(services: &Services, args: ContinueRun, by: Answerer) -> Result<Value, ToolError> {
  let reading = services.clone();
  let resolves = matches!(args.choice, Choice::ResolveConflicts { .. });
  let (project, run, request) = blocking(move || {
    let decision = match args.choice {
      Choice::AllowRetry | Choice::ReturnTo { .. } | Choice::Accept | Choice::ResolveConflicts { .. } => {
        Some(DecisionKind::Stuck)
      }
      Choice::RaiseLimits { .. } | Choice::RaiseStepBudget { .. } => Some(DecisionKind::Limit),
      Choice::Resume => None,
    };
    if let Some(decision) = decision.filter(|_| by == Answerer::Automation) {
      decision.require(&reading.workspace)?;
    }
    let project = reading.project(&args.project)?;
    let jobs = reading.jobs.of(project.project.id());
    let (run, request) = request(&project.repo_root, &args, (&reading.programs, &jobs, by))?;
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
  if resolves {
    // As after *Ask an agent to resolve*: the merge that failed no longer stands for the run that works again.
    services.jobs.of(project.project.id()).of(run.id).forget_merge_failure();
  }
  status_now(&project, run.id).await
}

/// The run the tool continues.
struct Named {
  id: RunId,
  task: TaskId,
}

/// What [`offered`] found the tool would do: the run, the step a return goes back to, and the conflicts a return to
/// resolve them carries.
pub(crate) struct Offered {
  pub run: RunId,
  pub target: Option<StepKey>,
  pub conflicts: Option<MergeConflicts>,
}

/// Checks continuing the run `args` names in the project at `root` as the tool checks it, and changes nothing; a
/// refusal when its task page would not offer the choice now. `programs` read the worktree of a run that ended;
/// `jobs`, the project's merges, say which run's last merge stopped at conflicts. Blocks on SQLite and git.
pub(crate) fn offered(
  root: &Path,
  args: &ContinueRun,
  (programs, jobs): (&Programs, &IntegrationJobs),
) -> Result<Offered, ToolError> {
  // Who would decide does not change what is offered.
  let (named, request) = request(root, args, (programs, jobs, Answerer::Person))?;
  let (target, conflicts) = match request {
    Request::Command(
      Command::DecideStuckRunAs {
        decision: StuckDecision::ReturnTo { step, .. },
        ..
      }
      | Command::DecideStuckRun {
        decision: StuckDecision::ReturnTo { step, .. },
        ..
      },
    ) => (Some(step), None),
    Request::Command(Command::ReturnRun(WorkReturn { step, conflicts, .. })) => (Some(step), conflicts),
    Request::Command(_) | Request::Resume => (None, None),
  };
  Ok(Offered {
    run: named.id,
    target,
    conflicts,
  })
}

/// The run `args` names in the project at `root` and what continuing it asks of the runner, decided by `by`; a
/// refusal when its task page would not offer the choice now. `programs` read the worktree of a run that ended;
/// `jobs` hold what the run's last merge stopped at.
fn request(
  root: &Path,
  args: &ContinueRun,
  (programs, jobs, by): (&Programs, &IntegrationJobs, Answerer),
) -> Result<(Named, Request), ToolError> {
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
    (_, Choice::ResolveConflicts { step, note }) => {
      let asked = (step.as_deref(), note.as_deref());
      resolve_conflicts((root, programs, jobs), (&detail, view), asked, (by, &number))?
    }
    (RunStatus::Paused(PauseReason::LimitReached(limit)), choice) => at_limit(view, (*limit, choice), &number)?,
    (RunStatus::WaitingForPerson(_), choice) => Request::Command(Command::DecideStuckRunAs {
      run: run.id(),
      expected: view.run.last_sequence(),
      decision: decision(view, (choice, by), &number)?,
      by,
    }),
    (_, Choice::ReturnTo { step, note }) if view.run.can_return_ended() => {
      let worktree = resolve::read(programs, root, (view.id(), false))
        .map_err(|message| ToolError::refused("worktree_unreadable", vec![message]))?;
      if let Some(refusal) = return_work::refusal(&detail, view, &worktree) {
        return Err(ToolError::refused(refusal.code(), vec![refusal.message()]));
      }
      let (step, note) = return_target(view, (step, note), &number)?;
      Request::Command(return_work::command(view, step, (note, None), by))
    }
    (_, Choice::ReturnTo { .. }) => {
      let refusal = return_work::ReturnRefusal::NotEnded;
      return Err(ToolError::refused(refusal.code(), vec![refusal.message()]));
    }
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
fn decision(view: &RunView, (choice, by): (&Choice, Answerer), number: &str) -> Result<StuckDecision, ToolError> {
  match choice {
    Choice::AllowRetry => Ok(StuckDecision::Retry),
    Choice::Accept if matches!(view.run.status(), RunStatus::WaitingForPerson(stuck) if stuck.can_accept()) => {
      Ok(StuckDecision::Accept { by })
    }
    Choice::ReturnTo { step, note } => {
      let (step, note) = return_target(view, (step, note), number)?;
      Ok(StuckDecision::ReturnTo { step, note })
    }
    _ => Err(refused("choice_not_offered", "appMcp.errors.choiceNotOffered", number)),
  }
}

/// The step `step` names, by its key or name, among those `view`'s work may go back to, and the note for its agent.
fn return_target(view: &RunView, (step, note): (&str, &str), number: &str) -> Result<(StepKey, ReviewNote), ToolError> {
  let lowered = step.to_lowercase();
  let target = return_work::targets(view)
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
  Ok((target, note))
}

/// *Ask an agent to resolve* on `detail`'s latest run `view`, whose last merge into its base branch stopped at
/// conflicts as `jobs` hold it: its work returned to the writing step `step` names (by key or name; the implementing
/// one when it names none) with the conflict's note, `note` after it, and the conflicting files as data, decided by
/// `by`; refused as the task page would not offer it. `programs` read the run's worktree.
fn resolve_conflicts(
  (root, programs, jobs): (&Path, &Programs, &IntegrationJobs),
  (detail, view): (&TaskDetail, &RunView),
  (step, note): (Option<&str>, Option<&str>),
  (by, number): (Answerer, &str),
) -> Result<Request, ToolError> {
  if !view.run.can_return_ended() {
    let refusal = return_work::ReturnRefusal::NotEnded;
    return Err(ToolError::refused(refusal.code(), vec![refusal.message()]));
  }
  let worktree = resolve::read(programs, root, (view.id(), false))
    .map_err(|message| ToolError::refused("worktree_unreadable", vec![message]))?;
  if let Some(refusal) = return_work::refusal(detail, view, &worktree) {
    return Err(ToolError::refused(refusal.code(), vec![refusal.message()]));
  }
  let job = jobs.of(view.id()).state();
  if job.busy() {
    return Err(ToolError::refused(
      "integration_running",
      vec![Message::new("appMcp.errors.integrationRunning")],
    ));
  }
  if !matches!(job.merge, JobState::Failed(_)) || job.conflicts.is_empty() {
    return Err(refused(
      "merge_not_conflicted",
      "appMcp.errors.mergeNotConflicted",
      number,
    ));
  }
  let no_writing_step = || refused("no_writing_step", "appMcp.errors.noWritingStep", number);
  let writing = return_work::writing_targets(view);
  let target = match step {
    None => return_work::implementing_step(view).ok_or_else(no_writing_step)?,
    Some(named) => {
      let lowered = named.to_lowercase();
      let shown = view
        .steps
        .iter()
        .find(|shown| shown.key.as_str() == named || shown.name.to_lowercase() == lowered)
        .ok_or_else(|| refused("step_not_offered", "appMcp.errors.returnStepNotOffered", number))?;
      if writing.is_empty() {
        return Err(no_writing_step());
      }
      if !writing.iter().any(|step| step.key == shown.key) {
        return Err(ToolError::refused(
          "step_cannot_write",
          vec![Message::new("appMcp.errors.stepCannotWrite").with("step", shown.name.to_string())],
        ));
      }
      shown.key.clone()
    }
  };
  let command = return_work::resolve_command(view, target, (&job.conflicts, note), by)
    .ok_or_else(|| ToolError::refused("no_base_branch", vec![Message::new("appMcp.errors.noBaseBranch")]))?;
  Ok(Request::Command(command))
}

fn refused(code: &str, key: &'static str, run: &str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key).with("run", run.to_owned())])
}
