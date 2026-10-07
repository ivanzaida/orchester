//! The code index and the tokens before the first edit in the Conversation tab's Details (ORC-38 `BbGhA`,
//! `rd1Vo`, `F3XhQ`), headlessly: the real runner runs the step on `orchester-fake-codex`, which
//! reports a file change, with the project's code index turned on or off (the default), relative to the
//! worktree or at its absolute path there.

use std::{path::Path, process::Command};

use orchester_core::{
  code_index::CodeIndexSetting,
  store::{ActivityStore, SettingsStore},
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{create_and_run, finished_on_page, has, init_repository, latest_run, programs, seed},
};

const PANEL: &str = "activity-invocation";

/// The finished run's Conversation tab with its Details pane open.
fn activity(desktop: &mut Desktop) {
  activity_of(desktop, "Append one line to README.md.");
}

/// [`activity`] of a run whose agent was told `brief`.
fn activity_of(desktop: &mut Desktop, brief: &str) {
  create_and_run(desktop, brief, "1400000");
  finished_on_page(desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_until("the timeline", |desktop| has(desktop, "activity-step-header"));
  desktop.click("conversation-details");
  desktop.wait_for(PANEL);
}

fn short_head(desktop: &Desktop) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(desktop.repo())
    .args(["rev-parse", "--short=7", "main"])
    .output()
    .expect("run git");
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn activity_shows_the_code_index_and_the_tokens_before_the_first_edit() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed(repo, &fake, false);
    ProjectDb::open(repo)
      .expect("open")
      .save_code_index(CodeIndexSetting::On)
      .expect("turn the code index on");
  });
  activity(&mut desktop);

  assert!(shows(&mut desktop, "activity-step-header", " before first edit"));
  assert!(shows(&mut desktop, PANEL, "Code index"));
  let index = format!("{} · main", short_head(&desktop));
  assert!(shows(&mut desktop, PANEL, &index));
  assert!(shows(&mut desktop, PANEL, "Tokens before first edit"));
  assert!(shows(&mut desktop, PANEL, "README.md"));
  assert!(shows(&mut desktop, PANEL, "Before first edit adds up the turns"));
  assert!(shows(
    &mut desktop,
    "activity-invocation-title",
    "01 Implement · attempt 1"
  ));
  // The note ends the panel's content above its padding, at the bottom of the panel or, when the
  // content is taller, where the panel scrolls to.
  let (panel, content, note) = (
    desktop.bounds(PANEL),
    desktop.bounds("activity-invocation-content"),
    desktop.bounds("activity-invocation-note"),
  );
  let under = (content.y + content.height) - (note.y + note.height);
  assert!((19.0..=21.0).contains(&under), "{under} px under the note");
  assert!(content.y + content.height >= panel.y + panel.height - 0.5);

  // With the step picked in the rail, the panel is named after it and says when it ran: Steps
  // brings the rail back in the pane's place, and Details opens on the step picked.
  desktop.click("conversation-details-steps");
  desktop.wait_for("conversation-rail-step-1");
  desktop.click("conversation-rail-step-1");
  desktop.click("conversation-details");
  desktop.wait_until("the panel follows the step", |desktop| {
    shows(desktop, "activity-invocation-title", "01 Implement · attempt 1")
  });
  assert!(shows(&mut desktop, PANEL, "Ran"));
  assert!(shows(&mut desktop, PANEL, "Tokens before first edit"));
}

/// The first edit's path as the runner recorded it.
fn recorded_first_edit(repo: &Path) -> String {
  let run = latest_run(repo).expect("the run");
  let db = ProjectDb::open(repo).expect("open the project");
  db.invocations(run.id())
    .expect("invocations")
    .iter()
    .find_map(|invocation| db.edit_progress(invocation.id).expect("edit progress")?.first_edit)
    .expect("a first edit")
    .path
}

/// Codex and Claude Code report a changed file at its absolute path in the worktree
/// (`H:\…\.orchester/worktrees\<run>\README.md` on Windows): the row names it as the repository does.
fn shows_the_first_edit_relative_to_the_worktree(mut desktop: Desktop) {
  activity_of(&mut desktop, "Report the absolute path. Append one line to README.md.");

  let recorded = recorded_first_edit(desktop.repo());
  assert!(Path::new(&recorded).is_absolute(), "{recorded}");
  assert!(recorded.contains("worktrees"), "{recorded}");
  assert!(shows(&mut desktop, PANEL, " · README.md"));
  assert!(!shows(&mut desktop, PANEL, &recorded));
}

#[test]
fn activity_shows_the_first_edit_relative_to_the_worktree() {
  let (runner, fake) = programs();
  let desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  shows_the_first_edit_relative_to_the_worktree(desktop);
}

/// A repository inside another project's run worktree: the path is relative to the run's own.
#[test]
fn activity_shows_the_first_edit_relative_to_the_innermost_worktree() {
  let (runner, fake) = programs();
  let folder = "outer/.orchester/worktrees/01a0f437-outer/kontur";
  let desktop = Desktop::with_runner_at(folder, runner, init_repository, |repo| seed(repo, &fake, false));
  shows_the_first_edit_relative_to_the_worktree(desktop);
}

#[test]
fn activity_says_the_code_index_was_off() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  activity(&mut desktop);

  assert!(shows(&mut desktop, PANEL, "Off · project setting"));
  assert!(shows(&mut desktop, PANEL, "Tokens before first edit"));
}
