//! The QA app's Agents hub states start: the hub's seeds link Kontur to the team whose configuration they cache, and
//! the QA copy moves that link to the stand-in team server rather than link it again (which the store refuses as
//! already shared). `cargo test -p orchester-app --test team_qa team_qa_hub_states -- --test-threads=3`

use orchester_app::routes::{HubFilter, HubView};

use super::{hub_edit::opened, team::State};

/// The hub of `scenario` in `state` opens on its first profile's panel.
fn starts(of: (&str, State)) {
  let (mut window, _server, _) = opened(of, "team-hub-panel-kind", |_| {
    (HubView::new(HubFilter::Profiles, None), None)
  });
  assert_eq!(window.text("team-hub-panel-kind"), "Agent profile");
}

#[test]
fn team_qa_hub_states_the_admins_hub_starts() {
  starts(("team-hub", State::TeamHub));
}

#[test]
fn team_qa_hub_states_the_members_hub_starts() {
  starts(("team-hub-member", State::TeamHubMember));
}
