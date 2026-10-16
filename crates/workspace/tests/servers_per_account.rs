//! Team servers per Orchester Cloud account (the person's decision, 2026-10-16), on one application database: each
//! account signed in sees and uses only its own servers, and another account's is not found; the servers joined while
//! no account was signed in go to the first one that signs in; the same server joined by two accounts is two entries,
//! two devices on the server and two tokens, which signing out of the cloud leaves in the keychain; nothing is linked
//! through another account's server; and the background sync keeps a hidden account's shared projects in step through
//! its own servers only.
//!
//! `cargo test -p orchester-workspace --test servers_per_account per_account_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "sharing_support/desktop.rs"]
mod desktop;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::{fs, path::PathBuf};

use account_cloud::{AccountCloud, EMAIL, OTHER_EMAIL, PASSWORD};
use desktop::Desktop;
use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, ServerId},
  name::Name,
  project::{Project, ProjectKey},
  repository::RepositoryIdentity,
  secret::{SecretValue, SecretVault, VaultKey},
  store::TaskStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  project_setup::NewProject,
  sharing::{
    CodeRequest, ProjectAddress, ServerEntry, SharingError, Trust,
    project::{LinkFirst, ProjectServer, ShareError, link::LinkProblem},
  },
  tasks::writer::{TaskWriter, WriteError},
};
use server_support::TestServer;
use support::{REMOTE, details};
use tempfile::TempDir;

/// Signs `desktop` out of the cloud, then in as `email`.
fn switch_to(desktop: &Desktop, email: &str) {
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  desktop
    .block_on(desktop.cloud.sign_in(email.into(), Credential::new(PASSWORD.into())))
    .expect("signed in");
}

/// `username` joins `server` from `desktop` with a code; the entry's id.
fn join(desktop: &Desktop, server: &TestServer, username: &str) -> ServerId {
  let code = server.code_for(username, false);
  let address = ServerAddress::parse(&server.url()).expect("address");
  let request = CodeRequest {
    code,
    username: Some(username.into()),
    display_name: Some(username.into()),
  };
  desktop
    .block_on(
      desktop
        .servers
        .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
    )
    .expect("join with a code")
}

fn entries(desktop: &Desktop) -> Vec<ServerEntry> {
  desktop.servers.entries().expect("entries")
}

fn token(desktop: &Desktop, server: ServerId) -> String {
  desktop
    .vault
    .read(&VaultKey::server_token(server))
    .expect("the server's token is in the keychain")
    .expose()
    .to_owned()
}

/// `name` set up in its own repository under `repos` with one task, as the signed-in account's; its id and folder.
fn add(desktop: &Desktop, repos: &TempDir, name: &str) -> (ProjectId, PathBuf) {
  let repo = repos.path().join(name);
  fs::create_dir_all(repo.join(".git")).expect("a repository");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("a key"),
    Name::new(name).expect("a name"),
  );
  let id = project.id();
  desktop
    .workspace
    .create(NewProject {
      project,
      repo_root: repo.clone(),
    })
    .expect("the project is set up");
  ProjectDb::open(&repo)
    .expect("project")
    .create_tasks(
      IdempotencyKey::generate(),
      vec![details("Fix reconnect")],
      Timestamp::now(),
    )
    .expect("a task");
  (id, repo)
}

fn link_first() -> LinkFirst {
  LinkFirst {
    slug: "kontur".into(),
    remote: Some(RepositoryIdentity::from_remote_url(REMOTE).expect("identity")),
    key: IdempotencyKey::generate(),
    write_discovery: false,
  }
}

fn title(root: &std::path::Path) -> String {
  ProjectDb::open(root).expect("project").tasks().expect("tasks")[0]
    .details()
    .title
    .as_str()
    .to_owned()
}

/// A team server; the first to join it is its admin.
fn team_server() -> TestServer {
  TestServer::start()
}

#[test]
fn per_account_each_sees_and_uses_only_its_own_servers() {
  let (cloud, server) = (AccountCloud::start(), team_server());
  let desktop = Desktop::signed_in(&cloud);
  let kontur = join(&desktop, &server, "bohdan");
  let bohdans = entries(&desktop).remove(0);
  assert_eq!(bohdans.stored.id, kontur);

  switch_to(&desktop, OTHER_EMAIL);
  assert!(entries(&desktop).is_empty(), "another account sees none of them");
  assert!(desktop.workspace.servers().expect("servers").is_empty());
  let asked = desktop.block_on(desktop.servers.me(&bohdans));
  assert!(matches!(asked, Err(SharingError::ServerMissing)), "{asked:?}");
  let projects = desktop.block_on(desktop.servers.server_projects(&bohdans));
  assert!(matches!(projects, Err(SharingError::ServerMissing)), "{projects:?}");
  let signed_out = desktop.block_on(desktop.servers.sign_out(&bohdans));
  assert!(matches!(signed_out, Err(SharingError::ServerMissing)), "{signed_out:?}");
  let pairing = desktop.block_on(desktop.servers.pairing_code(&bohdans));
  assert!(matches!(pairing, Err(SharingError::ServerMissing)), "{pairing:?}");
  assert!(
    server.accepts(&SecretValue::new(token(&desktop, kontur)).expect("a token")),
    "Bohdan's device stays signed in there"
  );

  switch_to(&desktop, EMAIL);
  let back = entries(&desktop);
  assert_eq!(back.len(), 1, "the first account finds its server again");
  assert_eq!(
    back[0].stored.account.as_ref().map(|account| account.username.as_str()),
    Some("bohdan")
  );
  desktop
    .block_on(desktop.servers.me(&back[0]))
    .expect("its token stayed in the keychain and works");
}

#[test]
fn per_account_servers_joined_signed_out_go_to_the_first_account_to_sign_in() {
  let (cloud, server) = (AccountCloud::start(), team_server());
  let desktop = Desktop::new(&cloud.url());
  let kontur = join(&desktop, &server, "olena");
  assert_eq!(entries(&desktop).len(), 1, "nobody's yet");

  desktop
    .block_on(
      desktop
        .cloud
        .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into())),
    )
    .expect("signed in");
  assert_eq!(
    entries(&desktop)
      .iter()
      .map(|entry| entry.stored.id)
      .collect::<Vec<_>>(),
    [kontur],
    "the first account took it"
  );
  switch_to(&desktop, EMAIL);
  assert!(entries(&desktop).is_empty(), "it stays the first account's");
}

#[test]
fn per_account_the_same_server_is_two_entries_with_two_tokens() {
  let (cloud, server) = (AccountCloud::start(), team_server());
  let desktop = Desktop::signed_in(&cloud);
  let bohdans = join(&desktop, &server, "bohdan");
  switch_to(&desktop, OTHER_EMAIL);
  let olenas = join(&desktop, &server, "olena");
  assert_ne!(bohdans, olenas, "Olena's own entry");
  let listed = entries(&desktop);
  assert_eq!(listed.len(), 1);
  assert_eq!(listed[0].stored.url, server.url());
  assert_eq!(
    listed[0]
      .stored
      .account
      .as_ref()
      .map(|account| account.username.as_str()),
    Some("olena")
  );
  let (bohdans_token, olenas_token) = (token(&desktop, bohdans), token(&desktop, olenas));
  assert_ne!(bohdans_token, olenas_token, "each account is its own device there");
  for value in [&bohdans_token, &olenas_token] {
    let value = SecretValue::new(value.clone()).expect("a token");
    assert!(server.accepts(&value), "both devices are signed in");
  }

  switch_to(&desktop, EMAIL);
  let listed = entries(&desktop);
  assert_eq!(listed.len(), 1);
  assert_eq!(listed[0].stored.id, bohdans);
  assert_eq!(
    listed[0]
      .stored
      .account
      .as_ref()
      .map(|account| account.username.as_str()),
    Some("bohdan"),
    "Olena's join changed nothing of Bohdan's"
  );
  assert_eq!(token(&desktop, bohdans), bohdans_token);
}

#[test]
fn per_account_nothing_is_linked_through_another_accounts_server() {
  let (cloud, server, repos) = (
    AccountCloud::start(),
    team_server(),
    TempDir::new().expect("repositories"),
  );
  let desktop = Desktop::signed_in(&cloud);
  join(&desktop, &server, "bohdan");
  let kontur = server.create_project("kontur", Some(REMOTE));
  let bohdans = entries(&desktop).remove(0);

  switch_to(&desktop, OTHER_EMAIL);
  let (_, ledger) = add(&desktop, &repos, "Ledger");
  let linked = desktop.block_on(desktop.servers.link_first(&bohdans, &ledger, link_first()));
  assert!(
    matches!(linked, Err(ShareError::Sharing(SharingError::ServerMissing))),
    "{linked:?}"
  );
  let review = desktop.block_on(desktop.servers.join_review_at(&bohdans, &ledger, "kontur"));
  assert!(matches!(review, Err(SharingError::ServerMissing)), "{:?}", review.err());
  let address = ProjectAddress::parse(&format!("{}/kontur", server.url())).expect("address");
  let target = desktop.block_on(desktop.servers.link_target(&address, None));
  assert!(
    matches!(target, Err(LinkProblem::NotConnected(_))),
    "{:?}",
    target.err()
  );
  assert!(
    ProjectDb::open(&ledger)
      .expect("project")
      .sharing()
      .expect("link")
      .is_none(),
    "Ledger stays local"
  );
  assert!(server.task_keys(&kontur).is_empty(), "nothing reached the server");
}

#[test]
fn per_account_sync_keeps_a_hidden_accounts_projects_in_step_and_shows_nothing() {
  let (cloud, server, repos) = (
    AccountCloud::start(),
    team_server(),
    TempDir::new().expect("repositories"),
  );
  let desktop = Desktop::signed_in(&cloud);
  join(&desktop, &server, "bohdan");
  let kontur_there = server.create_project("kontur", Some(REMOTE));
  let notes_there = server.create_named_project(("Notes", "NOT"), "notes", None);
  let bohdans = entries(&desktop).remove(0);
  let (_, kontur) = add(&desktop, &repos, "Kontur");
  desktop
    .block_on(desktop.servers.link_first(&bohdans, &kontur, link_first()))
    .expect("Bohdan links Kontur");
  // A checkout Bohdan linked and forgot, which Olena opens later: its link names Bohdan's server.
  let (notes_id, notes) = add(&desktop, &repos, "Notes");
  let notes_link = LinkFirst {
    slug: "notes".into(),
    remote: None,
    ..link_first()
  };
  desktop
    .block_on(desktop.servers.link_first(&bohdans, &notes, notes_link))
    .expect("Bohdan links Notes");
  desktop.workspace.forget(notes_id).expect("Bohdan forgets Notes");

  switch_to(&desktop, OTHER_EMAIL);
  desktop.workspace.open(&notes).expect("Olena opens the checkout");
  assert!(entries(&desktop).is_empty(), "Olena sees no server");
  let opened = ProjectServer::open(&desktop.servers, &kontur);
  assert!(matches!(opened, Err(SharingError::ServerMissing)), "{:?}", opened.err());
  let written = {
    let _runtime = desktop.runtime.enter();
    TaskWriter::open(&notes, &desktop.servers).err()
  };
  assert!(
    matches!(&written, Some(WriteError::Sharing(failure)) if matches!(failure.source, SharingError::ServerMissing)),
    "Olena's checkout reaches no server of Bohdan's: {written:?}"
  );

  let kontur_key = server.task_keys(&kontur_there).remove(0);
  server.retitle(&kontur_there, &kontur_key, "Fix reconnect after sleep");
  let notes_key = server.task_keys(&notes_there).remove(0);
  server.retitle(&notes_there, &notes_key, "Write the notes again");
  desktop.block_on(desktop.servers.sync_shared(true));
  assert_eq!(
    title(&kontur),
    "Fix reconnect after sleep",
    "Bohdan's Kontur keeps in step through his server while Olena is signed in"
  );
  assert_eq!(
    title(&notes),
    "Fix reconnect",
    "Olena's checkout is not synced through Bohdan's server"
  );
  assert!(entries(&desktop).is_empty(), "and the sync showed Olena nothing");
}
