//! A run's execution inputs (`orchester_core::run::inputs`, `schema::run_inputs`), kept in the transaction that
//! records the run and never changed. The canonical content is kept as its canonical bytes with their hash, which is
//! checked again when it is read; the local bindings and consent beside it. A run whose task definition is gone
//! still reads them.

use orchester_core::{
  id::{OperationId, RunId},
  run::{
    bundle::canonical_bytes,
    inputs::{BundleContent, InputsOrigin, LocalBindings, LocalConsent, RunInputs},
  },
};
use rusqlite::{Connection, OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const BINDINGS_SCHEMA: u32 = 1;
const CONSENT_SCHEMA: u32 = 1;

impl ProjectDb {
  /// The inputs run `run` was created with; `None` for a run created without them.
  pub fn run_inputs(&self, run: RunId) -> Result<Option<RunInputs>, StoreError> {
    let row: Option<(String, String, String, String)> = self
      .connection
      .query_row(
        "SELECT bundles.sha256, bundles.body, bindings.body, consent.body FROM run_bundles AS bundles \
         JOIN run_local_bindings AS bindings ON bindings.run_id = bundles.run_id \
         JOIN run_local_consent AS consent ON consent.run_id = bundles.run_id WHERE bundles.run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?;
    let Some((sha256, bundle, bindings, consent)) = row else {
      return Ok(None);
    };
    let bundle: BundleContent = serde_json::from_str(&bundle).map_err(|error| corrupt("run bundle", error))?;
    let hash = bundle.sha256().map_err(|error| corrupt("run bundle", error))?;
    if hash.as_str() != sha256 {
      return Err(corrupt("run bundle", "its content does not have its hash"));
    }
    let bindings: LocalBindings = decode_json("run bindings", BINDINGS_SCHEMA, &bindings)?;
    let consent: LocalConsent = decode_json("run consent", CONSENT_SCHEMA, &consent)?;
    RunInputs::new(bundle, bindings, consent)
      .map(Some)
      .map_err(|error| corrupt("run inputs", error))
  }
}

/// Keeps `inputs` as the inputs of the new run `run`.
pub(super) fn insert(connection: &Connection, run: RunId, inputs: &RunInputs) -> Result<(), StoreError> {
  let bundle = inputs.bundle();
  let bytes = canonical_bytes(bundle).map_err(|_| StoreError::Unrepresentable("run bundle"))?;
  let body = String::from_utf8(bytes).map_err(|_| StoreError::Unrepresentable("run bundle"))?;
  let sha256 = bundle.sha256().map_err(|_| StoreError::Unrepresentable("run bundle"))?;
  let (origin, operation) = match bundle.origin() {
    InputsOrigin::Local => ("local", None),
    InputsOrigin::Team(accepted) => ("team", Some(accepted.operation.to_string())),
  };
  let run = run.to_string();
  connection.execute(
    "INSERT INTO run_bundles (run_id, origin, operation_id, sha256, body) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![run, origin, operation, sha256.as_str(), body],
  )?;
  connection.execute(
    "INSERT INTO run_local_bindings (run_id, body) VALUES (?1, ?2)",
    params![run, encode_json("run bindings", BINDINGS_SCHEMA, inputs.bindings())?],
  )?;
  connection.execute(
    "INSERT INTO run_local_consent (run_id, body) VALUES (?1, ?2)",
    params![run, encode_json("run consent", CONSENT_SCHEMA, inputs.consent())?],
  )?;
  Ok(())
}

/// The run the team operation `operation` started; `None` when it started none here.
pub(super) fn run_of_operation(connection: &Connection, operation: OperationId) -> Result<Option<RunId>, StoreError> {
  let run: Option<String> = connection
    .query_row(
      "SELECT run_id FROM run_bundles WHERE operation_id = ?1",
      [operation.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  run.map(|run| parse("run id", &run)).transpose()
}
