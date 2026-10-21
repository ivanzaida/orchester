//! `create_task` and `update_task` organize a task as the task form does (ORC-58): the manual Blocked with its
//! reason, the tasks that block it and its tags are set, cleared and kept as asked, refused with the form's reasons,
//! and recorded in the task's history as automation's, from a client of the server and from the Orchestrator chat;
//! `list_tasks` tells what is blocked, and the chat's `run_task` refuses a task marked Blocked.

use std::path::Path;

use orchester_core::{
  id::TaskId,
  store::TaskStore,
  task::{
    Task,
    organization::{ChangeActor, DependencyGate},
  },
};
use orchester_store::ProjectDb;
use orchester_workspace::app_mcp::ToolError;
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{chat, chat_call},
};

/// Why ORC-57 waits, as the Orchestrator chat was asked to mark it.
const LURQ_RELEASE: &str =
  "Waits for a new lurq release with clipboard images, a paste event, file drag and drop and runtime images";

/// Kontur seeded empty, its Codex connection's program any file the seed records as checked; and its repository.
fn served() -> (Served, std::path::PathBuf) {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  let (served, seeded) = Served::seeded_with("empty", Some(codex));
  (served, seeded.repo)
}

/// A draft titled `title` that could run: its key.
fn draft(served: &mut Served, title: &str) -> String {
  let created = served.ok(
    "create_task",
    &json!({
      "project": "Kontur", "title": title, "brief": "Do it.", "criteria": ["It is done."], "pipeline": "Quick fix",
    }),
  );
  created["task"]["key"].as_str().expect("a key").to_owned()
}

/// The task of `repo` with `key`, as stored.
fn stored(repo: &Path, key: &str) -> Task {
  ProjectDb::open(repo)
    .expect("the project's data")
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .unwrap_or_else(|| panic!("task {key}"))
}

/// The names of the tags of `task`, in the order of the project's tags.
fn tag_names(repo: &Path, task: &Task) -> Vec<String> {
  ProjectDb::open(repo)
    .expect("the project's data")
    .project_tags()
    .expect("tags")
    .into_iter()
    .filter(|tag| task.organization().tags().contains(&tag.id()))
    .map(|tag| tag.name().as_str().to_owned())
    .collect()
}

/// Who changed the organization of `task` last.
fn last_actor(repo: &Path, task: &Task) -> ChangeActor {
  ProjectDb::open(repo)
    .expect("the project's data")
    .task_organization_history(task.id())
    .expect("history")
    .last()
    .expect("a change")
    .actor
}

/// `update_task`'s arguments for task `key` of Kontur, with `fields`.
fn update(key: &str, fields: Value) -> Value {
  let Value::Object(fields) = fields else {
    panic!("fields are an object")
  };
  let mut args = json!({ "project": "Kontur", "key": key });
  args.as_object_mut().expect("an object").extend(fields);
  args
}

#[test]
fn task_organization_create_task_blocks_links_and_tags_as_automation() {
  let (mut served, repo) = served();
  let blocker = draft(&mut served, "Ship the lurq release");
  served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Tag it first", "tags": ["Backend"] }),
  );
  let created = served.ok(
    "create_task",
    &json!({
      "project": "Kontur",
      "title": "Paste images into the composer",
      "blocked_reason": " Waits for the release ",
      "blocked_by": [{ "task": blocker.to_lowercase() }],
      "tags": ["backend", "Composer"],
    }),
  );
  assert_eq!(created["task"]["blocked_reason"], "Waits for the release", "{created}");
  let key = created["task"]["key"].as_str().expect("a key");
  let task = stored(&repo, key);
  let organization = task.organization();
  assert_eq!(organization.blocked_reason(), Some("Waits for the release"));
  let links = organization.dependencies();
  assert_eq!(links.len(), 1);
  assert_eq!(links[0].task, stored(&repo, &blocker).id());
  assert_eq!(links[0].gate, DependencyGate::Merge, "the form's gate when left out");
  let mut tags = tag_names(&repo, &task);
  tags.sort();
  assert_eq!(
    tags,
    ["Backend", "Composer"],
    "an existing tag ignoring case, a new one created"
  );
  assert_eq!(last_actor(&repo, &task), ChangeActor::Orchester);
}

#[test]
fn task_organization_update_task_sets_keeps_and_clears_each_field() {
  let (mut served, repo) = served();
  let blocker = draft(&mut served, "Ship the lurq release");
  let key = draft(&mut served, "Paste images into the composer");
  let blocker_id = stored(&repo, &blocker).id();
  served.ok(
    "update_task",
    &update(
      &key,
      json!({
        "blocked_reason": "Waits for the release",
        "blocked_by": [{ "task": blocker_id.to_string(), "gate": "merge" }],
        "tags": ["Composer"],
      }),
    ),
  );
  let set = stored(&repo, &key);
  assert_eq!(set.organization().blocked_reason(), Some("Waits for the release"));
  assert_eq!(set.organization().dependencies()[0].task, blocker_id);
  assert_eq!(tag_names(&repo, &set), ["Composer"]);
  assert_eq!(last_actor(&repo, &set), ChangeActor::Orchester);

  served.ok(
    "update_task",
    &update(&key, json!({ "title": "Paste and drop images" })),
  );
  let kept = stored(&repo, &key);
  assert_eq!(kept.details().title.as_str(), "Paste and drop images");
  assert_eq!(kept.organization(), set.organization(), "left out, each stays as it is");

  served.ok("update_task", &update(&key, json!({ "blocked_reason": null })));
  let unblocked = stored(&repo, &key);
  assert_eq!(unblocked.organization().blocked_reason(), None);
  assert_eq!(unblocked.organization().dependencies().len(), 1, "the links stay");
  served.ok("update_task", &update(&key, json!({ "blocked_by": [] })));
  assert!(stored(&repo, &key).organization().dependencies().is_empty());
  served.ok("update_task", &update(&key, json!({ "tags": [] })));
  let cleared = stored(&repo, &key);
  assert!(cleared.organization().tags().is_empty());
  assert_eq!(last_actor(&repo, &cleared), ChangeActor::Orchester);
}

#[test]
fn task_organization_refuses_what_the_form_refuses() {
  let (mut served, repo) = served();
  let first = draft(&mut served, "First");
  let second = draft(&mut served, "Second");
  let refused = |served: &mut Served, fields: Value| served.refused("update_task", &update(&first, fields));

  let blank = refused(&mut served, json!({ "blocked_reason": "   " }));
  assert_eq!(blank["code"], "invalid_organization", "{blank}");
  assert!(
    blank["message"]
      .as_str()
      .is_some_and(|text| text.contains("Enter a reason")),
    "{blank}"
  );
  for unknown in [
    "KON-999".to_owned(),
    "ORC-57".to_owned(),
    TaskId::generate().to_string(),
  ] {
    let missing = refused(&mut served, json!({ "blocked_by": [{ "task": unknown }] }));
    assert_eq!(missing["code"], "task_not_found", "{missing}");
  }
  let unknown_gate = refused(
    &mut served,
    json!({ "blocked_by": [{ "task": second, "gate": "review" }] }),
  );
  assert_eq!(
    unknown_gate["code"], "invalid_arguments",
    "only the form's gates: {unknown_gate}"
  );
  let blank_tag = refused(&mut served, json!({ "tags": ["  "] }));
  assert_eq!(blank_tag["code"], "invalid_tag", "{blank_tag}");

  let itself = refused(&mut served, json!({ "blocked_by": [{ "task": first }] }));
  assert_eq!(itself["code"], "invalid_organization", "{itself}");
  served.ok(
    "update_task",
    &update(&second, json!({ "blocked_by": [{ "task": first }] })),
  );
  let cycle = refused(&mut served, json!({ "blocked_by": [{ "task": second }] }));
  assert_eq!(cycle["code"], "invalid_organization", "{cycle}");
  assert!(
    cycle["message"]
      .as_str()
      .is_some_and(|text| text.contains("dependency cycle")),
    "{cycle}"
  );
  let untouched = stored(&repo, &first);
  assert!(untouched.organization().blocked_reason().is_none());
  assert!(untouched.organization().dependencies().is_empty());
  assert!(untouched.organization().tags().is_empty(), "nothing saved");
}

#[test]
fn task_organization_list_tasks_tells_what_is_blocked() {
  let (mut served, _repo) = served();
  let blocker = draft(&mut served, "Ship the lurq release");
  let marked = draft(&mut served, "Marked Blocked");
  let held = draft(&mut served, "Held by a task");
  served.ok("update_task", &update(&marked, json!({ "blocked_reason": "Waits" })));
  served.ok(
    "update_task",
    &update(&held, json!({ "blocked_by": [{ "task": blocker }] })),
  );
  let listed = served.ok("list_tasks", &json!({ "project": "Kontur" }));
  let item = |key: &str| {
    listed["tasks"]
      .as_array()
      .expect("tasks")
      .iter()
      .find(|task| task["key"] == key)
      .unwrap_or_else(|| panic!("{key}: {listed}"))
      .clone()
  };
  assert_eq!(item(&marked)["blocked_reason"], "Waits");
  assert_eq!(item(&marked)["held_by_blocking_tasks"], false);
  assert_eq!(item(&held)["blocked_reason"], Value::Null);
  assert_eq!(item(&held)["held_by_blocking_tasks"], true);
  assert_eq!(item(&blocker)["held_by_blocking_tasks"], false);
}

#[test]
fn task_organization_chat_marks_a_task_blocked_and_never_starts_it() {
  let (mut served, repo) = served();
  let key = draft(&mut served, "Paste images and drop files into the composer");
  let mut db = ProjectDb::open(&repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  chat_call(
    &mut served,
    &scope,
    "update_task",
    update(
      &key,
      json!({ "blocked_reason": LURQ_RELEASE, "workflow_status": "backlog" }),
    ),
  )
  .expect("a free organization change");
  let task = stored(&repo, &key);
  assert_eq!(task.organization().blocked_reason(), Some(LURQ_RELEASE));
  assert_eq!(
    last_actor(&repo, &task),
    ChangeActor::Orchester,
    "the chat's change is automation's"
  );
  let read = served.ok("get_task", &json!({ "project": "Kontur", "key": key }));
  assert_eq!(read["blocked_reason"], LURQ_RELEASE);

  let start = json!({ "project": "Kontur", "key": key });
  let refused = chat_call(&mut served, &scope, "run_task", start.clone()).expect_err("a task marked Blocked");
  let ToolError::Refused { code, reasons } = &refused else {
    panic!("refused: {refused:?}")
  };
  assert_eq!(code, "task_blocked");
  assert_eq!(reasons[0].key, "orchestrator.start.markedBlocked");
  assert!(
    reasons[0].args.contains(&("reason", LURQ_RELEASE.to_owned())),
    "names the reason: {reasons:?}"
  );

  served.ok("update_task", &update(&key, json!({ "blocked_reason": null })));
  let proposed = chat_call(&mut served, &scope, "run_task", start).expect("proposed once it is not marked");
  assert_eq!(proposed["waits_for_person"], true, "{proposed}");
}
