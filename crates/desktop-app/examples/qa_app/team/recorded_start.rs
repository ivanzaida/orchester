//! KON-5's start as the project left it before the window opened (ORC-885c), for the states whose notice the task page
//! shows without a press of Run: a start that waits for the team server (`team-start-waiting`), one the runner refuses
//! when recovery asks (`team-start-runner-refused`), and one that failed for good in an earlier window
//! (`team-start-past-failure`). The records are those the workspace's team work writes
//! (`orchester_workspace::team_work`); the claim and work ids are made up, as the stand-in server does not check them
//! on the paths these states take. Fixture data for QA, never shipped.

use std::{error::Error, path::Path};

use orchester_api::v2::{EXECUTION_BUNDLE_SCHEMA, Subject};
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, ProjectId, ServerId, WorkId},
  time::Timestamp,
};
use rusqlite::{Connection, params};
use serde_json::json;

use super::State;

/// The number of the task every start state records its start for: API rate limits, held by nobody.
pub const KON_5: u32 = 5;

/// Records the start of the team task `subject` in the project at `root`, linked to team project `project` of
/// `server`, as `state` says it stands: the task's identity is recorded first, as the first sync records it.
pub fn record(
  root: &Path,
  (server, project): (ServerId, ProjectId),
  (state, subject): (State, Subject),
) -> Result<(), Box<dyn Error>> {
  let Subject::Task { id: task } = subject else {
    return Err("a start is of a task".into());
  };
  let task = task.to_string();
  let (stage, failure) = match state {
    State::StartWaiting => ("accepted", None),
    State::StartRunnerRefused => ("inputs_stored", None),
    State::StartPastFailure => ("failed", Some(failed())),
    other => return Err(format!("{other:?} records no start").into()),
  };
  let connection = Connection::open(root.join(".orchester").join("orchester.db"))?;
  connection.execute(
    "INSERT OR IGNORE INTO task_identities (id, owner, server_id, team_project_id, team_task_id, recorded_revision) \
     VALUES (?1, 'team', ?2, ?3, ?1, 1)",
    params![task, server.to_string(), project.to_string()],
  )?;
  let at = Timestamp::now().unix_millis() - 3 * 60 * 1000;
  connection.execute(
    "INSERT INTO team_starts (operation_id, task_id, request, stage, work_id, bundle_sha256, failure, created_at, \
     updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
    params![
      OperationId::generate().to_string(),
      task,
      request(&task),
      stage,
      WorkId::generate().to_string(),
      "0".repeat(64),
      failure,
      at
    ],
  )?;
  Ok(())
}

/// What a start keeps of its request.
fn request(task: &str) -> String {
  json!({
    "authority": {
      "subject": {"kind": "task", "id": task},
      "claim_id": ClaimId::generate().to_string(),
      "work_id": null,
    },
    "base_commit": "0123456789abcdef0123456789abcdef01234567",
    "read": 1,
    "module": ModuleId::generate().to_string(),
    "module_read": 1,
    "programs": [],
  })
  .to_string()
}

/// A bundle built in the schema one newer than this computer reads, and the claim released with the failure.
fn failed() -> String {
  json!({
    "failure": {"kind": "bundle_schema", "reason": {"read": EXECUTION_BUNDLE_SCHEMA, "built": EXECUTION_BUNDLE_SCHEMA + 1}},
    "claim": "released",
  })
  .to_string()
}
