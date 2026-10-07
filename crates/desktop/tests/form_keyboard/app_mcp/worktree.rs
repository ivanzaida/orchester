//! `remove_worktree` and the worktree in `get_task` and `get_run`, on the real runner,
//! `orchester-fake-codex` and git: an ended run's worktree is removed as the task page's *Remove
//! worktree* removes it, and the page shows it; and the refusals: a removal done already, uncommitted
//! changes (a person decides those on the page), a removal running on the page, and a run at work.

use std::{fs, path::PathBuf, sync::atomic::Ordering};

use orchester_core::{run::Run, store::WorktreeStore};
use orchester_desktop::programs::Programs;
use orchester_store::ProjectDb;
use serde_json::json;

use super::{agent, ok, refused};
use crate::{
  harness::Desktop,
  integration::blocked_git,
  task_run::{create_and_run, has, init_repository, latest_run, programs, seed},
};

fn worktree_of(desktop: &Desktop, run: &Run) -> PathBuf {
  desktop
    .repo()
    .join(".orchester")
    .join("worktrees")
    .join(run.id().to_string())
}

/// A task whose run #1 failed on the fake Codex and kept its worktree, its page open.
fn failed_run(program_set: Programs) -> (Desktop, Run) {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_programs_and_app_mcp(runner, program_set, init_repository, |repo| {
    seed(repo, &fake, false);
  });
  create_and_run(&mut desktop, "Report failure.", "1400000");
  desktop.wait_until("run #1 failed with its worktree", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status().can_run_again()) && has(desktop, "task-remove-worktree")
  });
  let run = latest_run(desktop.repo()).expect("run #1");
  (desktop, run)
}

const RUN: &str = "#1";

#[test]
fn app_mcp_remove_worktree_removes_an_ended_runs_worktree_as_the_task_page_does() {
  let (mut desktop, run) = failed_run(Programs::processes());
  let mut client = agent(&desktop);
  let in_run = json!({"project": "Kontur", "run": RUN});
  assert_eq!(ok(&mut client, "get_run", &in_run)["worktree"], "present");
  let task = ok(&mut client, "get_task", &json!({"project": "Kontur", "key": "KON-1"}));
  assert_eq!(task["runs"][0]["worktree"], "present");

  let removed = ok(&mut client, "remove_worktree", &in_run);
  assert_eq!(removed["run"]["worktree"], "removed");
  assert_eq!(removed["worktrees_removed"], 1);
  assert!(!worktree_of(&desktop, &run).exists());
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert!(
    db.worktree_removal(run.id()).expect("read").is_some(),
    "recorded as the page records it"
  );
  // The task page shows it: no Remove worktree any more.
  desktop.wait_until("the page without Remove worktree", |desktop| {
    !has(desktop, "task-remove-worktree")
  });
  assert_eq!(ok(&mut client, "get_run", &in_run)["worktree"], "removed");
  let task = ok(&mut client, "get_task", &json!({"project": "Kontur", "key": "KON-1"}));
  assert_eq!(task["runs"][0]["worktree"], "removed");
  assert_eq!(
    refused(&mut client, "remove_worktree", &in_run)["code"],
    "already_removed"
  );
}

#[test]
fn app_mcp_remove_worktree_leaves_uncommitted_changes_to_a_person() {
  let (desktop, run) = failed_run(Programs::processes());
  let worktree = worktree_of(&desktop, &run);
  fs::write(worktree.join("notes.txt"), "not committed\n").expect("an uncommitted file");
  let mut client = agent(&desktop);
  let answer = refused(
    &mut client,
    "remove_worktree",
    &json!({"project": "Kontur", "run": RUN}),
  );
  assert_eq!(answer["code"], "uncommitted_changes");
  assert!(worktree.join("notes.txt").is_file(), "nothing is removed");
}

#[test]
fn app_mcp_remove_worktree_waits_for_a_removal_the_task_page_runs() {
  let (program_set, gate) = blocked_git(&["worktree", "remove"]);
  let (mut desktop, run) = failed_run(program_set);
  desktop.click("task-remove-worktree");
  desktop.wait_for("remove-worktree-dialog");
  if has(&mut desktop, "remove-worktree-discard") {
    desktop.click("remove-worktree-discard");
  }
  desktop.click("remove-worktree-confirm");
  desktop.wait_until("git removing the worktree", |_| gate.entered.load(Ordering::SeqCst));
  let mut client = agent(&desktop);
  let answer = refused(
    &mut client,
    "remove_worktree",
    &json!({"project": "Kontur", "run": RUN}),
  );
  assert_eq!(answer["code"], "merge_running");
  gate.release();
  desktop.wait_until("the page's removal", |desktop| !has(desktop, "remove-worktree-dialog"));
  assert!(!worktree_of(&desktop, &run).exists());
}

#[test]
fn app_mcp_remove_worktree_refuses_a_run_at_work() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the run at work", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status().has_live_work())
  });
  let mut client = agent(&desktop);
  let answer = refused(
    &mut client,
    "remove_worktree",
    &json!({"project": "Kontur", "run": RUN}),
  );
  assert_eq!(answer["code"], "run_not_ended");
  desktop.click("task-cancel-run");
}
