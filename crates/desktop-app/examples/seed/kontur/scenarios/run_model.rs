//! `conversation-model`, `-changed`, `-waits` and `-fixed` (ORC-322; `Task page · redesign` `C3wC8`: 08 `DPofN`, 08c
//! `b3a6z`, 08d `ff1nD`, 08e `X2Hmq`): the `tasks` scenario with the Builder's connection stored and checked, so the
//! model chip shows for Implement, the step at work in KON-13's run, where it waits for the person's answer to its
//! question. `conversation-model` lists three models with their efforts; `-changed` has a change for this step applied
//! (its event line, the chip on the new model); `-waits` has a change made while Implement's turn runs, which waits for
//! it (start the QA copy with no runner: a runner ends a turn left running when it attaches); `-fixed` lists one model
//! without efforts, so nothing can change; `-back` has a change for the step before Implement only, made before
//! Implement started, so Implement's line says it starts on its profile (08f `Yzb3F`).

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  resource::{AgentProfile, ModelId, StepKey},
  run::{Answerer, ModelChoice, ModelEvent, ModelEventKind, ModelScope, Run, StepBefore, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore},
};

use super::tasks;
use crate::kontur::{Kontur, SeedError, name};

/// The Builder's model as the catalogue publishes it, and the model a change moves Implement to.
const PROFILE_MODEL: &str = "gpt-5.5";
const CHOSEN_MODEL: &str = "gpt-5.5-codex";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_models(kontur, three_models()).map(|_| ())
}

pub fn changed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let run = with_models(kontur, three_models())?;
  change(kontur, &run, kontur.ago(2))
}

pub fn waits(kontur: &mut Kontur) -> Result<(), SeedError> {
  let run = with_models(kontur, three_models())?;
  let implement = implement(&run)?;
  kontur
    .db
    .start_invocation(run.id(), implement, &model(PROFILE_MODEL)?, kontur.ago(3))?;
  change(kontur, &run, kontur.ago(1))
}

pub fn fixed(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_models(kontur, vec![CatalogModel::new(model(PROFILE_MODEL)?, Vec::new())]).map(|_| ())
}

/// A change for the step before Implement only, made before Implement's agent first started: Implement started on its
/// own profile again, and its line says so (`08f` `Yzb3F`, the last event line).
pub fn back(kontur: &mut Kontur) -> Result<(), SeedError> {
  let run = with_models(kontur, three_models())?;
  let implement = implement(&run)?;
  let key = implement_key(&run)?;
  let steps = run.snapshot().body().steps();
  let at = steps
    .iter()
    .position(|step| step.key == key)
    .and_then(|position| position.checked_sub(1))
    .ok_or(SeedError::NotAtWork)?;
  let earlier = steps[at].key.clone();
  let started = kontur
    .db
    .invocations(run.id())?
    .into_iter()
    .filter(|invocation| invocation.execution == implement)
    .map(|invocation| invocation.started_at)
    .min()
    .ok_or(SeedError::NotAtWork)?;
  let to = ModelChoice {
    model: model(CHOSEN_MODEL)?,
    effort: Some("xhigh".to_owned()),
  };
  let event = ModelEvent {
    at: orchester_core::time::Timestamp::from_unix_millis(started.unix_millis() - 60_000),
    by: Answerer::Person,
    step: earlier.clone(),
    kind: ModelEventKind::Changed {
      scope: ModelScope::Step,
      to: to.clone(),
      was: ModelChoice {
        model: model(PROFILE_MODEL)?,
        effort: None,
      },
      also: Vec::new(),
      before: vec![StepBefore {
        step: earlier.clone(),
        choice: None,
      }],
    },
  };
  kontur.db.change_step_models(run.id(), &[(earlier, Some(to))], &event)?;
  Ok(())
}

fn model(id: &str) -> Result<ModelId, SeedError> {
  ModelId::new(id).map_err(|error| SeedError::Invalid(error.to_string()))
}

fn three_models() -> Vec<CatalogModel> {
  let efforts = |levels: &[&str]| levels.iter().map(|level| (*level).to_owned()).collect();
  [
    (PROFILE_MODEL, &["low", "medium", "high"][..]),
    (CHOSEN_MODEL, &["low", "medium", "high", "xhigh"][..]),
    ("gpt-5.5-mini", &["low", "medium"][..]),
  ]
  .into_iter()
  .map(|(id, levels)| {
    CatalogModel::new(
      ModelId::new(id).expect("the seed's model ids are valid"),
      efforts(levels),
    )
  })
  .collect()
}

/// The `tasks` project with the connection Implement's profile names in KON-13's run stored as a checked Codex listing
/// `models`; KON-13's latest run.
fn with_models(kontur: &mut Kontur, models: Vec<CatalogModel>) -> Result<Run, SeedError> {
  tasks::project(kontur)?;
  let task = kontur.find("KON-13")?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .ok_or(SeedError::NotAtWork)?;
  let key = implement_key(&run)?;
  let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(&key).map(|step| step.executor()) else {
    return Err(SeedError::NotAtWork);
  };
  let id = kontur
    .db
    .version::<AgentProfile>(profile.resource, profile.number)?
    .ok_or(SeedError::NotAtWork)?
    .body()
    .connection();
  // A program that is never there: the copy's runs are recorded, not run.
  let program = kontur.repo.join(".orchester").join("recorded").join("codex.exe");
  let mut connection = AgentConnection::new(
    id,
    name("Codex · Work"),
    AgentProvider::CodexCli,
    program,
    kontur.ago(60),
  )?;
  connection.record_check(ConnectionCheck {
    at: kontur.ago(60),
    result: CheckResult::Available {
      version: "0.159.0".to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, models),
    },
  })?;
  kontur.db.create_connection(&connection.into_connection())?;
  Ok(run)
}

/// The step KON-13's run waits at.
fn implement_key(run: &Run) -> Result<StepKey, SeedError> {
  let execution = implement(run)?;
  Ok(run.execution(execution).ok_or(SeedError::NotAtWork)?.step().clone())
}

fn implement(run: &Run) -> Result<orchester_core::id::StepExecutionId, SeedError> {
  run
    .current_attempt()
    .and_then(|attempt| {
      attempt
        .executions()
        .iter()
        .find(|execution| matches!(execution.status(), orchester_core::run::StepStatus::Waiting(_)))
    })
    .map(orchester_core::run::StepExecution::id)
    .ok_or(SeedError::NotAtWork)
}

/// Implement's next turns on the chosen model at Extra high, for this step, changed by the person at `at`.
fn change(kontur: &mut Kontur, run: &Run, at: orchester_core::time::Timestamp) -> Result<(), SeedError> {
  let step = implement_key(run)?;
  let to = ModelChoice {
    model: model(CHOSEN_MODEL)?,
    effort: Some("xhigh".to_owned()),
  };
  let event = ModelEvent {
    at,
    by: Answerer::Person,
    step: step.clone(),
    kind: ModelEventKind::Changed {
      scope: ModelScope::Step,
      to: to.clone(),
      was: ModelChoice {
        model: model(PROFILE_MODEL)?,
        effort: None,
      },
      also: Vec::new(),
      before: vec![StepBefore {
        step: step.clone(),
        choice: None,
      }],
    },
  };
  kontur.db.change_step_models(run.id(), &[(step, Some(to))], &event)?;
  Ok(())
}
