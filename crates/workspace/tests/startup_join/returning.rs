//! The returning start's All projects (`docs/design/startup.md`, decision 8): where each listed project is, local or a
//! team's, and the one team project with no checkout on this computer that it offers, until *Not now* puts it off for
//! good. `cargo test -p orchester-workspace --test startup_join startup_join_returning`

use orchester_api::test_support::team_server::TeamServer;
use orchester_core::id::{ProjectId, ServerId};
use orchester_workspace::{
  project_list::Listing,
  sharing::{OfferedProject, Place},
};

use super::{Desk, join_by_code};

/// The team the design names.
const TEAM: &str = "Acme";

/// The project offered now, by its name.
fn offered(desk: &Desk) -> Option<String> {
  let offer: Option<OfferedProject> = desk.run(desk.servers.offered()).expect("the offer");
  offer.map(|offer| offer.project.name)
}

/// The id on the server of its project `name`.
fn server_project(desk: &Desk, server: ServerId, name: &str) -> ProjectId {
  desk
    .run(desk.join.projects(server))
    .expect("the team's projects")
    .into_iter()
    .find(|listed| listed.project.name == name)
    .unwrap_or_else(|| panic!("{name} is shared"))
    .project
    .id
}

#[test]
fn startup_join_returning_offers_a_team_project_not_on_this_computer() {
  let server = TeamServer::named(TEAM);
  server.share(&[("Kontur", "kontur", None), ("Billing", "billing", None)]);
  let desk = Desk::new();
  let team = join_by_code(&desk, &server);
  let kontur = server_project(&desk, team.id, "Kontur");
  desk.linked_project("kontur", "Kontur", (team.id, kontur));

  let offer = desk
    .run(desk.servers.offered())
    .expect("the offer")
    .expect("a project is offered");

  assert_eq!(
    (offer.server, offer.team.as_str(), offer.project.name.as_str()),
    (team.id, TEAM, "Billing"),
    "Kontur has a checkout here, so Billing is offered"
  );
  assert_eq!(offer.project.slug, "billing");
}

#[test]
fn startup_join_returning_not_now_is_remembered_per_project() {
  let server = TeamServer::named(TEAM);
  server.share(&[("Billing", "billing", None), ("Atlas", "atlas", None)]);
  let desk = Desk::new();
  let team = join_by_code(&desk, &server);
  assert_eq!(offered(&desk).as_deref(), Some("Billing"));

  let billing = server_project(&desk, team.id, "Billing");
  desk.servers.not_now(team.id, billing).expect("put off");

  assert_eq!(offered(&desk).as_deref(), Some("Atlas"), "the next one is offered");
  let later = desk.servers_again();
  let offer = desk.run(later.offered()).expect("the offer after a new start");
  assert_eq!(
    offer.map(|offer| offer.project.name).as_deref(),
    Some("Atlas"),
    "Billing stays put off after a new start"
  );
  let atlas = server_project(&desk, team.id, "Atlas");
  later.not_now(team.id, atlas).expect("put off");
  assert_eq!(offered(&desk), None, "nothing is left to offer");
}

#[test]
fn startup_join_returning_offers_nothing_without_a_team() {
  let desk = Desk::new();
  desk.project("ledger", "Ledger");

  assert_eq!(offered(&desk), None);
}

#[test]
fn startup_join_returning_offers_nothing_while_the_server_is_down() {
  let server = TeamServer::named(TEAM);
  server.share(&[("Billing", "billing", None)]);
  let desk = Desk::new();
  join_by_code(&desk, &server);
  server.set_down(true);

  assert_eq!(offered(&desk), None, "a team that does not answer offers nothing");
  server.set_down(false);
  assert_eq!(
    offered(&desk).as_deref(),
    Some("Billing"),
    "and offers again once it answers"
  );
}

#[test]
fn startup_join_returning_places_say_local_or_the_team() {
  let server = TeamServer::named(TEAM);
  server.share(&[("Kontur", "kontur", None)]);
  let desk = Desk::new();
  let team = join_by_code(&desk, &server);
  let kontur = desk.linked_project("kontur", "Kontur", (team.id, server_project(&desk, team.id, "Kontur")));
  let ledger = desk.project("ledger", "Ledger");
  let elsewhere = desk.linked_project("elsewhere", "Elsewhere", (ServerId::generate(), ProjectId::generate()));
  let gone = desk.project("gone", "Gone");
  std::fs::remove_dir_all(&gone.repo_root).expect("the folder is gone");
  let Listing::Projects(entries) = desk.workspace.listing().expect("the list") else {
    panic!("the projects are listed");
  };

  let places = desk.servers.places(&entries).expect("the places");

  assert_eq!(
    places.of(kontur.project.id()),
    Some(&Place::Shared {
      team: Some(TEAM.to_owned())
    })
  );
  assert_eq!(places.of(ledger.project.id()), Some(&Place::Local));
  assert_eq!(
    places.of(elsewhere.project.id()),
    Some(&Place::Shared { team: None }),
    "a server not in the list has no name to give"
  );
  assert_eq!(places.of(gone.project.id()), None, "a missing folder says nothing");
}
