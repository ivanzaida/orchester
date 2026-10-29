//! The run's approval mode on the status line (ORC-353, `docs/design/approval-modes.md`, 04 `H6kPd`, 04b `p6tXIO`, 04c
//! `vZcoN`): the chip names the mode the running run answers its requests in and why it is Manual; while Autopilot
//! answers them, *Switch to Manual for this run* narrows the run for good, the chip then reads *Manual · this run* and
//! the Conversation records the switch. A run that ended shows no chip.
//!
//! `cargo test -p orchester-app --test task_details run_mode`

use orchester_app::routes::TaskTab;
use orchester_core::{
  permission::{
    AgentPermissions, ApprovalMode, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands,
    RepositoryAccess,
  },
  resource::ModelId,
  run::{AllowAutopilot, Answerer, Run},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::Page;

const CHIP: &str = "task-run-mode-label";
const SWITCH: &str = "task-run-mode-switch";
const ROW: &str = "conversation-switched-to-manual";

/// The latest run of the task `key`.
fn latest_run(page: &Page, key: &str) -> Run {
  let task = page.task(key);
  ProjectDb::open(&page.repo)
    .expect("the project")
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .last()
    .expect("a run")
}

/// The running task, its run's latest agent launched in Autopilot, in a project that allows it.
fn autopilot(page: &Page) -> (String, Run) {
  let key = page.key("running");
  let run = latest_run(page, &key);
  let execution = run
    .current_attempt()
    .and_then(|attempt| attempt.executions().last())
    .expect("a step execution")
    .id();
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  let invocation = db
    .start_invocation(
      run.id(),
      execution,
      &ModelId::new("claude-opus-5-5").expect("model"),
      Timestamp::now(),
    )
    .expect("an agent launch");
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  );
  permissions.set_approval_mode(ApprovalMode::Autopilot);
  db.save_invocation_permissions(
    invocation.id,
    &LaunchPermissions::new(permissions, OutsideCommands::PermissionPrompt),
  )
  .expect("launched in Autopilot");
  db.save_allow_autopilot(AllowAutopilot::ON).expect("Autopilot allowed");
  (key, run)
}

#[test]
fn run_mode_chip_names_why_a_running_run_is_manual_and_offers_no_switch() {
  let mut page = Page::seeded("task-states");
  let key = page.key("running");
  page.open(&key, TaskTab::Summary, CHIP);
  assert_eq!(page.text(CHIP), "Manual · profile");
  assert!(!page.has(SWITCH), "only Autopilot switches to Manual");
}

#[test]
fn run_mode_chip_is_gone_once_the_run_ended() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  page.open(&key, TaskTab::Summary, "task-run-line");
  assert!(!page.has(CHIP) && !page.has(SWITCH));
}

#[test]
fn run_mode_project_that_disallows_autopilot_reads_manual_project() {
  let mut page = Page::seeded("task-states");
  let (key, _) = autopilot(&page);
  ProjectDb::open(&page.repo)
    .expect("the project")
    .save_allow_autopilot(AllowAutopilot::OFF)
    .expect("Autopilot not allowed");
  page.open(&key, TaskTab::Summary, CHIP);
  assert_eq!(page.text(CHIP), "Manual · project");
  assert!(!page.has(SWITCH));
}

#[test]
fn run_mode_switch_to_manual_narrows_the_run_updates_the_chip_and_adds_the_conversation_row() {
  let mut page = Page::seeded("task-states");
  let (key, run) = autopilot(&page);
  page.open(&key, TaskTab::Summary, CHIP);
  assert_eq!(page.text(CHIP), "Autopilot");
  assert_eq!(page.text(SWITCH), "Switch to Manual for this run");
  page.window.press(SWITCH);
  page.window.until("the chip reads Manual · this run", |window| {
    window.texts(CHIP) == Some(vec!["Manual · this run".to_owned()])
  });
  assert!(!page.has(SWITCH), "no way back for the run");
  let db = ProjectDb::open(&page.repo).expect("the project");
  assert!(db.run_switched_to_manual(run.id()).expect("read"));
  let switch = db.run_manual_switch(run.id()).expect("read").expect("recorded");
  assert_eq!(switch.by, Answerer::Person);
  page.open(&key, TaskTab::Conversation, ROW);
  assert_eq!(page.text(&format!("{ROW}-text")), "Switched to Manual for this run");
  let detail = page.text(&format!("{ROW}-detail"));
  assert!(detail.starts_with("by you · "), "{detail}");
}
