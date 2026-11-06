//! Decision connection pages use their own ids, recorded check states and real check/delete actions.
use orchester_app::routes::{ConnectionsView, ProjectPage, Route};
use orchester_core::{
  connection::{DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl},
  id::DecisionConnectionId,
  name::Name,
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice},
  time::Timestamp,
};

use super::harness::Seeded;

pub(super) fn connection(seeded: &Seeded, result: DecisionCheckResult) -> DecisionConnection {
  let mut connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Decision server").expect("name"),
    DecisionServerUrl::new("http://127.0.0.1:1/v1").expect("URL"),
    None,
  )
  .with_models(vec![ModelId::new("jev-latest").expect("model")]);
  connection.record_check(DecisionCheck {
    at: Timestamp::now(),
    result,
  });
  seeded.db().create_decision_connection(&connection).expect("store");
  connection
}

pub(super) fn open(seeded: &mut Seeded, id: DecisionConnectionId) {
  seeded.go(ProjectPage::Connections(ConnectionsView::decision(id)));
  seeded.until("decision page", |seeded| seeded.exists("decision-connection-page"));
}

#[test]
fn connections_decision_page_lists_server_models_and_usage_and_opens_from_list() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, DecisionCheckResult::Connected);
  seeded
    .db()
    .save_auto_mode_settings(&AutoModeSettings {
      reviewer: Some(ReviewerChoice::decision(
        connection.id(),
        ModelId::new("jev-latest").expect("model"),
        None,
      )),
    })
    .expect("reviewer");
  seeded.list();
  seeded.click(&format!("decision-row-{}-row", connection.id()));
  seeded.until("decision page", |seeded| seeded.exists("decision-connection-page"));
  for words in [
    "Decision server",
    "System One API",
    "http://127.0.0.1:1/v1",
    "This computer",
    "jev-latest",
    "0.80",
  ] {
    assert!(seeded.text("decision-connection-page").contains(words), "{words}");
  }
  seeded.click("decision-more");
  assert!(seeded.text("decision-delete").contains("Choose another reviewer"));
  seeded.click("decision-back-back");
  seeded.until("connections", |seeded| seeded.exists("connections-list"));
}

#[test]
fn connections_decision_page_shows_each_recorded_header_state() {
  let mut seeded = Seeded::new("connections");
  for (result, words, note) in [
    (DecisionCheckResult::Connected, "Connected", None),
    (
      DecisionCheckResult::KeyRefused,
      "The server refused the key",
      Some("Requests come to you"),
    ),
    (
      DecisionCheckResult::NothingAnswers,
      "Nothing answers at 127.0.0.1:1",
      Some("Orchester does not start"),
    ),
    (
      DecisionCheckResult::NotSystemOneApi,
      "not on /v1/systemone",
      Some("Only a server"),
    ),
    (
      DecisionCheckResult::ModelGone {
        model: ModelId::new("jev-preview").expect("model"),
      },
      "jev-preview is no longer listed",
      Some("Every request"),
    ),
  ] {
    let connection = connection(&seeded, result);
    open(&mut seeded, connection.id());
    assert!(seeded.text("decision-status").contains(words));
    if let Some(note) = note {
      assert!(seeded.text("decision-note").contains(note));
    }
    seeded
      .db()
      .delete_decision_connection(connection.id())
      .expect("remove fixture");
  }
}

#[test]
fn connections_decision_check_and_refresh_show_pending_and_persist_the_answer() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  for button in ["decision-check", "decision-refresh"] {
    seeded.click(button);
    assert!(seeded.text("decision-status").contains("Checking"));
    seeded.until("check answer", |seeded| {
      seeded.text("decision-status").contains("Nothing answers")
    });
    assert_eq!(
      seeded
        .db()
        .decision_connection(connection.id())
        .expect("read")
        .expect("stored")
        .last_check()
        .expect("check")
        .result,
      DecisionCheckResult::NothingAnswers
    );
  }
}

#[test]
fn connections_decision_delete_confirms_and_removes_only_the_selected_connection() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  seeded.click("decision-more");
  seeded.click("decision-delete");
  assert!(seeded.exists("decision-delete-dialog"));
  seeded.click("decision-delete-dialog-cancel");
  assert!(
    seeded
      .db()
      .decision_connection(connection.id())
      .expect("read")
      .is_some()
  );
  seeded.click("decision-more");
  seeded.click("decision-delete");
  seeded.click("decision-delete-confirm");
  seeded.until("removed", |seeded| {
    seeded
      .db()
      .decision_connection(connection.id())
      .expect("read")
      .is_none()
  });
}

#[test]
fn connections_decision_route_roundtrips_and_rejects_two_selected_kinds() {
  let seeded = Seeded::new("connections");
  let id = DecisionConnectionId::generate();
  let route = Route::Project(seeded.project, ProjectPage::Connections(ConnectionsView::decision(id)));
  assert_eq!(Route::parse(&route.path()), Ok(route.clone()));
  assert!(Route::parse(&format!("{}&selected={id}", route.path())).is_err());
  assert!(ConnectionsView::decision(id).selected().is_none());
  assert!(!ConnectionsView::decision(id).is_root());
  let state = orchester_app::mcp::tools::navigate(
    &seeded.window.navigation(),
    &serde_json::json!({
      "route": "connections", "project": seeded.project.to_string(), "decision": id.to_string()
    }),
  )
  .expect("navigate to a decision connection");
  assert_eq!(state["route"]["params"]["decision"], id.to_string());
}

#[test]
fn connections_decision_rejected_reread_hides_stale_actions_until_successful_retry() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  seeded.click("decision-more");
  seeded.click("decision-delete");
  let raw = rusqlite::Connection::open(seeded.repo.join(".orchester/orchester.db")).expect("fixture");
  let settings: String = raw
    .query_row(
      "SELECT settings FROM decision_connections WHERE id = ?1",
      [connection.id().to_string()],
      |row| row.get(0),
    )
    .expect("settings");
  raw
    .execute(
      "UPDATE decision_connections SET settings = 'broken' WHERE id = ?1",
      [connection.id().to_string()],
    )
    .expect("damage disposable fixture");
  seeded
    .window
    .projects()
    .feed(seeded.project)
    .outside
    .update(|counts| counts.connections += 1);
  seeded.until("failed re-read", |s| s.exists("decision-load-error"));
  for id in [
    "decision-check",
    "decision-more",
    "decision-editor",
    "decision-delete-confirm",
    "decision-refresh",
  ] {
    assert!(!seeded.exists(id), "{id} must not act on unreadable data");
  }
  seeded.click("decision-load-error-retry");
  assert!(!seeded.exists("decision-check"), "retry must not expose retained data");
  seeded.until("failed retry", |s| s.exists("decision-load-error"));
  raw
    .execute(
      "UPDATE decision_connections SET settings = ?1 WHERE id = ?2",
      rusqlite::params![settings, connection.id().to_string()],
    )
    .expect("repair fixture");
  seeded.click("decision-load-error-retry");
  seeded.until("successful read", |s| s.exists("decision-check"));
}
