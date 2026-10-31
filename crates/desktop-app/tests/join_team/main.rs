//! *Join a team* in the window (`docs/design/startup.md` decision 4; `Startup · redesign` `qzzYw`), signed in to the
//! stand-in Orchester Cloud, which names a stand-in team server on loopback as the account's and has an invite to one
//! of its projects: the step opens from the welcome on the account's teams and invites (`drCQd`); *Join* and *Accept*
//! give this computer its own sign-in there; *Use a code* shows the two ways side by side with the account in the
//! Cloud column; a code joins, and a wrong or expired code (`aWsVx`) and a server that does not answer (`oOn6j`) say
//! so in place; only a person joins. The step's roles follow the design ([`roles`]).
//!
//! `cargo test -p orchester-app --test join_team join_team_`

mod roles;
#[path = "../support/mod.rs"]
mod support;

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::{CODE, LATER_CODE, NAME, TeamServer, USERNAME};
use orchester_app::routes::{JoinWay, Route};
use support::{Data, Window, signed_in::account_cloud::EMAIL};

/// The team server's id at the stand-in cloud.
const TEAM: &str = "0191c7c0-0000-7000-8000-0000000005e1";
/// Alice's invite to Billing on it.
const INVITE: &str = "0191c7c0-0000-7000-8000-00000000171e";

/// The team's row and its *Join*.
fn team_row() -> String {
  format!("join-team-teams-{TEAM}")
}

/// The invite's row.
fn invite_row() -> String {
  format!("join-team-invites-{INVITE}")
}

/// The window of `data`, signed in, with the stand-in cloud naming `server` as the account's team and the invite.
fn window_of(data: Data, server: &TeamServer, add_kontur: bool) -> Window {
  data.cloud.name_server(TEAM, &server.url(), None);
  data
    .cloud
    .invite(INVITE, Some(("Billing", &format!("{}/billing", server.url()))));
  let launch = data.launch();
  if add_kontur {
    data.add_kontur(launch.workspace().expect("the session started"));
  }
  let window = Window::of(data, launch);
  window.launch.services().expect("services").links.record_instead();
  window
}

/// The window at `way` of the step, with Kontur in the list.
fn opened(server: &TeamServer, way: JoinWay) -> Window {
  let mut window = window_of(Data::new(), server, true);
  window.navigation().go(Route::JoinTeam(way));
  shows(&mut window, "screen-join_team");
  window
}

fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

fn has(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

fn value(window: &mut Window, id: &str) -> String {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .value()
}

/// The names of the servers this computer joined.
fn joined(window: &Window) -> Vec<String> {
  let servers = &window.launch.services().expect("services").servers;
  let entries = servers.entries().expect("the servers are read");
  entries
    .into_iter()
    .filter(|entry| entry.in_use())
    .map(|entry| entry.stored.name)
    .collect()
}

/// Types `address` and `code` in the code column and presses *Join with the code*.
fn join_with(window: &mut Window, (address, code): (&str, &str)) {
  shows(window, "join-team-code-address");
  window.set("join-team-code-address", address);
  window.set("join-team-code-code", code);
  window.press("join-team-code-submit");
}

#[test]
fn join_team_opens_from_welcome_on_your_teams() {
  let server = TeamServer::start();
  let mut window = window_of(Data::new(), &server, false);
  shows(&mut window, "welcome-your-teams");
  window.press("welcome-your-teams");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Cloud));
  shows(&mut window, "screen-join_team");

  assert_eq!(window.text("join_team-crumbs"), "Start Join a team");
  assert_eq!(
    window.text("join_team-steps"),
    "1 Join the team 2 Choose a project 3 Link a folder 4 Download tasks"
  );
  assert_eq!(window.text("join_team-title"), "Choose a team");
  assert_eq!(window.text("join-team-account-email"), EMAIL);
  shows(&mut window, &team_row());
  assert_eq!(window.text(&format!("{}-title", team_row())), NAME);
  assert_eq!(window.text(&format!("{}-role", team_row())), "Member");
  assert!(has(&mut window, &format!("{}-join", team_row())));
  assert_eq!(
    window.text(&format!("{}-title", invite_row())),
    format!("{NAME} invited you to Billing")
  );
  assert!(has(&mut window, &format!("{}-accept", invite_row())));
  assert!(has(&mut window, &format!("{}-decline", invite_row())));
  assert!(
    window
      .text("join-team-not-listed")
      .starts_with(&format!("A team not listed? Ask its owner to invite {EMAIL}")),
  );

  window.press("join_team-crumb-start");
  assert_eq!(window.navigation().route_now(), Route::AllProjects);
}

#[test]
fn join_team_joins_a_listed_team() {
  let server = TeamServer::start();
  let mut window = opened(&server, JoinWay::Cloud);
  shows(&mut window, &format!("{}-join", team_row()));
  window.press(&format!("{}-join", team_row()));

  shows(&mut window, "join-team-teams-outcome-joined");
  assert_eq!(
    window.text("join-team-teams-outcome-joined-title"),
    format!("Joined {NAME} as {USERNAME}")
  );
  assert_eq!(server.cloud_joins(), 1);
  assert_eq!(joined(&window), [NAME]);
  shows(&mut window, &format!("{}-open", team_row()));
  assert!(!has(&mut window, &format!("{}-join", team_row())));
}

/// Signed in with a team this computer joined, the first start's third way names the team and the invite waiting, and
/// leads to the team's projects (01b `Knacr`).
#[test]
fn join_team_first_start_names_the_joined_team() {
  let server = TeamServer::start();
  let mut window = window_of(Data::new(), &server, false);
  window.navigation().go(Route::JoinTeam(JoinWay::Cloud));
  shows(&mut window, &format!("{}-join", team_row()));
  window.press(&format!("{}-join", team_row()));
  shows(&mut window, "join-team-teams-outcome-joined");

  window.navigation().go(Route::AllProjects);
  window.until("the joined team on the third way", |window| {
    window.tree.get_element_by_id("welcome-shared-meta").is_some()
      && window.text("welcome-shared-meta") == format!("Joined · {NAME}")
  });
  window.until("how many projects are the person's", |window| {
    window
      .text("welcome-shared-body")
      .starts_with(&format!("{NAME} shares"))
  });
  shows(&mut window, "welcome-invites");
  assert_eq!(window.text("welcome-invites"), "1 invite");
  window.press("welcome-choose-project");
  assert!(
    matches!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Projects(_))),
    "the team's projects"
  );
}

#[test]
fn join_team_accepts_an_invite() {
  let server = TeamServer::start();
  let mut window = opened(&server, JoinWay::Cloud);
  shows(&mut window, &format!("{}-accept", invite_row()));
  window.press(&format!("{}-accept", invite_row()));

  shows(&mut window, "join-team-invites-outcome-joined");
  assert_eq!(window.data.cloud.tokens_for(INVITE), 1);
  assert_eq!(joined(&window), [NAME]);
  window.until("the accepted invite listed no more", |window| {
    window.tree.get_element_by_id(&invite_row()).is_none()
  });
}

#[test]
fn join_team_use_a_code_shows_the_account_card() {
  let server = TeamServer::start();
  let mut window = opened(&server, JoinWay::Cloud);
  shows(&mut window, "join-team-use-code");
  window.press("join-team-use-code");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Code));
  shows(&mut window, "join-team-ways");

  assert_eq!(window.text("join_team-title"), "Join a team");
  assert_eq!(window.text("join-team-cloud-title"), "With Orchester Cloud");
  assert_eq!(window.text("join-team-account-email"), EMAIL);
  assert_eq!(window.text("join-team-code-title"), "With a code");
  assert!(!has(&mut window, "join-team-cloud-email"), "signed in, no sign-in form");
  assert!(
    window
      .text("join-team-note")
      .starts_with("Orchester Cloud is only for team features.")
  );

  window.press("join-team-cloud-your-teams");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Cloud));
}

#[test]
fn join_team_code_fields_lead_with_their_glyphs_while_empty() {
  let server = TeamServer::start();
  let mut window = opened(&server, JoinWay::Code);
  shows(&mut window, "join-team-code-address");
  assert!(has(&mut window, "join-team-code-address-icon"));
  assert!(has(&mut window, "join-team-code-code-icon"));

  window.set("join-team-code-address", &server.url());
  window.set("join-team-code-code", LATER_CODE);
  window.until("the filled fields lose their glyphs", |window| {
    window.tree.get_element_by_id("join-team-code-address-icon").is_none()
      && window.tree.get_element_by_id("join-team-code-code-icon").is_none()
  });
}

#[test]
fn join_team_joins_with_a_code() {
  let server = TeamServer::start();
  let mut window = opened(&server, JoinWay::Code);
  join_with(&mut window, (&server.url(), CODE));

  shows(&mut window, "join-team-code-joined");
  assert_eq!(
    window.text("join-team-code-joined-title"),
    format!("Joined {NAME} as {USERNAME}")
  );
  assert_eq!(server.code_joins().len(), 1);
  assert_eq!(joined(&window), [NAME]);
}

#[test]
fn join_team_wrong_code_shows_in_place_of_the_hint() {
  let server = TeamServer::start();
  server.expire_later_code();
  let mut window = opened(&server, JoinWay::Code);
  assert!(has(&mut window, "join-team-code-code-field-hint"));
  join_with(&mut window, (&server.url(), LATER_CODE));

  shows(&mut window, "join-team-code-wrong");
  assert_eq!(
    window.text("join-team-code-wrong"),
    "Wrong or expired. A code works once, within 10 minutes: ask for a new one."
  );
  assert!(
    !has(&mut window, "join-team-code-code-field-hint"),
    "the error takes the hint's place"
  );
  assert_eq!(value(&mut window, "join-team-code-address"), server.url());
  assert_eq!(value(&mut window, "join-team-code-code"), LATER_CODE);
  assert!(joined(&window).is_empty());
}

#[test]
fn join_team_unreachable_server_offers_try_again() {
  let server = TeamServer::start();
  server.set_down(true);
  let mut window = opened(&server, JoinWay::Code);
  join_with(&mut window, (&server.url(), CODE));

  shows(&mut window, "join-team-code-unreachable");
  let host = server.url();
  assert_eq!(
    window.text("join-team-code-unreachable-title"),
    format!("Can't reach {host}")
  );
  assert!(
    window
      .text("join-team-code-unreachable-body")
      .starts_with("No answer in 10 seconds.")
  );
  assert_eq!(value(&mut window, "join-team-code-address"), host);
  assert_eq!(value(&mut window, "join-team-code-code"), CODE);
  assert!(has(&mut window, "join-team-code-code-field-hint"), "the hint stays");

  server.set_down(false);
  window.press("join-team-code-try-again");
  shows(&mut window, "join-team-code-joined");
  assert!(!has(&mut window, "join-team-code-unreachable"));
  assert_eq!(joined(&window), [NAME]);
}

#[test]
fn join_team_only_a_person_joins() {
  let server = TeamServer::start();
  let mut window = opened(&server, JoinWay::Cloud);
  let join = format!("{}-join", team_row());
  shows(&mut window, &join);
  with_synthetic_input(|| window.press(&join));
  shows(&mut window, &format!("{join}-person-only"));
  assert_eq!(server.cloud_joins(), 0);

  let accept = format!("{}-accept", invite_row());
  with_synthetic_input(|| window.press(&accept));
  shows(&mut window, &format!("{accept}-person-only"));
  assert_eq!(window.data.cloud.tokens_for(INVITE), 0);

  window.navigation().go(Route::JoinTeam(JoinWay::Code));
  shows(&mut window, "join-team-code-address");
  window.set("join-team-code-address", &server.url());
  window.set("join-team-code-code", CODE);
  with_synthetic_input(|| window.press("join-team-code-submit"));
  shows(&mut window, "join-team-code-submit-person-only");
  assert!(server.code_joins().is_empty());
  assert!(joined(&window).is_empty());
}
