//! Joining a server project that has tasks already (`docs/architecture/server.md`, "Two people share
//! one repository", phase S2c, rows 49–50) against a real `orchester-server` on loopback, with two
//! people: Alice linked Kontur to the project at `<server>/kontur` as ORC; Bohdan has the same
//! repository with local tasks KON-1 … KON-3. A person who is not a member learns nothing of the
//! project and nothing is uploaded; a first-checkout link that meets tasks leads to Join; a
//! checkout is found by the project's address, with the repository remote only a hint; the review ticks every task,
//! hints at similar titles and keeps a task with an unfinished run ticked; the join uploads the ticked tasks with new
//! numbers, ids and former keys (the unfinished one claimed), drops a link to a task that stays here and names it,
//! keeps the others *Not shared*, keeps local runs attached and names the next branch by the new key; *Share
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
  ProjectAddress, RemoteMatch, SharingError,
  project::{
    JoinReview, LinkFirst, ShareError,
    landing::{CheckoutRefusal, clone_command},
    lookup::Lookup,
  },
  remote_match,
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
  let code = server.code_for("bohdan", false);
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
  let person = Person::with_repo(server, ("bohdan", "Bohdan Koval", &code), kontur.into_repo());
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

/// What linking Bohdan's checkout to the project at `<server>/kontur` would do.
fn review(bohdan: &Person) -> JoinReview {
  bohdan
    .block_on(bohdan.servers.join_review_at(&bohdan.entry(), bohdan.root(), "kontur"))
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
fn joined_a_person_who_is_not_a_member_learns_nothing_of_the_project_and_uploads_nothing() {
  let server = TestServer::start();
  let (alice, oleh) = with_oleh(&server);
  let (bohdan, _) = bohdan(&server);
  let found = bohdan.block_on(bohdan.servers.find_project(&bohdan.entry(), "kontur"));
  assert!(matches!(&found, Err(SharingError::NoProject)), "{found:?}");
  let missing = bohdan.block_on(bohdan.servers.find_project(&bohdan.entry(), "billing"));
  assert!(matches!(missing, Err(SharingError::NoProject)));
  // Neither the first-checkout link nor Join uploads anything of Bohdan's.
  let request = LinkFirst {
    slug: "kontur".into(),
    remote: Some(identity()),
    key: IdempotencyKey::generate(),
    write_discovery: false,
  };
  let linked = bohdan.block_on(bohdan.servers.link_first(&bohdan.entry(), bohdan.root(), request));
  assert!(matches!(linked, Err(ShareError::Sharing(SharingError::NoProject))));
  assert_eq!(bohdan.db().sharing().expect("link"), None);
  let review = bohdan.block_on(bohdan.servers.join_review_at(&bohdan.entry(), bohdan.root(), "kontur"));
  assert!(matches!(review, Err(SharingError::NoProject)));
  assert_eq!(
    bohdan
      .block_on(bohdan.servers.lookup(&bohdan.entry(), identity()))
      .expect("lookup"),
    Lookup::NotShared,
    "a lookup by repository sees only the person's projects"
  );
  member(&alice, "bohdan");
  let lookup = bohdan
    .block_on(bohdan.servers.lookup(&bohdan.entry(), identity()))
    .expect("check again");
  assert_eq!(lookup, Lookup::Shared { key: "ORC".into() });
  // A member's checkout of a project with tasks joins it; it is not its first checkout.
  let request = LinkFirst {
    slug: "kontur".into(),
    remote: Some(identity()),
    key: IdempotencyKey::generate(),
    write_discovery: false,
  };
  let linked = bohdan.block_on(bohdan.servers.link_first(&bohdan.entry(), bohdan.root(), request));
  assert!(matches!(linked, Err(ShareError::Taken { member: true, .. })));
  assert_eq!(on_server(&alice, &oleh).len(), 3, "nothing was uploaded");
}

#[test]
fn joined_a_checkout_is_linked_by_the_project_s_address_with_the_remote_as_a_hint() {
  let server = TestServer::start();
  let alice = shared(&server);
  let (bohdan, _) = bohdan(&server);
  member(&alice, "bohdan");
  let address = ProjectAddress::parse(&format!("{}/kontur", server.url())).expect("project address");
  assert_eq!(address.slug(), "kontur");
  assert_eq!(address.server().as_str(), server.url());
  assert!(
    ProjectAddress::parse(&server.url()).is_err(),
    "a server's address names no project"
  );
  let project = bohdan
    .block_on(bohdan.servers.find_project(&bohdan.entry(), address.slug()))
    .expect("the project at the address");
  assert_eq!(project.address, address.to_text());
  let hint = project.remote_hint;
  assert_eq!(hint, Some(identity()));
  assert_eq!(remote_match(Some(&identity()), hint.as_ref()), RemoteMatch::Same);
  let fork = RepositoryIdentity::from_remote_url("git@github.com:bohdan/kontur-fork.git").expect("fork");
  let mismatch = remote_match(Some(&fork), hint.as_ref());
  assert!(mismatch.warns(), "another repository warns, and may still be linked");
  assert_eq!(remote_match(None, hint.as_ref()), RemoteMatch::NoOrigin);
  assert_eq!(remote_match(Some(&fork), None), RemoteMatch::NoHint);
  let review = review(&bohdan);
  assert_eq!(review.address, address.to_text());
  let joined = bohdan
    .block_on(bohdan.servers.join(
      &bohdan.entry(),
      bohdan.root(),
      &review,
      (Vec::new(), IdempotencyKey::generate()),
    ))
    .expect("join");
  assert_eq!(joined.open.project.key().as_str(), "ORC");
  let link = bohdan.db().sharing().expect("link").expect("linked");
  assert_eq!(link.address.as_deref(), Some(address.to_text().as_str()));
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
  let token = server.device_of("bohdan");
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
    module: None,
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
        modules: Vec::new(),
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

/// The invite landing (N3, `xeFRB`): the person's local projects as checkouts for the project, the one whose `origin`
/// matches its hint opened at once; a folder that isn't a git checkout is refused, and a checkout Orchester doesn't
/// know yet is added as a local project.
#[test]
fn joined_the_invite_landing_finds_the_checkout_whose_origin_matches() {
  let server = TestServer::start();
  let alice = shared(&server);
  let (bohdan, _) = bohdan(&server);
  member(&alice, "bohdan");
  let programs = orchester_exec::programs::Programs::processes();
  let landing = bohdan
    .block_on(bohdan.servers.landing(&bohdan.entry(), "kontur", &programs))
    .expect("landing");
  assert_eq!(landing.checkouts.len(), 1, "Bohdan's Kontur, which has no origin");
  assert_eq!(landing.checkouts[0].remote, RemoteMatch::NoOrigin);
  assert!(landing.direct().is_none(), "nothing matches the hint");
  assert_eq!(landing.candidates().count(), 0);
  assert_eq!(
    landing.clone_command().as_deref(),
    Some("git clone https://github.com/ivanzaida/kontur.git")
  );

  let archive = tempfile::TempDir::new().expect("a downloaded archive");
  let refused = bohdan.block_on(bohdan.servers.add_checkout(archive.path().to_owned()));
  assert!(matches!(refused, Err(CheckoutRefusal::NotGit)));

  let parent = tempfile::TempDir::new().expect("where the clone goes");
  let clone = parent.path().join("kontur");
  std::fs::create_dir(&clone).expect("the clone's folder");
  for args in [&["init", "-q"][..], &["remote", "add", "origin", REMOTE]] {
    let status = std::process::Command::new("git")
      .args(args)
      .current_dir(&clone)
      .status()
      .expect("git runs");
    assert!(status.success(), "git {args:?}");
  }
  let added = bohdan.block_on(bohdan.servers.add_checkout(clone)).expect("added");
  assert_eq!(added.project.name().as_str(), "kontur");
  let landing = bohdan
    .block_on(bohdan.servers.landing(&bohdan.entry(), "kontur", &programs))
    .expect("landing");
  let direct = landing.direct().expect("the one checkout that matches");
  assert_eq!(direct.id, added.project.id());
  assert!(direct.tasks.is_empty(), "a new local project has no tasks");
}

/// The landing's `git clone` (N3 review, M3): only a hint of a plain host and plain path segments gets the command a
/// person may paste into a shell; anything else is shown as text only.
#[test]
fn joined_the_clone_command_is_offered_only_for_a_plain_repository() {
  let plain = RepositoryIdentity::parse("github.com/acme/kontur-app_2.x").expect("plain");
  assert_eq!(
    clone_command(&plain).as_deref(),
    Some("git clone https://github.com/acme/kontur-app_2.x.git")
  );
  let with_port = RepositoryIdentity::parse("git.example.com:8443/team/kontur").expect("port");
  assert!(clone_command(&with_port).is_some());
  let ipv6 = RepositoryIdentity::from_remote_url("https://[::1]:3000/team/kontur.git").expect("ipv6");
  assert!(clone_command(&ipv6).is_some());
  for text in [
    "github.com/acme/$(touch pwned)",
    "github.com/acme/$(id)",
    "github.com/acme/`id`",
    "github.com/acme/kontur;id",
    "github.com/acme/kontur|id",
    "github.com/acme/kon\"tur",
    "github.com/acme/kon'tur",
    "github.com/acme/kon\u{7}tur",
    "github.com/acme/kon\ntur",
    "github.com/acme/kon&tur",
    "evil$(id).com/acme/kontur",
  ] {
    if let Ok(identity) = RepositoryIdentity::parse(text) {
      assert_eq!(clone_command(&identity), None, "{text:?} gets no command");
    }
  }
}

/// N3 review, M1: a checkout links to one project. Once Bohdan's Kontur joined ORC, neither a review, a link's plan nor
/// a join of another project reads or sends anything for it: its tasks never go up into another project.
#[test]
fn joined_a_linked_checkout_uploads_nothing_into_another_project() {
  let server = TestServer::start();
  let alice = shared(&server);
  let (bohdan, tasks) = bohdan(&server);
  member(&alice, "bohdan");
  let atlas = server.create_named_project(("Atlas", "ATL"), "atlas", None);
  server.add_member(&atlas, "bohdan");
  let before = review(&bohdan);
  join(&bohdan, &tasks);

  let refused = |result: Result<(), SharingError>| {
    assert!(
      matches!(
        result,
        Err(SharingError::Store(orchester_store::StoreError::AlreadyShared))
      ),
      "{result:?}"
    );
  };
  let entry = bohdan.entry();
  refused(
    bohdan
      .block_on(bohdan.servers.join_review_at(&entry, bohdan.root(), "atlas"))
      .map(drop),
  );
  refused(
    bohdan
      .block_on(bohdan.servers.link_plan(&entry, bohdan.root(), atlas.clone()))
      .map(drop),
  );
  // The review read before the join, confirmed again for the project it named: nothing goes up twice.
  let again = bohdan.block_on(bohdan.servers.join(
    &entry,
    bohdan.root(),
    &before,
    (vec![tasks[0]], IdempotencyKey::generate()),
  ));
  refused(again.map(drop).map_err(|failed| failed.error));
  assert!(server.task_keys(&atlas).is_empty(), "nothing went up into Atlas");
}
