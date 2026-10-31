//! Local use without an Orchester Cloud account (the person's decisions, 2026-10-28 and 2026-10-30): a project added
//! while nobody is signed in is this computer's and no sign-in claims it; an account's waits while it is signed out;
//! another account sees this computer's and its own, never the first account's; a person moves an unshared project
//! between the two; and a project and its server always have the same owner, which a real first link, against the
//! stand-in team server, makes so only once the server accepted it.
//!
//! `cargo test -p orchester-workspace --test local_owner local_owner_`

#[allow(dead_code, reason = "shared with startup_join, which uses more of it")]
#[path = "startup_join/support.rs"]
mod support;

use orchester_api::{
  Credential, SharedProject,
  client::{CloudAddress, PlainHttp, ServerAddress},
  test_support::{
    account_cloud::{AccountCloud, EMAIL, OTHER_EMAIL, PASSWORD},
    team_server::{CODE, TeamServer, USERNAME},
  },
};
use orchester_core::id::{IdempotencyKey, ProjectId, ServerId};
use orchester_store::{RegisteredProject, StoreError};
use orchester_workspace::{
  sharing::{
    CodeRequest, ServerEntry, SharingError, Trust,
    project::{JoinFailed, JoinReview, Joined, LinkFirst, ShareError, Shared, first_link::LinkPlan},
    team_join::JoinOutcome,
  },
  workspace::{MoveError, MoveTo, OpenProject},
};
use support::Desk;
use tempfile::TempDir;

/// The address of the stand-in team server's project.
const SLUG: &str = "team";

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

/// The stand-in team server, with the project at [`SLUG`] the person is a member of.
fn team_server() -> TeamServer {
  let team = TeamServer::start();
  team.share(&[("Team project", SLUG, None)]);
  team
}

/// Joins `team` with its code from `desk`: the account's while one is signed in, this computer's otherwise.
fn join(desk: &Desk, team: &TeamServer) -> ServerEntry {
  let address = ServerAddress::parse(&team.url()).expect("the team server's address");
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  let outcome = desk
    .run(
      desk
        .join
        .join_by_code(&address, Trust::System(PlainHttp::NotAllowed), request),
    )
    .expect("the code joins");
  let JoinOutcome::Joined(joined) = outcome else {
    panic!("not joined: {outcome:?}");
  };
  desk
    .entries()
    .into_iter()
    .find(|entry| entry.stored.id == joined.id)
    .expect("the server is in the list")
}

fn project_on(desk: &Desk, entry: &ServerEntry) -> SharedProject {
  desk
    .run(desk.servers.find_project(entry, SLUG))
    .expect("the project there")
}

fn plan(desk: &Desk, entry: &ServerEntry, local: &OpenProject) -> LinkPlan {
  let project = project_on(desk, entry);
  desk
    .run(desk.servers.link_plan(entry, &local.repo_root, project))
    .expect("what linking would do")
}

/// Links `local` to the project at [`SLUG`] as its first checkout.
fn link_first(desk: &Desk, entry: &ServerEntry, local: &OpenProject) -> Result<Shared, ShareError> {
  desk.run(desk.servers.link_first(
    entry,
    &local.repo_root,
    LinkFirst {
      slug: SLUG.to_owned(),
      remote: None,
      key: IdempotencyKey::generate(),
      write_discovery: false,
    },
  ))
}

fn review(desk: &Desk, entry: &ServerEntry, local: &OpenProject) -> JoinReview {
  desk
    .run(desk.servers.join_review_at(entry, &local.repo_root, SLUG))
    .expect("what joining would do")
}

/// Joins `local` to the project at [`SLUG`], uploading nothing.
fn join_project(desk: &Desk, entry: &ServerEntry, local: &OpenProject) -> Result<Joined, JoinFailed> {
  let review = review(desk, entry, local);
  desk.run(desk.servers.join(
    entry,
    &local.repo_root,
    &review,
    (Vec::new(), IdempotencyKey::generate()),
  ))
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
fn local_owner_a_first_link_to_a_server_of_the_account_moves_the_project() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let notes = desk.project("notes", "Notes");
  sign_in(&desk, EMAIL);
  let account = desk.cloud.sign_in_state().account();
  let team = team_server();
  let entry = join(&desk, &team);
  assert_eq!(
    desk.workspace.servers_moving_project(notes.project.id()).expect("read"),
    [entry.stored.id],
    "what confirms the link says so before it moves"
  );
  assert!(plan(&desk, &entry, &notes).moves_to_account);

  link_first(&desk, &entry, &notes).expect("linked");
  assert_eq!(
    registered(&desk, "Notes").owner,
    account,
    "shared to the account's server, it moves"
  );
  sign_out(&desk);
  assert!(shown(&desk).is_empty(), "and waits with the account");
}

#[test]
fn local_owner_a_join_of_a_server_of_the_account_moves_the_project() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let notes = desk.project("notes", "Notes");
  sign_in(&desk, EMAIL);
  let account = desk.cloud.sign_in_state().account();
  let team = team_server();
  let entry = join(&desk, &team);

  join_project(&desk, &entry, &notes).expect("joined");
  assert_eq!(registered(&desk, "Notes").owner, account, "joined, it moves too");
}

#[test]
fn local_owner_a_first_link_to_a_server_of_this_computer_keeps_the_project_here() {
  let desk = Desk::new();
  let team = team_server();
  let entry = join(&desk, &team);
  let (notes, ledger) = (desk.project("notes", "Notes"), desk.project("ledger", "Ledger"));
  assert!(!plan(&desk, &entry, &notes).moves_to_account, "nothing says it moves");

  link_first(&desk, &entry, &notes).expect("linked with a code's server");
  join_project(&desk, &entry, &ledger).expect("joined with a code's server");
  assert_eq!(registered(&desk, "Notes").owner, None, "it stays this computer's");
  assert_eq!(registered(&desk, "Ledger").owner, None);
  assert_eq!(shown(&desk), ["Ledger", "Notes"], "and shows with nobody signed in");
}

#[test]
fn local_owner_a_first_link_that_fails_moves_nothing() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let notes = desk.project("notes", "Notes");
  sign_in(&desk, EMAIL);
  let team = team_server();
  let entry = join(&desk, &team);

  team.refuse_links();
  let refused = link_first(&desk, &entry, &notes);
  assert!(refused.is_err(), "{:?}", refused.map(|shared| shared.link));
  assert_eq!(
    registered(&desk, "Notes").owner,
    None,
    "a refused link leaves it this computer's"
  );

  let review = review(&desk, &entry, &notes);
  team.set_down(true);
  let failed = desk
    .run(desk.servers.join(
      &entry,
      &notes.repo_root,
      &review,
      (Vec::new(), IdempotencyKey::generate()),
    ))
    .expect_err("the server is down");
  assert_eq!(failed.uploaded, Some(false), "and said that nothing went up");
  assert_eq!(
    registered(&desk, "Notes").owner,
    None,
    "a join that did not reach the server too"
  );
  sign_out(&desk);
  assert_eq!(shown(&desk), ["Notes"], "so it still shows after a sign-out");
}

#[test]
fn local_owner_a_link_that_is_not_written_gives_the_project_back() {
  let cloud = AccountCloud::start();
  let desk = Desk::signed_in(&cloud);
  let account = desk.cloud.sign_in_state().account();
  let server = add_server(&desk, "orchester.kontur.lan");
  sign_out(&desk);
  let notes = desk.project("notes", "Notes");
  sign_in(&desk, EMAIL);

  let during = desk.workspace.link_with_owner(notes.project.id(), server, || {
    assert_eq!(
      registered(&desk, "Notes").owner,
      account,
      "the owner meets the server's before the link exists"
    );
    Err::<(), _>(SharingError::NoProject)
  });
  assert!(matches!(during, Err(SharingError::NoProject)), "{during:?}");
  assert_eq!(registered(&desk, "Notes").owner, None, "given back");

  desk
    .workspace
    .link_with_owner(notes.project.id(), server, || Ok(()))
    .expect("written");
  assert_eq!(registered(&desk, "Notes").owner, account, "and kept once written");
}

#[test]
fn local_owner_a_project_of_the_account_is_refused_a_server_of_this_computer() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let team = team_server();
  let entry = join(&desk, &team);
  sign_in(&desk, EMAIL);
  let account = desk.cloud.sign_in_state().account();
  let notes = desk.project("notes", "Notes");

  let refused = link_first(&desk, &entry, &notes);
  assert!(
    matches!(refused, Err(ShareError::Sharing(SharingError::AccountProject))),
    "{:?}",
    refused.map(|shared| shared.link)
  );
  let refused = desk.run(desk.servers.join_review_at(&entry, &notes.repo_root, SLUG));
  assert!(matches!(refused, Err(SharingError::AccountProject)), "{refused:?}");
  assert_eq!(registered(&desk, "Notes").owner, account, "nothing moved");
}

#[test]
fn local_owner_a_checkout_added_for_a_link_has_the_owner_of_its_server() {
  let cloud = AccountCloud::start();
  let desk = Desk::new();
  use_cloud(&desk, &cloud);
  let team = team_server();
  let computers = join(&desk, &team);
  sign_in(&desk, EMAIL);
  let account = desk.cloud.sign_in_state().account();
  let accounts = add_server(&desk, "orchester.kontur.lan");
  let folders = TempDir::new().expect("the checkouts' folder");
  let checkout = |name: &str| {
    let root = folders.path().join(name);
    std::fs::create_dir_all(root.join(".git")).expect("a checkout");
    root
  };

  let open = desk
    .run(desk.servers.add_checkout(checkout("site"), computers.stored.id))
    .expect("added");
  assert_eq!(
    registered(&desk, open.project.name().as_str()).owner,
    None,
    "for a server of this computer's, it is this computer's and may link to it"
  );
  desk
    .run(desk.servers.join_review_at(&computers, &open.repo_root, SLUG))
    .expect("not refused");

  let open = desk
    .run(desk.servers.add_checkout(checkout("docs"), accounts))
    .expect("added");
  assert_eq!(
    registered(&desk, open.project.name().as_str()).owner,
    account,
    "for a server of the account's, it is the account's"
  );
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
