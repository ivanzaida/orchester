//! Run inputs recorded before the reference bindings were removed still read.

use orchester_core::id::SecretId;

use super::*;

/// A run recorded before the team configuration model kept the references its bodies named by the publisher's ids with
/// this computer's bindings (schema 1); they are gone from what is read, the rest is as it was.
#[test]
fn team_run_inputs_recorded_with_references_read_without_them() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let inputs = team.inputs(accepted(task, OperationId::generate()));
  let run = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), &inputs, (at(6), None))
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
  document["body"]["references"] = serde_json::json!([
    {"kind": "secret", "named": SecretId::generate(), "local": SecretId::generate()}
  ]);
  raw
    .execute(
      "UPDATE run_local_bindings SET body = ?2 WHERE run_id = ?1",
      [run.id().to_string(), document.to_string()],
    )
    .expect("an older record");
  drop(raw);
  let db = ProjectDb::open(team.repo.path()).expect("open");
  assert_eq!(db.run_inputs(run.id()).expect("read"), Some(inputs));
}
