//! Local use without an Orchester Cloud account (the person's decisions, 2026-10-28 and 2026-10-30): a project added
//! while nobody is signed in is this computer's and no sign-in claims it; an account's waits while it is signed out;
//! another account sees this computer's and its own, never the first account's; a person moves an unshared project
//! between the two; and a project and its server always have the same owner.
//!
//! `cargo test -p orchester-workspace --test local_owner local_owner_`

#[allow(dead_code, reason = "shared with startup_join, which uses more of it")]
#[path = "startup_join/support.rs"]
mod support;

use orchester_api::{
  Credential,
  client::{CloudAddress, PlainHttp, ServerAddress},
  test_support::account_cloud::{AccountCloud, EMAIL, OTHER_EMAIL, PASSWORD},
};
use orchester_core::id::{ProjectId, ServerId};
use orchester_store::{RegisteredProject, StoreError};
use orchester_workspace::{
  sharing::SharingError,
  workspace::{MoveError, MoveTo},
};
use support::Desk;

fn use_cloud(desk: &Desk, cloud: &AccountCloud) {
  let address = ServerAddress::parse(&cloud.url()).expect("the stand-in cloud's address");
  desk
    .run(
      desk
        .cloud
        .set_address(Some(CloudAddress::new(address.clone(), Some(address)))),
    )
    .expect("a test cloud");
}

fn sign_in(desk: &Desk, email: &str) {
  desk
    .run(desk.cloud.sign_in(email.into(), Credential::new(PASSWORD.into())))
    .expect("signed in");
}

fn sign_out(desk: &Desk) {
  desk.run(desk.cloud.sign_out()).expect("signed out");
}

fn shown(desk: &Desk) -> Vec<String> {
  let mut names: Vec<String> = desk
    .workspace
    .projects()
    .expect("the list")
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect();
  names.sort();
  names
}

fn registered(desk: &Desk, name: &str) -> RegisteredProject {
  desk
    .workspace
    .projects()
    .expect("the list")
    .into_iter()
    .find(|project| project.name.as_str() == name)
    .expect("the project is shown")
}

fn add_server(desk: &Desk, host: &str) -> ServerId {
  let address = ServerAddress::parse(&format!("https://{host}")).expect("an address");
  desk
    .run(desk.servers.add(&address, host.to_owned(), PlainHttp::NotAllowed))
    .expect("the server is in the list")
}

#[test]
fn local_owner_a_fresh_computer_works_and_a_sign_in_claims_nothing() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let notes = desk.project("notes", "Notes");
  assert!(!desk.cloud.is_signed_in());
  assert_eq!(shown(&desk), ["Notes"]);
  assert_eq!(registered(&desk, "Notes").owner, None, "it is this computer's");

  sign_in(&desk, EMAIL);
  assert_eq!(shown(&desk), ["Notes"], "it shows after the sign-in");
  assert_eq!(registered(&desk, "Notes").owner, None, "and is not claimed");
  desk.workspace.open(&notes.repo_root).expect("opened signed in");
  assert_eq!(
    registered(&desk, "Notes").owner,
    None,
    "opening it claims nothing either"
  );

  desk.project("ledger", "Ledger");
  let account = desk.cloud.sign_in_state().account();
  assert_eq!(
    registered(&desk, "Ledger").owner,
    account,
    "added signed in: the account's"
  );
  assert_eq!(shown(&desk), ["Ledger", "Notes"], "both show");
}

#[test]
fn local_owner_signing_out_hides_the_accounts_projects_and_says_they_wait() {
  let cloud = AccountCloud::start();
  let desk = Desk::signed_in(&cloud);
  let ledger = desk.project("ledger", "Ledger");
  assert!(desk.workspace.waiting_projects().expect("what waits").is_empty());

  sign_out(&desk);
  assert!(shown(&desk).is_empty(), "only this computer's show");
  let waiting = desk.workspace.waiting_projects().expect("what waits");
  assert_eq!(waiting.len(), 1, "the account's project waits");
  let refused = desk.workspace.open(&ledger.repo_root);
  assert!(
    matches!(refused, Err(StoreError::ProjectOfSignedOutAccount(_))),
    "and does not open: {refused:?}"
  );
  assert!(
    matches!(desk.workspace.listing(), Ok(orchester_workspace::project_list::Listing::Projects(ref none)) if none.is_empty()),
    "a list that says what waits, no welcome"
  );

  sign_in(&desk, EMAIL);
  assert_eq!(shown(&desk), ["Ledger"], "it comes back with its account");
}

#[test]
fn local_owner_another_account_sees_this_computers_and_its_own() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  desk.project("notes", "Notes");
  sign_in(&desk, EMAIL);
  desk.project("ledger", "Ledger");

  sign_out(&desk);
  sign_in(&desk, OTHER_EMAIL);
  assert_eq!(shown(&desk), ["Notes"], "never the first account's");
  assert!(
    desk.workspace.waiting_projects().expect("what waits").is_empty(),
    "another account is told nothing of it"
  );
  desk.project("site", "Site");
  assert_eq!(shown(&desk), ["Notes", "Site"]);

  sign_out(&desk);
  sign_in(&desk, EMAIL);
  assert_eq!(shown(&desk), ["Ledger", "Notes"]);
}

#[test]
fn local_owner_a_person_moves_an_unshared_project_both_ways() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  desk.project("notes", "Notes");
  let notes = registered(&desk, "Notes");
  let refused = desk.servers.move_project(&notes, MoveTo::Account);
  assert!(matches!(refused, Err(MoveError::SignedOut)), "{refused:?}");

  sign_in(&desk, EMAIL);
  desk.servers.move_project(&notes, MoveTo::Account).expect("moved");
  assert_eq!(registered(&desk, "Notes").owner, desk.cloud.sign_in_state().account());
  sign_out(&desk);
  assert!(shown(&desk).is_empty(), "it now waits with the account");

  sign_in(&desk, EMAIL);
  desk.servers.move_project(&notes, MoveTo::Computer).expect("moved back");
  sign_out(&desk);
  assert_eq!(shown(&desk), ["Notes"], "it shows signed out again");
}

#[test]
fn local_owner_moves_are_refused_while_shared_and_without_a_folder() {
  let cloud = AccountCloud::start();
  let desk = Desk::signed_in(&cloud);
  let server = add_server(&desk, "orchester.kontur.lan");
  desk.linked_project("linked", "Linked", (server, ProjectId::generate()));
  let linked = registered(&desk, "Linked");
  let refused = desk.servers.move_project(&linked, MoveTo::Computer);
  assert!(matches!(refused, Err(MoveError::Shared)), "{refused:?}");

  let gone = desk.project("gone", "Gone");
  let project = registered(&desk, "Gone");
  std::fs::remove_dir_all(&gone.repo_root).expect("the folder goes");
  let refused = desk.servers.move_project(&project, MoveTo::Computer);
  assert!(matches!(refused, Err(MoveError::FolderMissing)), "{refused:?}");
  assert!(registered(&desk, "Gone").owner.is_some(), "nothing moved");
}

#[test]
fn local_owner_a_project_goes_with_its_server() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let computers = add_server(&desk, "code.kontur.lan");
  let notes = desk.project("notes", "Notes");
  desk
    .workspace
    .follow_server(notes.project.id(), computers)
    .expect("this computer's project on this computer's server");
  assert_eq!(
    registered(&desk, "Notes").owner,
    None,
    "joined with a code: it stays this computer's"
  );

  sign_in(&desk, EMAIL);
  let account = desk.cloud.sign_in_state().account();
  let accounts = add_server(&desk, "orchester.kontur.lan");
  assert_eq!(
    desk.workspace.servers_moving_project(notes.project.id()).expect("read"),
    [accounts],
    "the share page says so before it moves"
  );
  desk
    .workspace
    .follow_server(notes.project.id(), accounts)
    .expect("into the account");
  assert_eq!(
    registered(&desk, "Notes").owner,
    account,
    "shared to the account's server, it moves"
  );
  assert!(
    desk
      .workspace
      .servers_moving_project(notes.project.id())
      .expect("read")
      .is_empty(),
    "it has moved"
  );

  let refused = desk.workspace.follow_server(notes.project.id(), computers);
  assert!(matches!(refused, Err(SharingError::AccountProject)), "{refused:?}");
  assert_eq!(registered(&desk, "Notes").owner, account, "nothing moved");
}

#[test]
fn local_owner_a_server_joined_while_signed_out_is_this_computers() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let joined = add_server(&desk, "code.kontur.lan");
  sign_in(&desk, EMAIL);
  let accounts = add_server(&desk, "orchester.kontur.lan");
  assert_eq!(
    desk.entries().iter().map(|entry| entry.stored.id).collect::<Vec<_>>(),
    [joined, accounts],
    "it shows beside the account's"
  );
  sign_out(&desk);
  assert_eq!(
    desk.entries().iter().map(|entry| entry.stored.id).collect::<Vec<_>>(),
    [joined],
    "and stays, while the account's waits"
  );
  sign_in(&desk, OTHER_EMAIL);
  assert_eq!(
    desk.entries().iter().map(|entry| entry.stored.id).collect::<Vec<_>>(),
    [joined],
    "another account sees it too, not the first account's"
  );
}
