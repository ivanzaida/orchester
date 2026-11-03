//! Scoped invalidation subscriptions (S11): the wire shapes of query scopes and subscription events, unknown fields
//! refused, and scope sets that de-duplicate stably.

use std::collections::{BTreeSet, HashSet};

use orchester_api::v2::{
  EventCursor,
  subscription::{QueryScope, SubscriptionEvent},
};
use orchester_core::id::{ModuleId, ResourceId, TaskId};
use serde_json::{Value, json};

fn cursor(position: &str) -> EventCursor {
  EventCursor::new(position).expect("cursor")
}

fn scopes_with_wire() -> Vec<(QueryScope, Value)> {
  let (task, module, resource) = (TaskId::generate(), ModuleId::generate(), ResourceId::generate());
  vec![
    (QueryScope::Workspace {}, json!({"kind": "workspace"})),
    (QueryScope::TaskList {}, json!({"kind": "task_list"})),
    (QueryScope::Task { task }, json!({"kind": "task", "task": task})),
    (
      QueryScope::TaskComments { task },
      json!({"kind": "task_comments", "task": task}),
    ),
    (QueryScope::ModuleList {}, json!({"kind": "module_list"})),
    (
      QueryScope::Module { module },
      json!({"kind": "module", "module": module}),
    ),
    (QueryScope::Library {}, json!({"kind": "library"})),
    (
      QueryScope::Resource { resource },
      json!({"kind": "resource", "resource": resource}),
    ),
  ]
}

fn events_with_wire() -> Vec<(SubscriptionEvent, Value)> {
  let task = TaskId::generate();
  vec![
    (
      SubscriptionEvent::Ready { cursor: cursor("1") },
      json!({"kind": "ready", "cursor": "1"}),
    ),
    (
      SubscriptionEvent::Invalidated {
        cursor: cursor("2"),
        scopes: vec![QueryScope::TaskList {}, QueryScope::Task { task }],
      },
      json!({
        "kind": "invalidated",
        "cursor": "2",
        "scopes": [{"kind": "task_list"}, {"kind": "task", "task": task}]
      }),
    ),
    (
      SubscriptionEvent::Reset { cursor: cursor("3") },
      json!({"kind": "reset", "cursor": "3"}),
    ),
    (SubscriptionEvent::Revoked {}, json!({"kind": "revoked"})),
  ]
}

#[test]
fn subscription_query_scope_wire_shapes_are_exact() {
  let scopes = scopes_with_wire();
  assert_eq!(scopes.len(), 8);
  for (scope, wire) in scopes {
    assert_eq!(serde_json::to_value(scope).expect("serializes"), wire);
    assert_eq!(serde_json::from_value::<QueryScope>(wire).expect("reads"), scope);
  }
}

#[test]
fn subscription_event_wire_shapes_are_exact() {
  for (event, wire) in events_with_wire() {
    assert_eq!(serde_json::to_value(&event).expect("serializes"), wire);
    assert_eq!(serde_json::from_value::<SubscriptionEvent>(wire).expect("reads"), event);
  }
}

#[test]
fn subscription_refuses_unknown_and_malformed_fields() {
  for (_, wire) in scopes_with_wire() {
    let mut unknown = wire.clone();
    unknown["snapshot"] = json!({});
    let refused = serde_json::from_value::<QueryScope>(unknown).expect_err("refused");
    assert!(refused.to_string().contains("unknown field"), "{wire}: {refused}");
  }
  for (_, wire) in events_with_wire() {
    let mut unknown = wire.clone();
    unknown["snapshot"] = json!({});
    let refused = serde_json::from_value::<SubscriptionEvent>(unknown).expect_err("refused");
    assert!(refused.to_string().contains("unknown field"), "{wire}: {refused}");
  }
  let mut nested = json!({"kind": "invalidated", "cursor": "2", "scopes": [{"kind": "library", "id": "x"}]});
  assert!(serde_json::from_value::<SubscriptionEvent>(nested.clone()).is_err());
  nested["scopes"] = json!([{"kind": "project"}]);
  assert!(serde_json::from_value::<SubscriptionEvent>(nested).is_err());

  for malformed in [
    json!({"kind": "ready"}),
    json!({"kind": "ready", "cursor": ""}),
    json!({"kind": "ready", "cursor": "c-1"}),
    json!({"kind": "reset", "cursor": "3", "scopes": []}),
    json!({"kind": "invalidated", "cursor": "2"}),
    json!({"kind": "task"}),
    json!({"kind": "module", "task": TaskId::generate()}),
  ] {
    assert!(
      serde_json::from_value::<SubscriptionEvent>(malformed.clone()).is_err()
        && serde_json::from_value::<QueryScope>(malformed.clone()).is_err(),
      "{malformed}"
    );
  }
}

#[test]
fn subscription_scopes_deduplicate_stably() {
  let task = TaskId::generate();
  let repeated = [
    QueryScope::Task { task },
    QueryScope::Workspace {},
    QueryScope::Task { task },
    QueryScope::TaskComments { task },
    QueryScope::Workspace {},
  ];
  let mut reversed = repeated;
  reversed.reverse();

  let set: BTreeSet<QueryScope> = repeated.into_iter().collect();
  let expected = vec![
    QueryScope::Workspace {},
    QueryScope::Task { task },
    QueryScope::TaskComments { task },
  ];
  assert_eq!(set.iter().copied().collect::<Vec<_>>(), expected);
  assert_eq!(reversed.into_iter().collect::<BTreeSet<_>>(), set);
  assert_eq!(repeated.into_iter().collect::<HashSet<_>>().len(), 3);

  assert_ne!(QueryScope::Task { task }, QueryScope::TaskComments { task });
  assert_ne!(
    QueryScope::Task { task },
    QueryScope::Task {
      task: TaskId::generate()
    }
  );
}
