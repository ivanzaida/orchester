//! The run's first line while auto mode's *approve everything in each run* answers its requests (ORC-199): who
//! turned it on, and *Project settings*, where it is turned off. Gone while the run's auto mode or the setting is off.
//! `cargo test -p orchester-app --test conversation everything`.

use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::{
  run::{Answerer, ApproveEverything},
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;

use super::{all_text, click, key_of, open, seeded, to_start};

const LINE: &str = "conversation-approve-everything";

/// Every run of the seeded project with its auto mode `on`, and the project's setting as `setting`.
fn approving(repo: &std::path::Path, on: bool, setting: ApproveEverything) {
  let mut db = ProjectDb::open(repo).expect("open");
  for task in db.tasks().expect("tasks") {
    for run in db.runs_of_task(task.id()).expect("runs").into_iter().flatten() {
      db.set_run_auto_mode(run.id(), on).expect("auto mode");
    }
  }
  db.save_approve_everything(setting).expect("setting");
}

#[test]
fn everything_line_leads_the_run_while_the_setting_answers_it() {
  let (mut window, seeded) = seeded("conversation");
  approving(&seeded.repo, true, ApproveEverything::on(Answerer::Person));
  let key = key_of(&seeded, "Add retry to webhook sender");
  open(&mut window, &seeded, &key, "conversation-chat");
  to_start(&mut window);
  window.until("the line", |window| window.tree.get_element_by_id(LINE).is_some());
  assert_eq!(
    window.texts(&format!("{LINE}-text")),
    Some(vec!["Auto mode approves everything in this run · turned on by you".to_owned()])
  );
  click(&mut window, &format!("{LINE}-link"));
  window.until("Project settings", |window| {
    window.navigation().route_now()
      == Route::Project(seeded.project, ProjectPage::Settings(ProjectSettingsTab::Runs))
  });
}

#[test]
fn everything_line_names_automation_and_is_gone_without_auto_mode() {
  let (mut window, seeded) = seeded("conversation");
  approving(&seeded.repo, true, ApproveEverything::on(Answerer::Automation));
  let key = key_of(&seeded, "Add retry to webhook sender");
  open(&mut window, &seeded, &key, "conversation-chat");
  to_start(&mut window);
  window.until("the line", |window| window.tree.get_element_by_id(LINE).is_some());
  assert!(
    all_text(&mut window).contains("Auto mode approves everything in this run · turned on by automation")
  );

  let (mut window, seeded) = super::seeded("conversation");
  approving(&seeded.repo, false, ApproveEverything::on(Answerer::Person));
  open(&mut window, &seeded, &key_of(&seeded, "Add retry to webhook sender"), "conversation-chat");
  to_start(&mut window);
  window.until("the timeline's start", |window| {
    all_text(window).contains("01–04 · 4 steps done")
  });
  assert!(window.tree.get_element_by_id(LINE).is_none(), "the run does not use auto mode");
}
