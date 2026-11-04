//! The team commands on the S4 envelopes, field for field the team server's `Command` and `CommandValue`: each
//! internally tagged by its kind, unknown kinds and fields refused, values carrying their local effects.
//!
//! `cargo test -p orchester-api --test authority_commands authority_commands_`

use orchester_api::v2::{
  CommandRequest, ExpectedRevision, ExpectedRevisions, LocalEffect, RevisionTarget, Subject, TeamCommand,
  TeamCommandValue,
};
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, TaskId, UserId},
  run::{BranchName, CommitId},
};
use serde_json::json;

#[test]
fn authority_commands_requests_carry_the_servers_kinds_and_shapes() {
  let (operation, module) = (OperationId::generate(), ModuleId::generate());
  let request = CommandRequest {
    operation_id: operation,
    expected: ExpectedRevisions::new(vec![
      ExpectedRevision::new(RevisionTarget::Module(module), 3).expect("revision"),
    ])
    .expect("expected"),
    command: TeamCommand::ModuleBranchSet {
      module_id: module,
      branch: BranchName::new("module/billing").expect("branch"),
    },
  };
  let wire = json!({
    "operation_id": operation,
    "expected": [{"kind": "module", "id": module, "revision": 3}],
    "command": {"kind": "module.branch.set", "module_id": module, "branch": "module/billing"}
  });
  assert_eq!(serde_json::to_value(&request).expect("serialize"), wire);
  assert_eq!(
    serde_json::from_value::<CommandRequest<TeamCommand>>(wire).expect("read"),
    request
  );
  assert_eq!(request.command.kind(), "module.branch.set");

  let (claim, commit) = (ClaimId::generate(), "c".repeat(40));
  let authorize = TeamCommand::DeliveryAuthorize {
    subject: Subject::Module { id: module },
    claim_id: claim,
    branch: BranchName::new("module/billing").expect("branch"),
    source_commit: CommitId::new(&commit).expect("commit"),
    base_commit: CommitId::new(&commit).expect("commit"),
  };
  assert_eq!(
    serde_json::to_value(&authorize).expect("serialize"),
    json!({"kind": "delivery.authorize", "subject": {"kind": "module", "id": module}, "claim_id": claim,
      "branch": "module/billing", "source_commit": commit, "base_commit": commit})
  );

  let task = TaskId::generate();
  for refused in [
    json!({"kind": "module.branch.set", "module_id": module, "branch": null}),
    json!({"kind": "module.branch", "module_id": module, "branch": "module/billing"}),
    json!({"kind": "task.patch", "task_id": task, "changes": []}),
    json!({"kind": "policy.set", "automatic_merge": true}),
    json!({"kind": "tag.create", "name": "ui"}),
    json!({"kind": "assignment.set", "subject": {"kind": "task", "id": task}}),
  ] {
    assert!(
      serde_json::from_value::<TeamCommand>(refused.clone()).is_err(),
      "{refused}"
    );
  }
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
  let (task, user) = (TaskId::generate(), UserId::generate());
  let attribution = json!({"by": {"id": user, "display_name": "Alice"}, "at": 1});
  let wire = json!({"kind": "task", "effects": [], "task": {
    "id": task, "number": 1, "key": "KON-1", "former_key": null, "revision": 1, "title": "Sign in", "brief": "",
    "criteria": [], "pipeline": null, "limits": null, "workflow": "backlog", "priority": "none", "tags": [],
    "blocking": [], "blocked_reason": null, "module": null, "status": "draft", "has_run": false,
    "created": attribution, "updated": attribution, "claim": null, "run": null
  }});
  let value: TeamCommandValue = serde_json::from_value(wire).expect("a task value");
  let TeamCommandValue::Task { task: read, .. } = &value else {
    panic!("not a task: {value:?}");
  };
  assert_eq!((read.id, read.content.title.as_str()), (task, "Sign in"));
  assert!(value.effects().is_empty());

  let (module, claim, commit) = (ModuleId::generate(), ClaimId::generate(), "d".repeat(40));
  let wire = json!({"kind": "delivery", "effects": [], "delivery": {
    "subject": {"kind": "module", "id": module}, "claim_id": claim, "authorization_operation_id": null,
    "branch": "module/billing", "source_commit": commit, "base_commit": commit, "state": "authorized", "revision": 1
  }});
  let value: TeamCommandValue = serde_json::from_value(wire.clone()).expect("a delivery value");
  assert_eq!(serde_json::to_value(&value).expect("serialize"), wire);

  let bundle = json!({"sha256": "a".repeat(64), "schema_version": 1});
  let start = json!({"kind": "start", "work_id": orchester_core::id::WorkId::generate(), "bundle": bundle});
  let wire = json!({"kind": "work", "bundle": bundle, "effects": [start], "work": {
    "id": start["work_id"], "subject": {"kind": "task", "id": task}, "claim_id": claim, "revision": 1,
    "state": "running", "runs": [], "bundle": bundle, "created_at": 1, "updated_at": 1
  }});
  let value: TeamCommandValue = serde_json::from_value(wire).expect("a work value");
  assert!(matches!(value.effects(), [LocalEffect::Start { .. }]), "{value:?}");
  let policy = json!({"kind": "policy", "policy": {"revision": 2, "automatic_merge": true}, "effects": []});
  assert!(
    serde_json::from_value::<TeamCommandValue>(policy).is_err(),
    "the policy is no command"
  );
}
