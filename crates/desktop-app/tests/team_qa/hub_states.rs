//! The QA app's Agents hub states start: the hub's seeds link Kontur to the team whose configuration they cache, and
//! the QA copy moves that link to the stand-in team server rather than link it again (which the store refuses as
//! already shared). `cargo test -p orchester-app --test team_qa team_qa_hub_states -- --test-threads=3`

use orchester_app::routes::{HubFilter, HubView};

use super::{
  hub_edit::{on_pencil_until, opened},
  team::State,
};

/// The hub of `scenario` in `state` opens on its first profile's panel.
fn starts(of: (&str, State)) {
  let (mut window, _server, _) = opened(of, "team-hub-panel-kind", |_| {
    (HubView::new(HubFilter::Profiles, None), None)
  });
  assert_eq!(window.text("team-hub-panel-kind"), "Agent profile");
  // The team holds its own profiles alone: the catalogue's stay this computer's and are not the team's (A1 `t4mtQL`).
  assert_eq!(window.text("team-hub-filter-profiles-count"), "2");
}

#[test]
fn team_qa_hub_states_the_admins_hub_starts() {
  starts(("team-hub", State::TeamHub));
}

#[test]
fn team_qa_hub_states_the_members_hub_starts() {
  starts(("team-hub-member", State::TeamHubMember));
}

/// The program path the seed set for `pencil-design` goes with the link to the stand-in, so its file being gone is what
/// the panel says (C5 `h2md5`): the path marked, *No file there*, and *Choose another file* and *Check again* pinned.
#[test]
fn team_qa_hub_states_a_set_path_whose_file_is_gone_says_so() {
  let (mut window, _server, _) =
    on_pencil_until(("team-hub-mcp-path-gone", State::TeamHub), "team-hub-path-value-text");
  assert!(window.text("team-hub-path-value-text").ends_with("pencil-mcp"));
  assert_eq!(window.text("team-hub-path-sub"), "No file there");
  assert_eq!(window.text("team-hub-fix-set-path"), "Choose another file");
  assert!(window.texts("team-hub-fix-check-again").is_some());
}
