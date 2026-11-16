//! The lifecycle rule and the capability names, apart from any server.

use orchester_api::{
  TaskChanges, TaskContent,
  v2::{
    Action, ActionCapability, ArtifactKind, ArtifactRef, CommandKind, DenialReason, LifecycleAction, LifecycleRequest,
    TaskCaller, completes_on_success,
  },
};
use orchester_core::{
  blob::ContentHash,
  resource::Deliverable,
  task::organization::WorkflowStatus::{self, Backlog, Canceled, Done, InProgress, InReview, Todo},
};
use serde_json::{Value, json};

const ADMIN: TaskCaller = TaskCaller {
  admin: true,
  holder: false,
};
const HOLDER: TaskCaller = TaskCaller {
  admin: false,
  holder: true,
};
const MEMBER: TaskCaller = TaskCaller {
  admin: false,
  holder: false,
};

fn content(workflow: &str, blocked: Option<&str>) -> TaskContent {
  serde_json::from_value(json!({"title": "A task", "workflow": workflow, "blocked_reason": blocked})).expect("content")
}

fn changes(wire: Value) -> TaskChanges {
  serde_json::from_value(wire).expect("changes")
}

fn of(from: WorkflowStatus, to: WorkflowStatus) -> Option<LifecycleAction> {
  LifecycleAction::of_workflow(from, to)
}

#[test]
fn team_lifecycle_workflow_changes_are_the_named_actions() {
  use LifecycleAction::{Cancel, Complete, Reopen, Triage};
  for open in [Backlog, Todo, InProgress, InReview] {
    assert_eq!(of(open, Done), Some(Complete), "{open:?}");
    assert_eq!(of(open, Canceled), Some(Cancel), "{open:?}");
  }
  for closed in [Done, Canceled] {
    assert_eq!(of(closed, Backlog), Some(Reopen), "{closed:?}");
    assert_eq!(of(closed, Todo), Some(Reopen), "{closed:?}");
  }
  assert_eq!(of(Backlog, Todo), Some(Triage));
  assert_eq!(of(Todo, Backlog), Some(Triage));
  // What D4 does not name stays an Admin's: no action.
  for (from, to) in [
    (Done, Canceled),
    (Canceled, Done),
    (Todo, InProgress),
    (InProgress, Todo),
    (InReview, Backlog),
    (Done, InProgress),
    (Done, Done),
  ] {
    assert_eq!(of(from, to), None, "{from:?} to {to:?}");
  }
}

#[test]
fn team_lifecycle_who_may_each_action() {
  for action in LifecycleAction::ALL {
    let status = if action == LifecycleAction::Reopen { Done } else { Todo };
    assert_eq!(action.denial(status, ADMIN), None, "{action:?}: an Admin may");
    let (holder, member) = (action.denial(status, HOLDER), action.denial(status, MEMBER));
    if action.holder_may() {
      assert_eq!(
        (holder, member),
        (None, Some(DenialReason::ClaimRequired)),
        "{action:?}"
      );
    } else {
      let refused = Some(DenialReason::RoleRequired);
      assert_eq!((holder, member), (refused, refused), "{action:?}");
    }
  }
  assert_eq!(
    LifecycleAction::ALL.map(LifecycleAction::holder_may),
    [true, false, false, false, true],
    "only Done and Blocked are the holder's"
  );
  assert_eq!(
    LifecycleAction::ALL.map(LifecycleAction::closes),
    [true, true, false, false, false]
  );
}

#[test]
fn team_lifecycle_a_task_without_the_action_is_transition_denied_for_everybody() {
  let denied = Some(DenialReason::TransitionDenied);
  for caller in [ADMIN, HOLDER, MEMBER] {
    assert_eq!(LifecycleAction::Complete.denial(Done, caller), denied);
    assert_eq!(LifecycleAction::Cancel.denial(Canceled, caller), denied);
    assert_eq!(LifecycleAction::Reopen.denial(Todo, caller), denied);
    assert_eq!(LifecycleAction::Triage.denial(InProgress, caller), denied);
    assert_eq!(LifecycleAction::Triage.denial(Done, caller), denied);
  }
  assert_eq!(
    LifecycleAction::Block.denial(Done, MEMBER),
    Some(DenialReason::ClaimRequired),
    "blocking has every state"
  );
}

#[test]
fn team_lifecycle_actions_are_named_in_capabilities_and_are_no_command_kind() {
  let names = LifecycleAction::ALL.map(LifecycleAction::as_str);
  assert_eq!(
    names,
    [
      "task.complete",
      "task.cancel",
      "task.reopen",
      "task.triage",
      "task.block"
    ]
  );
  for (action, name) in LifecycleAction::ALL.into_iter().zip(names) {
    assert_eq!(serde_json::to_value(Action::Task(action)).expect("wire"), json!(name));
    assert_eq!(
      serde_json::from_value::<Action>(json!(name)).expect("action"),
      Action::Task(action)
    );
    assert!(
      CommandKind::new(name.to_owned()).is_err(),
      "{name} is an action of its own"
    );
  }
  let task = json!({"kind": "task", "id": "0191c7c0-0000-7000-8000-000000000001"});
  let capability = json!({"action": "task.cancel", "subject": task, "allowed": false, "reason": "transition_denied"});
  let read: ActionCapability = serde_json::from_value(capability.clone()).expect("a capability");
  assert_eq!(read.action(), &Action::Task(LifecycleAction::Cancel));
  assert_eq!(read.reason(), Some(DenialReason::TransitionDenied));
  assert_eq!(serde_json::to_value(&read).expect("wire"), capability);
}

#[test]
fn team_lifecycle_a_patch_is_read_against_the_task_as_it_is() {
  let open = content("todo", None);
  let done = LifecycleRequest::of(&open, &changes(json!({"workflow": "done"})));
  assert_eq!(done.actions(), [LifecycleAction::Complete]);
  assert!(done.closes() && done.holder_may());
  assert!(done.permitted(HOLDER) && done.permitted(ADMIN) && !done.permitted(MEMBER));

  let both = LifecycleRequest::of(
    &open,
    &changes(json!({"workflow": "done", "blocked_reason": "waiting for QA"})),
  );
  assert_eq!(both.actions(), [LifecycleAction::Complete, LifecycleAction::Block]);
  assert!(both.holder_may());

  let cancel = LifecycleRequest::of(&open, &changes(json!({"workflow": "canceled"})));
  assert!(cancel.closes() && !cancel.holder_may() && !cancel.permitted(HOLDER) && cancel.permitted(ADMIN));

  let blocked = content("todo", Some("waiting"));
  let cleared = LifecycleRequest::of(&blocked, &changes(json!({"blocked_reason": null})));
  assert_eq!(cleared.actions(), [LifecycleAction::Block], "null clears the reason");
  assert!(cleared.permitted(HOLDER) && !cleared.closes());
}

#[test]
fn team_lifecycle_anything_else_in_the_patch_is_an_admins() {
  let open = content("todo", None);
  for extra in [
    json!({"workflow": "done", "title": "Renamed"}),
    json!({"workflow": "done", "priority": "high"}),
    json!({"workflow": "done", "tags": []}),
    json!({"workflow": "done", "release": null}),
    json!({"workflow": "in_progress"}),
    json!({"title": "Renamed"}),
  ] {
    let request = LifecycleRequest::of(&open, &changes(extra.clone()));
    assert!(!request.holder_may(), "{extra}");
    assert!(request.permitted(ADMIN) && !request.permitted(HOLDER), "{extra}");
  }
  // A status or reason the task has already is no change; an empty patch is an Admin's as it was.
  for unchanged in [
    json!({"workflow": "todo"}),
    json!({"blocked_reason": null}),
    json!({"actor": "person"}),
  ] {
    let request = LifecycleRequest::of(&open, &changes(unchanged.clone()));
    assert!(request.actions().is_empty(), "{unchanged}");
    assert!(!request.permitted(HOLDER), "{unchanged}");
  }
}

fn artifact(kind: ArtifactKind) -> ArtifactRef {
  ArtifactRef {
    sha256: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    kind,
    size_bytes: 8,
  }
}

#[test]
fn team_lifecycle_a_successful_run_completes_only_work_without_a_base_merge() {
  let (report, patch) = (artifact(ArtifactKind::Report), artifact(ArtifactKind::Patch));
  assert!(!completes_on_success(
    Deliverable::Changes,
    std::slice::from_ref(&report)
  ));
  assert!(!completes_on_success(
    Deliverable::Changes,
    &[patch.clone(), report.clone()]
  ));
  assert!(completes_on_success(Deliverable::Report, std::slice::from_ref(&report)));
  assert!(completes_on_success(Deliverable::Outside, &[]));
  assert!(
    completes_on_success(Deliverable::ChangesOrReport, std::slice::from_ref(&report)),
    "no changes: a report"
  );
  assert!(
    !completes_on_success(Deliverable::ChangesOrReport, &[patch, report]),
    "changes: its module delivers them"
  );
}
