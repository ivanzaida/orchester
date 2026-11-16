//! The QA app's Agents hub states start: the hub's seeds link Kontur to the team whose configuration they cache, and
//! the QA copy moves that link to the stand-in team server rather than link it again (which the store refuses as
//! already shared). `cargo test -p orchester-app --test team_qa team_qa_hub_states -- --test-threads=3`

use orchester_app::routes::{HubFilter, HubView};

use super::{
  hub_edit::{on_pencil_until, opened, says_unreachable},
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

/// A team server that stopped answering before this computer read anything of the team: the hub says it cannot reach
/// it, which server did not answer, and offers *Try again*, with no *New* (A25 `Zhcif`).
#[test]
fn team_qa_hub_states_a_server_never_read_says_it_cannot_be_reached() {
  let (mut window, _server, _) = opened(
    ("team-hub-down", State::TeamHubDown),
    "team-hub-unreachable-title",
    |_| (HubView::new(HubFilter::All, None), None),
  );
  assert_eq!(window.text("team-hub-unreachable-title"), "Can't reach the team server");
  let line = window.text("team-hub-unreachable-line");
  assert!(line.contains("didn't answer"), "{line}");
  assert!(line.contains("nothing older is shown"), "{line}");
  assert!(window.text("team-hub-unreachable-reason").contains("tried"));
  assert_eq!(window.text("team-hub-unreachable-again"), "Try again");
  window.until("the head to say it cannot reach the team", says_unreachable);
  assert!(
    window.texts("team-hub-new").is_none(),
    "nothing is created while the team cannot be reached"
  );
}
