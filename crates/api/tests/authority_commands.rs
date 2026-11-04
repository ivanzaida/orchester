//! The team commands of P2 on the S4 envelopes: each internally tagged by its kind, patches as `{field, value}`
//! entries with `null` apart from a missing field, values carrying their local effects, unknown fields refused.
//!
//! `cargo test -p orchester-api --test authority_commands authority_commands_`

use orchester_api::v2::{
  CommandRequest, ExpectedRevision, ExpectedRevisions, LocalEffect, PolicyRevision, RevisionTarget, Subject, TaskField,
  TeamCommand, TeamCommandValue, WorkspacePolicy,
};
use orchester_core::{
  id::{ModuleId, OperationId, ProjectId, TaskId, UserId},
  task::organization::Priority,
};
use serde_json::json;

#[test]
fn authority_commands_requests_are_tagged_and_patches_name_their_fields() {
  let (operation, task, module) = (OperationId::generate(), TaskId::generate(), ModuleId::generate());
  let request = CommandRequest {
    operation_id: operation,
    expected: ExpectedRevisions::new(vec![
      ExpectedRevision::new(RevisionTarget::Task(task), 3).expect("revision"),
    ])
    .expect("expected"),
    command: TeamCommand::TaskPatch {
      task_id: task,
      changes: vec![
        TaskField::Title("Sign in".to_owned()),
        TaskField::Priority(Priority::High),
        TaskField::Module(None),
        TaskField::Module(Some(module)),
      ],
    },
  };
  let wire = json!({
    "operation_id": operation,
    "expected": [{"kind": "task", "id": task, "revision": 3}],
    "command": {"kind": "task.patch", "task_id": task, "changes": [
      {"field": "title", "value": "Sign in"},
      {"field": "priority", "value": "high"},
      {"field": "module", "value": null},
      {"field": "module", "value": module}
    ]}
  });
  assert_eq!(serde_json::to_value(&request).expect("serialize"), wire);
  assert_eq!(
    serde_json::from_value::<CommandRequest<TeamCommand>>(wire).expect("read"),
    request
  );
  assert_eq!(request.command.kind(), "task.patch");

  let unknown = json!({"kind": "task.patch", "task_id": task, "changes": [{"field": "pipeline", "value": "x"}]});
  assert!(
    serde_json::from_value::<TeamCommand>(unknown).is_err(),
    "no patch of an open field"
  );
  let extra = json!({"kind": "policy.set", "automatic_merge": true, "revision": 2});
  assert!(serde_json::from_value::<TeamCommand>(extra).is_err());
  let assignment = json!({"kind": "assignment.set", "subject": {"kind": "task", "id": task}});
  assert!(
    serde_json::from_value::<TeamCommand>(assignment).is_err(),
    "user_id is sent, null for nobody"
  );
  let user = UserId::generate();
  let set = TeamCommand::AssignmentSet {
    subject: Subject::Task { id: task },
    user_id: Some(user),
  };
  assert_eq!(
    serde_json::to_value(&set).expect("serialize"),
    json!({"kind": "assignment.set", "subject": {"kind": "task", "id": task}, "user_id": user})
  );
}

#[test]
fn authority_commands_values_carry_their_effects() {
  let value = TeamCommandValue::Policy {
    policy: WorkspacePolicy {
      revision: PolicyRevision::try_from(2).expect("revision"),
      automatic_merge: true,
    },
    effects: vec![LocalEffect::None {}],
  };
  let wire =
    json!({"kind": "policy", "policy": {"revision": 2, "automatic_merge": true}, "effects": [{"kind": "none"}]});
  assert_eq!(serde_json::to_value(&value).expect("serialize"), wire);
  assert_eq!(serde_json::from_value::<TeamCommandValue>(wire).expect("read"), value);
  assert_eq!(value.effects(), [LocalEffect::None {}]);
  let project = ProjectId::generate();
  let expected = ExpectedRevision::new(RevisionTarget::Project(project), 2).expect("revision");
  assert_eq!(
    serde_json::to_value(expected).expect("serialize"),
    json!({"kind": "project", "id": project, "revision": 2})
  );
}
