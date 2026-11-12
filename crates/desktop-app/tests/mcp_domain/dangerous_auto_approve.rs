//! The *Approve mode* over MCP (`docs/design/approval-modes.md`): only a person moves it up, whatever automation may
//! do, so `set_approve_mode` up and an MCP client's press of the confirmation's danger button are refused, while moving
//! it down is always taken, by the tool and by a client's press of a segment; `set_allow_autopilot` keeps the former
//! switch's meaning; `get_approvals` reports both. Filter: `dangerous_auto_approve`.
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::run::ApproveMode;
use orchester_store::ProjectDb;
use serde_json::json;

use super::{Served, connections_window::controlled};

const DANGEROUS: &str = "settings-approve-mode-dangerous";
const CONFIRM: &str = "settings-dangerous-dialog-confirm";

fn mode(repo: &std::path::Path) -> ApproveMode {
  ProjectDb::open(repo).expect("db").approve_mode().expect("mode")
}

/// Every switch a person can give automation, in Settings.
fn lets_all(s: &Served) {
  let workspace = s.launch.as_ref().expect("launch").workspace().expect("workspace");
  workspace
    .set_automation_answers_approvals(true)
    .expect("answer approvals");
  workspace
    .set_automation_changes_permissions(true)
    .expect("change agent permissions");
}

#[test]
fn dangerous_auto_approve_tool_moves_down_only() {
  let (mut s, project, repo) = controlled();
  let to = |mode: &str| json!({"project": project.to_string(), "approve_mode": mode});
  lets_all(&s);
  for up in ["autopilot", "dangerous_auto_approve"] {
    let refused = s.refused("set_approve_mode", &to(up));
    assert_eq!(refused["code"], "person_only", "{up}: {refused}");
  }
  assert_eq!(mode(&repo), ApproveMode::Manual);
  let read = s.ok("get_approvals", &json!({"project": project.to_string()}));
  assert_eq!(read["approvals"]["approve_mode"], "manual", "{read}");
  assert_eq!(read["approvals"]["allow_autopilot"], false, "{read}");
  ProjectDb::open(&repo)
    .expect("db")
    .save_approve_mode(ApproveMode::DangerousAutoApprove)
    .expect("a person chooses it");
  let read = s.ok("get_approvals", &json!({"project": project.to_string()}));
  assert_eq!(read["approvals"]["approve_mode"], "dangerous_auto_approve", "{read}");
  assert_eq!(
    read["approvals"]["allow_autopilot"], true,
    "it allows Autopilot: {read}"
  );
  // The former switch's on changes nothing in Dangerous auto-approve; its off moves to Manual.
  let on = json!({"project": project.to_string(), "allow_autopilot": true});
  s.ok("set_allow_autopilot", &on);
  assert_eq!(mode(&repo), ApproveMode::DangerousAutoApprove);
  let down = s.ok("set_approve_mode", &to("autopilot"));
  assert_eq!(down["approvals"]["approve_mode"], "autopilot", "{down}");
  let off = json!({"project": project.to_string(), "allow_autopilot": false});
  s.ok("set_allow_autopilot", &off);
  assert_eq!(mode(&repo), ApproveMode::Manual);
  ProjectDb::open(&repo)
    .expect("db")
    .save_approve_mode(ApproveMode::DangerousAutoApprove)
    .expect("a person chooses it");
  s.ok("set_allow_autopilot", &off);
  assert_eq!(
    mode(&repo),
    ApproveMode::Manual,
    "off leaves Dangerous auto-approve too"
  );
}

#[test]
fn dangerous_auto_approve_window_takes_a_persons_danger_button_alone() {
  let (mut s, project, repo) = controlled();
  s.until("navigation attached", |s| s.link.navigation().is_ok());
  s.link
    .navigation()
    .expect("navigation")
    .go(Route::Project(project, ProjectPage::Settings(ProjectSettingsTab::Runs)));
  s.until("the selector", |s| s.shows(DANGEROUS));
  lets_all(&s);
  s.click_by_client(DANGEROUS).expect("the click is delivered");
  s.until("the confirmation", |s| s.shows(CONFIRM));
  s.click_by_client(CONFIRM).expect("the click is delivered");
  s.until("the refusal", |s| s.shows("settings-dangerous-person-only"));
  for _ in 0..5 {
    s.pass();
  }
  assert_eq!(mode(&repo), ApproveMode::Manual);
  s.click_by_person(CONFIRM);
  s.until("chosen by the person", |_| {
    mode(&repo) == ApproveMode::DangerousAutoApprove
  });
  s.until("the status bar's chip", |s| s.shows("status-dangerous"));
  s.click_by_client("settings-approve-mode-manual")
    .expect("the click is delivered");
  s.until("down by the client", |_| mode(&repo) == ApproveMode::Manual);
  s.until("the chip gone", |s| !s.shows("status-dangerous"));
}

/// In Dangerous auto-approve no run reads as Manual or Autopilot: each one's mode is `dangerous_auto_approve` because
/// of the project, its own switch to Manual kept beside it.
#[test]
fn dangerous_auto_approve_reads_say_every_running_run_is_approved_at_once() {
  let (mut served, seeded) = Served::seeded("cards");
  ProjectDb::open(&seeded.repo)
    .expect("db")
    .save_approve_mode(ApproveMode::DangerousAutoApprove)
    .expect("a person chooses it");
  let read = served.ok("get_approvals", &json!({ "project": "Kontur" }));
  let runs = read["approvals"]["runs"].as_array().expect("runs");
  assert!(!runs.is_empty(), "{read}");
  for run in runs {
    assert_eq!(
      run["approval_mode"],
      json!({"mode": "dangerous_auto_approve", "because": "project"}),
      "{run}"
    );
    assert!(run["switched_to_manual"].is_boolean(), "{run}");
  }
  let number = runs[0]["number"].to_string();
  let one = served.ok("get_run", &json!({ "project": "Kontur", "run": number }));
  assert_eq!(one["approval_mode"]["mode"], "dangerous_auto_approve", "{one}");
  assert_eq!(one["approve_mode"], "dangerous_auto_approve", "{one}");
}
