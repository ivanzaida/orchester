//! A run on the Claude Code CLI, headlessly: the real runner runs the step on
//! `orchester-fake-claude`, a stand-in that reads and prints stream-json, and the Conversation tab
//! names the provider with the model and shows the transcript.

use orchester_core::connection::AgentProvider;

use crate::{
  conversation::open_group,
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, fake_claude, finished_on_page, has, init_repository, programs, seed_agent},
};

#[test]
fn claude_run_names_the_provider_in_the_conversation_and_shows_its_transcript() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(builder, "implement", "Implement", "Append one line.", &[])]
    });
  });
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_until("the timeline", |desktop| has(desktop, "activity-step-header"));
  assert!(shows(&mut desktop, "activity-step-header", "Claude / haiku"));
  desktop.click("conversation-details");
  desktop.wait_for("activity-invocation");
  assert!(shows(&mut desktop, "activity-invocation", "Builder · Claude / haiku"));
  desktop.click("activity-step-header");
  open_group(&mut desktop, "git status --short");
  desktop.wait_until("the group's entries", |desktop| {
    has(desktop, "activity-open-file:README.md")
  });
  assert!(shows(&mut desktop, "conversation", "git status --short"));
}
