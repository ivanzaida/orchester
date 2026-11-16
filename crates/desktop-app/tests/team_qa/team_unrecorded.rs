//! Continue on a team run that kept no team record: one whose start stopped after the run and its team inputs were
//! written and before the record was. The window refuses it in the words the MCP tools use, and takes no claim from the
//! team server. On the QA state `team-claims`, where KON-11's run is interrupted; its team record is removed and team
//! inputs are given to it before the window opens. A run of the old sharing stream has neither and goes the local way,
//! which `orchester-workspace`'s `team_mcp` tests cover.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_unrecorded`

use std::path::Path;

use orchester_app::ui::icon::Icon;
use orchester_core::id::OperationId;

use super::{not_ready::opened_after_setup, team::State};

/// Removes every team record of the project's runs and gives each run the team inputs a start writes with it, as one
/// whose start stopped before its record was written has. The inputs' content is not read: only that the run was
/// created with the team's, and a run's inputs without the local bindings read as none.
fn forget_team_records(repo: &Path) {
  let database =
    rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("open the project database");
  database
    .execute_batch(
      "DROP TRIGGER team_runs_no_delete; DROP TRIGGER work_report_outbox_no_delete; DELETE FROM work_report_outbox; DELETE FROM team_runs;",
    )
    .expect("the team records go");
  let runs: Vec<String> = database
    .prepare("SELECT id FROM runs")
    .expect("prepare")
    .query_map([], |row| row.get(0))
    .expect("runs")
    .collect::<Result<_, _>>()
    .expect("run ids");
  for run in runs {
    database
      .execute(
        "INSERT INTO run_bundles (run_id, origin, operation_id, sha256, body) VALUES (?1, 'team', ?2, ?3, '{}')",
        [run, OperationId::generate().to_string(), "0".repeat(64)],
      )
      .expect("team inputs");
  }
}

#[test]
fn team_qa_unrecorded_continue_is_refused_in_the_mcp_words_and_claims_nothing() {
  let (mut window, server, _) = opened_after_setup(("team-claims", State::Claims), "KON-11", forget_team_records);
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
  assert!(said.contains("did not finish recording its team work"), "{said}");
  assert_eq!(server.commands().len(), before, "the server was asked for nothing");
}
