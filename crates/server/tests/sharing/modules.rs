//! Modules on the server (`docs/architecture/server.md`, "Modules"): made and changed by members under a revision with
//! core's rules, answered once per idempotency key, a task's module changed under the task's revision with every move
//! kept, uploads that keep or map modules by name, and the `modules` event.

use orchester_api::{
  ErrorCode, Joined, ModuleResponse, Modules, SharedModule, SharingErrorDetails, TaskResponse, TasksCreated,
};
use orchester_core::{
  id::{ModuleId, ProjectId, TaskId},
  module::ModuleState,
  task::organization::ChangeActor,
};
use serde_json::{Value, json};

use crate::{
  events::Stream,
  fixture::*,
  support::{Response, TestServer},
};

fn modules_path(project: ProjectId) -> String {
  format!("{}/modules", path(project))
}

async fn create(server: &TestServer, token: &orchester_api::Credential, project: ProjectId, body: Value) -> Response {
  server
    .request("POST", &modules_path(project), Some(token), Some(body))
    .await
}

fn module(response: &Response, status: u16) -> SharedModule {
  assert_eq!(response.status, status, "{}", response.text());
  response.json::<ModuleResponse>().module
}

fn new_module(name: &str) -> Value {
  json!({"idempotency_key":key(),"id":ModuleId::generate(),"module":{"name":name,"description":"  Ship it  ","start":"2026-10-01","target":"2026-10-31"}})
}

async fn listed(server: &TestServer, token: &orchester_api::Credential, project: ProjectId, after: u64) -> Modules {
  let response = server
    .request(
      "GET",
      &format!("{}?after={after}", modules_path(project)),
      Some(token),
      None,
    )
    .await;
  assert_eq!(response.status, 200, "{}", response.text());
  response.json()
}

#[tokio::test]
async fn modules_are_made_once_per_key_by_core_rules_with_unique_names() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let body = new_module("Checkout");
  let made = module(&create(&server, &bob.token, project, body.clone()).await, 201);
  assert_eq!(made.fields.description, "Ship it");
  assert_eq!(made.fields.state, ModuleState::Open);
  assert_eq!(made.created.by.id, bob.user.id);
  assert_eq!(made.revision.get(), 1);
  assert_eq!(module(&create(&server, &bob.token, project, body).await, 201), made);
  let duplicate = create(&server, &owner.token, project, new_module("checkout")).await;
  error(&duplicate, 409, ErrorCode::ModuleExists);
  let SharingErrorDetails::Module { module: existing } = duplicate.json::<orchester_api::SharingError>().details else {
    panic!("module_exists names the module");
  };
  assert_eq!(existing.id, made.id);
  let backwards = json!({"idempotency_key":key(),"id":ModuleId::generate(),"module":{"name":"Late","start":"2026-11-02","target":"2026-11-01"}});
  let refused = create(&server, &owner.token, project, backwards).await;
  error(&refused, 422, ErrorCode::Module);
  assert_eq!(refused.json::<Value>()["reason"], "target_before_start");
  let malformed =
    json!({"idempotency_key":key(),"id":ModuleId::generate(),"module":{"name":"Late","start":"next week"}});
  error(
    &create(&server, &owner.token, project, malformed).await,
    422,
    ErrorCode::Invalid,
  );
  let mut taken = new_module("Billing");
  taken["id"] = json!(made.id);
  error(
    &create(&server, &owner.token, project, taken).await,
    409,
    ErrorCode::IdTaken,
  );
  let stranger = server.invited("carol", false).await;
  error(
    &create(&server, &stranger.token, project, new_module("Mine")).await,
    404,
    ErrorCode::NotFound,
  );
  assert_eq!(listed(&server, &owner.token, project, 0).await.modules, [made]);
  server.finish().await;
}

#[tokio::test]
async fn modules_change_under_their_revision() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let made = module(
    &create(&server, &owner.token, project, new_module("Checkout")).await,
    201,
  );
  module(
    &create(&server, &owner.token, project, new_module("Billing")).await,
    201,
  );
  let patch = |revision: u64, fields: Value| json!({"idempotency_key":key(),"revision":revision,"module":fields});
  let path = format!("{}/{}", modules_path(project), made.id);
  let closing = patch(1, json!({"name":"Checkout v2","target":"2026-12-01","state":"closed"}));
  let closed = module(
    &server
      .request("PATCH", &path, Some(&owner.token), Some(closing.clone()))
      .await,
    200,
  );
  assert_eq!(closed.revision.get(), 2);
  assert_eq!(closed.fields.state, ModuleState::Closed);
  assert_eq!(closed.fields.start, None);
  assert_eq!(
    module(
      &server.request("PATCH", &path, Some(&owner.token), Some(closing)).await,
      200
    ),
    closed,
    "a repeated key gets the first answer"
  );
  let stale = server
    .request(
      "PATCH",
      &path,
      Some(&owner.token),
      Some(patch(1, json!({"name":"Stale"}))),
    )
    .await;
  error(&stale, 409, ErrorCode::RevisionConflict);
  let SharingErrorDetails::ModuleRevision { current } = stale.json::<orchester_api::SharingError>().details else {
    panic!("a module's conflict carries the module");
  };
  assert_eq!(*current, closed);
  let renamed = server
    .request(
      "PATCH",
      &path,
      Some(&owner.token),
      Some(patch(2, json!({"name":"BILLING"}))),
    )
    .await;
  error(&renamed, 409, ErrorCode::ModuleExists);
  let unchanged = patch(2, serde_json::to_value(&closed.fields).expect("module fields"));
  let same = module(
    &server
      .request("PATCH", &path, Some(&owner.token), Some(unchanged))
      .await,
    200,
  );
  assert_eq!(same.revision.get(), 2, "the same fields change nothing");
  let missing = format!("{}/{}", modules_path(project), ModuleId::generate());
  error(
    &server
      .request(
        "PATCH",
        &missing,
        Some(&owner.token),
        Some(patch(1, json!({"name":"Gone"}))),
      )
      .await,
    404,
    ErrorCode::NotFound,
  );
  server.finish().await;
}

#[tokio::test]
async fn modules_task_membership_changes_under_the_task_revision_and_is_kept() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  let project = created.project.id;
  let checkout = module(
    &create(&server, &owner.token, project, new_module("Checkout")).await,
    201,
  );
  let billing = module(
    &create(&server, &owner.token, project, new_module("Billing")).await,
    201,
  );
  let mut stream = Stream::open(&server, &owner.token, project, created.cursor).await;
  for name in ["Checkout", "Billing"] {
    let event = stream.next().await;
    assert_eq!(event.kind.as_deref(), Some("modules"));
    assert!(event.data.contains(name));
  }
  let mut content = content("In a module");
  content["module"] = json!(checkout.id);
  let response = server
    .request(
      "POST",
      &format!("{}/tasks", path(project)),
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"tasks":[content]})),
    )
    .await;
  assert_eq!(response.status, 201);
  let task = response.json::<TasksCreated>().tasks.remove(0);
  assert_eq!(task.content.module, Some(checkout.id));
  let moved = changed(
    &patch(
      &server,
      &owner.token,
      project,
      &task,
      json!({"module":billing.id,"actor":"orchester"}),
    )
    .await,
  );
  assert_eq!(moved.content.module, Some(billing.id));
  assert_eq!(moved.revision.get(), task.revision.get() + 1);
  let event = stream.next().await;
  assert_eq!(event.kind.as_deref(), Some("task"));
  let event = stream.next().await;
  assert_eq!(event.kind.as_deref(), Some("task"));
  let snapshot: orchester_api::SharedTask = serde_json::from_str(&event.data).expect("task snapshot");
  assert_eq!(snapshot.content.module, Some(billing.id));
  error(
    &patch(&server, &owner.token, project, &task, json!({"module":null})).await,
    409,
    ErrorCode::RevisionConflict,
  );
  let out = changed(&patch(&server, &owner.token, project, &moved, json!({"module":null})).await);
  assert_eq!(out.content.module, None);
  let other = self::project(&server, &owner.token, vec![]).await.project.id;
  let foreign = module(
    &create(&server, &owner.token, other, new_module("Elsewhere")).await,
    201,
  );
  let refused = patch(&server, &owner.token, project, &out, json!({"module":foreign.id})).await;
  error(&refused, 422, ErrorCode::Module);
  assert_eq!(refused.json::<Value>()["reason"], "module_not_found");
  let expected = [
    (task.id, None, Some(checkout.id), ChangeActor::Person),
    (task.id, Some(checkout.id), Some(billing.id), ChangeActor::Orchester),
    (task.id, Some(billing.id), None, ChangeActor::Person),
  ];
  assert_moves_kept(&server, &owner, project, &expected).await;
  server.finish().await;
}

type Move = (TaskId, Option<ModuleId>, Option<ModuleId>, ChangeActor);

/// The project's moves are `expected`, each by `owner`, listed after a sequence and append-only.
async fn assert_moves_kept(server: &TestServer, owner: &Joined, project: ProjectId, expected: &[Move]) {
  let history = listed(server, &owner.token, project, 0).await.changes;
  let moves: Vec<Move> = history
    .iter()
    .map(|change| (change.task, change.from, change.to, change.actor))
    .collect();
  assert_eq!(moves, expected);
  assert!(history.iter().all(|change| change.by.id == owner.user.id));
  let later = listed(server, &owner.token, project, history[0].sequence).await.changes;
  assert_eq!(later, history[1..]);
  assert!(
    server.database().execute("DELETE FROM module_changes", []).is_err(),
    "the moves are append-only"
  );
}

fn upload_module(id: ModuleId, name: &str) -> Value {
  json!({"id":id,"module":{"name":name,"state":"open"},"created_at":77})
}

#[tokio::test]
async fn modules_uploads_keep_their_ids_or_join_the_module_of_their_name() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let checkout = module(
    &create(&server, &owner.token, project, new_module("Checkout")).await,
    201,
  );
  let (mine, billing) = (ModuleId::generate(), ModuleId::generate());
  let (first, second) = (upload(1), upload(2));
  let mut tasks = vec![first, second];
  tasks[0]["module"] = json!(mine);
  tasks[1]["module"] = json!(billing);
  let ids: Vec<TaskId> = tasks
    .iter()
    .map(|task| serde_json::from_value(task["id"].clone()).expect("task id"))
    .collect();
  let response = server
    .request(
      "POST",
      &format!("{}/tasks/import", path(project)),
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"tasks":tasks,"modules":[upload_module(mine,"CHECKOUT"),upload_module(billing,"Billing")]})),
    )
    .await;
  assert_eq!(response.status, 201, "{}", response.text());
  let created = response.json::<TasksCreated>();
  assert_eq!(created.mapped.len(), 1);
  assert_eq!((created.mapped[0].from, created.mapped[0].to), (mine, checkout.id));
  let names: Vec<_> = created
    .modules
    .iter()
    .map(|module| (module.id, module.fields.name.as_str()))
    .collect();
  // By when they were made: an uploaded module keeps its time.
  assert_eq!(names, [(billing, "Billing"), (checkout.id, "Checkout")]);
  assert_eq!(created.modules[0].created.at, 77);
  let modules: Vec<_> = created.tasks.iter().map(|task| task.content.module).collect();
  assert_eq!(modules, [Some(checkout.id), Some(billing)]);
  let history = listed(&server, &owner.token, project, 0).await.changes;
  let moves: Vec<_> = history
    .iter()
    .map(|change| (change.task, change.from, change.to))
    .collect();
  assert_eq!(
    moves,
    [(ids[0], None, Some(checkout.id)), (ids[1], None, Some(billing))]
  );
  let mut unknown = upload(3);
  unknown["module"] = json!(ModuleId::generate());
  let refused = server
    .request(
      "POST",
      &format!("{}/tasks/import", path(project)),
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"tasks":[unknown]})),
    )
    .await;
  error(&refused, 422, ErrorCode::Module);
  let task = server
    .request(
      "GET",
      &format!("{}/tasks/{}", path(project), ids[1]),
      Some(&owner.token),
      None,
    )
    .await
    .json::<TaskResponse>()
    .task;
  assert_eq!(task.content.module, Some(billing));
  server.finish().await;
}

#[tokio::test]
async fn modules_migration_keeps_event_ids_growing() {
  let server = TestServer::start(|config| {
    let db = rusqlite::Connection::open(config.data_dir.join("orchester-server.db")).expect("old database");
    // The old events name a project that is gone, as the pruned rows of a real database may.
    db.pragma_update(None, "foreign_keys", false).expect("foreign keys off");
    db.execute_batch(include_str!("../../src/db/schema.sql"))
      .expect("v1 schema");
    db.execute_batch(include_str!("../../src/db/sharing.sql"))
      .expect("v2 schema");
    for _ in 0..3 {
      db.execute(
        "INSERT INTO events(project,kind,payload,at) VALUES ('gone','task','{}',1)",
        [],
      )
      .expect("an old event");
    }
    db.execute("DELETE FROM events", []).expect("pruned");
  })
  .await;
  let counter: i64 = server
    .database()
    .query_row("SELECT seq FROM sqlite_sequence WHERE name='events'", [], |row| {
      row.get(0)
    })
    .expect("the event counter");
  assert_eq!(counter, 3);
  let owner = server.bootstrap().await;
  let id = project(&server, &owner.token, vec![]).await.project.id;
  module(&create(&server, &owner.token, id, new_module("After")).await, 201);
  let first: i64 = server
    .database()
    .query_row("SELECT MIN(sequence) FROM events WHERE kind='modules'", [], |row| {
      row.get(0)
    })
    .expect("a modules event");
  assert!(first > 3);
  server.finish().await;
}
