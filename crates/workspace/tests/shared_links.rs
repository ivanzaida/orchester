//! N2's domain rules against a real `orchester-server` on loopback: what linking a checkout would do before the person
//! confirms it (an upload that keeps numbers, the server's tasks coming down to a checkout without tasks, or a join
//! to review), the link itself, the members kept as last read for when the server cannot be reached, what Make local
//! would do, and Make local once this computer signed out of the server.
//!
//! `cargo test -p orchester-workspace --test shared_links links_`

#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use orchester_core::id::IdempotencyKey;
use orchester_workspace::sharing::project::{
  discovery::DiscoveryState,
  first_link::{LinkKind, LinkRequest},
};
use server_support::TestServer;
use support::{Person, shared};

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
fn links_keep_the_members_as_last_read_and_plan_make_local() {
  let server = TestServer::start();
  let alice = shared(&server);
  let members = alice.block_on(alice.server().current_members()).expect("members");
  assert!(members.live);
  assert_eq!(members.owners().count(), 1);
  let plan = alice.servers.make_local_plan(alice.root()).expect("plan");
  assert_eq!((plan.tasks, plan.not_shared), (3, 0));
  assert!(plan.releases && plan.member && plan.claims.is_empty());

  drop(server);
  let offline = alice.block_on(alice.server().current_members()).expect("as last read");
  assert!(!offline.live);
  assert_eq!(offline.members, members.members);
  assert_eq!(offline.read_at, members.read_at);
}

#[test]
fn links_make_a_project_local_once_this_computer_signed_out_of_its_server() {
  let server = TestServer::start();
  let alice = shared(&server);
  let entry = alice.entry();
  alice
    .block_on(alice.servers.sign_out(&entry))
    .expect("signed out of the server");
  let plan = alice.servers.make_local_plan(alice.root()).expect("plan");
  assert!(!plan.releases, "nothing can be released without a sign-in");
  let made = alice.block_on(alice.servers.make_local(alice.root())).expect("local");
  assert_eq!(made.tasks, 3);
  assert!(alice.db().sharing().expect("link").is_none());
  assert!(alice.db().shared_members().expect("members").is_none());
  assert_eq!(alice.tasks().len(), 3);
}
