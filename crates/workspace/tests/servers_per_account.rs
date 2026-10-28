//! Team servers per Orchester Cloud account (the person's decision, 2026-10-16), on one application database: each
//! account signed in sees and uses only its own servers, and another account's is not found; the servers joined while
//! no account was signed in go to the first one that signs in; the same server joined by two accounts is two entries,
//! two devices on the server and two tokens, which signing out of the cloud leaves in the keychain; nothing is linked
//! through another account's server; a cloud join or a code join that another sign-in overtakes keeps the server for
//! the account whose identity joined; a server nobody owns goes to the account whose projects link to it, never to the
//! first to sign in; and the background sync keeps a hidden account's shared projects in step through its own servers
//! only.
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

use account_cloud::{ACCOUNT, AccountCloud, EMAIL, OTHER_EMAIL, PASSWORD};
use desktop::{Desktop, linked_server};
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
    CloudJoined, CodeRequest, PresentedCertificate, ProjectAddress, ServerEntry, SharingError, Trust,
    cloud::ServerJoin,
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
    proof: Some(server.code_proof(username)),
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
  desktop.block_on(
    desktop
      .servers
      .sync_shared(true, &orchester_exec::programs::Programs::processes()),
  );
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

#[test]
fn per_account_a_cloud_join_overtaken_by_another_sign_in_stays_the_joining_accounts() {
  let cloud = AccountCloud::start();
  let (server, audience) = linked_server(&cloud);
  server.set_join_approval(false);
  let invite = server.email_invite(EMAIL);
  cloud.name_server(&audience, &server.url(), None);
  cloud.invite(&invite, None);
  let desktop = Desktop::signed_in(&cloud);
  let listed = desktop.block_on(desktop.cloud.invites()).expect("invites").remove(0);
  desktop
    .block_on(desktop.cloud.accept(&listed))
    .expect("Bohdan's account is on the server");
  let first = entries(&desktop).remove(0).stored.id;

  // Bohdan joins again with his account; Olena signs in while the cloud's registry is answering.
  cloud.hold_registry();
  let asked = cloud.registry_arrived();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let joining = {
    let cloud = desktop.cloud.clone();
    let id = audience.parse().expect("server id");
    desktop
      .runtime
      .spawn(async move { cloud.join_server(&address, Some(id)).await })
  };
  desktop.block_on(async {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while cloud.registry_arrived() == asked {
      assert!(tokio::time::Instant::now() < deadline, "the join reached the registry");
      tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
  });
  switch_to(&desktop, OTHER_EMAIL);
  cloud.let_registry_go();
  let joined = desktop.block_on(joining).expect("the join ran").expect("joined");
  assert!(
    matches!(joined, ServerJoin::Joined(CloudJoined::Joined(id)) if id == first),
    "Bohdan's own entry: {joined:?}"
  );
  assert!(entries(&desktop).is_empty(), "nothing of Bohdan's join is Olena's");

  switch_to(&desktop, EMAIL);
  let back = entries(&desktop);
  assert_eq!(back.len(), 1);
  desktop
    .block_on(desktop.servers.me(&back[0]))
    .expect("the new token is Bohdan's and works");
}

#[test]
fn per_account_a_code_join_overtaken_by_another_sign_in_stays_the_joining_accounts() {
  let cloud = AccountCloud::start();
  let (mut server, _) = linked_server(&cloud);
  // A code's token is asked for only over a connection whose certificate the server's key proof names.
  server.serve_self_signed();
  let desktop = Desktop::signed_in(&cloud);
  let request = CodeRequest {
    code: server.code_for("bohdan", false),
    username: Some("bohdan".into()),
    display_name: Some("Bohdan Koval".into()),
    proof: None,
  };

  // Bohdan enters a code; Olena signs in while the cloud answers the check of his sign-in for the code's proof.
  cloud.hold_checks();
  let asked = cloud.checks_arrived();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let trust = desktop
    .block_on(PresentedCertificate::read(&address))
    .expect("certificate")
    .confirm();
  let joining = {
    let cloud = desktop.cloud.clone();
    desktop
      .runtime
      .spawn(async move { cloud.join_with_code(&address, trust, request).await })
  };
  desktop.block_on(async {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while cloud.checks_arrived() == asked {
      assert!(tokio::time::Instant::now() < deadline, "the proof reached the cloud");
      tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
  });
  switch_to(&desktop, OTHER_EMAIL);
  cloud.let_checks_go();
  let joined = desktop.block_on(joining).expect("the join ran").expect("joined");
  assert!(entries(&desktop).is_empty(), "nothing of Bohdan's code join is Olena's");

  switch_to(&desktop, EMAIL);
  let back = entries(&desktop);
  assert_eq!(back.len(), 1);
  assert_eq!(back[0].stored.id, joined, "Bohdan's own entry");
  desktop
    .block_on(desktop.servers.me(&back[0]))
    .expect("the new token is Bohdan's and works");
}

#[test]
fn per_account_a_server_nobody_owns_goes_to_the_account_whose_projects_link_to_it() {
  let (cloud, repos) = (AccountCloud::start(), TempDir::new().expect("repositories"));
  let (linked, duplicated, unlinked) = (team_server(), team_server(), team_server());
  let desktop = Desktop::signed_in(&cloud);
  let kontur_server = join(&desktop, &linked, "bohdan");
  let kontur_there = linked.create_project("kontur", Some(REMOTE));
  let (_, kontur) = add(&desktop, &repos, "Kontur");
  desktop
    .block_on(desktop.servers.link_first(&entries(&desktop)[0], &kontur, link_first()))
    .expect("Bohdan links Kontur");
  let bohdans_duplicate = join(&desktop, &duplicated, "bohdan");
  let lab = join(&desktop, &unlinked, "bohdan");
  switch_to(&desktop, OTHER_EMAIL);
  let olenas = join(&desktop, &duplicated, "olena");
  // Bohdan's servers as after the update while nobody was signed in: nobody's yet.
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  desktop.disown_servers_of(ACCOUNT);

  desktop
    .block_on(
      desktop
        .cloud
        .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into())),
    )
    .expect("Olena signs in first");
  let mut olenas_list: Vec<ServerId> = entries(&desktop).iter().map(|entry| entry.stored.id).collect();
  olenas_list.sort();
  let mut expected = vec![olenas, lab];
  expected.sort();
  assert_eq!(
    olenas_list, expected,
    "Olena gets her own and the server no project links to, not the one Bohdan's Kontur links to"
  );
  assert!(
    desktop.vault.read(&VaultKey::server_token(bohdans_duplicate)).is_err(),
    "the duplicate of Olena's own entry went with its token"
  );
  assert!(desktop.vault.read(&VaultKey::server_token(kontur_server)).is_ok());

  switch_to(&desktop, EMAIL);
  let bohdans: Vec<ServerId> = entries(&desktop).iter().map(|entry| entry.stored.id).collect();
  assert_eq!(bohdans, [kontur_server], "Kontur's server is Bohdan's");
  let key = linked.task_keys(&kontur_there).remove(0);
  linked.retitle(&kontur_there, &key, "Fix reconnect after sleep");
  let project = ProjectServer::open(&desktop.servers, &kontur)
    .expect("Kontur's server is found")
    .expect("shared");
  desktop.block_on(project.sync()).expect("Kontur syncs");
  assert_eq!(title(&kontur), "Fix reconnect after sleep");
}

#[test]
fn per_account_a_server_nobody_owns_waits_while_a_project_cannot_be_read() {
  let (cloud, server, repos) = (
    AccountCloud::start(),
    team_server(),
    TempDir::new().expect("repositories"),
  );
  let desktop = Desktop::signed_in(&cloud);
  let kontur_server = join(&desktop, &server, "bohdan");
  server.create_project("kontur", Some(REMOTE));
  let (_, kontur) = add(&desktop, &repos, "Kontur");
  desktop
    .block_on(desktop.servers.link_first(&entries(&desktop)[0], &kontur, link_first()))
    .expect("Bohdan links Kontur");
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  desktop.disown_servers_of(ACCOUNT);
  // Kontur's folder is away (an unplugged drive) when Olena signs in for the first time.
  let away = repos.path().join("Kontur-away");
  fs::rename(&kontur, &away).expect("the folder is away");

  desktop
    .block_on(
      desktop
        .cloud
        .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into())),
    )
    .expect("Olena signs in first");
  assert!(entries(&desktop).is_empty(), "Olena does not get Bohdan's server");
  assert!(
    desktop.vault.read(&VaultKey::server_token(kontur_server)).is_ok(),
    "its token stays"
  );

  fs::rename(&away, &kontur).expect("the folder is back");
  switch_to(&desktop, EMAIL);
  let bohdans: Vec<ServerId> = entries(&desktop).iter().map(|entry| entry.stored.id).collect();
  assert_eq!(bohdans, [kontur_server], "Bohdan gets his server once Kontur reads");
  ProjectServer::open(&desktop.servers, &kontur)
    .expect("Kontur's server is found")
    .expect("shared");
}
