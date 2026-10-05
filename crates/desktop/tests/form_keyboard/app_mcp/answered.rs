//! Answers that no person gave, produced live through the runner and `orchester-fake-claude`: the
//! profile's permissions run a line whose parts each run alone, automation approves a test once and
//! denies an install, through Orchester's MCP server. Each inline approval card says who answered, in
//! the words of the run's Approvals, which count the profile's answers on their own.
//!
//! With `ORCHESTER_KEEP_ANSWERED` set to a folder that does not exist yet, the test copies the project
//! there when it is done, so the three runs can be opened in a debug build for design verification:
//! `$env:ORCHESTER_KEEP_ANSWERED = "<repo>\.tmp\answered\kontur"; cargo test -p orchester-desktop --test
//! form_keyboard app_mcp_answered`.

use std::{fs, io, path::Path};

use orchester_core::connection::AgentProvider;
use serde_json::json;

use super::{
  agent,
  approvals::{answer, ended, let_automation_answer, run_until_asked},
  ok,
};
use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, fake_claude, init_repository, programs, seed_agent},
};

/// Copies folder `from` to `to`, which must not exist yet.
fn copy_folder(from: &Path, to: &Path) -> io::Result<()> {
  fs::create_dir_all(to)?;
  for entry in fs::read_dir(from)? {
    let entry = entry?;
    let target = to.join(entry.file_name());
    if entry.file_type()?.is_dir() {
      copy_folder(&entry.path(), &target)?;
    } else {
      fs::copy(entry.path(), &target)?;
    }
  }
  Ok(())
}

/// Opens task `key`'s Conversation tab and waits until its request's card says `state`.
fn card_says(desktop: &mut Desktop, key: &str, state: &str) {
  desktop.press("nav-tasks");
  desktop.wait_for(&format!("task-row-{key}"));
  desktop.click(&format!("task-row-{key}"));
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
  desktop.wait_until(state, |desktop| shows(desktop, "conversation", state));
}

#[test]
fn app_mcp_answered_by_the_profile_and_automation_say_so() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(
        builder,
        "implement",
        "Implement",
        "Do what the task asks.",
        &[],
      )]
    });
  });
  let mut agent = agent(&desktop);
  let_automation_answer(&mut desktop, true);

  // A known-safe read and a listed `git`: the profile answers, and no one is asked.
  let brief = "Run `f=README.md; cat $f && git status`.";
  ok(
    &mut agent,
    "create_task",
    &json!({"project": "Kontur", "title": "KON-1", "brief": brief, "criteria": ["It ran"], "pipeline": "Delivery"}),
  );
  ok(&mut agent, "run_task", &json!({"project": "Kontur", "key": "KON-1"}));
  let counts = ended(&mut desktop, "KON-1");
  assert_eq!((counts.by_profile, counts.always_allowed, counts.by_you), (1, 0, 0));

  let test = run_until_asked(&mut desktop, &mut agent, ("KON-2", "Run `cargo test -p core`."));
  ok(&mut agent, "decide_approval", &answer(&test.id, "approve_once"));
  assert_eq!(ended(&mut desktop, "KON-2").by_automation, 1);
  let install = run_until_asked(&mut desktop, &mut agent, ("KON-3", "Run `cargo add serde`."));
  ok(&mut agent, "decide_approval", &answer(&install.id, "deny"));
  assert_eq!(ended(&mut desktop, "KON-3").denied_by_automation, 1);

  card_says(&mut desktop, "KON-1", "Allowed by the profile");
  card_says(&mut desktop, "KON-2", "Approved by automation");
  card_says(&mut desktop, "KON-3", "Denied by automation");
  // The agent was told that automation denied it, by no person; Tools shows the denial open.
  desktop.click("conversation-show-2");
  let told = "Told the agent: Denied by automation through Orchester's MCP server. Continue without it.";
  desktop.wait_until("what the agent was told", |desktop| {
    shows(desktop, "conversation", told)
  });

  if let Some(keep) = std::env::var_os("ORCHESTER_KEEP_ANSWERED") {
    let keep = Path::new(&keep);
    assert!(
      !keep.exists(),
      "ORCHESTER_KEEP_ANSWERED names a folder that does not exist yet"
    );
    copy_folder(desktop.repo(), keep).expect("copy the project for verification");
  }
}
