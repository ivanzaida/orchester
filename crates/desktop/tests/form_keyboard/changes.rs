//! The task page's Changes tab and Ready for review (02.13 `vpgrJ`) end to end, headlessly, on the
//! real runner and `orchester-fake-codex`: the files of the run's diff to its latest checkpoint and
//! of its worktree beyond it, a new checkpoint shown while the tab is open, the empty tab before
//! the first checkpoint, an Activity file change opening its file, and the state of a run whose
//! every step passed. Windows only, like the runner's named pipe.
#![cfg(windows)]

use std::fs;

use orchester_core::run::RunStatus;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{create_and_run, finished_on_page, has, init_repository, latest_run, programs, run_finished, seed},
};

/// Whether `id` is shown and holds `text`.
fn holds(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

#[test]
fn changes_tab_follows_new_checkpoints_and_lists_work_not_checkpointed() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, true));
  create_and_run(&mut desktop, "Work slowly.", "1400000");
  desktop.wait_until("Pause run", |desktop| has(desktop, "task-pause-run"));
  desktop.click("task-pause-run");
  desktop.wait_until("Resume run", |desktop| has(desktop, "task-resume-run"));
  // Paused at Implement's checkpoint; a file written in the run's worktree since is not in it.
  let run = latest_run(desktop.repo()).expect("run");
  let worktree = desktop
    .repo()
    .join(".orchester")
    .join("worktrees")
    .join(run.id().to_string());
  fs::write(worktree.join("notes.md"), "draft\n").expect("write into the worktree");
  desktop.click("task-tab-changes");
  desktop.wait_until("the checkpointed and uncheckpointed files", |desktop| {
    holds(desktop, "diff-file:README.md", "M · +1 / −0") && has(desktop, "diff-worktree-file:notes.md")
  });
  assert!(shows(&mut desktop, "diff-files", "FILES  1"));
  assert!(shows(&mut desktop, "diff-files", "NOT CHECKPOINTED  1"));
  assert!(shows(&mut desktop, "task-tab-changes", "1"));
  assert!(shows(&mut desktop, "changes-toolbar", "Checkpointed"));
  assert!(shows(
    &mut desktop,
    "changes-checkpoint",
    "from its base to its latest checkpoint · Implement"
  ));
  assert!(shows(&mut desktop, "diff-view", "hello"));
  desktop.click("diff-worktree-file:notes.md");
  desktop.wait_until("the file not checkpointed", |desktop| {
    holds(desktop, "changes-toolbar", "Not checkpointed") && holds(desktop, "diff-file-header", "notes.md")
  });
  assert!(shows(&mut desktop, "changes-range", "→ worktree"));
  assert!(shows(&mut desktop, "diff-view", "draft"));
  // Review's checkpoint takes the file and another line; the open tab shows it.
  desktop.click("task-resume-run");
  desktop.wait_until("the new checkpoint on the open tab", |desktop| {
    holds(desktop, "diff-files", "FILES  2")
      && holds(desktop, "diff-file:README.md", "M · +2 / −0")
      && !has(desktop, "diff-worktree-file:notes.md")
  });
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  ready_for_review(&mut desktop);
}

/// The Summary of the run above, which succeeded: Ready for review (02.13).
fn ready_for_review(desktop: &mut Desktop) {
  let run = latest_run(desktop.repo()).expect("run");
  let head = run.last_checkpoint().expect("a checkpoint").revision().as_str()[..7].to_owned();
  desktop.click("task-tab-summary");
  desktop.wait_until("Ready for review", |desktop| {
    has(desktop, "task-ready") && holds(desktop, "task-ready-scope", "2 files changed  ·  +3 / −0")
  });
  assert!(shows(desktop, "task-heading", "Ready for review"));
  assert!(shows(desktop, "task-ready-meta", "Run #1  ·  Delivery  ·  Finished at"));
  assert!(shows(desktop, "task-ready-notice", "Ready for your review"));
  assert!(shows(desktop, "task-ready-stages", "Implement"));
  assert!(shows(desktop, "task-ready-stages", "Review"));
  assert!(shows(desktop, "task-ready-revision", &format!("commit {head}")));
  assert!(shows(desktop, "task-ready-details", "orchester/kon-1-run-1"));
  assert!(shows(desktop, "task-ready-details", &run.base().as_str()[..7]));
  assert!(shows(desktop, "task-ready-agents", "Builder"));
  assert!(shows(
    desktop,
    "task-ready-unavailable",
    "which no pipeline step opens yet"
  ));
  // Review PR does nothing yet; Ask agent opens the task conversation.
  desktop.click("task-review-pull-request");
  assert!(has(desktop, "task-ready"));
  desktop.click("task-ask-agent");
  desktop.wait_for("task-conversation-panel");
}

#[test]
fn changes_tab_is_empty_before_the_first_checkpoint() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("Pause run", |desktop| has(desktop, "task-pause-run"));
  desktop.click("task-tab-changes");
  desktop.wait_until("the empty tab", |desktop| {
    holds(desktop, "changes-empty", "No checkpoint yet")
  });
  assert!(shows(
    &mut desktop,
    "changes-empty",
    "Run 1 has not committed any work yet."
  ));
  desktop.click("task-cancel-run");
  desktop.wait_until("the run to be cancelled", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Cancelled)
  });
}

#[test]
fn changes_tab_opens_on_the_file_an_activity_entry_changed() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  // Its step starts collapsed only once the page shows the run finished: pressing the header of
  // a step the page still shows at work would close it.
  finished_on_page(&mut desktop);
  desktop.click("task-tab-activity");
  desktop.wait_until("the transcript", |desktop| has(desktop, "activity-step-header"));
  desktop.click("activity-step-header");
  desktop.wait_until("the file change", |desktop| {
    has(desktop, "activity-open-file:README.md")
  });
  assert!(shows(
    &mut desktop,
    "activity-open-file:README.md",
    "M · open in Changes"
  ));
  desktop.click("activity-open-file:README.md");
  desktop.wait_until("the file in the Changes tab", |desktop| {
    has(desktop, "task-changes") && holds(desktop, "diff-file-header", "README.md")
  });
  assert!(shows(&mut desktop, "diff-view", "hello"));
  // Back returns to the transcript.
  desktop
    .tree
    .mouse_up(720.0, 480.0, lurq::app::events::MouseButton::Other(4));
  desktop.wait_until("the transcript again", |desktop| {
    has(desktop, "activity-step-header") && !has(desktop, "task-changes")
  });
}
