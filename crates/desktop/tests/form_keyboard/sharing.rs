//! Project settings › Sharing (10.11–10.14, 10.17) and the shared task list (10.42, 10.44) against a
//! real `orchester-server` on loopback: the repository's identity and what the server has for it,
//! *Share on…* with its confirmation and the discovery file, the members and their roles with the
//! last owner refused, *Make local* keeping every task, the offer of the repository's
//! `orchester.toml` that contacts nothing before the person asks, the read-only list while the
//! server cannot be reached, and the list of a person who is no longer a member.

use std::sync::Arc;

use orchester_api::{
  MemberRole,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{TaskDetails, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  agent_permissions::shows,
  harness::Desktop,
  servers::{add_server, server_support::TestServer, servers_page, sign_in_first_time, type_into},
};

const ORIGIN: &str = "https://github.com/ivanzaida/kontur.git";

/// git of a clone of Kontur: its remotes, and whether `orchester.toml` is committed (it is not).
struct Clone;

impl CommandRunner for Clone {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let output = |code, stdout: &str| CommandOutput {
      code: Some(code),
      stdout: stdout.to_owned(),
      stderr: String::new(),
    };
    Ok(match command.args().first().map(String::as_str) {
      Some("remote") => output(0, &format!("origin\t{ORIGIN} (fetch)\norigin\t{ORIGIN} (push)\n")),
      Some("status") => output(0, "?? orchester.toml\n"),
      _ => output(1, ""),
    })
  }
}

/// Kontur in a clone, with `tasks` drafts, and `username` (a first admin when `admin`) signed in to
/// a fresh server.
pub(crate) fn signed_in(username: &str, admin: bool, tasks: &[&str]) -> (Desktop, TestServer) {
  let server = TestServer::start();
  if !admin {
    // The server takes other users once it has its first admin.
    server.create_user("root", "Root", true);
  }
  let first = server.create_user(username, "Alice Moreau", admin);
  let desktop = signed_in_on(&server, (username, &first), tasks);
  (desktop, server)
}

/// Kontur in a clone, with `tasks` drafts, and `username` signed in to `server` with their first
/// password `first`, choosing their own.
pub(crate) fn signed_in_on(
  server: &TestServer,
  (username, first): (&str, &orchester_api::Credential),
  tasks: &[&str],
) -> Desktop {
  signed_in_via(&server.url(), (username, first), tasks)
}

/// [`signed_in_on`] through `url`, e.g. a port in front of the server.
pub(crate) fn signed_in_via(
  url: &str,
  (username, first): (&str, &orchester_api::Credential),
  tasks: &[&str],
) -> Desktop {
  let mut desktop = Desktop::with_project_and_programs(Arc::new(Clone));
  if !tasks.is_empty() {
    ProjectDb::open(desktop.repo())
      .expect("project")
      .create_tasks(
        IdempotencyKey::generate(),
        tasks.iter().map(|title| draft(title)).collect(),
        Timestamp::now(),
      )
      .expect("tasks");
  }
  servers_page(&mut desktop);
  add_server(&mut desktop, url);
  sign_in_first_time(&mut desktop, username, first.expose(), "alice's own long password");
  desktop
}

fn draft(title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  }
}

pub(crate) fn sharing_tab(desktop: &mut Desktop) {
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for("settings-tab-sharing");
  desktop.click("settings-tab-sharing");
  desktop.wait_for("settings-sharing");
}

/// Shares the project from its Sharing tab.
pub(crate) fn share(desktop: &mut Desktop) {
  sharing_tab(desktop);
  desktop.wait_until("the repository was asked about", |desktop| {
    shows(desktop, "not shared there yet")
  });
  desktop.click("sharing-share");
  desktop.wait_for("share-dialog");
  desktop.click("share-confirm");
  desktop.wait_for("sharing-members");
}

/// Another computer's client of `server`, signed in as `username` with their own password.
pub(crate) fn elsewhere(
  server: &TestServer,
  username: &str,
  name: &str,
) -> (Client, orchester_core::secret::SecretValue) {
  let first = server.create_user(username, name, false);
  let token = server.sign_in_elsewhere(username, first.expose(), &format!("{username}'s own long password"));
  let client = Client::new(
    ServerAddress::parse(&server.url()).expect("address"),
    PlainHttp::NotAllowed,
  )
  .expect("client");
  (client, token)
}

pub(crate) fn add_member(desktop: &mut Desktop, username: &str) {
  type_into(desktop, "sharing-member-username", username);
  desktop.click("sharing-member-add");
}

fn member_id(desktop: &mut Desktop, name: &str) -> String {
  let found = std::cell::RefCell::new(None);
  desktop.tree.find_element(|element| {
    if let Some(id) = element
      .id()
      .filter(|id| id.starts_with("sharing-member-") && id.len() > 40)
      && crate::agent_permissions::contains_text(element, name)
    {
      found
        .borrow_mut()
        .get_or_insert_with(|| id.trim_start_matches("sharing-member-").to_owned());
    }
    false
  });
  found.into_inner().expect("the member's row")
}

#[test]
fn sharing_share_uploads_the_project_and_writes_the_discovery_file() {
  let (mut desktop, _server) = signed_in("alice", true, &["Repository switcher", "Fix reconnect"]);
  sharing_tab(&mut desktop);
  desktop.wait_for("sharing-identity");
  assert!(shows(&mut desktop, "github.com/ivanzaida/kontur"));
  assert!(shows(&mut desktop, "Local only"));
  desktop.wait_until("the repository was asked about", |desktop| {
    shows(desktop, "checked on Kontur team: not shared there yet")
  });
  desktop.click("sharing-share");
  desktop.wait_for("share-dialog");
  crate::modal_layer::assert_dialog_in_screen(&mut desktop, "share-dialog");
  assert!(shows(&mut desktop, "Share Kontur on Kontur team?"));
  assert!(shows(&mut desktop, "2 · their numbers are kept, KON-1 … KON-2"));
  assert!(shows(&mut desktop, "Continue at KON-3, numbered by the server"));
  desktop.click("share-confirm");
  desktop.wait_for("sharing-members");
  assert!(shows(&mut desktop, "Shared on Kontur team"));
  assert!(shows(&mut desktop, "you are an owner"));
  desktop.wait_for("sharing-discovery-file");
  assert!(shows(&mut desktop, "Not committed yet · commit it like any file"));
  assert!(desktop.repo().join("orchester.toml").is_file());
  let db = ProjectDb::open(desktop.repo()).expect("project");
  assert!(db.sharing().expect("link").is_some());
  assert_eq!(db.tasks().expect("tasks").len(), 2);
}

#[test]
fn sharing_owners_add_members_and_change_roles() {
  let (mut desktop, server) = signed_in("alice", true, &[]);
  let _bohdan = elsewhere(&server, "bohdan", "Bohdan Koval");
  share(&mut desktop);
  add_member(&mut desktop, "maria.l");
  desktop.wait_for("sharing-member-error");
  assert!(shows(&mut desktop, "Kontur team has no user maria.l"));
  crate::servers::clear(&mut desktop, "sharing-member-username", "maria.l");
  desktop.type_text("bohdan");
  desktop.click("sharing-member-add");
  desktop.wait_until("Bohdan's row", |desktop| shows(desktop, "Bohdan Koval"));
  let bohdan = member_id(&mut desktop, "Bohdan Koval");
  desktop.click(&format!("sharing-member-make-owner-{bohdan}"));
  desktop.wait_for("make-owner-dialog");
  assert!(shows(&mut desktop, "Make Bohdan Koval an owner of KON?"));
  desktop.click("make-owner-confirm");
  desktop.wait_for(&format!("sharing-member-make-member-{bohdan}"));

  let alice = member_id(&mut desktop, "Alice Moreau");
  desktop.click(&format!("sharing-member-make-member-{alice}"));
  desktop.wait_for("make-self-member-dialog");
  desktop.wait_until("the other owner named", |desktop| {
    shows(desktop, "Bohdan Koval stays an owner")
  });
  desktop.click("make-self-member-confirm");
  // Alice is the server's admin: she still manages the members, as a member of KON.
  desktop.wait_until("Alice's role", |desktop| shows(desktop, "you are a member"));
  assert_eq!(
    ProjectDb::open(desktop.repo())
      .expect("project")
      .sharing()
      .expect("link")
      .expect("shared")
      .role,
    Some(MemberRole::Member)
  );
}

#[test]
fn sharing_the_last_owner_cannot_leave_and_removing_asks() {
  let (mut desktop, server) = signed_in("alice", true, &[]);
  let _bohdan = elsewhere(&server, "bohdan", "Bohdan Koval");
  share(&mut desktop);
  add_member(&mut desktop, "bohdan");
  desktop.wait_until("Bohdan's row", |desktop| shows(desktop, "Bohdan Koval"));
  let alice = member_id(&mut desktop, "Alice Moreau");
  desktop.click(&format!("sharing-member-leave-{alice}"));
  desktop.wait_for("leave-dialog");
  desktop.wait_until("the last owner refused", |desktop| {
    shows(desktop, "You are the last owner of KON")
  });
  desktop.wait_for("leave-make-owner");
  desktop.click("leave-cancel");

  let bohdan = member_id(&mut desktop, "Bohdan Koval");
  desktop.click(&format!("sharing-member-remove-{bohdan}"));
  desktop.wait_for("remove-member-dialog");
  assert!(shows(&mut desktop, "Remove Bohdan Koval from KON?"));
  desktop.click("remove-member-confirm");
  desktop.settle("Bohdan removed", |tree| {
    tree.get_element_by_id("remove-member-dialog").is_none()
  });
  desktop.wait_until("the row is gone", |desktop| !shows(desktop, "Bohdan Koval"));
}

#[test]
fn sharing_make_local_keeps_every_task() {
  let (mut desktop, _server) = signed_in("alice", true, &["Repository switcher", "Fix reconnect"]);
  share(&mut desktop);
  desktop.click("sharing-make-local");
  desktop.wait_for("make-local-dialog");
  assert!(shows(&mut desktop, "Make Kontur local on this computer?"));
  assert!(shows(&mut desktop, "All 2 stay as local tasks with their numbers"));
  desktop.click("make-local-confirm");
  desktop.wait_until("the list is local again", |desktop| shows(desktop, "Local only"));
  let db = ProjectDb::open(desktop.repo()).expect("project");
  assert_eq!(db.sharing().expect("link"), None);
  assert_eq!(db.tasks().expect("tasks").len(), 2);
}

#[test]
fn sharing_the_discovery_file_offers_sign_in_without_contacting_its_server() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let mut desktop = Desktop::with_project_and_programs(Arc::new(Clone));
  std::fs::write(
    desktop.repo().join("orchester.toml"),
    format!("server = \"{}\"\n", server.url()),
  )
  .expect("discovery file");
  sharing_tab(&mut desktop);
  desktop.wait_for("sharing-discovery-offer");
  assert!(shows(&mut desktop, "orchester.toml names a server for this repository"));
  assert!(shows(&mut desktop, "Orchester hasn't contacted it"));
  desktop.click("sharing-discovery-start");
  desktop.wait_for("add-server-address");
  assert_eq!(desktop.value("add-server-address"), server.url());
  assert!(
    desktop.tree.get_element_by_id("add-server-found").is_none(),
    "nothing is checked before the person asks"
  );
  desktop
    .tree
    .get_element_by_id_mut("add-server-address")
    .expect("address")
    .focus();
  desktop.pass();
  desktop.enter();
  desktop.wait_for("add-server-found");
  // Signed in, the person is back on the Sharing tab to share.
  desktop.enter();
  desktop.wait_for("sign-in-username");
  type_into(&mut desktop, "sign-in-username", "alice");
  type_into(&mut desktop, "sign-in-password", first.expose());
  desktop.enter();
  desktop.wait_for("choose-password-new");
  type_into(&mut desktop, "choose-password-new", "alice's own long password");
  type_into(&mut desktop, "choose-password-repeat", "alice's own long password");
  desktop.enter();
  desktop.wait_for("settings-sharing");
  desktop.wait_for("sharing-share");
}

#[test]
fn sharing_offline_the_list_is_read_only() {
  let (mut desktop, server) = signed_in("alice", true, &["Repository switcher"]);
  share(&mut desktop);
  drop(server);
  // Reading the members fails: the server is being reconnected to.
  desktop.click("settings-tab-general");
  desktop.click("settings-tab-sharing");
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-shared-notice");
  assert!(shows(&mut desktop, "Offline — read-only"));
  assert!(shows(
    &mut desktop,
    "Search, filters and opening tasks work while read-only."
  ));
  desktop.wait_for("tasks-shared-try-now");
}

#[test]
fn sharing_a_removed_member_s_list_is_read_only_with_make_local() {
  let (mut desktop, server) = signed_in("alice", true, &["Repository switcher"]);
  let (client, oleh) = elsewhere(&server, "oleh", "Oleh Petrenko");
  share(&mut desktop);
  add_member(&mut desktop, "oleh");
  desktop.wait_until("Oleh's row", |desktop| shows(desktop, "Oleh Petrenko"));
  let oleh_id = member_id(&mut desktop, "Oleh Petrenko");
  desktop.click(&format!("sharing-member-make-owner-{oleh_id}"));
  desktop.click("make-owner-confirm");
  desktop.wait_for(&format!("sharing-member-make-member-{oleh_id}"));
  let link = ProjectDb::open(desktop.repo())
    .expect("project")
    .sharing()
    .expect("link")
    .expect("shared");
  let alice = member_id(&mut desktop, "Alice Moreau").parse().expect("user id");
  server
    .block_on(client.remove_member(&oleh, link.project, alice))
    .expect("Oleh removes Alice");
  desktop.click("settings-tab-general");
  desktop.click("settings-tab-sharing");
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-shared-notice");
  assert!(shows(&mut desktop, "You are no longer a member of KON on Kontur team"));
  desktop.wait_for("tasks-shared-make-local");
  desktop.wait_for("tasks-shared-check-again");
  desktop.wait_until("the status bar", |desktop| shows(desktop, "Not a member of KON"));
}

/// 10.14 B: a member who is not the server's admin sees the members without adding, removing or
/// roles, and *Leave* on their own row.
#[test]
fn sharing_a_member_sees_the_members_and_leaves_from_their_row() {
  let (mut desktop, server) = signed_in("bohdan", false, &[]);
  let _oleh = elsewhere(&server, "oleh", "Oleh Petrenko");
  share(&mut desktop);
  add_member(&mut desktop, "oleh");
  desktop.wait_until("Oleh's row", |desktop| shows(desktop, "Oleh Petrenko"));
  let oleh = member_id(&mut desktop, "Oleh Petrenko");
  desktop.click(&format!("sharing-member-make-owner-{oleh}"));
  desktop.click("make-owner-confirm");
  desktop.wait_for(&format!("sharing-member-make-member-{oleh}"));
  let me = member_id(&mut desktop, "Alice Moreau");
  desktop.click(&format!("sharing-member-make-member-{me}"));
  desktop.wait_for("make-self-member-dialog");
  desktop.click("make-self-member-confirm");
  desktop.settle("a member's view", |tree| {
    tree.get_element_by_id("sharing-member-add").is_none()
  });
  assert!(shows(
    &mut desktop,
    "Only owners of KON and admins of Kontur team add or remove members"
  ));
  assert!(
    desktop
      .tree
      .get_element_by_id(&format!("sharing-member-remove-{oleh}"))
      .is_none()
  );
  desktop.click(&format!("sharing-member-leave-{me}"));
  desktop.wait_for("leave-dialog");
  desktop.wait_until("leaving is possible", |desktop| {
    shows(desktop, "You can no longer read or change KON")
  });
  desktop.click("leave-confirm");
  desktop.wait_until("no longer a member", |desktop| {
    ProjectDb::open(desktop.repo())
      .expect("project")
      .sharing()
      .expect("link")
      .is_some_and(|link| link.removed_at.is_some())
  });
}

/// A run's status Orchester did not send because the server's changed meanwhile says so, quietly,
/// on the task until the person dismisses it.
#[test]
fn sharing_a_run_status_that_was_not_sent_shows_on_the_task_until_dismissed() {
  let (mut desktop, _server) = signed_in("alice", true, &["Repository switcher"]);
  share(&mut desktop);
  let task = ProjectDb::open(desktop.repo())
    .expect("project")
    .tasks()
    .expect("tasks")
    .remove(0);
  let status = |status: WorkflowStatus| serde_json::json!({ "schema": 1, "body": status }).to_string();
  // As the desktop leaves it after the server's status changed under a run's.
  rusqlite::Connection::open(desktop.repo().join(".orchester").join("orchester.db"))
    .expect("database")
    .execute(
      "INSERT INTO shared_automatic_status (task_id, from_status, status, changed_at, not_sent, server_status) \
       VALUES (?1, ?2, ?3, ?4, 'changed', ?5)",
      rusqlite::params![
        task.id().to_string(),
        status(WorkflowStatus::Todo),
        status(WorkflowStatus::InReview),
        Timestamp::now().unix_millis(),
        status(WorkflowStatus::Canceled)
      ],
    )
    .expect("the unsent status");
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-row-KON-1");
  desktop.wait_for("task-unsent-status");
  assert!(shows(&mut desktop, "A run's status was not sent"));
  assert!(shows(&mut desktop, "changed meanwhile to Canceled"));
  desktop.click("task-unsent-status-dismiss");
  desktop.settle("the notice is gone", |tree| {
    tree.get_element_by_id("task-unsent-status").is_none()
  });
  assert_eq!(
    ProjectDb::open(desktop.repo())
      .expect("project")
      .unsent_status(task.id())
      .expect("unsent"),
    None
  );
}

/// 10.14 B after the server drops the person's admin flag: the Sharing tab follows what the server
/// answers now, so a plain member sees the members without adding, removing or roles.
#[test]
fn sharing_a_dropped_admin_flag_leaves_the_member_s_view() {
  let (mut desktop, server) = signed_in("alice", true, &[]);
  let first = server.create_user("root", "Root", true);
  let root = server.sign_in_elsewhere("root", first.expose(), "root's own long password");
  let _oleh = elsewhere(&server, "oleh", "Oleh Petrenko");
  share(&mut desktop);
  add_member(&mut desktop, "oleh");
  desktop.wait_until("Oleh's row", |desktop| shows(desktop, "Oleh Petrenko"));
  let oleh = member_id(&mut desktop, "Oleh Petrenko");
  desktop.click(&format!("sharing-member-make-owner-{oleh}"));
  desktop.click("make-owner-confirm");
  desktop.wait_for(&format!("sharing-member-make-member-{oleh}"));
  let me = member_id(&mut desktop, "Alice Moreau");
  desktop.click(&format!("sharing-member-make-member-{me}"));
  desktop.wait_for("make-self-member-dialog");
  desktop.click("make-self-member-confirm");
  desktop.wait_for(&format!("sharing-member-leave-{me}"));
  assert!(
    desktop.tree.get_element_by_id("sharing-member-add").is_some(),
    "an admin of the server still manages"
  );
  server.set_admin(&root, "alice", false);
  desktop.click("settings-tab-general");
  desktop.click("settings-tab-sharing");
  desktop.wait_for("sharing-members");
  desktop.wait_until("the member's view", |desktop| {
    desktop.tree.get_element_by_id("sharing-member-add").is_none()
      && desktop
        .tree
        .get_element_by_id(&format!("sharing-member-leave-{me}"))
        .is_some()
  });
  assert!(
    desktop
      .tree
      .get_element_by_id(&format!("sharing-member-remove-{oleh}"))
      .is_none()
  );
}

/// An error belongs to the change it reports: the next dialog opens without it (10.14 C).
#[test]
fn sharing_a_form_error_does_not_follow_into_the_next_dialog() {
  let (mut desktop, server) = signed_in("alice", true, &[]);
  let _oleh = elsewhere(&server, "oleh", "Oleh Petrenko");
  share(&mut desktop);
  add_member(&mut desktop, "oleh");
  desktop.wait_until("Oleh's row", |desktop| shows(desktop, "Oleh Petrenko"));
  add_member(&mut desktop, "oleh");
  desktop.wait_for("sharing-member-error");
  assert!(shows(&mut desktop, "already a member"));
  let oleh = member_id(&mut desktop, "Oleh Petrenko");
  desktop.click(&format!("sharing-member-remove-{oleh}"));
  desktop.wait_for("remove-member-dialog");
  assert!(!shows(&mut desktop, "already a member"));
}
