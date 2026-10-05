//! Joining a repository that is shared already (10.15, 10.16 A/C, 10.21, 10.22, 10.23, 10.24,
//! 10.25, 10.26) against a real `orchester-server` on loopback: Alice shared Kontur as ORC from her
//! computer; Bohdan has the same repository with KON tasks. Not yet a member, Sharing names the
//! owners and his username, and *Check again* offers the join once he is added; Share meeting the
//! project leads to the same place. The review ticks every task; the join uploads the ticked ones
//! with new numbers, shows the mapping and keeps the unticked one *Not shared* behind the task
//! list's filter, read-only, until *Share task*; searching a former key finds the task.

use orchester_api::{MemberCreate, MemberRole, ProjectCreate, UploadTask, client::Client};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, TaskId},
  secret::SecretValue,
  store::TaskStore,
  task::{
    TaskStatus,
    organization::{Priority, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  agent_permissions::shows,
  harness::Desktop,
  servers::{server_support::TestServer, type_into},
  sharing::{elsewhere, sharing_tab, signed_in},
};

const ORIGIN: &str = "https://github.com/ivanzaida/kontur.git";

fn upload(title: &str, number: u32) -> UploadTask {
  UploadTask {
    id: TaskId::generate(),
    number: Some(number),
    former_key: None,
    title: title.to_owned(),
    brief: String::new(),
    criteria: Vec::new(),
    pipeline: None,
    limits: None,
    workflow: WorkflowStatus::Backlog,
    priority: Priority::None,
    tag_names: Vec::new(),
    blocking: Vec::new(),
    blocked_reason: None,
    status: TaskStatus::Draft,
    created_at: Timestamp::now().unix_millis(),
    claim: false,
    has_run: false,
  }
}

/// Alice's computer, which talks to the server's API directly.
struct Alice {
  client: Client,
  token: SecretValue,
  project: ProjectId,
}

/// Alice shares Kontur as ORC on `server` from her own computer, with two tasks.
fn alice_shares(server: &TestServer) -> Alice {
  let (client, token) = elsewhere(server, "alice", "Alice Moreau");
  let created = server
    .block_on(client.create_project(
      &token,
      &ProjectCreate {
        idempotency_key: IdempotencyKey::generate(),
        name: "Kontur".into(),
        key: "ORC".into(),
        remote: ORIGIN.into(),
        tasks: vec![upload("Repository switcher", 1), upload("Fix reconnect", 2)],
      },
    ))
    .expect("alice shares Kontur");
  Alice {
    client,
    token,
    project: created.project.id,
  }
}

/// Bohdan's Kontur (KON-1 *Add repository switcher*, KON-2 *Notes for the migration*), signed in
/// to a server where Alice shared the repository as ORC.
fn bohdan() -> (Desktop, TestServer, Alice) {
  let (desktop, server) = signed_in("bohdan", false, &["Add repository switcher", "Notes for the migration"]);
  let alice = alice_shares(&server);
  (desktop, server, alice)
}

fn add_bohdan(server: &TestServer, alice: &Alice) {
  server
    .block_on(alice.client.add_member(
      &alice.token,
      alice.project,
      &MemberCreate {
        username: "bohdan".into(),
        role: MemberRole::Member,
      },
    ))
    .expect("alice adds bohdan");
}

/// Joins ORC from the Sharing tab with KON-2 unticked.
fn join(desktop: &mut Desktop) {
  desktop.wait_for("sharing-review-join");
  desktop.click("sharing-review-join");
  desktop.wait_for("join-review-bar");
  desktop.click("join-tick-KON-2");
  desktop.wait_until("KON-2 stays Not shared", |desktop| shows(desktop, "1 of 2 ticked"));
  desktop.click("join-confirm");
  desktop.wait_for("sharing-joined");
}

#[test]
fn joining_a_person_who_is_not_a_member_sees_the_owners_and_checks_again() {
  let (mut desktop, server, alice) = bohdan();
  sharing_tab(&mut desktop);
  desktop.wait_for("sharing-not-member");
  assert!(shows(
    &mut desktop,
    "Kontur team has this repository, but you are not a member"
  ));
  assert!(shows(&mut desktop, "Alice Moreau"));
  desktop.wait_for("sharing-username");
  assert!(shows(&mut desktop, "bohdan"));
  assert!(
    shows(
      &mut desktop,
      "Share is not offered while the repository's project exists there"
    ),
    "Share is not offered"
  );
  add_bohdan(&server, &alice);
  desktop.click("sharing-check-again");
  desktop.wait_for("sharing-join-offer");
  assert!(shows(&mut desktop, "This repository is shared on Kontur team as ORC"));
  assert!(
    ProjectDb::open(desktop.repo())
      .expect("project")
      .sharing()
      .expect("link")
      .is_none()
  );
}

#[test]
fn joining_the_review_uploads_ticked_tasks_and_shows_the_mapping() {
  let (mut desktop, server, alice) = bohdan();
  add_bohdan(&server, &alice);
  sharing_tab(&mut desktop);
  desktop.wait_for("sharing-review-join");
  desktop.click("sharing-review-join");
  desktop.wait_for("join-review-bar");
  assert!(shows(&mut desktop, "Join ORC on Kontur team"));
  assert!(shows(&mut desktop, "2 of 2 ticked"));
  assert!(shows(
    &mut desktop,
    "Similar on ORC: ORC-1 Repository switcher · Alice Moreau"
  ));
  assert!(shows(&mut desktop, "Join and upload 2"));
  desktop.click("join-tick-KON-2");
  desktop.wait_until("the count follows the ticks", |desktop| {
    shows(desktop, "Join and upload 1")
  });
  assert!(shows(&mut desktop, "1 is uploaded as ORC-3"));
  desktop.click("join-confirm");
  desktop.wait_for("sharing-joined");
  assert!(shows(&mut desktop, "You joined ORC on Kontur team"));
  desktop.wait_for("sharing-mapping");
  assert!(shows(&mut desktop, "Keys before joining"));
  desktop.wait_for("sharing-mapping-KON-1");
  assert!(shows(&mut desktop, "ORC-3"));
  assert!(shows(&mut desktop, "read-only · Share task on its page"));
  let db = ProjectDb::open(desktop.repo()).expect("project");
  assert_eq!(db.project().key().as_str(), "ORC");
  let keys: Vec<String> = db
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3"]);
}

#[test]
fn joining_not_shared_tasks_wait_behind_the_filter_until_shared() {
  let (mut desktop, server, alice) = bohdan();
  add_bohdan(&server, &alice);
  sharing_tab(&mut desktop);
  join(&mut desktop);
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-not-shared");
  assert!(shows(&mut desktop, "Not shared · 1"));
  desktop.click("tasks-not-shared");
  desktop.wait_for("tasks-not-shared-notice");
  assert!(shows(&mut desktop, "1 task is Not shared"));
  desktop.wait_for("task-row-KON-2");
  desktop.click("task-row-KON-2");
  desktop.wait_for("task-not-shared-notice");
  assert!(shows(&mut desktop, "kept on this computer when you joined ORC"));
  assert!(shows(&mut desktop, "Not shared · this computer only"));
  desktop.click("task-share");
  desktop.wait_for("share-task-dialog");
  assert!(shows(&mut desktop, "Share KON-2 on ORC?"));
  desktop.click("share-task-confirm");
  desktop.wait_until("the task is ORC's now", |desktop| {
    !shows(desktop, "Share KON-2 on ORC?") && shows(desktop, "· was KON-2")
  });
  let db = ProjectDb::open(desktop.repo()).expect("project");
  assert!(db.not_shared_tasks().expect("kept").is_empty());
  assert!(
    shows(&mut desktop, "On Kontur team"),
    "the panel says how the task stands there"
  );
}

#[test]
fn joining_searching_a_former_key_finds_the_task() {
  let (mut desktop, server, alice) = bohdan();
  add_bohdan(&server, &alice);
  sharing_tab(&mut desktop);
  join(&mut desktop);
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-search");
  type_into(&mut desktop, "tasks-search", "KON-1");
  desktop.wait_until("only the former KON-1 is listed", |desktop| {
    shows(desktop, "was KON-1") && !shows(desktop, "Repository switcher")
  });
  desktop.wait_for("task-title-ORC-3");
}

#[test]
fn joining_share_meeting_the_project_leads_to_join() {
  let (mut desktop, server) = signed_in("bohdan", false, &["Add repository switcher"]);
  sharing_tab(&mut desktop);
  desktop.wait_until("the repository was asked about", |desktop| {
    shows(desktop, "not shared there yet")
  });
  // Alice shares a moment before Bohdan confirms.
  let alice = alice_shares(&server);
  add_bohdan(&server, &alice);
  desktop.click("sharing-share");
  desktop.wait_for("share-dialog");
  desktop.click("share-confirm");
  desktop.wait_for("sharing-join-offer");
  assert!(shows(&mut desktop, "Kontur team already has this repository as ORC"));
  assert!(shows(&mut desktop, "Nothing of yours was uploaded"));
  assert!(
    ProjectDb::open(desktop.repo())
      .expect("project")
      .sharing()
      .expect("link")
      .is_none()
  );
}

#[test]
fn joining_make_local_returns_not_shared_tasks_and_shows_their_keys() {
  let (mut desktop, server, alice) = bohdan();
  add_bohdan(&server, &alice);
  sharing_tab(&mut desktop);
  join(&mut desktop);
  desktop.click("sharing-make-local");
  desktop.wait_for("make-local-dialog");
  desktop.click("make-local-confirm");
  desktop.wait_for("sharing-made-local");
  assert!(shows(&mut desktop, "Kontur is local again"));
  desktop.wait_for("sharing-mapping-KON-2");
  assert!(shows(&mut desktop, "ORC-4"));
  let keys: Vec<String> = ProjectDb::open(desktop.repo())
    .expect("project")
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3", "ORC-4"]);
}

#[test]
fn joining_a_join_that_fails_on_the_way_keeps_the_ticks() {
  let (mut desktop, server, alice) = bohdan();
  add_bohdan(&server, &alice);
  sharing_tab(&mut desktop);
  desktop.wait_for("sharing-review-join");
  desktop.click("sharing-review-join");
  desktop.wait_for("join-review-bar");
  desktop.click("join-tick-KON-2");
  drop(server);
  desktop.click("join-confirm");
  desktop.wait_for("join-failed");
  assert!(shows(&mut desktop, "Couldn't join ORC"));
  assert!(shows(
    &mut desktop,
    "Nothing was uploaded and Kontur is still local with its KON tasks"
  ));
  assert!(shows(&mut desktop, "1 of 2 ticked"), "the ticks are kept");
  assert!(
    ProjectDb::open(desktop.repo())
      .expect("project")
      .sharing()
      .expect("link")
      .is_none()
  );
}
