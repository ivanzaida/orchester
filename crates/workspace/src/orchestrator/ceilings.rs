//! The ceilings of work a conversation leads to the end (`orchester_core::orchestrator::watch::lead`,
//! `docs/plan/orchestrator-autonomy/OA-09-lead-mode-and-ceilings.md`), held by the chat's tools in the turns Orchester
//! starts by itself: a return, a retry or a raise of a run past its ceiling, one more run of a task past its own, and
//! any of them once the subscription's work spent its budget, is refused `ceiling_reached`, naming the ceiling, and the
//! chat leaves the task to the person. They hold for every way the chat moves a run: `continue_run`, a message that
//! continues a run or runs its task again, and `run_task`. A turn a person's message started is not bound: the person
//! asked. Nor is a task that is only followed, which the ceilings say nothing of; a task followed by a subscription
//! of its own inside a module led to the end is such a task, as the more specific subscription decides.

use orchester_core::{
  orchestrator::{
    Subscription, Subscriptions,
    change::{Change, MessageThen},
    watch::{Ceiling, CeilingReached, Ceilings, own_returns, raises},
  },
  run::{PauseReason, Run, RunLimit, RunStatus},
  store::RunStore,
  task::Task,
  tokens::TokensUsed,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{team_status::module_now, tools::ChatScope};
use crate::{
  app_mcp::{ContinueChoice, ContinueRun, ToolError, blocking, find_run},
  messages::Message,
  tasks::{
    detail::{run_store_error, run_view},
    raise,
    recorded::{self, task_of},
  },
};

/// Refuses the chat's `continue_run` with `arguments` in `scope` when its turn is one nobody asked for, the run's task
/// is led to the end and the choice would pass a ceiling of its subscription: its budget, one more return or retry of
/// automation's, one more raise, or a raise by more than the ceiling allows. Arguments that do not parse, a run that
/// is not found and a choice the run does not offer are the tool's to refuse.
pub(super) async fn continue_within(scope: &ChatScope, arguments: &JsonObject) -> Result<(), ToolError> {
  if !scope.on_its_own {
    return Ok(());
  }
  let Ok(asked) = serde_json::from_value::<ContinueRun>(Value::Object(arguments.clone())) else {
    return Ok(());
  };
  let root = scope.root.clone();
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let Ok(run) = find_run(&db, &asked.run) else {
      return Ok(());
    };
    let Some(led) = Led::of_run(&db, &run)? else {
      return Ok(());
    };
    led.within_budget(&db)?.map_err(refusal)?;
    let events = db.events(run.id()).map_err(run_store_error)?;
    let ceilings = led.ceilings;
    let within = match asked.choice {
      ContinueChoice::AllowRetry | ContinueChoice::ReturnTo { .. } | ContinueChoice::ResolveConflicts { .. } => {
        ceilings.another_return(own_returns(&events))
      }
      ContinueChoice::RaiseLimits {
        budget_tokens,
        time_limit_minutes,
      } => {
        let raised = raises(&events);
        let tokens = run.budget().map(|budget| budget.get());
        // Limits are set in whole minutes; one that is not is read as the minute it ends in.
        let minutes = run.max_duration().map(|limit| limit.as_secs().div_ceil(60));
        raise_within(ceilings, raised, (tokens, budget_tokens))
          .and_then(|()| raise_within(ceilings, raised, (minutes, time_limit_minutes)))
      }
      ContinueChoice::RaiseStepBudget { budget_tokens } => {
        raise_within(ceilings, raises(&events), (step_budget(&db, run)?, budget_tokens))
      }
      ContinueChoice::Resume | ContinueChoice::Accept => Ok(()),
    };
    within.map_err(refusal)
  })
  .await
}

/// Refuses a message of the chat's, kept as `change`, that would move a run nobody works on, in a turn nobody asked
/// for, when the run's task is led to the end and moving it would pass a ceiling: one that continues the run or
/// follows it up is one more return of automation's, one that runs the task again is one more run of the task, and
/// neither goes once the budget is spent. A message an agent reads as it waits or works moves nothing.
pub(super) async fn message_within(scope: &ChatScope, change: &Change) -> Result<(), ToolError> {
  let Change::Message {
    run, then: Some(then), ..
  } = *change
  else {
    return Ok(());
  };
  if !scope.on_its_own || matches!(then, MessageThen::Resume | MessageThen::RaiseLimit) {
    return Ok(());
  }
  let root = scope.root.clone();
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let Some(run) = db.run(run).map_err(run_store_error)? else {
      return Ok(());
    };
    let Some(led) = Led::of_run(&db, &run)? else {
      return Ok(());
    };
    led.within_budget(&db)?.map_err(refusal)?;
    let within = if then == MessageThen::RunAgain {
      let runs = db.runs_of_task(run.task()).map_err(run_store_error)?;
      led.ceilings.another_run(runs.len())
    } else {
      let events = db.events(run.id()).map_err(run_store_error)?;
      led.ceilings.another_return(own_returns(&events))
    };
    within.map_err(refusal)
  })
  .await
}

/// Whether a limit that stands at `from` may be raised to `to`; a limit the run does not have is not raised.
fn raise_within(
  ceilings: Ceilings,
  raised: usize,
  (from, to): (Option<u64>, Option<u64>),
) -> Result<(), CeilingReached> {
  match from {
    Some(from) => ceilings.raise(raised, from, to),
    None => Ok(()),
  }
}

/// The budget of the step `run` stopped at, in tokens; `None` while it stopped at no step's budget.
fn step_budget(db: &ProjectDb, run: Run) -> Result<Option<u64>, ToolError> {
  let RunStatus::Paused(PauseReason::LimitReached(limit @ RunLimit::StepBudget { .. })) = *run.status() else {
    return Ok(None);
  };
  let view = run_view(db, run)?;
  Ok(raise::budget_use(&view, limit).and_then(|(_, budget)| budget.map(|budget| budget.get())))
}

/// The refusal of the chat's start of `task` in a turn nobody asked for, when a subscription leads it to the end and
/// that subscription's work spent its budget; `None` while it may start.
pub(super) fn start_refused(db: &ProjectDb, task: &Task) -> Result<Option<Message>, StoreError> {
  let Some(led) = Led::of_task(db, task)? else {
    return Ok(None);
  };
  Ok(led.within_budget(db)?.err().map(message))
}

/// A task led to the end: its subscription's ceilings, and the subscriptions it was found among.
struct Led {
  ceilings: Ceilings,
  subscriptions: Subscriptions,
  leading: Subscription,
}

impl Led {
  /// The leading of `task`; `None` while no subscription leads it to the end.
  fn of_task(db: &ProjectDb, task: &Task) -> Result<Option<Self>, StoreError> {
    let subscriptions = db.active_subscriptions()?;
    let Some(leading) = subscriptions.leader_of(task.id(), module_now(db, task)).cloned() else {
      return Ok(None);
    };
    Ok(leading.mode().ceilings().map(|ceilings| Self {
      ceilings,
      subscriptions,
      leading,
    }))
  }

  /// The leading of `run`'s task; `None` while no subscription leads it to the end, or the task is gone.
  fn of_run(db: &ProjectDb, run: &Run) -> Result<Option<Self>, StoreError> {
    match task_of(db, run.task())? {
      Some(task) => Self::of_task(db, &task),
      None => Ok(None),
    }
  }

  /// Whether the subscription's work may spend more: read only when it has a budget, as it reads every run it leads.
  fn within_budget(&self, db: &ProjectDb) -> Result<Result<(), CeilingReached>, StoreError> {
    if self.ceilings.budget().is_none() {
      return Ok(Ok(()));
    }
    Ok(
      self
        .ceilings
        .within_budget(spent(db, &self.subscriptions, &self.leading)?),
    )
  }
}

/// What the work `leading` leads spent since it leads it: the runs, started since, of the tasks whose events go to
/// it, and every turn of its conversation since, whatever the turn was about. The turn under way is not in it yet.
pub fn spent(db: &ProjectDb, subscriptions: &Subscriptions, leading: &Subscription) -> Result<TokensUsed, StoreError> {
  spent_among(db, &recorded::all_with_runs(db)?, (subscriptions, leading))
}

/// [`spent`] for a caller that read the project's `tasks` with their runs already.
pub(super) fn spent_among(
  db: &ProjectDb,
  tasks: &[recorded::WithRuns],
  (subscriptions, leading): (&Subscriptions, &Subscription),
) -> Result<TokensUsed, StoreError> {
  let mut used = db.chat_tokens_since(leading.chat(), leading.since())?;
  for (task, runs) in tasks {
    if subscriptions.leader_of(task.id(), module_now(db, task)) != Some(leading) {
      continue;
    }
    for run in runs.iter().flatten() {
      if run.created_at() >= leading.since() {
        used += run.used();
      }
    }
  }
  Ok(used)
}

fn refusal(reached: CeilingReached) -> ToolError {
  ToolError::refused("ceiling_reached", vec![message(reached)])
}

/// What the agent reads when a task that had `runs` runs may not run again under `ceilings`; `None` while it may.
pub(super) fn again_refused(ceilings: Ceilings, runs: usize) -> Option<Message> {
  ceilings.another_run(runs).err().map(message)
}

/// What the agent reads of a ceiling it reached: which, how much it allows, and that the task is now the person's.
fn message(reached: CeilingReached) -> Message {
  let key = match reached.ceiling {
    Ceiling::RunsOfTask => "orchestrator.ceilings.runs",
    Ceiling::ReturnsOfRun => "orchestrator.ceilings.returns",
    Ceiling::RaisesOfRun => "orchestrator.ceilings.raises",
    Ceiling::RaiseBy => "orchestrator.ceilings.raiseBy",
    Ceiling::Budget => "orchestrator.ceilings.budget",
  };
  Message::new(key)
    .with("allowed", reached.allowed.to_string())
    .with("used", reached.used.to_string())
}
