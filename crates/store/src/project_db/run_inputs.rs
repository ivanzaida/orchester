//! A run's execution inputs (`orchester_core::run::inputs`, `schema::run_inputs`), kept in the transaction that
//! records the run and never changed. The canonical content is kept as its canonical bytes with their hash, which is
//! checked again when it is read; the local bindings and consent beside it. A run whose task definition is gone
//! still reads them. Team inputs the server accepted are kept by their operation before the runner starts their run
//! ([`ProjectDb::accept_team_inputs`]), so the runner starts it from what was kept, never from a task's definition.

use orchester_core::{
  id::{OperationId, RunId, TaskId},
  run::{
    bundle::canonical_bytes,
    inputs::{BundleContent, InputsOrigin, LocalBindings, LocalConsent, RunInputs},
  },
  task::identity::TaskOwner,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{ProjectDb, task_identities};
use crate::{
  RunStoreError, StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const BINDINGS_SCHEMA: u32 = 1;
const CONSENT_SCHEMA: u32 = 1;

/// Team inputs the server accepted, for the run of team task `task` the runner has not started yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedInputs {
  pub task: TaskId,
  pub inputs: RunInputs,
}

/// Inputs as their rows keep them: the canonical bytes with their hash, the bindings and the consent.
struct Encoded {
  sha256: String,
  body: String,
  bindings: String,
  consent: String,
}

impl Encoded {
  fn of(inputs: &RunInputs) -> Result<Self, StoreError> {
    let bundle = inputs.bundle();
    let bytes = canonical_bytes(bundle).map_err(|_| StoreError::Unrepresentable("run bundle"))?;
    let body = String::from_utf8(bytes).map_err(|_| StoreError::Unrepresentable("run bundle"))?;
    let sha256 = bundle.sha256().map_err(|_| StoreError::Unrepresentable("run bundle"))?;
    Ok(Self {
      sha256: sha256.as_str().to_owned(),
      body,
      bindings: encode_json("run bindings", BINDINGS_SCHEMA, inputs.bindings())?,
      consent: encode_json("run consent", CONSENT_SCHEMA, inputs.consent())?,
    })
  }

  /// The inputs these rows hold, their content checked against its hash.
  fn decode(&self) -> Result<RunInputs, StoreError> {
    let bundle: BundleContent = serde_json::from_str(&self.body).map_err(|error| corrupt("run bundle", error))?;
    let hash = bundle.sha256().map_err(|error| corrupt("run bundle", error))?;
    if hash.as_str() != self.sha256 {
      return Err(corrupt("run bundle", "its content does not have its hash"));
    }
    let bindings: LocalBindings = decode_json("run bindings", BINDINGS_SCHEMA, &self.bindings)?;
    let consent: LocalConsent = decode_json("run consent", CONSENT_SCHEMA, &self.consent)?;
    RunInputs::new(bundle, bindings, consent).map_err(|error| corrupt("run inputs", error))
  }
}

impl ProjectDb {
  /// The inputs run `run` was created with; `None` for a run created without them.
  pub fn run_inputs(&self, run: RunId) -> Result<Option<RunInputs>, StoreError> {
    let row: Option<Encoded> = self
      .connection
      .query_row(
        "SELECT bundles.sha256, bundles.body, bindings.body, consent.body FROM run_bundles AS bundles \
         JOIN run_local_bindings AS bindings ON bindings.run_id = bundles.run_id \
         JOIN run_local_consent AS consent ON consent.run_id = bundles.run_id WHERE bundles.run_id = ?1",
        [run.to_string()],
        |row| {
          Ok(Encoded {
            sha256: row.get(0)?,
            body: row.get(1)?,
            bindings: row.get(2)?,
            consent: row.get(3)?,
          })
        },
      )
      .optional()?;
    row.map(|row| row.decode()).transpose()
  }

  /// Keeps `inputs`, which the team server accepted for the team task whose identity is `task`, by the operation that
  /// accepted them, until the runner starts their run. Keeping the same inputs again changes nothing; other inputs for
  /// the same operation are refused, as are local inputs and inputs of another team task.
  pub fn accept_team_inputs(&mut self, task: TaskId, inputs: &RunInputs, now: Timestamp) -> Result<(), RunStoreError> {
    let mismatch = |reason| RunStoreError::InputsMismatch { task, reason };
    let InputsOrigin::Team(accepted) = inputs.bundle().origin() else {
      return Err(mismatch("the inputs are not a team's"));
    };
    let encoded = Encoded::of(inputs)?;
    let transaction = self.connection.transaction()?;
    let identity = task_identities::load(&transaction, task)?.ok_or(StoreError::TaskNotFound(task))?;
    if !matches!(identity.owner, TaskOwner::Team { task: team, .. } if team == accepted.task) {
      return Err(mismatch("the task is not the team task the inputs were accepted for"));
    }
    let operation = accepted.operation.to_string();
    if let Some(kept) = load_accepted(&transaction, &operation)? {
      let same = kept.0 == task
        && kept.1.sha256 == encoded.sha256
        && kept.1.bindings == encoded.bindings
        && kept.1.consent == encoded.consent;
      return if same {
        Ok(())
      } else {
        Err(mismatch("the operation accepted other inputs"))
      };
    }
    transaction.execute(
      "INSERT INTO accepted_run_inputs (operation_id, task_id, sha256, body, bindings, consent, accepted_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        operation,
        task.to_string(),
        encoded.sha256,
        encoded.body,
        encoded.bindings,
        encoded.consent,
        now.unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// The team inputs operation `operation` accepted, as [`Self::accept_team_inputs`] kept them; `None` when none were.
  pub fn accepted_team_inputs(&self, operation: OperationId) -> Result<Option<AcceptedInputs>, StoreError> {
    load_accepted(&self.connection, &operation.to_string())?
      .map(|(task, encoded)| {
        Ok(AcceptedInputs {
          task,
          inputs: encoded.decode()?,
        })
      })
      .transpose()
  }

  /// The run the team operation `operation` started; `None` when it started none here.
  pub fn run_of_operation(&self, operation: OperationId) -> Result<Option<RunId>, StoreError> {
    run_of_operation(&self.connection, operation)
  }
}

fn load_accepted(connection: &Connection, operation: &str) -> Result<Option<(TaskId, Encoded)>, StoreError> {
  let row: Option<(String, Encoded)> = connection
    .query_row(
      "SELECT task_id, sha256, body, bindings, consent FROM accepted_run_inputs WHERE operation_id = ?1",
      [operation],
      |row| {
        Ok((
          row.get(0)?,
          Encoded {
            sha256: row.get(1)?,
            body: row.get(2)?,
            bindings: row.get(3)?,
            consent: row.get(4)?,
          },
        ))
      },
    )
    .optional()?;
  row
    .map(|(task, encoded)| Ok((parse("task id", &task)?, encoded)))
    .transpose()
}

/// Keeps `inputs` as the inputs of the new run `run`.
pub(super) fn insert(connection: &Connection, run: RunId, inputs: &RunInputs) -> Result<(), StoreError> {
  let encoded = Encoded::of(inputs)?;
  let (origin, operation) = match inputs.bundle().origin() {
    InputsOrigin::Local => ("local", None),
    InputsOrigin::Team(accepted) => ("team", Some(accepted.operation.to_string())),
  };
  let run = run.to_string();
  connection.execute(
    "INSERT INTO run_bundles (run_id, origin, operation_id, sha256, body) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![run, origin, operation, encoded.sha256, encoded.body],
  )?;
  connection.execute(
    "INSERT INTO run_local_bindings (run_id, body) VALUES (?1, ?2)",
    params![run, encoded.bindings],
  )?;
  connection.execute(
    "INSERT INTO run_local_consent (run_id, body) VALUES (?1, ?2)",
    params![run, encoded.consent],
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

/// Whether the team server accepted the inputs `run` executes with: its task's definition, status and readiness are
/// the server's, never changed here.
pub(super) fn is_team_run(connection: &Connection, run: RunId) -> Result<bool, StoreError> {
  Ok(connection.query_row(
    "SELECT EXISTS (SELECT 1 FROM run_bundles WHERE run_id = ?1 AND origin = 'team')",
    [run.to_string()],
    |row| row.get(0),
  )?)
}
