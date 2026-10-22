//! `run_rebases`: the proven rebase of a run's branch, one row per run, and a run's checkpoints, which a rebase is
//! checked against and agent feedback names the commits of.

use orchester_core::{
  id::RunId,
  integration::local::{RebaseStore, RebasedCheckpoint, RunRebase},
  revision::Revision,
  run::Checkpoint,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl RebaseStore for ProjectDb {
  type Error = StoreError;

  fn save_run_rebase(&mut self, rebase: &RunRebase) -> Result<(), StoreError> {
    let checkpoint = rebase.checkpoint();
    self.connection.execute(
      "INSERT INTO run_rebases (run_id, base_revision, replaced_checkpoint, checkpoint_tip, recorded_at) \
       VALUES (?1, ?2, ?3, ?4, ?5) \
       ON CONFLICT (run_id) DO UPDATE SET base_revision = excluded.base_revision, \
       replaced_checkpoint = excluded.replaced_checkpoint, checkpoint_tip = excluded.checkpoint_tip, \
       recorded_at = excluded.recorded_at",
      params![
        rebase.run().to_string(),
        rebase.base().as_str(),
        checkpoint.map(|checkpoint| checkpoint.replaces.as_str()),
        checkpoint.map(|checkpoint| checkpoint.tip.as_str()),
        rebase.recorded_at().unix_millis(),
      ],
    )?;
    Ok(())
  }

  fn run_rebase(&self, run: RunId) -> Result<Option<RunRebase>, StoreError> {
    let row: Option<(String, Option<String>, Option<String>, i64)> = self
      .connection
      .query_row(
        "SELECT base_revision, replaced_checkpoint, checkpoint_tip, recorded_at FROM run_rebases WHERE run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?;
    let Some((base, replaces, tip, recorded_at)) = row else {
      return Ok(None);
    };
    let checkpoint = match (replaces, tip) {
      (Some(replaces), Some(tip)) => Some(RebasedCheckpoint {
        replaces: revision("replaced checkpoint", &replaces)?,
        tip: revision("checkpoint tip", &tip)?,
      }),
      _ => None,
    };
    Ok(Some(RunRebase::restore(
      run,
      revision("rebase base", &base)?,
      checkpoint,
      Timestamp::from_unix_millis(recorded_at),
    )))
  }
}

impl ProjectDb {
  /// The revisions of every checkpoint of `run`, in the order they were recorded.
  pub fn checkpoint_revisions(&self, run: RunId) -> Result<Vec<Revision>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT revision FROM checkpoints WHERE run_id = ?1 ORDER BY sequence")?;
    let revisions = statement
      .query_map([run.to_string()], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    revisions
      .iter()
      .map(|text| revision("checkpoint revision", text))
      .collect()
  }

  /// Every checkpoint of `run`, in the order they were recorded.
  pub fn run_checkpoints(&self, run: RunId) -> Result<Vec<Checkpoint>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT id, step_execution_id, revision, created_at FROM checkpoints WHERE run_id = ?1 ORDER BY sequence",
    )?;
    let rows = statement
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .iter()
      .map(|(id, execution, text, at)| {
        Ok(Checkpoint::new(
          parse("checkpoint id", id)?,
          parse("step execution id", execution)?,
          revision("checkpoint revision", text)?,
          Timestamp::from_unix_millis(*at),
        ))
      })
      .collect()
  }
}

fn revision(what: &'static str, text: &str) -> Result<Revision, StoreError> {
  Revision::new(text).map_err(|error| corrupt(what, error))
}
