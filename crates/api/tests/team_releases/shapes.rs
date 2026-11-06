//! The definitions, the history, the changelog, a task's release, the module's horizon and the refusals.

use orchester_api::{
  Attribution, ChangelogDocument, ChangelogError, ChangelogPatch, KeptEntry, MarkReleased, ModuleFields, ModulePatch,
  ReleaseChange, ReleaseCreate, ReleaseDelete, ReleaseDeleted, ReleaseFields, ReleaseMark, ReleaseMarked, ReleasePatch,
  ReleaseRefusal, SharedChangelog, SharedRelease, SharingError, SharingErrorDetails, TaskChanges, TaskContent, UserRef,
};
use orchester_core::{
  docs::blocks::Body,
  id::{IdempotencyKey, ReleaseId, TaskId, UserId},
  module::Horizon,
  release::ReleaseState,
  run::CommitId,
  sharing::TaskRevision,
  task::organization::ChangeActor,
};
use serde_json::{Value, json};

use super::{refuses_unknown_and_missing, roundtrip};

const COMMIT: &str = "3f2a9c1e0b4d5a6f7e8d9c0b1a2f3e4d5c6b7a89";

fn user() -> UserRef {
  UserRef {
    id: UserId::generate(),
    display_name: "Olha".to_owned(),
  }
}

fn release(id: ReleaseId, by: &UserRef, released: Option<ReleaseMark>) -> SharedRelease {
  SharedRelease {
    id,
    fields: ReleaseFields {
      name: "0.4.0".to_owned(),
      description: "The *Docs* release.".to_owned(),
      planned: Some("2026-11-15".to_owned()),
    },
    released,
    revision: TaskRevision::FIRST,
    created: Attribution { by: by.clone(), at: 1 },
    updated: Attribution { by: by.clone(), at: 2 },
  }
}

fn release_wire(id: ReleaseId, by: &UserRef, released: (&str, Value, Value, Value)) -> Value {
  let attribution = |at| json!({"by": by, "at": at});
  let (state, on, marked_by, commit) = released;
  json!({
    "id": id, "name": "0.4.0", "description": "The *Docs* release.", "planned": "2026-11-15", "state": state,
    "released_on": on, "released_by": marked_by, "released_commit": commit, "revision": 1,
    "created": attribution(1), "updated": attribution(2),
  })
}

#[test]
fn shared_release_round_trips_planned_and_released() {
  let (id, by) = (ReleaseId::generate(), user());
  let planned = release(id, &by, None);
  let wire = release_wire(id, &by, ("planned", Value::Null, Value::Null, Value::Null));
  roundtrip(&planned, &wire);
  assert_eq!(planned.state(), ReleaseState::Planned);
  for field in [
    "planned",
    "released_on",
    "released_by",
    "released_commit",
    "description",
  ] {
    refuses_unknown_and_missing::<SharedRelease>(&wire, field);
  }

  let mark = ReleaseMark {
    on: "2026-11-14".to_owned(),
    by: by.clone(),
    commit: Some(CommitId::new(COMMIT).expect("a commit")),
  };
  let released = release(id, &by, Some(mark));
  let wire = release_wire(id, &by, ("released", json!("2026-11-14"), json!(by), json!(COMMIT)));
  roundtrip(&released, &wire);
  assert_eq!(released.state(), ReleaseState::Released);
}

#[test]
fn shared_release_refuses_a_state_its_released_fields_contradict() {
  let (id, by) = (ReleaseId::generate(), user());
  for (state, on, marked_by, commit) in [
    ("released", Value::Null, Value::Null, Value::Null),
    ("released", json!("2026-11-14"), Value::Null, Value::Null),
    ("planned", Value::Null, Value::Null, json!(COMMIT)),
    ("planned", json!("2026-11-14"), json!(by), Value::Null),
    ("shipped", Value::Null, Value::Null, Value::Null),
  ] {
    let wire = release_wire(id, &by, (state, on, marked_by, commit));
    assert!(serde_json::from_value::<SharedRelease>(wire).is_err(), "{state}");
  }
  let short = release_wire(id, &by, ("released", json!("2026-11-14"), json!(by), json!("3f2a9c1")));
  assert!(
    serde_json::from_value::<SharedRelease>(short).is_err(),
    "a commit is a full id"
  );
}

#[test]
fn release_fields_are_core_details() {
  let fields: ReleaseFields = serde_json::from_value(json!({"name": " 0.4.0 "})).expect("fields default");
  let details = fields.details().expect("details");
  assert_eq!((details.name().as_str(), details.planned()), ("0.4.0", None));
  assert_eq!(ReleaseFields::from_core(&details).name, "0.4.0");
  for invalid in [json!({"name": "  "}), json!({"name": "0.4.0", "planned": "Nov 15"})] {
    let fields: ReleaseFields = serde_json::from_value(invalid).expect("readable");
    assert!(fields.details().is_err());
  }
  assert!(serde_json::from_value::<ReleaseFields>(json!({"name": "0.4.0", "state": "planned"})).is_err());
}

#[test]
fn release_writes_round_trip_and_refuse_unknown_fields() {
  let (id, key) = (ReleaseId::generate(), IdempotencyKey::generate());
  let fields = json!({"name": "0.5.0", "description": "", "planned": null});
  let create = json!({"idempotency_key": key, "id": id, "release": fields});
  let parsed: ReleaseCreate = serde_json::from_value(create.clone()).expect("a create");
  roundtrip(&parsed, &create);
  refuses_unknown_and_missing::<ReleaseCreate>(&create, "id");

  let patch = json!({"idempotency_key": key, "revision": 3, "release": fields});
  let parsed: ReleasePatch = serde_json::from_value(patch.clone()).expect("a patch");
  roundtrip(&parsed, &patch);
  refuses_unknown_and_missing::<ReleasePatch>(&patch, "revision");

  for commit in [json!(COMMIT), Value::Null] {
    let mark = json!({"idempotency_key": key, "revision": 3, "commit": commit});
    let parsed: MarkReleased = serde_json::from_value(mark.clone()).expect("a mark");
    roundtrip(&parsed, &mark);
    refuses_unknown_and_missing::<MarkReleased>(&mark, "commit");
  }

  let delete = json!({"idempotency_key": key, "revision": 3});
  let parsed: ReleaseDelete = serde_json::from_value(delete.clone()).expect("a delete");
  roundtrip(&parsed, &delete);
  refuses_unknown_and_missing::<ReleaseDelete>(&delete, "revision");
}

#[test]
fn release_moves_round_trip_as_history_and_answers() {
  let (id, next, by, task) = (ReleaseId::generate(), ReleaseId::generate(), user(), TaskId::generate());
  let change = ReleaseChange {
    sequence: 7,
    task,
    from: Some(id),
    to: Some(next),
    by: by.clone(),
    actor: ChangeActor::Orchester,
    at: 5,
  };
  let wire = json!({"sequence": 7, "task": task, "from": id, "to": next, "by": by, "actor": "orchester", "at": 5});
  roundtrip(&change, &wire);
  for field in ["from", "to", "actor"] {
    refuses_unknown_and_missing::<ReleaseChange>(&wire, field);
  }

  let marked = ReleaseMarked {
    release: release(id, &by, None),
    moves: vec![change.clone()],
  };
  let marked_wire = json!({"release": release_wire(id, &by, ("planned", Value::Null, Value::Null, Value::Null)),
    "moves": [wire]});
  roundtrip(&marked, &marked_wire);
  refuses_unknown_and_missing::<ReleaseMarked>(&marked_wire, "moves");

  let out = ReleaseChange {
    to: None,
    actor: ChangeActor::Person,
    ..change
  };
  let deleted = ReleaseDeleted { moves: vec![out] };
  let deleted_wire = serde_json::to_value(&deleted).expect("serialize");
  assert_eq!(deleted_wire["moves"][0]["to"], Value::Null);
  roundtrip(&deleted, &deleted_wire);
}

#[test]
fn release_changelog_document_keeps_block_ids() {
  let body = Body::parse("## Docs\n\n- Card form ORC-12\n- Search ORC-14\n");
  let document = ChangelogDocument::of_body(&body).expect("a document");
  assert_eq!(document.blocks(), [0, 1, 2]);
  assert_eq!(Body::parse(document.markdown()), body);
  assert_eq!(
    ChangelogDocument::new("## Docs\n".to_owned(), vec![4, 9]),
    Err(ChangelogError::BlockCount { blocks: 1, ids: 2 })
  );
  assert_eq!(
    ChangelogDocument::new("## Docs\n\nIntro\n".to_owned(), vec![4, 4]),
    Err(ChangelogError::DuplicateBlock(4))
  );
  assert!(serde_json::from_value::<ChangelogDocument>(json!({"markdown": "Intro\n", "blocks": []})).is_err());

  let (release, task, key) = (ReleaseId::generate(), TaskId::generate(), IdempotencyKey::generate());
  let changelog = SharedChangelog {
    release,
    revision: TaskRevision::FIRST,
    document: document.clone(),
    hidden: vec![task],
    kept_aside: vec![KeptEntry {
      task,
      markdown: "Card form, **faster**".to_owned(),
    }],
  };
  let document_wire = json!({"markdown": document.markdown(), "blocks": [0, 1, 2]});
  let kept = json!([{"task": task, "markdown": "Card form, **faster**"}]);
  let wire = json!({"release": release, "revision": 1, "document": document_wire, "hidden": [task],
    "kept_aside": kept});
  roundtrip(&changelog, &wire);
  refuses_unknown_and_missing::<SharedChangelog>(&wire, "kept_aside");

  let patch = json!({"idempotency_key": key, "revision": 1, "document": document_wire, "hidden": [],
    "kept_aside": []});
  let parsed: ChangelogPatch = serde_json::from_value(patch.clone()).expect("a patch");
  roundtrip(&parsed, &patch);
  refuses_unknown_and_missing::<ChangelogPatch>(&patch, "hidden");
}

#[test]
fn task_release_is_in_its_content_and_changes() {
  let release = ReleaseId::generate();
  let content: TaskContent =
    serde_json::from_value(json!({"title": "Card form", "release": release})).expect("content");
  assert_eq!(content.release, Some(release));
  let without: TaskContent = serde_json::from_value(json!({"title": "Card form"})).expect("content");
  assert_eq!(without.release, None);

  let into: TaskChanges = serde_json::from_value(json!({"release": release})).expect("changes");
  assert_eq!(into.release, Some(Some(release)));
  let out: TaskChanges = serde_json::from_value(json!({"release": null})).expect("changes");
  assert_eq!(out.release, Some(None));
  let untouched: TaskChanges = serde_json::from_value(json!({"title": "Card form"})).expect("changes");
  assert_eq!(untouched.release, None);
  assert!(
    serde_json::to_value(&untouched)
      .expect("serialize")
      .get("release")
      .is_none()
  );
  assert_eq!(serde_json::to_value(&out).expect("serialize")["release"], Value::Null);
}

#[test]
fn module_horizon_and_place_default_to_later_for_a_new_release_roadmap() {
  let fields: ModuleFields = serde_json::from_value(json!({"name": "Docs"})).expect("fields");
  assert_eq!((fields.horizon, fields.place), (Horizon::Later, 0));
  let placed: ModuleFields =
    serde_json::from_value(json!({"name": "Docs", "horizon": "now", "place": 3})).expect("fields");
  assert_eq!((placed.horizon, placed.place), (Horizon::Now, 3));
  assert_eq!(
    ModuleFields::from_core(&placed.details().expect("details")).horizon,
    Horizon::Later
  );
  assert!(serde_json::from_value::<ModuleFields>(json!({"name": "Docs", "horizon": "soon"})).is_err());
  let patch = json!({"idempotency_key": IdempotencyKey::generate(), "revision": 2,
    "module": {"name": "Docs", "description": "", "start": null, "target": null, "state": "open", "horizon": "next",
      "place": 1}});
  let parsed: ModulePatch = serde_json::from_value(patch.clone()).expect("a module patch");
  assert_eq!(parsed.module.horizon, Horizon::Next);
  assert_eq!(serde_json::to_value(&parsed).expect("serialize"), patch);
}

#[test]
fn release_refusals_read_as_sharing_errors() {
  let (id, by) = (ReleaseId::generate(), user());
  let current = release_wire(id, &by, ("planned", Value::Null, Value::Null, Value::Null));
  let conflict: SharingError =
    serde_json::from_value(json!({"error": "revision_conflict", "message": "changed meanwhile", "current": current}))
      .expect("a conflict");
  assert_eq!(
    conflict.details,
    SharingErrorDetails::ReleaseRevision {
      current: Box::new(release(id, &by, None))
    }
  );
  let exists: SharingError =
    serde_json::from_value(json!({"error": "release_exists", "message": "taken", "release": current}))
      .expect("release_exists");
  assert_eq!(
    serde_json::to_value(exists.error.error).expect("code"),
    json!("release_exists")
  );
  assert!(matches!(exists.details, SharingErrorDetails::Release { .. }));
  for reason in [ReleaseRefusal::Released, ReleaseRefusal::ReleaseNotFound] {
    let refused: SharingError =
      serde_json::from_value(json!({"error": "release", "message": "refused", "reason": reason.as_str()}))
        .expect("release");
    assert_eq!(
      serde_json::to_value(refused.error.error).expect("code"),
      json!("release")
    );
    assert_eq!(serde_json::to_value(reason).expect("reason"), json!(reason.as_str()));
  }
}
