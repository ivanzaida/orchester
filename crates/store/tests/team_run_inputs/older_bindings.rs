//! Run inputs recorded before the reference bindings were removed still read.

use orchester_core::id::{RunId, SecretId};
use serde_json::json;

use super::*;

/// Records a team run of `inputs` under `task`, then rewrites its bindings as schema 1 kept them: `references` in the
/// place of `local_mcp`. Returns the project's directory and the run.
fn recorded_as_schema_1(mut team: Team, task: TeamTaskRef, inputs: &RunInputs, references: Value) -> (TempDir, RunId) {
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let run = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), inputs, (at(6), None))
    .expect("a team run");
  drop(team.db);
  let raw = connection(team.repo.path());
  let triggers: Vec<String> = raw
    .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'run_local_bindings'")
    .expect("triggers")
    .query_map([], |row| row.get(0))
    .expect("names")
    .collect::<Result<_, _>>()
    .expect("rows");
  for trigger in triggers {
    raw
      .execute_batch(&format!("DROP TRIGGER \"{trigger}\""))
      .expect("the guard goes");
  }
  let text: String = raw
    .query_row(
      "SELECT body FROM run_local_bindings WHERE run_id = ?1",
      [run.id().to_string()],
      |row| row.get(0),
    )
    .expect("stored");
  let mut document: Value = serde_json::from_str(&text).expect("json");
  document["schema"] = Value::from(1);
  let body = document["body"].as_object_mut().expect("bindings");
  body.remove("local_mcp");
  body.insert("references".into(), references);
  raw
    .execute(
      "UPDATE run_local_bindings SET body = ?2 WHERE run_id = ?1",
      [run.id().to_string(), document.to_string()],
    )
    .expect("an older record");
  (team.repo, run.id())
}

/// A run recorded before the team configuration model kept the references its bodies named by the publisher's ids with
/// this computer's bindings (schema 1); they are gone from what is read, the rest is as it was.
#[test]
fn team_run_inputs_recorded_with_references_read_without_them() {
  let team = team_project();
  let task = team.task();
  let inputs = team.inputs(accepted(task, OperationId::generate()));
  let references = json!([{"kind": "secret", "named": SecretId::generate(), "local": SecretId::generate()}]);
  let (repo, run) = recorded_as_schema_1(team, task, &inputs, references);
  let db = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(db.run_inputs(run).expect("read"), Some(inputs));
}

/// A run whose profile chose this computer's MCP servers through references (schema 1) reads with the servers it ran
/// with, bound as the local servers of its bindings, and so is still found whole by recovery and by continuing it.
#[test]
fn team_run_inputs_recorded_with_mcp_server_references_keep_the_servers_they_ran_with() {
  let team = team_project();
  let task = team.task();
  let (first, second) = (ConnectionId::generate(), ConnectionId::generate());
  let original = team.inputs(accepted(task, OperationId::generate()));
  let mut bindings = original.bindings().clone();
  bindings.local_mcp = vec![first, second];
  bindings.profiles[0].mcp_servers = vec![first, second];
  let inputs = RunInputs::new(original.bundle().clone(), bindings, original.consent().clone()).expect("inputs");
  let references = json!([
    {"kind": "connection", "named": ConnectionId::generate(), "local": ConnectionId::generate()},
    {"kind": "mcp_server", "named": ConnectionId::generate(), "local": first},
    {"kind": "secret", "named": SecretId::generate(), "local": SecretId::generate()},
    {"kind": "mcp_server", "named": ConnectionId::generate(), "local": second},
  ]);
  let (repo, run) = recorded_as_schema_1(team, task, &inputs, references);
  let db = ProjectDb::open(repo.path()).expect("open");
  let read = db
    .run_inputs(run)
    .expect("an older run with MCP servers reads")
    .expect("inputs");
  assert_eq!(read, inputs);
  assert_eq!(read.bindings().local_mcp, vec![first, second]);
}
