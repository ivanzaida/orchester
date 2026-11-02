//! A run's own model and effort for its steps (`orchester_core::run::ModelChoice`, `run_step_models`), and the changes
//! that set them (`run_model_events`): written together as a person changes them from the run's Conversation, the
//! choices read by the runner at each turn's start and the changes for the Conversation's event lines.

use orchester_core::{
  id::RunId,
  resource::{ModelId, StepKey},
  run::{ModelChoice, ModelEvent, ModelEventKind},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb,
  questions::{answerer, read_answerer},
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json},
};

const EVENT: &str = "run model event";
/// The stored form of [`ModelEventKind`].
const EVENT_SCHEMA: u32 = 1;

impl ProjectDb {
  /// The model and effort `step` of `run` runs its next turns on in place of its profile's, if the run chose them.
  pub fn step_model(&self, run: RunId, step: &StepKey) -> Result<Option<ModelChoice>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT model, effort FROM run_step_models WHERE run_id = ?1 AND step_key = ?2",
        params![run.to_string(), step.as_str()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
      )
      .optional()?
      .map(|(model, effort)| choice(&model, effort))
      .transpose()
  }

  /// Sets what each step of `changes` runs its next turns on (`None`: its profile's again) and appends `event`, which
  /// records the change, in one transaction.
  pub fn change_step_models(
    &mut self,
    run: RunId,
    changes: &[(StepKey, Option<ModelChoice>)],
    event: &ModelEvent,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    for (step, chosen) in changes {
      match chosen {
        Some(chosen) => transaction.execute(
          "INSERT INTO run_step_models (run_id, step_key, model, effort) VALUES (?1, ?2, ?3, ?4) \
           ON CONFLICT (run_id, step_key) DO UPDATE SET model = excluded.model, effort = excluded.effort",
          params![
            run.to_string(),
            step.as_str(),
            chosen.model.as_str(),
            chosen.effort.as_deref()
          ],
        )?,
        None => transaction.execute(
          "DELETE FROM run_step_models WHERE run_id = ?1 AND step_key = ?2",
          params![run.to_string(), step.as_str()],
        )?,
      };
    }
    transaction.execute(
      "INSERT INTO run_model_events (run_id, at, by, step_key, event) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        run.to_string(),
        event.at.unix_millis(),
        answerer(event.by),
        event.step.as_str(),
        encode_json(EVENT, EVENT_SCHEMA, &event.kind)?,
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// `run`'s changes of its model or effort and their cancelations, oldest first.
  pub fn model_events(&self, run: RunId) -> Result<Vec<ModelEvent>, StoreError> {
    let mut query = self
      .connection
      .prepare("SELECT at, by, step_key, event FROM run_model_events WHERE run_id = ?1 ORDER BY id")?;
    let rows = query
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, String>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(at, by, step, event)| {
        Ok(ModelEvent {
          at: Timestamp::from_unix_millis(at),
          by: read_answerer("model event author", Some(&by))?,
          step: StepKey::new(&step).map_err(|error| corrupt("model event step", error))?,
          kind: decode_json::<ModelEventKind>(EVENT, EVENT_SCHEMA, &event)?,
        })
      })
      .collect()
  }
}

fn choice(model: &str, effort: Option<String>) -> Result<ModelChoice, StoreError> {
  Ok(ModelChoice {
    model: ModelId::new(model).map_err(|error| corrupt("run step model", error))?,
    effort,
  })
}
