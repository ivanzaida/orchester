//! N2's domain rules against a real `orchester-server` on loopback: what linking a checkout would do before the person
//! confirms it (an upload that keeps numbers, the server's tasks coming down to a checkout without tasks, or a join
//! to review), the link itself, the members kept as last read for when the server cannot be reached, what Make local
//! would do, and Make local once this computer signed out of the server. A link whose tasks changed after its plan
//! sends nothing, the members show as last read once the person is not a member or without the server, Make local says
//! which claims it could not release, and an address is used only on the server that reported it.
//!
//! `cargo test -p orchester-workspace --test shared_links links_`

#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::sync::Arc;

use orchester_api::client::ServerAddress;
use orchester_core::{id::IdempotencyKey, store::TaskStore, time::Timestamp};
use orchester_store::AppDb;
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    ProjectAddress, Servers, SharingError,
    project::{
      ClaimRelease, MembersRead, ShareError,
      discovery::DiscoveryState,
      first_link::{LinkKind, LinkRequest},
    },
  },
  workspace::Workspace,
};
use server_support::TestServer;
use support::{MemoryVault, Person, details, shared};

/// A second computer of `username`, with a local project of `titles`, a member of Alice's Kontur.
fn colleague(server: &TestServer, alice: &Person, username: &str, titles: &[&str]) -> Person {
  let code = server.code_for(username, false);
  let person = Person::signed_in(server, (username, "Maria Lind", &code), titles);
  support::member(alice, username);
  person
}

fn kontur(person: &Person) -> orchester_api::SharedProject {
  person
    .block_on(person.servers.find_project(&person.entry(), "kontur"))
    .expect("Kontur")
}

#[test]
fn links_plan_an_upload_with_numbers_and_link_it() {
  let server = TestServer::start();
  let code = server.code_for("alice", true);
  let alice = Person::signed_in(&server, ("alice", "Alice Moreau", &code), &["One", "Two"]);
  server.create_project("kontur", None);
  let plan = alice
    .block_on(alice.servers.link_plan(&alice.entry(), alice.root(), kontur(&alice)))
    .expect("plan");
  let LinkKind::Upload { tasks, next, claimed } = &plan.kind else {
    panic!("an empty project takes the tasks: {:?}", plan.kind);
  };
  let keys: Vec<String> = tasks.iter().map(ToString::to_string).collect();
  assert_eq!(keys, ["ORC-1", "ORC-2"]);
  assert_eq!(next.to_string(), "ORC-3");
  assert!(claimed.is_empty());
  assert_eq!(plan.discovery, DiscoveryState::Absent);
  let request = LinkRequest {
    remote: None,
    key: IdempotencyKey::generate(),
    write_discovery: true,
  };
  alice
    .block_on(
      alice
        .servers
        .link_checkout(&alice.entry(), alice.root(), &plan, request),
    )
    .expect("linked");
  let keys: Vec<String> = alice.tasks().iter().map(|task| task.key().to_string()).collect();
  assert_eq!(keys, ["ORC-1", "ORC-2"]);
  assert_eq!(
    orchester_workspace::sharing::project::discovery::discovery_state(
      alice.root(),
      &format!("{}/kontur", server.url())
    ),
    DiscoveryState::Names
  );
}

#[test]
fn links_bring_a_project_down_to_a_checkout_without_tasks_and_review_one_with_tasks() {
  let server = TestServer::start();
  let alice = shared(&server);
  let maria = colleague(&server, &alice, "maria", &[]);
  let plan = maria
    .block_on(maria.servers.link_plan(&maria.entry(), maria.root(), kontur(&maria)))
    .expect("plan");
  assert_eq!(plan.kind, LinkKind::Download { tasks: 3 });
  let request = LinkRequest {
    remote: None,
    key: IdempotencyKey::generate(),
    write_discovery: false,
  };
  maria
    .block_on(
      maria
        .servers
        .link_checkout(&maria.entry(), maria.root(), &plan, request),
    )
    .expect("linked");
  assert_eq!(maria.tasks().len(), 3);

  let oleh = colleague(&server, &alice, "oleh", &["Mine"]);
  let plan = oleh
    .block_on(oleh.servers.link_plan(&oleh.entry(), oleh.root(), kontur(&oleh)))
    .expect("plan");
  assert_eq!(plan.kind, LinkKind::Review { local: 1, server: 3 });
}

#[test]
fn links_keep_the_members_and_hint_as_last_read_and_plan_make_local() {
  let server = TestServer::start();
  let alice = shared(&server);
  let members = alice.block_on(alice.server().current_members()).expect("members");
  assert!(members.live());
  alice.block_on(alice.server().sync()).expect("synced");
  let (hint, _) = alice.db().remote_hint().expect("hint").expect("read once");
  assert_eq!(
    hint.as_deref(),
    Some("github.com/ivanzaida/kontur"),
    "the hint is kept as last read"
  );
  assert_eq!(members.owners().count(), 1);
  let plan = alice.servers.make_local_plan(alice.root()).expect("plan");
  assert_eq!((plan.tasks, plan.not_shared), (3, 0));
  assert!(plan.releases && plan.member);
  assert_eq!(plan.claims, Some(Vec::new()));

  drop(server);
  let offline = alice.block_on(alice.server().current_members()).expect("as last read");
  assert_eq!(offline.read, MembersRead::Unreachable);
  assert_eq!(offline.members, members.members);
  assert_eq!(offline.read_at, members.read_at);
}

#[test]
fn links_make_a_project_local_once_this_computer_signed_out_of_its_server() {
  let server = TestServer::start();
  let alice = shared(&server);
  alice
    .block_on(alice.server().current_members())
    .expect("members read once");
  let entry = alice.entry();
  alice
    .block_on(alice.servers.sign_out(&entry))
    .expect("signed out of the server");
  let members = alice
    .block_on(alice.servers.project_members(alice.root()))
    .expect("as last read");
  assert_eq!(members.read, MembersRead::NoServer);
  let plan = alice.servers.make_local_plan(alice.root()).expect("plan");
  assert!(!plan.releases, "nothing can be released without a sign-in");
  assert_eq!(plan.claims, None, "this computer no longer knows whose claims they are");
  let made = alice.block_on(alice.servers.make_local(alice.root())).expect("local");
  assert_eq!(made.tasks, 3);
  assert_eq!(made.claims, ClaimRelease::Unknown);
  assert!(alice.db().sharing().expect("link").is_none());
  assert!(alice.db().shared_members().expect("members").is_none());
  assert_eq!(alice.tasks().len(), 3);
}

/// A link request that writes no discovery file.
fn request() -> LinkRequest {
  LinkRequest {
    remote: None,
    key: IdempotencyKey::generate(),
    write_discovery: false,
  }
}

/// Adds a local task titled `title` to `person`'s project, as another screen or MCP would.
fn add_task(person: &Person, title: &str) {
  person
    .db()
    .create_tasks(IdempotencyKey::generate(), vec![details(title)], Timestamp::now())
    .expect("a task added meanwhile");
}

#[test]
fn links_refuse_an_upload_when_a_task_was_added_after_the_plan() {
  let server = TestServer::start();
  let code = server.code_for("alice", true);
  let alice = Person::signed_in(&server, ("alice", "Alice Moreau", &code), &["One", "Two"]);
  let project = server.create_project("kontur", None);
  let plan = alice
    .block_on(alice.servers.link_plan(&alice.entry(), alice.root(), kontur(&alice)))
    .expect("plan");
  add_task(&alice, "Three");
  let refused = alice.block_on(
    alice
      .servers
      .link_checkout(&alice.entry(), alice.root(), &plan, request()),
  );
  assert!(
    matches!(refused, Err(ShareError::Sharing(SharingError::ProjectChanged))),
    "{refused:?}"
  );
  assert!(server.task_keys(&project).is_empty(), "nothing went up");
  assert!(alice.db().sharing().expect("link").is_none(), "the project stays local");
  assert!(
    !alice.servers.share_in_progress(alice.root()),
    "its tasks take changes again"
  );

  let plan = alice
    .block_on(alice.servers.link_plan(&alice.entry(), alice.root(), kontur(&alice)))
    .expect("the plan read again");
  alice
    .block_on(
      alice
        .servers
        .link_checkout(&alice.entry(), alice.root(), &plan, request()),
    )
    .expect("linked as reviewed again");
  assert_eq!(server.task_keys(&project), ["ORC-1", "ORC-2", "ORC-3"]);
}

#[test]
fn links_refuse_a_download_when_a_task_was_added_after_the_plan() {
  let server = TestServer::start();
  let alice = shared(&server);
  let maria = colleague(&server, &alice, "maria", &[]);
  let plan = maria
    .block_on(maria.servers.link_plan(&maria.entry(), maria.root(), kontur(&maria)))
    .expect("plan");
  assert_eq!(plan.kind, LinkKind::Download { tasks: 3 });
  add_task(&maria, "Mine");
  let refused = maria.block_on(
    maria
      .servers
      .link_checkout(&maria.entry(), maria.root(), &plan, request()),
  );
  assert!(
    matches!(refused, Err(ShareError::Sharing(SharingError::ProjectChanged))),
    "{refused:?}"
  );
  assert!(
    maria.db().sharing().expect("link").is_none(),
    "the checkout stays local"
  );
  let titles: Vec<String> = maria
    .tasks()
    .iter()
    .map(|task| task.details().title.as_str().to_owned())
    .collect();
  assert_eq!(
    titles,
    ["Mine"],
    "the task added meanwhile is not set aside as Not shared"
  );
  assert!(maria.db().not_shared_tasks().expect("not shared").is_empty());
  assert_eq!(server.task_keys(&kontur(&alice)).len(), 3, "nothing went up");
}

#[test]
fn links_show_the_members_as_last_read_once_not_a_member_or_without_the_server() {
  let server = TestServer::start();
  let alice = shared(&server);
  let maria = colleague(&server, &alice, "maria", &[]);
  let plan = maria
    .block_on(maria.servers.link_plan(&maria.entry(), maria.root(), kontur(&maria)))
    .expect("plan");
  maria
    .block_on(
      maria
        .servers
        .link_checkout(&maria.entry(), maria.root(), &plan, request()),
    )
    .expect("linked");
  let live = maria.block_on(maria.server().current_members()).expect("members");
  assert!(live.live());

  server.remove_member(&kontur(&alice), "maria");
  assert!(maria.server().link().removed_at.is_none(), "not synced since");
  let removed = maria.block_on(maria.server().current_members()).expect("as last read");
  assert_eq!(removed.read, MembersRead::NotMember);
  assert_eq!(removed.members, live.members);

  // Another application database, without the project's server in its list.
  let data = tempfile::TempDir::new().expect("application data");
  let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
  let elsewhere = Servers::new(workspace, Vault::with(Arc::new(MemoryVault::default())));
  let missing = maria
    .block_on(elsewhere.project_members(maria.root()))
    .expect("as last read");
  assert_eq!(missing.read, MembersRead::NoServer);
  assert_eq!(missing.members, live.members);
}

#[test]
fn links_say_which_claims_were_not_released_when_the_server_dropped_meanwhile() {
  let server = TestServer::start();
  let alice = shared(&server);
  let task = alice.tasks()[0].id();
  let claim = alice
    .block_on(alice.servers.claim_for_work(alice.root(), task))
    .expect("claim");
  assert!(claim.newly_taken());
  let plan = alice.servers.make_local_plan(alice.root()).expect("plan");
  assert!(plan.releases);
  let claims: Vec<String> = plan.claims.expect("known").iter().map(ToString::to_string).collect();
  assert_eq!(claims, ["ORC-1"]);

  drop(server);
  let made = alice.block_on(alice.servers.make_local(alice.root())).expect("local");
  let ClaimRelease::Stale { tasks } = made.claims else {
    panic!("the claim could not be released: {:?}", made.claims);
  };
  let stale: Vec<String> = tasks.iter().map(ToString::to_string).collect();
  assert_eq!(stale, ["ORC-1"]);
  assert_eq!(alice.tasks().len(), 3);
}

#[test]
fn links_use_a_project_address_only_on_the_server_that_reported_it() {
  let server = ServerAddress::parse("https://orchester.kontur.dev").expect("server");
  let kept = ProjectAddress::reported("https://orchester.kontur.dev/kontur", &server).expect("on its server");
  assert_eq!(kept.to_text(), "https://orchester.kontur.dev/kontur");
  for elsewhere in [
    "https://evil.example/kontur",
    "http://orchester.kontur.dev/kontur",
    "https://orchester.kontur.dev:8443/kontur",
    "https://orchester.kontur.dev/team/kontur",
  ] {
    assert!(
      matches!(
        ProjectAddress::reported(elsewhere, &server),
        Err(SharingError::ForeignAddress)
      ),
      "{elsewhere} is not on {server}"
    );
  }
  assert!(matches!(
    ProjectAddress::reported("not an address", &server),
    Err(SharingError::InvalidAnswer(_))
  ));
}
