//! Auto mode's *approve everything in each run* (ORC-199) is a person's to turn on: an MCP client's press of Project
//! settings' switch, or `set_auto_mode`'s `approve_everything`, turns it on only while a person lets automation
//! answer approvals and change agent permissions; turning it off is always taken. `get_auto_mode` says who turned it
//! on.
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::run::{Answerer, ApproveEverything};
use orchester_store::ProjectDb;
use serde_json::json;

use super::connections_window::controlled;

const SWITCH: &str = "settings-auto-everything";

fn setting(repo: &std::path::Path) -> ApproveEverything {
  ProjectDb::open(repo)
    .expect("db")
    .approve_everything()
    .expect("setting")
}

/// What a person lets automation do, in Settings.
fn lets(s: &super::Served, (answers, permissions): (bool, bool)) {
  let workspace = s.launch.as_ref().expect("launch").workspace().expect("workspace");
  workspace
    .set_automation_answers_approvals(answers)
    .expect("answer approvals");
  workspace
    .set_automation_changes_permissions(permissions)
    .expect("change agent permissions");
}

#[test]
fn approve_everything_switch_turns_on_by_client_input_only_under_the_persons_switches() {
  let (mut s, project, repo) = controlled();
  s.until("navigation attached", |s| s.link.navigation().is_ok());
  s.link
    .navigation()
    .expect("navigation")
    .go(Route::Project(project, ProjectPage::Settings(ProjectSettingsTab::Runs)));
  s.until("the switch", |s| s.shows(SWITCH));
  assert_eq!(setting(&repo), ApproveEverything::OFF, "off by default");
  s.click_by_client(SWITCH).expect("the click is delivered");
  s.until("the refusal", |s| s.shows("settings-auto-everything-person-only"));
  for _ in 0..5 {
    s.pass();
  }
  assert_eq!(setting(&repo), ApproveEverything::OFF);
  s.click_by_person(SWITCH);
  s.until("on by the person", |_| {
    setting(&repo) == ApproveEverything::on(Answerer::Person)
  });
  s.until("the refusal cleared", |s| {
    !s.shows("settings-auto-everything-person-only")
  });
  // Turning it off is anyone's.
  s.click_by_client(SWITCH).expect("the click is delivered");
  s.until("off by the client", |_| setting(&repo) == ApproveEverything::OFF);
  // Letting automation change agent permissions alone is not enough: approving everything answers approvals too.
  lets(&s, (false, true));
  s.click_by_client(SWITCH).expect("the click is delivered");
  s.until("refused again", |s| s.shows("settings-auto-everything-person-only"));
  assert_eq!(setting(&repo), ApproveEverything::OFF);
  lets(&s, (true, true));
  s.click_by_client(SWITCH).expect("the click is delivered");
  s.until("on by the client", |_| {
    setting(&repo) == ApproveEverything::on(Answerer::Automation)
  });
}

#[test]
fn approve_everything_through_set_auto_mode_needs_both_switches_and_off_is_always_taken() {
  let (mut s, project, repo) = controlled();
  let on = json!({"project": project.to_string(), "approve_everything": true});
  let off = json!({"project": project.to_string(), "approve_everything": false});
  assert_eq!(s.refused("set_auto_mode", &on)["code"], "automation_not_allowed");
  lets(&s, (true, false));
  assert_eq!(s.refused("set_auto_mode", &on)["code"], "person_only");
  assert_eq!(setting(&repo), ApproveEverything::OFF);
  lets(&s, (true, true));
  let turned = s.ok("set_auto_mode", &on);
  assert_eq!(
    turned["auto_mode"]["approve_everything"],
    json!({"on": true, "turned_on_by": "automation"}),
    "{turned}"
  );
  assert_eq!(setting(&repo), ApproveEverything::on(Answerer::Automation));
  lets(&s, (false, false));
  let read = s.ok("get_auto_mode", &json!({"project": project.to_string()}));
  assert_eq!(read["auto_mode"]["approve_everything"]["on"], true, "{read}");
  let turned = s.ok("set_auto_mode", &off);
  assert_eq!(
    turned["auto_mode"]["approve_everything"],
    json!({"on": false, "turned_on_by": null}),
    "{turned}"
  );
  assert_eq!(setting(&repo), ApproveEverything::OFF);
}
