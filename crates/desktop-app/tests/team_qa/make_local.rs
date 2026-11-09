//! The confirmation of Make local on a team project (`woioY`, ORC-814, ORC-848): its *Agents configuration* row names
//! every kind of team resource that leaves and what this computer filled in for them, which leaves too.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_make_local`

use orchester_app::routes::{ProjectPage, Route};

use super::{not_ready::on_task, team::State};

#[test]
fn team_qa_make_local_says_the_mcp_servers_and_what_this_computer_filled_in_leave_with_the_team() {
  let (mut window, _server, project) = on_task(("team-not-ready", State::NotReadyHere), "KON-5");
  window.navigation().go(Route::Project(project, ProjectPage::MakeLocal));
  window.until("the configuration row", |window| {
    window.texts("sharing-make-local-configuration").is_some()
  });
  let words = window.text("sharing-make-local-configuration");
  for part in [
    "MCP servers",
    "the connection picked for a profile",
    "which MCP servers are on here",
    "the secret values",
    "Only this project's own stay",
  ] {
    assert!(words.contains(part), "{part:?} is missing from {words}");
  }
}
