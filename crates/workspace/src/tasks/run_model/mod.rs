//! A run's own model and effort (ORC-322, `08 · Conversation · model and effort` `DPofN`, `08f` `Yzb3F`): what the
//! model chip and its picker show of the step at work — the model and effort its next turn runs on, the profile's,
//! the models its connection offers with their efforts, which later steps a change for the rest of the run reaches —
//! and whether a change waits for the turn that runs. A person changes them from the Conversation ([`change`]); the
//! runner reads the run's choice at each turn's start, so a change made while a turn runs waits for its end and can be
//! canceled until then. The pipeline and the agent profile never change from it.

pub mod back;
pub mod change;

use std::sync::Arc;

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  resource::{AgentProfile, ModelId, StepKey},
  run::{Invocation, ModelChoice, ModelEvent, ModelEventKind, Run, StepExecutor, StepStatus},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use self::back::{BackToProfile, back_to_profile};
use super::detail::StepView;
use crate::agents::form::{effort_options, model_options, stored_effort};

/// A model the step's connection offers, with the efforts it lists for it (none: the CLI's default only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferedModel {
  pub id: ModelId,
  pub efforts: Vec<String>,
}

/// The later steps a change for the rest of the run reaches, and those that keep their model, in pipeline order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaterSteps {
  /// Later steps not started whose agent runs on the same connection.
  pub changing: Vec<StepKey>,
  /// Later agent steps not started that run on another connection.
  pub keeping: Vec<StepKey>,
}

/// The model of the step at work, as the chip and its picker show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepModel {
  pub step: StepKey,
  pub agent: Arc<str>,
  pub connection: Arc<str>,
  pub provider: AgentProvider,
  /// What the step's profile runs on, as the run pinned it.
  pub profile: ModelChoice,
  /// What the step's next turn runs on: the run's choice, else the profile's.
  pub now: ModelChoice,
  /// The models the connection offers, the profile's among them.
  pub offered: Vec<OfferedModel>,
  pub later: LaterSteps,
  /// A change made while the step's turn runs, which waits for that turn to end.
  pub waiting: Option<ModelChoice>,
}

impl StepModel {
  /// Whether nothing can be changed: the connection offers one model at most, without efforts to choose from.
  pub fn fixed(&self) -> bool {
    match self.offered.as_slice() {
      [] => true,
      [only] => only.efforts.is_empty(),
      _ => false,
    }
  }

  /// Whether the run runs the step on something else than its profile.
  pub fn changed(&self) -> bool {
    self.now != self.profile
  }

  /// The efforts the connection lists for `model`.
  pub fn efforts_of(&self, model: &ModelId) -> &[String] {
    self
      .offered
      .iter()
      .find(|offered| offered.id == *model)
      .map_or(&[], |offered| &offered.efforts)
  }
}

/// What a run keeps of its own model and effort: the changes and cancelations, oldest first, the later steps that
/// started on their own profile again after a change for one step, and the model of its step at work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunModels {
  pub events: Vec<ModelEvent>,
  pub back: Vec<BackToProfile>,
  pub step: Option<StepModel>,
}

impl RunModels {
  /// What `run` keeps, with its step at work `current` (as the run's view finds it) on its `invocations`.
  pub fn read(
    db: &ProjectDb,
    run: &Run,
    current: Option<&StepView>,
    invocations: &[Invocation],
  ) -> Result<Self, StoreError> {
    let events = db.model_events(run.id())?;
    let back = back_to_profile(db, run, (invocations, &events))?;
    let step = match current.filter(|_| !run.status().is_terminal()) {
      Some(current) => step_model(db, run, current, (invocations, &events))?,
      None => None,
    };
    Ok(Self { events, back, step })
  }
}

/// The model of agent step `current` of `run`; `None` for a tool step, or when its profile version or its connection
/// is gone.
fn step_model(
  db: &ProjectDb,
  run: &Run,
  current: &StepView,
  (invocations, events): (&[Invocation], &[ModelEvent]),
) -> Result<Option<StepModel>, StoreError> {
  let Some(profile) = profile_choice(db, run, &current.key)? else {
    return Ok(None);
  };
  let Some(connection) = db.step_agent_connection(run, &current.key)? else {
    return Ok(None);
  };
  let now = db
    .step_model(run.id(), &current.key)?
    .unwrap_or_else(|| profile.clone());
  let waiting = waiting_change(run, invocations, events, &current.key).map(|(_, to)| to.clone());
  Ok(Some(StepModel {
    step: current.key.clone(),
    agent: current.agent.clone().unwrap_or_else(|| "".into()),
    connection: connection.name().as_str().into(),
    provider: connection.provider(),
    offered: offered(&connection, &profile.model),
    later: later_on(db, run, &current.key, &connection)?,
    profile,
    now,
    waiting,
  }))
}

/// The models `connection` offers, `profile` among them, each with its efforts.
pub(crate) fn offered(connection: &AgentConnection, profile: &ModelId) -> Vec<OfferedModel> {
  model_options(connection, Some(profile))
    .into_iter()
    .map(|option| OfferedModel {
      efforts: effort_options(connection, &option.id),
      id: option.id,
    })
    .collect()
}

/// What the profile of agent step `key` runs on as `run` pinned it: its model and effort; `None` for a tool step or a
/// profile version that is gone.
pub(crate) fn profile_choice(db: &ProjectDb, run: &Run, key: &StepKey) -> Result<Option<ModelChoice>, StoreError> {
  let Some(StepExecutor::Agent { profile, model, .. }) = run.snapshot().step(key).map(|step| step.executor()) else {
    return Ok(None);
  };
  let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)? else {
    return Ok(None);
  };
  Ok(Some(ModelChoice {
    model: model.clone(),
    effort: stored_effort(version.body()),
  }))
}

/// The agent steps after `current` in `run`'s pipeline that have not started, in its order, split by whether their
/// agent runs on `connection`.
pub(crate) fn later_on(
  db: &ProjectDb,
  run: &Run,
  current: &StepKey,
  connection: &AgentConnection,
) -> Result<LaterSteps, StoreError> {
  let steps = run.snapshot().body().steps();
  let start = steps
    .iter()
    .position(|step| step.key == *current)
    .map_or(steps.len(), |at| at + 1);
  let mut later = LaterSteps::default();
  for step in &steps[start..] {
    let agent = matches!(
      run.snapshot().step(&step.key).map(|pinned| pinned.executor()),
      Some(StepExecutor::Agent { .. })
    );
    let started = run
      .latest_execution(&step.key)
      .is_some_and(|execution| execution.status() != StepStatus::Pending);
    if !agent || started {
      continue;
    }
    let on = db.step_agent_connection(run, &step.key)?;
    if on.is_some_and(|on| on.id() == connection.id()) {
      later.changing.push(step.key.clone());
    } else {
      later.keeping.push(step.key.clone());
    }
  }
  Ok(later)
}

/// The latest change for step `key` among `events` while it still waits: no cancelation followed it, and the step's
/// agent has been in a turn since before it, which has not ended. Its index among `events` and what it changes to.
pub fn waiting_change<'a>(
  run: &Run,
  invocations: &[Invocation],
  events: &'a [ModelEvent],
  key: &StepKey,
) -> Option<(usize, &'a ModelChoice)> {
  let (index, event) = events.iter().enumerate().rev().find(|(_, event)| event.step == *key)?;
  let ModelEventKind::Changed { to, .. } = &event.kind else {
    return None;
  };
  in_turn_since(run, invocations, key, event.at).then_some((index, to))
}

/// Whether step `key`'s agent is in a turn that started at or before `at` and has not ended.
fn in_turn_since(run: &Run, invocations: &[Invocation], key: &StepKey, at: Timestamp) -> bool {
  invocations.iter().any(|invocation| {
    invocation.ended_at.is_none()
      && invocation.started_at <= at
      && run
        .execution(invocation.execution)
        .is_some_and(|execution| execution.step() == key)
  })
}
