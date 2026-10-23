//! *Share task* after an answer that was lost (N3 re-review), against a real `orchester-server` on loopback: Bohdan
//! joined Alice's ORC with *Notes for the migration* kept *Not shared*; an earlier *Share task* reached the server and
//! its answer never came back. Sharing again meets `409 id_taken`: the server's copy is cached and the task is shared
//! here too, uploaded once. A task whose id the server holds in another project is refused and stays *Not shared*.
//!
//! `cargo test -p orchester-workspace --test joined_share_task joined_`

#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use orchester_api::{
  SharedLimits, TasksImport, UploadTask,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, TaskId},
  store::TaskStore,
  task::organization::{Priority, WorkflowStatus},
};
use server_support::TestServer;
use support::{Person, member, shared};

/// Bohdan's Kontur joined to Alice's ORC with its first task uploaded and its second kept *Not shared*; that task.
fn joined(server: &TestServer) -> (Person, TaskId) {
  let alice = shared(server);
  let code = server.code_for("bohdan", false);
  let bohdan = Person::signed_in(
    server,
    ("bohdan", "Bohdan Koval", &code),
    &["Add repository switcher", "Notes for the migration"],
  );
  member(&alice, "bohdan");
  let review = bohdan
    .block_on(bohdan.servers.join_review_at(&bohdan.entry(), bohdan.root(), "kontur"))
    .expect("review");
  let (first, kept) = (review.tasks[0].task.id(), review.tasks[1].task.id());
  bohdan
    .block_on(bohdan.servers.join(
      &bohdan.entry(),
      bohdan.root(),
      &review,
      (vec![first], IdempotencyKey::generate()),
    ))
    .expect("join");
  (bohdan, kept)
}

/// Uploads `task` of Bohdan's into `project` as an earlier *Share task* did.
fn upload(server: &TestServer, bohdan: &Person, (task, project): (TaskId, ProjectId)) {
  let read = bohdan.db().task(task).expect("read").expect("the task");
  let copy = UploadTask {
    id: read.id(),
    number: None,
    former_key: Some(read.key().to_string()),
    title: read.details().title.as_str().into(),
    brief: String::new(),
    criteria: Vec::new(),
    pipeline: None,
    limits: Some(SharedLimits {
      budget_tokens: None,
      max_duration_ms: None,
    }),
    workflow: WorkflowStatus::Backlog,
    priority: Priority::None,
    tag_names: Vec::new(),
    blocking: Vec::new(),
    blocked_reason: None,
    module: None,
    status: read.status(),
    created_at: read.created_at().unix_millis(),
    claim: false,
    has_run: false,
  };
  let client = Client::new(
    ServerAddress::parse(&server.url()).expect("address"),
    PlainHttp::NotAllowed,
  )
  .expect("client");
  let import = TasksImport {
    idempotency_key: IdempotencyKey::generate(),
    tasks: vec![copy],
    modules: Vec::new(),
  };
  bohdan
    .block_on(client.import_tasks(&server.device_of("bohdan"), project, &import))
    .expect("the earlier share's import");
}

#[test]
fn joined_share_task_after_a_lost_answer_caches_the_server_s_copy() {
  let server = TestServer::start();
  let (bohdan, kept) = joined(&server);
  let project = bohdan.server().link().project;
  upload(&server, &bohdan, (kept, project));
  let key = bohdan
    .block_on(bohdan.server().share_kept(kept, IdempotencyKey::generate()))
    .expect("shared as the server holds it");
  assert!(bohdan.db().not_shared_tasks().expect("kept").is_empty());
  let info = bohdan.db().shared_task_info(kept).expect("info").expect("shared");
  assert!(info.kept_tags.is_none(), "shared here too");
  let held = bohdan
    .tasks()
    .into_iter()
    .find(|task| task.id() == kept)
    .expect("the task, cached");
  assert_eq!(held.key(), &key);
  let uploads = bohdan.tasks().iter().filter(|task| task.id() == kept).count();
  assert_eq!(uploads, 1, "uploaded once");
}

#[test]
fn joined_share_task_of_an_id_held_elsewhere_is_refused() {
  let server = TestServer::start();
  let (bohdan, kept) = joined(&server);
  let atlas = server.create_named_project(("Atlas", "ATL"), "atlas", None);
  server.add_member(&atlas, "bohdan");
  upload(&server, &bohdan, (kept, atlas.id));
  let refused = bohdan.block_on(bohdan.server().share_kept(kept, IdempotencyKey::generate()));
  assert!(refused.is_err(), "{refused:?}");
  assert_eq!(
    bohdan.db().not_shared_tasks().expect("kept").len(),
    1,
    "still Not shared"
  );
}
