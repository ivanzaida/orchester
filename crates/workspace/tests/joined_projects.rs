//! Joining a repository that is shared already (`docs/architecture/server.md`, "Two people share
//! one repository", phase S2c) against a real `orchester-server` on loopback, with two people: Alice
//! shared Kontur as ORC; Bohdan has the same repository with local tasks KON-1 … KON-3. A person
//! who is not a member learns the owners and nothing is uploaded; a Share that meets the project
//! leads to Join; the review ticks every task, hints at similar titles and keeps a task with an
//! unfinished run ticked; the join uploads the ticked tasks with new numbers, ids and former keys
//! (the unfinished one claimed), drops a link to a task that stays here and names it, keeps the
//! others *Not shared*, keeps local runs attached and names the next branch by the new key; *Share
//! task* uploads a kept task later; *Make local* returns the kept tasks with new numbers.
//!
//! `cargo test -p orchester-desktop --test joined_projects joined_`

#[path = "support/mod.rs"]
mod fixture;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  repository::RepositoryIdentity,
  resource::ResourceKind,
  run::BranchName,
  sharing::ClaimKind,
  store::{ResourceStore, RunStore, TaskStore},
  task::{
    TaskDetails,
    organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  },
};
use orchester_workspace::sharing::{
  SharingError,
  project::{JoinReview, ShareError, lookup::Lookup},
};
use server_support::TestServer;
use support::{Other, Person, REMOTE, member, shared};

fn identity() -> RepositoryIdentity {
  RepositoryIdentity::from_remote_url(REMOTE).expect("identity")
}

/// Bohdan's computer: the repository with KON-1 *Add repository switcher*, KON-2 *Fix reconnect
/// after sleep* (a run working on it) and KON-3 *Notes for the migration*; KON-1 is blocked by
/// KON-3. Signed in to `server`; not yet a member of ORC.
fn bohdan(server: &TestServer) -> (Person, [TaskId; 3]) {
  let first = server.create_user("bohdan", "Bohdan Koval", false);
  let mut kontur = fixture::Fixture::keyed("KON");
  let now = orchester_core::time::Timestamp::now();
  let tasks = [
    kontur.task("Add repository switcher", now),
    kontur.task("Fix reconnect after sleep", now),
    kontur.task("Notes for the migration", now),
  ];
  // A task with a run stays complete: on the published pipeline Delivery, with its brief and criteria.
  let delivery = kontur
    .db
    .resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .remove(0)
    .id();
  let read = kontur.db.task(tasks[1]).expect("read").expect("task");
  let complete = TaskDetails {
    brief: "Reconnect after the computer sleeps.".into(),
    acceptance_criteria: vec!["Waking reconnects within 10 s.".into()],
    pipeline: Some(delivery),
    ..read.details().clone()
  };
  kontur
    .db
    .update_task(tasks[1], read.revision(), complete)
    .expect("complete the task")
    .saved()
    .expect("saved as read");
  kontur.started_run(tasks[1], now);
  let blocked = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::High,
    None,
    vec![BlockingTask {
      task: tasks[2],
      gate: DependencyGate::Merge,
    }],
    Vec::new(),
  )
  .expect("organization");
  kontur
    .db
    .set_task_organization(tasks[0], &blocked, ChangeActor::Person, now)
    .expect("organize");
  let person = Person::with_repo(server, ("bohdan", &first), kontur.into_repo());
  (person, tasks)
}

/// ORC's tasks as Oleh, another member, reads them from the server.
fn on_server(alice: &Person, oleh: &Other) -> Vec<orchester_api::SharedTask> {
  let project = alice.server().link().project;
  alice
    .block_on(oleh.client.tasks(&oleh.token, project))
    .expect("oleh reads ORC")
    .tasks
}

/// Alice's ORC with Oleh as a member, to read it.
fn with_oleh(server: &TestServer) -> (Person, Other) {
  let alice = shared(server);
  let oleh = Other::new(server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  (alice, oleh)
}

fn review(bohdan: &Person) -> JoinReview {
  bohdan
    .block_on(bohdan.servers.join_review(&bohdan.entry(), bohdan.root(), identity()))
    .expect("review")
}

/// Bohdan joins ORC with KON-1 ticked (and KON-2, whose run works); KON-3 stays here.
fn join(bohdan: &Person, tasks: &[TaskId; 3]) -> orchester_workspace::sharing::project::Joined {
  let review = review(bohdan);
  bohdan
    .block_on(bohdan.servers.join(
      &bohdan.entry(),
      bohdan.root(),
      &review,
      (vec![tasks[0]], IdempotencyKey::generate()),
    ))
    .expect("join")
}

#[test]
fn joined_a_person_who_is_not_a_member_learns_the_owners_and_uploads_nothing() {
  let server = TestServer::start();
  let (alice, oleh) = with_oleh(&server);
  let (bohdan, _) = bohdan(&server);
  let lookup = bohdan
    .block_on(bohdan.servers.lookup(&bohdan.entry(), identity()))
    .expect("lookup");
  assert_eq!(
    lookup,
    Lookup::NotMember {
      owners: vec!["Alice Moreau".into()]
    }
  );
  // Share meets the project (`409 remote_taken`): nothing of Bohdan's goes up, Join comes next.
  let shared = bohdan.block_on(bohdan.servers.share(
    &bohdan.entry(),
    bohdan.root(),
    identity(),
    IdempotencyKey::generate(),
    false,
  ));
  assert!(matches!(shared, Err(ShareError::Taken { member: false, ref owners, .. }) if owners.len() == 1));
  assert_eq!(bohdan.db().sharing().expect("link"), None);
  let review = bohdan.block_on(bohdan.servers.join_review(&bohdan.entry(), bohdan.root(), identity()));
  assert!(
    matches!(review, Err(SharingError::Client(error)) if error.code() == Some(orchester_api::ErrorCode::NotMember))
  );
  member(&alice, "bohdan");
  let lookup = bohdan
    .block_on(bohdan.servers.lookup(&bohdan.entry(), identity()))
    .expect("check again");
  assert_eq!(lookup, Lookup::Shared { key: "ORC".into() });
  assert_eq!(on_server(&alice, &oleh).len(), 3, "nothing was uploaded");
}

#[test]
fn joined_the_review_ticks_every_task_with_similar_titles_and_keeps_unfinished_runs() {
  let server = TestServer::start();
  let alice = shared(&server);
  let (bohdan, tasks) = bohdan(&server);
  member(&alice, "bohdan");
  let review = review(&bohdan);
  assert_eq!(review.key.as_str(), "ORC");
  assert_eq!(review.next, 4);
  let rows: Vec<(String, Option<String>, bool)> = review
    .tasks
    .iter()
    .map(|row| {
      (
        row.task.key().to_string(),
        row
          .similar
          .as_ref()
          .map(|similar| format!("{} {}", similar.key, similar.created_by)),
        row.locked(),
      )
    })
    .collect();
  assert_eq!(
    rows,
    [
      ("KON-1".into(), Some("ORC-1 Alice Moreau".into()), false),
      ("KON-2".into(), Some("ORC-2 Alice Moreau".into()), true),
      ("KON-3".into(), None, false),
    ]
  );
  assert_eq!(review.tasks[1].task.id(), tasks[1]);
}

#[test]
fn joined_ticked_tasks_go_up_with_new_numbers_and_the_others_stay_not_shared() {
  let server = TestServer::start();
  let (alice, oleh) = with_oleh(&server);
  let (bohdan, tasks) = bohdan(&server);
  member(&alice, "bohdan");
  let joined = join(&bohdan, &tasks);
  assert_eq!(joined.open.project.key().as_str(), "ORC");
  assert_eq!(
    joined.dropped,
    [("KON-1".to_owned(), "KON-3".to_owned())],
    "the link to KON-3 is named"
  );

  let db = bohdan.db();
  let keys: Vec<String> = db
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3", "ORC-4", "ORC-5"]);
  let kept: Vec<String> = db
    .not_shared_tasks()
    .expect("kept")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(kept, ["KON-3"]);
  let working = db.task(tasks[1]).expect("read").expect("task");
  assert_eq!(working.key().to_string(), "ORC-5");
  let info = db.shared_task_info(tasks[1]).expect("info").expect("shared");
  assert_eq!(info.former_key.as_deref(), Some("KON-2"));
  let claim = info.claim.expect("claimed for Bohdan");
  assert_eq!(
    (claim.holder.name.as_str(), claim.kind),
    ("Bohdan Koval", ClaimKind::Run)
  );
  // The run stays attached by the task's id; the next one is named by the new key.
  let runs = db.runs_of_task(tasks[1]).expect("runs");
  assert_eq!(runs.len(), 1);
  assert_eq!(
    BranchName::for_next_run(working.key(), runs.len()).as_str(),
    "orchester/orc-5-run-2"
  );
  let mapping: Vec<(String, String, bool)> = db
    .joined_keys()
    .expect("mapping")
    .into_iter()
    .map(|key| (key.former, key.key.to_string(), key.not_shared))
    .collect();
  assert_eq!(
    mapping,
    [
      ("KON-1".to_owned(), "ORC-4".to_owned(), false),
      ("KON-2".to_owned(), "ORC-5".to_owned(), false),
      ("KON-3".to_owned(), "KON-3".to_owned(), true),
    ]
  );
  // Every member sees the former key.
  let on_server = on_server(&alice, &oleh);
  let former: Vec<Option<&str>> = on_server.iter().map(|task| task.former_key.as_deref()).collect();
  assert_eq!(former, [None, None, None, Some("KON-1"), Some("KON-2")]);
}

#[test]
fn joined_share_task_uploads_a_kept_task_later() {
  let server = TestServer::start();
  let (alice, oleh) = with_oleh(&server);
  let (bohdan, tasks) = bohdan(&server);
  member(&alice, "bohdan");
  join(&bohdan, &tasks);
  let key = bohdan
    .block_on(bohdan.server().share_kept(tasks[2], IdempotencyKey::generate()))
    .expect("share task");
  assert_eq!(key.to_string(), "ORC-6");
  assert!(bohdan.db().not_shared_tasks().expect("kept").is_empty());
  let info = bohdan.db().shared_task_info(tasks[2]).expect("info").expect("shared");
  assert_eq!(info.former_key.as_deref(), Some("KON-3"));
  assert_eq!(on_server(&alice, &oleh).len(), 6);
}

#[test]
fn joined_make_local_returns_not_shared_tasks_after_the_highest_number() {
  let server = TestServer::start();
  let alice = shared(&server);
  let (bohdan, tasks) = bohdan(&server);
  member(&alice, "bohdan");
  join(&bohdan, &tasks);
  let returned = bohdan.block_on(bohdan.server().make_local()).expect("make local");
  let mapping: Vec<(String, String)> = returned
    .iter()
    .map(|task| (task.before.to_string(), task.after.to_string()))
    .collect();
  assert_eq!(mapping, [("KON-3".to_owned(), "ORC-6".to_owned())]);
  let keys: Vec<String> = bohdan.tasks().iter().map(|task| task.key().to_string()).collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3", "ORC-4", "ORC-5", "ORC-6"]);
  assert_eq!(bohdan.db().sharing().expect("link"), None);
}

/// A join whose import reached the server but whose answer was lost (or which could not finish
/// here) left KON-2, the task whose run works, on the server under another key. Joining again
/// uploads only what the server lacks and links the rest as the server holds it, so a task that can
/// never be unticked does not keep the project from joining.
#[test]
fn joined_a_join_after_a_lost_answer_does_not_upload_twice() {
  let server = TestServer::start();
  let alice = shared(&server);
  let (bohdan, tasks) = bohdan(&server);
  member(&alice, "bohdan");
  let token = server.sign_in_elsewhere("bohdan", "bohdan's own long password", "unused");
  let client = orchester_api::client::Client::new(
    orchester_api::client::ServerAddress::parse(&server.url()).expect("address"),
    orchester_api::client::PlainHttp::NotAllowed,
  )
  .expect("client");
  let project = review(&bohdan).project;
  let read = bohdan.db().task(tasks[1]).expect("read").expect("task");
  let earlier = orchester_api::UploadTask {
    id: read.id(),
    number: None,
    former_key: Some("KON-2".into()),
    title: read.details().title.as_str().into(),
    brief: read.details().brief.clone(),
    criteria: read.details().acceptance_criteria.clone(),
    pipeline: Some("delivery".into()),
    limits: Some(orchester_api::SharedLimits {
      budget_tokens: Some(1_000_000),
      max_duration_ms: Some(1_800_000),
    }),
    workflow: WorkflowStatus::InProgress,
    priority: Priority::None,
    tag_names: Vec::new(),
    blocking: Vec::new(),
    blocked_reason: None,
    status: read.status(),
    created_at: read.created_at().unix_millis(),
    claim: true,
    has_run: true,
  };
  bohdan
    .block_on(client.import_tasks(
      &token,
      project,
      &orchester_api::TasksImport {
        idempotency_key: IdempotencyKey::generate(),
        tasks: vec![earlier],
      },
    ))
    .expect("the earlier join's import");
  let joined = join(&bohdan, &tasks);
  assert_eq!(joined.open.project.key().as_str(), "ORC");
  let keys: Vec<String> = bohdan.tasks().iter().map(|task| task.key().to_string()).collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3", "ORC-4", "ORC-5"]);
  let working = bohdan.db().task(tasks[1]).expect("read").expect("task");
  assert_eq!(working.key().to_string(), "ORC-4", "linked as the server holds it");
  let first = bohdan.db().task(tasks[0]).expect("read").expect("task");
  assert_eq!(first.key().to_string(), "ORC-5");
}
