//! The project stream's events (`docs/architecture/server.md`, "Events", row 73): every kind's wire name and payload
//! read and written back unchanged, the common delta fields, and the removed and unknown kinds refused.
//!
//! `cargo test -p orchester-api --test events live_updates_`

use orchester_api::{ClaimAction, Event, EventKind, UnknownEventKind};
use orchester_core::task::organization::ChangeActor;
use serde_json::{Value, json};

const TASK: &str = "01a183cf-b73c-71db-8a62-bd509770cd4d";
const USER: &str = "01a183cf-b73c-71db-8a62-bd509770cd4e";
const TAG: &str = "01a183cf-b73c-71db-8a62-bd509770cd4f";
const OTHER_TAG: &str = "01a183cf-b73c-71db-8a62-bd509770cd50";
const MODULE: &str = "01a183cf-b73c-71db-8a62-bd509770cd51";
const PROJECT: &str = "01a183cf-b73c-71db-8a62-bd509770cd52";
const RESOURCE: &str = "01a183cf-b73c-71db-8a62-bd509770cd53";
const CLAIM: &str = "01a183cf-b73c-71db-8a62-bd509770cd54";

fn alice() -> Value {
  json!({ "id": USER, "display_name": "Alice" })
}

fn attribution(at: i64) -> Value {
  json!({ "by": alice(), "at": at })
}

fn claim() -> Value {
  json!({ "id": CLAIM, "holder": alice(), "kind": "run", "since": 10, "heartbeat_at": 20, "stale": true,
    "taken_over_from": null })
}

fn task() -> Value {
  json!({
    "id": TASK, "number": 108, "key": "ORC-108", "former_key": null, "revision": 4,
    "title": "Status bar", "brief": "", "criteria": ["Shows the phase"], "pipeline": null,
    "limits": { "budget_tokens": 1_000 }, "workflow": "in_progress", "priority": "high", "tags": [TAG],
    "blocking": [{ "task": TASK, "gate": "merge" }], "blocked_reason": null, "module": MODULE,
    "status": "queued", "has_run": true, "created": attribution(1), "updated": attribution(2),
    "claim": claim(),
    "run": { "holder": alice(), "number": 3, "phase": "running", "step": "Implement", "started_at": 1_000,
      "phase_since": 2_000, "updated_at": 2_001 }
  })
}

fn tag() -> Value {
  json!({ "id": TAG, "name": "ui" })
}

fn module() -> Value {
  json!({ "id": MODULE, "name": "Live updates", "description": "", "start": "2026-10-29", "target": null,
    "state": "open", "revision": 2, "created": attribution(1), "updated": attribution(2) })
}

fn member() -> Value {
  json!({ "user": alice(), "username": "alice", "role": "admin", "added_at": 5 })
}

fn resource() -> Value {
  json!({ "id": RESOURCE, "kind": "pipeline", "slug": "review", "name": "Review", "archived": false,
    "revision": 3, "latest": 4, "created": attribution(1), "updated": attribution(2) })
}

/// A delta of `kind` with the common fields beside `fields`.
fn delta(kind: &str, fields: Value) -> Value {
  let Value::Object(fields) = fields else {
    panic!("an object of fields");
  };
  let mut data = json!({ "id": 42, "by": alice(), "actor": "person", "at": 1_700 });
  for (field, value) in fields {
    data[field] = value;
  }
  json!({ "kind": kind, "data": data })
}

fn samples() -> Vec<Value> {
  vec![
    json!({ "kind": "snapshot.begin", "data": {
      "project": { "id": PROJECT, "slug": "orchester", "address": "https://team.example/orchester", "key": "ORC",
        "name": "Orchester", "remote_hint": null, "role": "member", "next_number": 400 },
      "members": [member()], "tags": [tag()], "modules": [module()], "resources": [resource()] } }),
    json!({ "kind": "snapshot.tasks", "data": { "tasks": [task()] } }),
    json!({ "kind": "snapshot.end", "data": { "cursor": 41 } }),
    delta("task.created", json!({ "task": task() })),
    delta(
      "task.changed",
      json!({ "task": TASK, "revision": 5, "base": 4, "title": "Status bar 2", "brief": "New", "criteria": [],
        "pipeline": null, "limits": null, "status": "draft", "module": null, "priority": "urgent",
        "workflow": "done", "tags_added": [TAG], "tags_removed": [OTHER_TAG],
        "blocked_by": [{ "task": TASK, "gate": "review" }], "blocked_reason": "Waits for the API" }),
    ),
    delta(
      "task.claim",
      json!({ "task": TASK, "claim": claim(), "action": "stale" }),
    ),
    delta(
      "task.run",
      json!({ "task": TASK, "number": 3, "phase": "stopped_at_limit",
        "step": { "key": "implement", "name": "Implement" }, "phase_since": 2_000, "updated_at": 2_001 }),
    ),
    delta("tag.created", json!({ "tag": tag() })),
    delta("tag.renamed", json!({ "tag": tag() })),
    delta("tag.deleted", json!({ "tag": TAG })),
    delta("tag.merged", json!({ "from": OTHER_TAG, "into": tag() })),
    delta("module.created", json!({ "module": module() })),
    delta("module.changed", json!({ "module": module() })),
    delta("member.added", json!({ "member": member() })),
    delta("member.removed", json!({ "user": USER })),
    delta("member.role_changed", json!({ "user": USER, "role": "admin" })),
    delta(
      "project.changed",
      json!({ "project": { "id": PROJECT, "key": "ORC", "name": "Orchester 2" } }),
    ),
    delta("resource.published", json!({ "resource": resource() })),
    delta(
      "resource.renamed",
      json!({ "resource": RESOURCE, "name": "Careful review" }),
    ),
    delta("resource.archived", json!({ "resource": RESOURCE, "archived": true })),
    delta("resource.archived", json!({ "resource": RESOURCE, "archived": false })),
    json!({ "kind": "revoked", "data": {} }),
  ]
}

#[test]
fn live_updates_every_kind_reads_and_writes_back_unchanged() {
  let samples = samples();
  for sample in &samples {
    let event: Event = serde_json::from_value(sample.clone()).unwrap_or_else(|error| panic!("{sample}: {error}"));
    assert_eq!(
      json!(event.kind().as_str()),
      sample["kind"],
      "the event's kind is its wire name"
    );
    assert_eq!(&serde_json::to_value(&event).expect("serialize"), sample);
  }
  let covered: Vec<&Value> = samples.iter().map(|sample| &sample["kind"]).collect();
  for kind in EventKind::ALL {
    assert!(
      covered.contains(&&json!(kind.as_str())),
      "{} has a sample",
      kind.as_str()
    );
  }
}

#[test]
fn live_updates_kinds_have_their_wire_names() {
  for kind in EventKind::ALL {
    assert_eq!(EventKind::try_from(kind.as_str()), Ok(kind));
    assert_eq!(serde_json::to_value(kind).expect("serialize"), json!(kind.as_str()));
    assert_eq!(
      serde_json::from_value::<EventKind>(json!(kind.as_str())).expect("deserialize"),
      kind
    );
  }
  let snapshot: Vec<EventKind> = EventKind::ALL.into_iter().filter(|kind| kind.is_snapshot()).collect();
  assert_eq!(
    snapshot,
    [
      EventKind::SnapshotBegin,
      EventKind::SnapshotTasks,
      EventKind::SnapshotEnd
    ]
  );
}

#[test]
fn live_updates_removed_and_unknown_kinds_are_refused() {
  for name in [
    "reset",
    "task",
    "tags",
    "modules",
    "members",
    "project",
    "resources",
    "activity",
    "Task.Changed",
  ] {
    assert_eq!(EventKind::try_from(name), Err(UnknownEventKind), "{name}");
    assert!(serde_json::from_value::<EventKind>(json!(name)).is_err(), "{name}");
    assert!(
      serde_json::from_value::<Event>(json!({ "kind": name, "data": {} })).is_err(),
      "{name}"
    );
  }
}

#[test]
fn live_updates_a_delta_carries_who_did_it_and_when() {
  let sample = delta(
    "task.claim",
    json!({ "task": TASK, "claim": null, "action": "released" }),
  );
  let Event::TaskClaim(released) = serde_json::from_value(sample).expect("a claim delta") else {
    panic!("a task.claim delta");
  };
  assert_eq!(released.id, 42);
  assert_eq!(released.by.display_name, "Alice");
  assert_eq!(released.actor, ChangeActor::Person);
  assert_eq!(released.at, 1_700);
  assert_eq!(released.change.action, ClaimAction::Released);
  assert!(released.change.claim.is_none());
  for missing in ["id", "by", "actor", "at"] {
    let mut sample = delta("tag.deleted", json!({ "tag": TAG }));
    sample["data"].as_object_mut().expect("data").remove(missing);
    assert!(
      serde_json::from_value::<Event>(sample).is_err(),
      "a delta without {missing}"
    );
  }
}

#[test]
fn live_updates_a_task_change_holds_only_the_fields_that_changed() {
  let sample = delta(
    "task.changed",
    json!({ "task": TASK, "revision": 5, "base": 4, "module": null }),
  );
  let event: Event = serde_json::from_value(sample.clone()).expect("a task change");
  let Event::TaskChanged(change) = &event else {
    panic!("a task.changed delta");
  };
  assert_eq!(
    change.change.module,
    Some(None),
    "a null module takes the task out of its module"
  );
  assert_eq!(change.change.title, None);
  assert!(change.change.tags_added.is_empty());
  assert_eq!(serde_json::to_value(&event).expect("serialize"), sample);
}
