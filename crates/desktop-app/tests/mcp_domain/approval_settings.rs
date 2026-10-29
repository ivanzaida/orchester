//! Approvals over MCP and in Project settings (`docs/design/approval-modes.md`): only a person allows Autopilot,
//! whatever automation may do, so an MCP client's press of the switch and `set_allow_autopilot` true are refused, and
//! turning it off is always taken; `get_approvals` reads the setting, the reviewer and each running run's mode with
//! why; `set_reviewer` needs the person's switches; `set_run_manual` is a narrowing, taken without any of the person's
//! switches, and a run never goes back; auto mode's tools answer `retired`, naming what replaces them. Filters:
//! `approval_mode_effective` (the window's switch), `approval_mode_mcp` (the tools).
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::run::AllowAutopilot;
use orchester_store::ProjectDb;
use serde_json::{Value, json};

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
fn approval_mode_mcp_set_allow_autopilot_never_allows_autopilot_and_always_disallows_it() {
  let (mut s, project, repo) = controlled();
  let on = json!({"project": project.to_string(), "allow_autopilot": true});
  let off = json!({"project": project.to_string(), "allow_autopilot": false});
  assert_eq!(s.refused("set_allow_autopilot", &on)["code"], "person_only");
  // Even with every switch of the person's on: only a person allows Autopilot.
  lets_all(&s);
  let refused = s.refused("set_allow_autopilot", &on);
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert_eq!(setting(&repo), AllowAutopilot::OFF);
  let read = s.ok("get_approvals", &json!({"project": project.to_string()}));
  assert_eq!(read["approvals"]["allow_autopilot"], false, "{read}");
  ProjectDb::open(&repo)
    .expect("db")
    .save_allow_autopilot(AllowAutopilot::ON)
    .expect("a person allows it");
  let read = s.ok("get_approvals", &json!({"project": project.to_string()}));
  assert_eq!(read["approvals"]["allow_autopilot"], true, "{read}");
  // Asking for what it is already changes nothing.
  s.ok("set_allow_autopilot", &on);
  assert_eq!(setting(&repo), AllowAutopilot::ON);
  let turned = s.ok("set_allow_autopilot", &off);
  assert_eq!(turned["approvals"]["allow_autopilot"], false, "{turned}");
  assert_eq!(setting(&repo), AllowAutopilot::OFF);
}

#[test]
fn approval_mode_mcp_get_approvals_names_the_reviewer_and_each_running_runs_mode() {
  let (mut served, _) = Served::seeded("cards");
  let read = served.ok("get_approvals", &json!({ "project": "Kontur" }));
  let approvals = &read["approvals"];
  assert_eq!(approvals["allow_autopilot"], false, "{read}");
  assert!(approvals["reviewer"].is_object(), "{read}");
  let runs = approvals["runs"].as_array().expect("the running runs");
  assert!(!runs.is_empty(), "the seeded runs at work: {read}");
  for run in runs {
    // The seed's profiles are Manual, and the project does not allow Autopilot: the profile says it first.
    assert_eq!(
      run["approval_mode"],
      json!({"mode": "manual", "because": "profile"}),
      "{run}"
    );
    assert!(run["number"].is_u64() && run["task"].is_string(), "{run}");
    assert!(
      run["link"].as_str().is_some_and(|link| link.contains("/tasks/")),
      "{run}"
    );
  }
}

#[test]
fn approval_mode_mcp_set_reviewer_needs_the_persons_switches() {
  let (mut served, _) = Served::seeded("empty");
  let default = json!({ "project": "Kontur" });
  let refused = served.refused("set_reviewer", &default);
  assert_eq!(refused["code"], "automation_not_allowed", "{refused}");
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer approvals");
  // The default reviewer, as it is already, widens nothing.
  let kept = served.ok("set_reviewer", &default);
  assert_eq!(kept["approvals"]["allow_autopilot"], false, "{kept}");
}

#[test]
fn approval_mode_mcp_set_run_manual_is_taken_without_the_persons_switches() {
  let (mut served, seeded) = Served::seeded("cards");
  let approvals = served.ok("list_approvals", &json!({ "project": "Kontur" }));
  let number = approvals["approvals"][0]["run"]["number"]
    .as_u64()
    .unwrap_or_else(|| panic!("a run waiting for the person: {approvals}"));
  let arguments = json!({ "project": "Kontur", "run": number.to_string() });
  let switched = served.ok("set_run_manual", &arguments);
  assert_eq!(switched["run"]["number"], number, "{switched}");
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
  served.ok("set_run_manual", &arguments);
  let read = served.ok("get_approvals", &json!({ "project": "Kontur" }));
  let listed = read["approvals"]["runs"]
    .as_array()
    .and_then(|runs| runs.iter().find(|run| run["number"] == number).cloned())
    .unwrap_or_else(|| panic!("the switched run is still at work: {read}"));
  assert_eq!(listed["approval_mode"]["mode"], "manual", "{listed}");
}

#[test]
fn approval_mode_mcp_retired_auto_mode_tools_name_the_tools_that_replace_them() {
  let (mut served, _) = Served::seeded("empty");
  let tools = served.tools();
  for name in ["get_approvals", "set_allow_autopilot", "set_reviewer", "set_run_manual"] {
    assert!(tools.iter().any(|tool| tool["name"] == name), "{name} is listed");
  }
  for (retired, replacement) in [
    ("get_auto_mode", "get_approvals"),
    ("set_auto_mode", "set_allow_autopilot"),
    ("set_auto_mode", "set_reviewer"),
    ("set_run_auto_mode", "set_run_manual"),
  ] {
    let listed: &Value = tools
      .iter()
      .find(|tool| tool["name"] == retired)
      .unwrap_or_else(|| panic!("{retired} stays listed for one release"));
    assert!(
      listed["description"].as_str().is_some_and(|told| told.contains(replacement)),
      "{retired}: {listed}"
    );
    // Whatever it is given, it changes nothing and says what to use instead.
    let refused = served.refused(retired, &json!({ "project": "Kontur", "run": "1", "new_runs": true }));
    assert_eq!(refused["code"], "retired", "{retired}: {refused}");
    let message = refused["message"].as_str().unwrap_or_default();
    assert!(
      message.contains(retired) && message.contains(replacement),
      "{retired}: {refused}"
    );
  }
}
