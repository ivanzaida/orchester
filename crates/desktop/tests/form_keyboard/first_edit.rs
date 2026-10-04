//! The code index and the tokens before the first edit in the Conversation tab's Details (ORC-38 `BbGhA`,
//! `rd1Vo`, `F3XhQ`), headlessly: the real runner runs the step on `orchester-fake-codex`, which
//! reports a file change, with the project's code index on (the default) or off.

use std::process::Command;

use orchester_core::{code_index::CodeIndexSetting, store::SettingsStore};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{create_and_run, finished_on_page, has, init_repository, programs, seed},
};

const PANEL: &str = "activity-invocation";

/// The finished run's Conversation tab with its Details pane open.
fn activity(desktop: &mut Desktop) {
  create_and_run(desktop, "Append one line to README.md.", "1400000");
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
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
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

#[test]
fn activity_says_the_code_index_was_off() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed(repo, &fake, false);
    ProjectDb::open(repo)
      .expect("open")
      .save_code_index(CodeIndexSetting::Off)
      .expect("turn the code index off");
  });
  activity(&mut desktop);

  assert!(shows(&mut desktop, PANEL, "Off · project setting"));
  assert!(shows(&mut desktop, PANEL, "Tokens before first edit"));
}
