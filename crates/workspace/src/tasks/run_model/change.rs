//! A person's change of a run's model or effort from its Conversation (ORC-322), and its cancelation while it waits
//! for a turn's end. A change holds for the step at work only, or also for every later step not started whose agent
//! runs on the same connection: a model's name means something on its own connection only. It is recorded with what
//! each step ran on before, so a cancelation gives exactly that back.

use std::path::Path;

use orchester_core::{
  id::RunId,
  resource::StepKey,
  run::{Answerer, ModelChoice, ModelEvent, ModelEventKind, ModelScope, StepBefore},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{later_on, offered, profile_choice, waiting_change};
use crate::tasks::detail::run_store_error;

/// A change asked of a run's model or effort: for which step, what to, and for which steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelAsked {
  pub step: StepKey,
  pub to: ModelChoice,
  pub scope: ModelScope,
}

/// Why a change of a run's model or effort, or its cancelation, was not made.
#[derive(Debug, Error)]
pub enum ModelChangeError {
  /// The run, its step, its profile version or its connection is gone, or the run ended.
  #[error("the step's model cannot be changed now")]
  Unavailable,
  /// The step's connection does not offer the model, or not the effort for it.
  #[error("the step's connection does not offer {model} with that effort")]
  NotOffered { model: String },
  /// The change asked to cancel no longer waits: the turn ended, or it was canceled already.
  #[error("the change no longer waits for a turn to end")]
  NotWaiting,
  #[error(transparent)]
  Store(#[from] StoreError),
}

/// Changes what `asked.step` of run `run` of the project at `repo_root` runs its next turns on, asked `by` a person or
/// automation, and records the change for the run's Conversation; a change to what it runs on already records nothing.
pub fn change_model_in(repo_root: &Path, run: RunId, asked: ModelAsked, by: Answerer) -> Result<(), ModelChangeError> {
  let mut db = ProjectDb::open(repo_root)?;
  let run = db
    .run(run)
    .map_err(run_store_error)?
    .filter(|run| !run.status().is_terminal())
    .ok_or(ModelChangeError::Unavailable)?;
  let profile = profile_choice(&db, &run, &asked.step)?.ok_or(ModelChangeError::Unavailable)?;
  let connection = db
    .step_agent_connection(&run, &asked.step)?
    .ok_or(ModelChangeError::Unavailable)?;
  let offered = offered(&connection, &profile.model);
  let efforts = offered
    .iter()
    .find(|model| model.id == asked.to.model)
    .map(|model| &model.efforts);
  // No effort keeps the CLI's default, which every model takes.
  let takes = efforts.is_some_and(|efforts| asked.to.effort.as_ref().is_none_or(|effort| efforts.contains(effort)));
  if !takes {
    return Err(ModelChangeError::NotOffered {
      model: asked.to.model.as_str().to_owned(),
    });
  }
  let now = db.step_model(run.id(), &asked.step)?.unwrap_or(profile);
  if now == asked.to {
    return Ok(());
  }
  let also: Vec<StepKey> = match asked.scope {
    ModelScope::Step => Vec::new(),
    ModelScope::Run => later_on(&db, &run, &asked.step, &connection)?.changing,
  };
  let mut before = Vec::new();
  let mut changes = Vec::new();
  for step in std::iter::once(&asked.step).chain(&also) {
    before.push(StepBefore {
      step: step.clone(),
      choice: db.step_model(run.id(), step)?,
    });
    changes.push((step.clone(), Some(asked.to.clone())));
  }
  let event = ModelEvent {
    at: Timestamp::now(),
    by,
    step: asked.step,
    kind: ModelEventKind::Changed {
      scope: asked.scope,
      to: asked.to,
      was: now,
      also,
      before,
    },
  };
  db.change_step_models(run.id(), &changes, &event)?;
  Ok(())
}

/// Cancels the change of `step` of run `run` of the project at `repo_root` that waits for the step's turn to end, asked
/// `by` a person or automation: each step it changed runs on what it ran on before, and the Conversation records it.
pub fn cancel_model_change_in(
  repo_root: &Path,
  run: RunId,
  step: &StepKey,
  by: Answerer,
) -> Result<(), ModelChangeError> {
  let mut db = ProjectDb::open(repo_root)?;
  let run = db
    .run(run)
    .map_err(run_store_error)?
    .ok_or(ModelChangeError::Unavailable)?;
  let events = db.model_events(run.id())?;
  let invocations = db.invocations(run.id())?;
  let (index, _) = waiting_change(&run, &invocations, &events, step).ok_or(ModelChangeError::NotWaiting)?;
  let ModelEventKind::Changed { was, before, .. } = &events[index].kind else {
    return Err(ModelChangeError::NotWaiting);
  };
  let changes: Vec<_> = before
    .iter()
    .map(|before| (before.step.clone(), before.choice.clone()))
    .collect();
  let event = ModelEvent {
    at: Timestamp::now(),
    by,
    step: step.clone(),
    kind: ModelEventKind::Canceled { stays: was.clone() },
  };
  db.change_step_models(run.id(), &changes, &event)?;
  Ok(())
}
