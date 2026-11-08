//! Team inputs kept by the operation that accepted them, before the runner starts their run; and the usage and plan
//! limits of a team run, which name its task from its inputs when this computer has no definition of it.

use super::*;

#[test]
fn team_run_inputs_are_kept_by_their_operation_until_their_run_starts() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let operation = OperationId::generate();
  let inputs = team.inputs(accepted(task, operation));
  assert_eq!(team.db.accepted_team_inputs(operation).expect("read"), None);
  team.db.accept_team_inputs(id, &inputs, at(5)).expect("kept");
  team
    .db
    .accept_team_inputs(id, &inputs, at(6))
    .expect("kept again, unchanged");
  let kept = AcceptedInputs {
    task: id,
    inputs: inputs.clone(),
  };
  assert_eq!(team.db.accepted_team_inputs(operation).expect("read"), Some(kept));

  let other = team.inputs(accepted(task, operation));
  let refused = team.db.accept_team_inputs(id, &other, at(7));
  assert!(
    matches!(refused, Err(RunStoreError::InputsMismatch { .. })),
    "{refused:?}"
  );
  let local = team.inputs(InputsOrigin::Local);
  let refused = team.db.accept_team_inputs(id, &local, at(7));
  assert!(
    matches!(refused, Err(RunStoreError::InputsMismatch { .. })),
    "{refused:?}"
  );
  let stranger = team.db.record_team_task(team.task(), revision(1)).expect("identity");
  let refused = team.db.accept_team_inputs(stranger, &inputs, at(7));
  assert!(
    matches!(refused, Err(RunStoreError::InputsMismatch { .. })),
    "{refused:?}"
  );

  assert_eq!(team.db.run_of_operation(operation).expect("read"), None);
  let run = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), &inputs, (at(8), None))
    .expect("a team run");
  assert_eq!(team.db.run_of_operation(operation).expect("read"), Some(run.id()));
  drop(team.db);
  let refused = connection(team.repo.path())
    .execute("DELETE FROM accepted_run_inputs", [])
    .expect_err("kept inputs never change");
  assert!(refused.to_string().contains("run inputs are immutable"), "{refused}");
}

#[test]
fn team_run_inputs_name_the_task_in_usage_and_plan_limits_without_its_definition() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let inputs = team.inputs(accepted(task, OperationId::generate()));
  let mut run = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), &inputs, (at(6), None))
    .expect("a team run");
  let events = run.start(at(7)).expect("start");
  team.db.append_events(&run, &events).expect("it executes");
  let execution = run.executions_to_start(1)[0];
  team
    .db
    .start_invocation(run.id(), execution, &model(), at(8))
    .expect("an agent starts");
  let reading = LimitReading {
    provider: AgentProvider::CodexCli,
    account: None,
    at: at(9),
    limits: PlanLimits {
      windows: Vec::new(),
      plan: None,
      credits: None,
      blocked: None,
    },
  };
  team.db.save_limit_snapshot(run.id(), &reading).expect("limits");
  assert_eq!(team.db.task(id).expect("read"), None, "no definition is kept");

  let usage = team.db.usage_facts(at(0), at(10)).expect("usage");
  assert_eq!(usage.runs.len(), 1, "the team run is counted");
  let facts = &usage.runs[0];
  assert_eq!(facts.task, id);
  assert_eq!(&*facts.task_key, "ORC-7");
  assert_eq!(&*facts.task_title, "Say hello");
  assert_eq!(facts.pipeline.as_deref(), Some("Delivery"));
  let snapshots = team.db.limit_snapshots(at(0)).expect("limits");
  let source = snapshots[0].source.as_ref().expect("the run's task");
  assert_eq!(&*source.task_key, "ORC-7");
}
