//! Tools change settings, runs, pipelines and worktrees in the background without disturbing the
//! person: Project settings shows a changed setting in place and a person's later change keeps the
//! tool's; a run's auto mode switch follows the tool; Pipelines lists a copy in place, keeping what
//! the person typed in its filter; an open Remove worktree dialog stays open and shows the worktree
//! removed; and a tool's failed removal shows nowhere but in its result. Run with
//! `cargo test -p orchester-desktop --test form_keyboard app_mcp_quiet`.

use std::{
  sync::{Arc, atomic::AtomicBool},
  time::{Duration, Instant},
};

use orchester_desktop::programs::Programs;
use orchester_store::AppDb;
use serde_json::json;

use super::{
  agent, ok, refused,
  worktree::{failed_run, worktree_of},
};
use crate::{
  approval_choices::open_task,
  auto_mode::{SWITCH, codex_connection, run_auto_mode, seeded, settings},
  code_index::{OFF, code_index_tab},
  github::{contains_text, shows},
  harness::Desktop,
  run_history::refusing_git,
  task_run::{has, seed_claude, set_text},
};

/// Lets automation answer approvals, which auto mode's tools need.
fn let_automation_answer(desktop: &Desktop) {
  AppDb::open(&desktop.repo().parent().expect("the test folder").join("orchester.db"))
    .expect("open")
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
}

/// Lets the window read what changed for `pause`.
fn settle_for(desktop: &mut Desktop, pause: Duration) {
  let start = Instant::now();
  desktop.wait_until("the window reads the change", |_| start.elapsed() >= pause);
}

#[test]
fn app_mcp_quiet_code_index_tab_shows_the_setting_a_tool_changed() {
  let mut desktop = Desktop::with_seeded_project_and_app_mcp(|_| {});
  code_index_tab(&mut desktop);
  let mut client = agent(&desktop);
  ok(
    &mut client,
    "set_code_index",
    &json!({"project": "Kontur", "enabled": false}),
  );
  desktop.wait_until("the tab shows it off", |desktop| has(desktop, OFF));
  ok(
    &mut client,
    "set_code_index",
    &json!({"project": "Kontur", "enabled": true}),
  );
  desktop.wait_until("the tab shows it on", |desktop| !has(desktop, OFF));
  assert!(!has(&mut desktop, "settings-code-index-changed-outside"));
}

#[test]
fn app_mcp_quiet_auto_mode_settings_show_a_tools_change_and_a_later_change_keeps_it() {
  let mut codex = None;
  let mut desktop = Desktop::with_seeded_project_and_app_mcp(|repo| {
    seed_claude(repo, &repo.join("orchester-fake-codex"));
    codex = Some(codex_connection(repo));
  });
  let codex = codex.expect("seeded");
  let_automation_answer(&desktop);
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for("settings-auto-mode-new-runs");
  let mut client = agent(&desktop);
  ok(
    &mut client,
    "set_auto_mode",
    &json!({"project": "Kontur", "reviewer": {"connection": "Codex · Review"}}),
  );
  desktop.wait_until("the section names the tool's reviewer", |desktop| {
    shows(desktop, "settings-auto-mode-reviewer", "gpt-5.5-codex")
  });
  // The person's change saves only what they changed: the tool's reviewer stays.
  desktop.click("settings-auto-mode-new-runs");
  let repo = desktop.repo().to_path_buf();
  desktop.wait_until("new runs use auto mode", |_| settings(&repo).new_runs);
  assert_eq!(settings(&repo).reviewer.map(|choice| choice.connection), Some(codex));
}

#[test]
fn app_mcp_quiet_run_switch_follows_the_tool() {
  let (mut desktop, run) = seeded(true, None);
  desktop.key("Escape", "Escape");
  open_task(&mut desktop);
  desktop.wait_for(SWITCH);
  let_automation_answer(&desktop);
  let mut client = agent(&desktop);
  ok(
    &mut client,
    "set_run_auto_mode",
    &json!({"project": "Kontur", "run": run.to_string(), "enabled": true}),
  );
  settle_for(&mut desktop, Duration::from_millis(500));
  // The switch reads on now: a person's click turns it off.
  desktop.click(SWITCH);
  let repo = desktop.repo().to_path_buf();
  desktop.wait_until("the person's click turns it off", |_| !run_auto_mode(&repo, run));
}

#[test]
fn app_mcp_quiet_pipelines_list_a_copy_in_place() {
  let mut desktop =
    Desktop::with_seeded_project_and_app_mcp(|repo| seed_claude(repo, &repo.join("orchester-fake-codex")));
  desktop.wait_for("nav-pipelines");
  desktop.click("nav-pipelines");
  desktop.wait_for("pipelines-new");
  // What the person has open in the library: New pipeline, with a name typed.
  desktop.click("pipelines-new");
  desktop.wait_for("new-pipeline-name");
  set_text(&mut desktop, "new-pipeline-name", "Hotfix");
  let mut client = agent(&desktop);
  ok(
    &mut client,
    "duplicate_pipeline",
    &json!({"project": "Kontur", "pipeline": "Delivery", "name": "Delivery copy"}),
  );
  desktop.wait_until("the copy in the library", |desktop| {
    desktop
      .tree
      .root()
      .is_some_and(|root| contains_text(root, "Delivery copy"))
  });
  assert!(has(&mut desktop, "new-pipeline-dialog"), "New pipeline stays open");
  assert_eq!(
    desktop.value("new-pipeline-name"),
    "Hotfix",
    "with what the person typed"
  );
}

#[test]
fn app_mcp_quiet_remove_worktree_keeps_the_open_dialog_and_shows_it_removed() {
  let (mut desktop, run) = failed_run(Programs::processes());
  desktop.click("task-remove-worktree");
  desktop.wait_for("remove-worktree-dialog");
  let mut client = agent(&desktop);
  ok(
    &mut client,
    "remove_worktree",
    &json!({"project": "Kontur", "run": "#1"}),
  );
  assert!(!worktree_of(&desktop, &run).exists());
  desktop.wait_until("the dialog says it was removed", |desktop| {
    has(desktop, "remove-worktree-removed")
  });
  assert!(has(&mut desktop, "remove-worktree-dialog"), "the dialog stays open");
  desktop.click("remove-worktree-close");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, "remove-worktree-dialog"));
}

#[test]
fn app_mcp_quiet_failed_removal_shows_only_in_the_tools_result() {
  let (mut desktop, run) = failed_run(refusing_git(Arc::new(AtomicBool::new(true))));
  let mut client = agent(&desktop);
  let answer = refused(
    &mut client,
    "remove_worktree",
    &json!({"project": "Kontur", "run": "#1"}),
  );
  assert_eq!(answer["code"], "removal_failed");
  assert!(worktree_of(&desktop, &run).is_dir());
  settle_for(&mut desktop, Duration::from_millis(300));
  assert!(
    desktop
      .activity
      .snapshot()
      .recent()
      .iter()
      .all(|event| !event.is_unresolved()),
    "no failure in the status bar"
  );
  assert!(!has(&mut desktop, "window-tab-attention-1"), "no dot on the tab");
  assert!(
    !has(&mut desktop, &format!("run-remove-error-{}", run.id())),
    "nothing under the run's row"
  );
}
