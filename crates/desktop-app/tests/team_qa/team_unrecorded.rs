//! Continue on a run of a team task that kept no team record (a run of the old sharing stream, or one whose start
//! stopped before its record was written): the window refuses it in the words the MCP tools use, and takes no claim
//! from the team server, where it used to fall through to the old claim. On the QA state `team-claims`, where KON-11's
//! run is interrupted; its team record is removed before the window opens.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_unrecorded`

use std::path::Path;

use orchester_app::ui::icon::Icon;

use super::{not_ready::opened_after_setup, team::State};

/// Removes every team record of the project's runs, as a database from before team runs were recorded has none.
fn forget_team_runs(repo: &Path) {
  let database =
    rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("open the project database");
  database
    .execute_batch(
      "DROP TRIGGER team_runs_no_delete; DROP TRIGGER work_report_outbox_no_delete; DELETE FROM work_report_outbox; DELETE FROM team_runs;",
    )
    .expect("the team records go");
}

#[test]
fn team_qa_unrecorded_continue_is_refused_in_the_mcp_words_and_claims_nothing() {
  let (mut window, server, _) = opened_after_setup(("team-claims", State::Claims), "KON-11", forget_team_runs);
  window.until("the runner", |window| {
    window
      .tree
      .get_element_by_id("task-continue-run-icon")
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  let before = server.commands().len();

  window.press("task-continue-run");
  window.until("the refusal", |window| window.texts("task-control-refused").is_some());

  let said = window.texts("task-control-refused").expect("the notice").join(" ");
  assert!(said.contains("kept no team record on this computer"), "{said}");
  assert_eq!(server.commands().len(), before, "the server was asked for nothing");
}
