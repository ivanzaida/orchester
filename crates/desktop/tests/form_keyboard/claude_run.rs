//! A run on the Claude Code CLI, headlessly: the real runner runs the step on
//! `orchester-fake-claude`, a stand-in that prints stream-json, and the Activity tab names the
//! provider with the model and shows the transcript. Windows only, like `task_run`.
#![cfg(windows)]

use orchester_core::connection::AgentProvider;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_step, create_and_run, fake_claude, has, init_repository, programs, run_finished, seed_agent, wait_long,
  },
};

#[test]
fn claude_run_names_the_provider_in_activity_and_shows_its_transcript() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(builder, "implement", "Implement", "Append one line.", &[])]
    });
  });
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  wait_long(&mut desktop, "the finished run on the page", |desktop| {
    run_finished(desktop.repo()) && shows(desktop, "task-notice", "Every step passed")
  });
  desktop.click("task-tab-activity");
  wait_long(&mut desktop, "the transcript", |desktop| {
    has(desktop, "activity-step-header")
  });
  assert!(shows(&mut desktop, "activity-step-header", "Claude / haiku"));
  assert!(shows(&mut desktop, "activity-invocation", "Builder · Claude / haiku"));
  desktop.click("activity-step-header");
  wait_long(&mut desktop, "the step's entries", |desktop| {
    has(desktop, "activity-entry")
  });
  assert!(shows(&mut desktop, "activity", "git status --short"));
}
