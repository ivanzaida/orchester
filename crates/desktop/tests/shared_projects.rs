//! Shared projects on the desktop (`docs/architecture/server.md`, phase S2b) against a real
//! `orchester-server` on loopback: Share uploads every task with its number and writes the discovery
//! file; `RemoteTaskStore` changes tasks under their revision, with only the fields that differ, a
//! conflict as a result and the organization merged without asking; a pipeline this computer lacks
//! stays on the server; starting work takes the run claim and names the holder when it cannot;
//! heartbeats write lost claims to the cache; members and roles; a sync writes others' changes and
//! notices a removed member; Make local keeps every task; offline nothing is sent.
//!
//! `cargo test -p orchester-desktop --test shared_projects shared_`

#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use orchester_api::{
  ClaimTake, MemberRole, TaskChanges, TaskPatch,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::{
  sharing::ClaimKind,
  store::TaskStore,
  task::{
    TaskUpdate,
    content::{Choice, ContentField},
    organization::{Priority, TaskOrganization, WorkflowStatus},
  },
};
use orchester_desktop::{
  sharing::{
    SharingError,
    project::{MemberChange, WorkRefusal, discovery},
  },
  tasks::{
    conflict::{Edit, EditSaved, save_edit},
    writer::TaskWriter,
  },
};
use server_support::TestServer;
use support::{Other, Person, details, member, shared};

#[test]
fn shared_share_keeps_numbers_writes_the_discovery_file_and_caches_the_server_s_tasks() {
  let server = TestServer::start();
  let alice = shared(&server);
  let link = alice.db().sharing().expect("link").expect("shared");
  assert_eq!(link.key.as_str(), "ORC");
  assert_eq!(link.role, Some(MemberRole::Owner));
  let keys: Vec<String> = alice.tasks().iter().map(|task| task.key().to_string()).collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3"]);
  assert!(
    alice
      .tasks()
      .iter()
      .all(|task| alice.db().shared_task_info(task.id()).expect("info").is_some()),
    "every task is the server's"
  );
  assert!(matches!(
    discovery::read(alice.root()),
    discovery::Discovery::Server(address) if address.as_str() == server.url()
  ));
  // Sharing again is refused locally: the project is shared already.
  let identity = orchester_core::repository::RepositoryIdentity::from_remote_url(support::REMOTE).expect("identity");
  let again = alice.block_on(alice.servers.share(
    &alice.entry(),
    alice.root(),
    identity,
    orchester_core::id::IdempotencyKey::generate(),
    false,
  ));
  assert!(again.is_err());
}

#[test]
fn shared_edits_go_with_their_revision_and_a_conflict_comes_back_with_the_current_task() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(1);
  let project = alice.server().link().project;

  let mut edited = task.details().clone();
  edited.brief = "Reconnect after the computer sleeps.".into();
  let saved = alice
    .with_store(|store| store.update_task(task.id(), task.revision(), edited.clone()))
    .expect("edit")
    .saved()
    .expect("saved on the revision read");
  assert_eq!(saved.revision(), task.revision().next().expect("next"));

  // Oleh changes the title meanwhile.
  alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      project,
      task.id(),
      &TaskPatch {
        revision: saved.revision(),
        changes: TaskChanges {
          title: Some("Fix reconnect after sleep".into()),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("oleh's edit");
  let mut again = saved.details().clone();
  again.brief = "Reconnect at once.".into();
  let update = alice
    .with_store(|store| store.update_task(task.id(), saved.revision(), again))
    .expect("edit");
  let TaskUpdate::Conflict { current } = update else {
    panic!("Oleh saved a newer version");
  };
  assert_eq!(current.details().title.as_str(), "Fix reconnect after sleep");
  assert_eq!(
    alice.db().task(task.id()).expect("read"),
    Some(current),
    "the cache has Oleh's version"
  );
  let info = alice.db().shared_task_info(task.id()).expect("info").expect("shared");
  assert_eq!(info.updated_by.as_deref(), Some("Oleh Petrenko"));
}

#[test]
fn shared_organization_merges_with_a_newer_revision_without_asking() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  let project = alice.server().link().project;
  alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      project,
      task.id(),
      &TaskPatch {
        revision: task.revision(),
        changes: TaskChanges {
          workflow: Some(WorkflowStatus::InProgress),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("oleh's status");
  // Alice's cache still has the older revision; her priority goes on top of Oleh's status.
  let base = task.organization().clone();
  let edited = base.clone().with_priority(Priority::Urgent);
  let changed = alice
    .with_store(|store| {
      store.change_organization(
        task.id(),
        (&base, &edited),
        orchester_core::task::organization::ChangeActor::Person,
      )
    })
    .expect("organize");
  assert!(changed);
  let now = alice.db().task_organization(task.id()).expect("organization");
  assert_eq!(
    (now.status(), now.priority()),
    (WorkflowStatus::InProgress, Priority::Urgent)
  );
  let on_server = alice
    .block_on(oleh.client.tasks(&oleh.token, project))
    .expect("tasks")
    .tasks
    .into_iter()
    .find(|shared| shared.id == task.id())
    .expect("the task");
  assert_eq!(
    (on_server.content.workflow, on_server.content.priority),
    (WorkflowStatus::InProgress, Priority::Urgent)
  );
}

#[test]
fn shared_a_pipeline_this_computer_lacks_stays_on_the_server_when_the_title_changes() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(2);
  let project = alice.server().link().project;
  let revised = alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      project,
      task.id(),
      &TaskPatch {
        revision: task.revision(),
        changes: TaskChanges {
          pipeline: Some(Some("design-review".into())),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("oleh's pipeline");
  alice.block_on(alice.server().sync()).expect("sync");
  let cached = alice.db().task(task.id()).expect("read").expect("task");
  assert_eq!(cached.details().pipeline, None, "missing on this computer");
  assert_eq!(cached.revision(), revised.revision);
  let mut retitled = cached.details().clone();
  retitled.title = orchester_core::name::Name::new("Merge queue for delivery").expect("title");
  alice
    .with_store(|store| store.update_task(task.id(), cached.revision(), retitled))
    .expect("edit")
    .saved()
    .expect("saved");
  let on_server = alice
    .block_on(oleh.client.tasks(&oleh.token, project))
    .expect("tasks")
    .tasks
    .into_iter()
    .find(|shared| shared.id == task.id())
    .expect("the task");
  assert_eq!(on_server.content.pipeline.as_deref(), Some("design-review"));
  assert_eq!(on_server.content.title, "Merge queue for delivery");
}

#[test]
fn shared_work_takes_the_run_claim_and_names_the_holder_when_it_cannot() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let tasks = alice.tasks();
  let project = alice.server().link().project;
  let taken = alice
    .block_on(alice.servers.claim_for_work(alice.root(), tasks[0].id()))
    .expect("alice's claim");
  assert!(taken.newly_taken());
  let info = alice
    .db()
    .shared_task_info(tasks[0].id())
    .expect("info")
    .expect("shared");
  assert_eq!(info.claim.map(|claim| claim.kind), Some(ClaimKind::Run));
  let confirmed = alice
    .block_on(alice.servers.claim_for_work(alice.root(), tasks[0].id()))
    .expect("the holder confirms her claim");
  assert!(!confirmed.newly_taken(), "a claim she held already");

  alice
    .block_on(oleh.client.claim(
      &oleh.token,
      project,
      tasks[1].id(),
      &ClaimTake {
        kind: ClaimKind::Manual,
      },
    ))
    .expect("oleh takes the task");
  let refused = alice.block_on(alice.servers.claim_for_work(alice.root(), tasks[1].id()));
  assert!(matches!(
    refused,
    Err(WorkRefusal::Held { holder, stale: false }) if holder == "Oleh Petrenko"
  ));
}

#[test]
fn shared_a_heartbeat_keeps_claims_and_writes_the_lost_ones_to_the_cache() {
  let server = TestServer::start();
  let alice = shared(&server);
  let task = alice.tasks().remove(0);
  let claim = alice
    .block_on(alice.servers.claim_for_work(alice.root(), task.id()))
    .expect("claim");
  assert!(claim.newly_taken());
  let me = alice.server().me().expect("signed in");
  assert_eq!(alice.db().claims_held_by(me).expect("claims"), vec![task.id()]);
  alice.block_on(alice.servers.sync_shared(true));
  assert_eq!(
    alice.db().claims_held_by(me).expect("claims"),
    vec![task.id()],
    "kept fresh"
  );

  // Released from Alice's other computer: the next heartbeat reports it lost.
  let token = server.sign_in_elsewhere("alice", "alice's own long password", "unused");
  let project = alice.server().link().project;
  let client = Client::new(
    ServerAddress::parse(&server.url()).expect("address"),
    PlainHttp::NotAllowed,
  )
  .expect("client");
  alice
    .block_on(client.release_claim(&token, project, task.id()))
    .expect("release elsewhere");
  alice.block_on(alice.servers.sync_shared(true));
  assert!(alice.db().claims_held_by(me).expect("claims").is_empty());
}

#[test]
fn shared_members_and_roles_with_the_last_owner_refused() {
  let server = TestServer::start();
  let alice = shared(&server);
  let _oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  let project = alice.server();
  let missing = alice.block_on(project.add_member("maria.l".into(), MemberRole::Member));
  assert_eq!(
    missing.expect_err("no such user").code(),
    Some(orchester_api::ErrorCode::NoSuchUser)
  );
  let oleh = alice
    .block_on(project.add_member("oleh".into(), MemberRole::Member))
    .expect("add");
  let me = project.me().expect("signed in");
  let last = alice.block_on(project.change_member(MemberChange::Role(me, MemberRole::Member)));
  assert_eq!(
    last.expect_err("last owner").code(),
    Some(orchester_api::ErrorCode::LastOwner)
  );
  alice
    .block_on(project.change_member(MemberChange::Role(oleh.user.id, MemberRole::Owner)))
    .expect("make owner");
  alice
    .block_on(project.change_member(MemberChange::Role(me, MemberRole::Member)))
    .expect("step down");
  assert_eq!(
    alice.db().sharing().expect("link").expect("shared").role,
    Some(MemberRole::Member)
  );
  let members = alice.block_on(project.members()).expect("members");
  assert_eq!(members.len(), 2);
}

#[test]
fn shared_a_sync_writes_others_changes_and_notices_a_removed_member() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let project = alice.server().link().project;
  let task = alice.tasks().remove(0);
  alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      project,
      task.id(),
      &TaskPatch {
        revision: task.revision(),
        changes: TaskChanges {
          brief: Some("Switch between repositories.".into()),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("oleh's edit");
  let before = alice.db().sharing().expect("link").expect("shared").last_synced_at;
  alice.block_on(alice.server().sync()).expect("sync");
  let after = alice.db().sharing().expect("link").expect("shared");
  assert!(after.last_synced_at >= before);
  assert_eq!(
    alice.db().task(task.id()).expect("read").expect("task").details().brief,
    "Switch between repositories."
  );

  // Oleh, now an owner, removes Alice.
  let me = alice.server().me().expect("signed in");
  alice
    .block_on(
      alice.server().change_member(MemberChange::Role(
        alice
          .block_on(alice.server().members())
          .expect("members")
          .into_iter()
          .find(|m| m.user.id != me)
          .expect("oleh")
          .user
          .id,
        MemberRole::Owner,
      )),
    )
    .expect("make oleh an owner");
  alice
    .block_on(oleh.client.remove_member(&oleh.token, project, me))
    .expect("remove alice");
  alice.block_on(alice.server().sync()).expect("sync");
  let link = alice.db().sharing().expect("link").expect("shared");
  assert!(link.removed_at.is_some(), "no longer a member");
  let refused = alice.with_store(|store| store.update_task(task.id(), task.revision(), details("Changed")));
  assert!(matches!(refused, Err(SharingError::NotMember)));
  assert_eq!(alice.tasks().len(), 3, "the list stays, read-only");
}

#[test]
fn shared_make_local_releases_claims_and_keeps_every_task() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let project = alice.server().link().project;
  let tasks = alice.tasks();
  let claim = alice
    .block_on(alice.servers.claim_for_work(alice.root(), tasks[0].id()))
    .expect("claim");
  assert!(claim.newly_taken());
  alice.block_on(alice.server().make_local()).expect("make local");
  assert_eq!(alice.db().sharing().expect("link"), None);
  assert_eq!(alice.tasks().len(), 3);
  let on_server = alice
    .block_on(oleh.client.tasks(&oleh.token, project))
    .expect("the project stays for everyone else")
    .tasks;
  assert!(
    on_server.iter().all(|task| task.claim.is_none()),
    "Alice's claim was released"
  );
  let mut db = alice.db();
  let task = db.task(tasks[0].id()).expect("read").expect("task");
  db.update_task(task.id(), task.revision(), details("Local again"))
    .expect("edit")
    .saved()
    .expect("a local task");
}

#[test]
fn shared_offline_nothing_is_sent_and_the_list_stays_readable() {
  let server = TestServer::start();
  let alice = shared(&server);
  let task = alice.tasks().remove(0);
  drop(server);
  let first = alice.with_store(|store| store.update_task(task.id(), task.revision(), details("Offline edit")));
  assert!(matches!(first, Err(SharingError::Client(error)) if error.is_connection()));
  let second = alice.with_store(|store| store.update_task(task.id(), task.revision(), details("Offline edit")));
  assert!(
    matches!(second, Err(SharingError::Offline)),
    "refused at once while offline"
  );
  let refused = alice.block_on(alice.servers.claim_for_work(alice.root(), task.id()));
  assert!(matches!(
    refused,
    Err(WorkRefusal::Sharing {
      source: SharingError::Offline,
      ..
    })
  ));
  assert_eq!(
    alice.db().task(task.id()).expect("read"),
    Some(task),
    "the cache is unchanged"
  );
  assert_eq!(
    alice
      .db()
      .task_organization(alice.tasks()[0].id())
      .expect("organization"),
    TaskOrganization::default()
  );
}

/// Oleh changes the brief of `task` on the server: the newer version Alice's edit meets.
fn olehs_brief(alice: &Person, oleh: &Other, task: &orchester_core::task::Task) {
  alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      alice.server().link().project,
      task.id(),
      &TaskPatch {
        revision: task.revision(),
        changes: TaskChanges {
          brief: Some("Oleh's brief.".into()),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("oleh's edit");
}

/// Alice's edit of `task`, read before Oleh's, changing what `change` changes.
fn alices_edit(
  alice: &Person,
  task: &orchester_core::task::Task,
  change: impl FnOnce(&mut orchester_core::task::TaskDetails),
) -> EditSaved {
  let _context = alice.runtime.enter();
  let mut writer = TaskWriter::open(alice.root(), &alice.servers).expect("writer");
  let mut details = task.details().clone();
  change(&mut details);
  let edit = Edit {
    task: task.id(),
    read: task.revision(),
    base: task.details(),
    details,
    organization: (task.organization(), task.organization()),
  };
  save_edit(&mut writer, edit, orchester_core::time::Timestamp::now()).expect("save")
}

#[test]
fn shared_an_edit_merges_into_a_newer_version_unless_both_changed_a_field() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let tasks = alice.tasks();

  olehs_brief(&alice, &oleh, &tasks[0]);
  let EditSaved::Saved { task, details } = alices_edit(&alice, &tasks[0], |details| {
    details.title = orchester_core::name::Name::new("Repository picker").expect("title");
  }) else {
    panic!("different fields merge without asking");
  };
  assert_eq!(
    (details.title.as_str(), details.brief.as_str()),
    ("Repository picker", "Oleh's brief.")
  );
  assert_eq!(task.details(), &details);

  olehs_brief(&alice, &oleh, &tasks[1]);
  let EditSaved::Conflict(conflict) = alices_edit(&alice, &tasks[1], |details| details.brief = "Alice's brief.".into())
  else {
    panic!("both changed the brief");
  };
  assert_eq!(conflict.merge.conflicts(), vec![ContentField::Brief]);
  assert_eq!(conflict.by.as_deref(), Some("Oleh Petrenko"));
  assert_eq!(
    conflict.merge.mine().brief,
    "Alice's brief.",
    "her text is kept until she chooses"
  );
  assert_eq!(conflict.resolved(|_| Choice::Theirs).brief, "Oleh's brief.",);
}

/// `orchester.toml` is written only where nothing is: a link, even one that leads nowhere, is
/// refused and nothing is written through it. (Windows needs a privilege to make a link, so this
/// runs where links can be made without one.)
#[cfg(unix)]
#[test]
fn shared_the_discovery_file_is_never_written_through_a_link() {
  let repo = tempfile::tempdir().expect("repository");
  let target = repo.path().join("elsewhere.toml");
  std::os::unix::fs::symlink(&target, repo.path().join(discovery::FILE)).expect("link");
  assert!(matches!(
    discovery::write(repo.path(), "https://orchester.example"),
    Err(discovery::DiscoveryError::Link)
  ));
  assert!(!target.exists(), "nothing was written through the link");
}
