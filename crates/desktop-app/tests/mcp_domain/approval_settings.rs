//! Approval settings over MCP and in Project settings (`docs/design/approval-modes.md`): only a person allows
//! Autopilot, whatever automation may do, so an MCP client's press of the switch and `set_approval_settings`'
//! `allow_autopilot` are refused, and turning it off is always taken; `switch_run_to_manual` is a narrowing, taken
//! without any of the person's switches, and a run never goes back. Filter: `approval_mode_effective`.
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::run::AllowAutopilot;
use orchester_store::ProjectDb;
use serde_json::json;

use super::{Served, connections_window::controlled};

const SWITCH: &str = "settings-allow-autopilot";
const REFUSAL: &str = "settings-allow-autopilot-person-only";

fn setting(repo: &std::path::Path) -> AllowAutopilot {
  ProjectDb::open(repo).expect("db").allow_autopilot().expect("setting")
}

/// What a person lets automation do, in Settings: answer approvals and change agent permissions.
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
fn approval_mode_effective_switch_allows_autopilot_by_a_persons_press_alone() {
  let (mut s, project, repo) = controlled();
  s.until("navigation attached", |s| s.link.navigation().is_ok());
  s.link
    .navigation()
    .expect("navigation")
    .go(Route::Project(project, ProjectPage::Settings(ProjectSettingsTab::Runs)));
  s.until("the switch", |s| s.shows(SWITCH));
  assert_eq!(setting(&repo), AllowAutopilot::OFF, "off by default");
  // Even with every switch of the person's on, a client's press is refused.
  lets_all(&s);
  s.click_by_client(SWITCH).expect("the click is delivered");
  s.until("the refusal", |s| s.shows(REFUSAL));
  for _ in 0..5 {
    s.pass();
  }
  assert_eq!(setting(&repo), AllowAutopilot::OFF);
  s.click_by_person(SWITCH);
  s.until("on by the person", |_| setting(&repo) == AllowAutopilot::ON);
  s.until("the refusal cleared", |s| !s.shows(REFUSAL));
  // Turning it off is anyone's.
  s.click_by_client(SWITCH).expect("the click is delivered");
  s.until("off by the client", |_| setting(&repo) == AllowAutopilot::OFF);
}

#[test]
fn approval_mode_effective_set_approval_settings_never_allows_autopilot_and_always_disallows_it() {
  let (mut s, project, repo) = controlled();
  let on = json!({"project": project.to_string(), "allow_autopilot": true});
  let off = json!({"project": project.to_string(), "allow_autopilot": false});
  assert_eq!(s.refused("set_approval_settings", &on)["code"], "person_only");
  lets_all(&s);
  assert_eq!(s.refused("set_approval_settings", &on)["code"], "person_only");
  assert_eq!(setting(&repo), AllowAutopilot::OFF);
  ProjectDb::open(&repo)
    .expect("db")
    .save_allow_autopilot(AllowAutopilot::ON)
    .expect("a person allows it");
  let read = s.ok("get_approval_settings", &json!({"project": project.to_string()}));
  assert_eq!(read["approvals"]["allow_autopilot"], true, "{read}");
  // Asking for what it is already changes nothing.
  s.ok("set_approval_settings", &on);
  let turned = s.ok("set_approval_settings", &off);
  assert_eq!(turned["approvals"]["allow_autopilot"], false, "{turned}");
  assert_eq!(setting(&repo), AllowAutopilot::OFF);
}

#[test]
fn approval_mode_effective_switch_run_to_manual_is_taken_without_the_persons_switches() {
  let (mut served, seeded) = Served::seeded("cards");
  let approvals = served.ok("list_approvals", &json!({ "project": "Kontur" }));
  let number = approvals["approvals"][0]["run"]["number"]
    .as_u64()
    .unwrap_or_else(|| panic!("a run waiting for the person: {approvals}"))
    .to_string();
  let arguments = json!({ "project": "Kontur", "run": number });
  let switched = served.ok("switch_run_to_manual", &arguments);
  assert_eq!(
    switched["run"]["approval_mode"],
    json!({"mode": "manual", "because": "profile"}),
    "{switched}"
  );
  let run = served.ok("get_run", &arguments);
  assert_eq!(run["approval_mode"]["mode"], "manual", "{run}");
  let id = switched["run"]["id"]
    .as_str()
    .expect("the run's id")
    .parse()
    .expect("a run id");
  assert!(
    ProjectDb::open(&seeded.repo)
      .expect("db")
      .run_switched_to_manual(id)
      .expect("read")
  );
  // Switching again is taken too: a run only ever drops to Manual.
  served.ok("switch_run_to_manual", &arguments);
}
