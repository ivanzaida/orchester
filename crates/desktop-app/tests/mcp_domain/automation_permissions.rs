//! Stored human consent governs app-owned widening, without authorizing Settings or unrelated controls.
use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_core::{resource::Tool, store::ResourceStore};
use orchester_store::ProjectDb;
use orchester_workspace::tools::data::ProjectTools;
use serde_json::json;

use super::{Served, connections_window::controlled, http::post};

fn question() -> (Served, std::path::PathBuf) {
  let (mut s, project, repo) = controlled();
  s.ok(
    "navigate",
    &json!({"route":"tools","project":project.to_string(),"tool":"new"}),
  );
  s.until("new tool form", |s| s.shows("tool-name"));
  s.set_by_client("tool-name", "Consented command");
  s.set_by_client(
    "tool-program",
    &std::env::current_exe().expect("test executable").to_string_lossy(),
  );
  // Create draft is pressable once the program resolved; a client's press only asks.
  s.until("command widening", |s| {
    if !s.shows("tool-confirm-confirm") {
      let _ = s.click_by_client("tool-create");
    }
    s.shows("tool-confirm-confirm")
  });
  (s, repo)
}

fn count(repo: &std::path::Path) -> usize {
  let db = ProjectDb::open(repo).expect("private project database");
  ProjectTools::load(&db)
    .expect("real resource list")
    .tools
    .iter()
    .filter(|t| t.name.as_ref() == "Consented command")
    .count()
}

#[test]
fn domain_automation_policy_off_refuses_on_confirms_save_and_publish() {
  let (mut s, repo) = question();
  assert_eq!(
    s.click_by_client("tool-confirm-confirm").expect_err("off is protected")["code"],
    "person_only"
  );
  assert_eq!(count(&repo), 0);
  s.let_automation_widen(true);
  s.click_by_client("tool-confirm-confirm")
    .expect("saved opt-in permits this app review");
  s.until("real draft saved", |_| count(&repo) == 1);
  s.until("saved editor", |s| {
    s.shows("tool-publish") && super::tools_window::text(s, "tool-draft-bar-status").contains("Draft")
  });
  s.click_by_client("tool-publish").expect("publish asks");
  s.until("publication review", |s| s.shows("tool-confirm-confirm"));
  s.click_by_client("tool-confirm-confirm")
    .expect("opt-in also permits publication widening");
  let db = ProjectDb::open(&repo).expect("private project database");
  let id = ProjectTools::load(&db)
    .expect("tools list")
    .tools
    .iter()
    .find(|t| t.name.as_ref() == "Consented command")
    .expect("saved command")
    .id;
  s.until("actual published version", |_| {
    db.latest_version::<Tool>(id).expect("version read").is_some()
  });
}

#[test]
fn domain_automation_policy_revocation_while_question_mounted_refuses() {
  let (mut s, repo) = question();
  s.let_automation_widen(true);
  s.ok("lurq_interact", &json!({"action":"move","x":100,"y":100}));
  s.let_automation_widen(false);
  assert_eq!(
    s.click_by_client("tool-confirm-confirm")
      .expect_err("revoked consent protects again")["code"],
    "person_only"
  );
  assert!(s.shows("tool-confirm-confirm"));
  assert_eq!(count(&repo), 0);
}

#[test]
fn domain_automation_policy_forwarded_confirmation_cannot_save_after_revoke() {
  let (mut s, repo) = question();
  s.let_automation_widen(true);
  // Finding is read-only and does not dispatch a UI decision.
  let found = s.ok("lurq_find_by_id", &json!({"id":"tool-confirm-confirm"}));
  let reference = found
    .as_str()
    .expect("element reference")
    .split_whitespace()
    .next()
    .expect("reference token")
    .to_owned();
  let (port, bearer, session) = (s.port, s.bearer(), s.session.clone());
  let body = json!({"jsonrpc":"2.0","id":91,"method":"tools/call","params":{"name":"lurq_interact","arguments":{"action":"click","ref":reference}}});
  let request = thread::spawn(move || post(port, &[("Authorization", &bearer), ("Mcp-Session-Id", &session)], &body));
  let deadline = Instant::now() + Duration::from_secs(10);
  while !s.link.input_gate().input_in_flight() {
    assert!(
      !request.is_finished(),
      "confirmation must actually be queued before revocation"
    );
    assert!(Instant::now() < deadline, "server did not forward input");
    thread::yield_now();
  }
  // No event-loop pass has delivered the authorized request yet.
  s.let_automation_widen(false);
  while !request.is_finished() {
    s.pass();
    thread::yield_now();
    assert!(Instant::now() < deadline, "queued request did not finish");
  }
  request.join().expect("queued client finished");
  for _ in 0..3 {
    s.pass();
  }
  assert!(s.shows("tool-confirm-confirm"), "late input cannot answer this review");
  assert_eq!(count(&repo), 0, "revoked queued input never creates the tool");
}

#[test]
fn domain_automation_policy_settings_always_protected_even_when_on() {
  let (mut s, ..) = controlled();
  s.let_automation_widen(true);
  s.ok("navigate", &json!({"route":"settings"}));
  s.until("Settings opt-in", |s| s.shows("settings-change-permissions"));
  assert_eq!(
    s.click_by_client("settings-change-permissions")
      .expect_err("automation never controls its own opt-in")["code"],
    "person_only"
  );
  assert!(
    s.services()
      .workspace
      .automation_changes_permissions()
      .expect("stored consent")
  );
  s.ok("lurq_read_tree", &json!({}));
}

#[test]
fn domain_automation_policy_timed_out_confirmation_is_dropped_even_after_new_grant() {
  let (mut s, repo) = question();
  s.let_automation_widen(true);
  let found = s.ok("lurq_find_by_id", &json!({"id":"tool-confirm-confirm"}));
  let reference = found
    .as_str()
    .expect("element reference")
    .split_whitespace()
    .next()
    .expect("reference token")
    .to_owned();
  let (port, bearer, session) = (s.port, s.bearer(), s.session.clone());
  let body = json!({"jsonrpc":"2.0","id":92,"method":"tools/call","params":{"name":"lurq_interact","arguments":{"action":"click","ref":reference}}});
  let request = thread::spawn(move || post(port, &[("Authorization", &bearer), ("Mcp-Session-Id", &session)], &body));
  let deadline = Instant::now() + Duration::from_secs(25);
  while !s.link.input_gate().input_in_flight() {
    assert!(!request.is_finished(), "request must actually be forwarded");
    assert!(Instant::now() < deadline, "server did not forward input");
    thread::yield_now();
  }
  // The real server reply deadline expires while its actual queue is deliberately not drained.
  let response = request.join().expect("timed-out request finishes");
  assert_eq!(
    response.message.as_ref().expect("JSON-RPC result")["result"]["isError"],
    true
  );
  assert!(response.text.contains("timed out waiting for the app"));
  assert!(!s.link.input_gate().input_in_flight(), "caller lifetime has ended");
  s.let_automation_widen(false);
  s.let_automation_widen(true);
  // Even a fresh explicit grant cannot revive input whose caller was already told it failed.
  s.pass();
  assert!(s.shows("tool-confirm-confirm"));
  assert_eq!(
    count(&repo),
    0,
    "closed queued request must be dropped, not merely blocked until re-grant"
  );
}

#[test]
fn domain_automation_policy_forwarded_keep_cannot_cancel_after_revoke() {
  let (mut s, repo) = question();
  s.let_automation_widen(true);
  // Finding is read-only and does not dispatch a UI decision.
  let found = s.ok("lurq_find_by_id", &json!({"id":"tool-confirm-keep"}));
  let reference = found
    .as_str()
    .expect("element reference")
    .split_whitespace()
    .next()
    .expect("reference token")
    .to_owned();
  let (port, bearer, session) = (s.port, s.bearer(), s.session.clone());
  let body = json!({"jsonrpc":"2.0","id":91,"method":"tools/call","params":{"name":"lurq_interact","arguments":{"action":"click","ref":reference}}});
  let request = thread::spawn(move || post(port, &[("Authorization", &bearer), ("Mcp-Session-Id", &session)], &body));
  let deadline = Instant::now() + Duration::from_secs(10);
  while !s.link.input_gate().input_in_flight() {
    assert!(
      !request.is_finished(),
      "confirmation must actually be queued before revocation"
    );
    assert!(Instant::now() < deadline, "server did not forward input");
    thread::yield_now();
  }
  // No event-loop pass has delivered the authorized request yet.
  s.let_automation_widen(false);
  while !request.is_finished() {
    s.pass();
    thread::yield_now();
    assert!(Instant::now() < deadline, "queued request did not finish");
  }
  request.join().expect("queued client finished");
  for _ in 0..3 {
    s.pass();
  }
  assert!(
    s.shows("tool-confirm-keep"),
    "revoked live input cannot cancel this review"
  );
  assert_eq!(count(&repo), 0, "revoked queued input never creates the tool");
}

#[test]
fn domain_automation_policy_keep_preserves_human_and_granted_automation_choices() {
  let (mut human, repo) = question();
  lurq::app::events::with_synthetic_input(|| human.click_by_person("tool-confirm-keep"));
  assert!(
    human.shows("tool-confirm-confirm"),
    "off synthetic Keep stays protected"
  );
  human.click_by_person("tool-confirm-keep");
  assert!(!human.shows("tool-confirm-confirm"), "ordinary human Keep stays usable");
  assert_eq!(count(&repo), 0);
  let (mut allowed, repo) = question();
  allowed.let_automation_widen(true);
  allowed
    .click_by_client("tool-confirm-keep")
    .expect("matching consent permits automation Keep");
  assert!(!allowed.shows("tool-confirm-confirm"));
  assert_eq!(count(&repo), 0, "Keep never creates a command");
}
