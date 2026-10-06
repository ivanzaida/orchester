//! Closing the window while agents work, headlessly, with the real runner on `orchester-fake-claude`
//! (`docs/architecture/storage.md`, "Lifecycle"): a close request reaches the window's close handler
//! through lurq's MCP server, as the title bar's close control and the OS send it, and the close
//! confirmation lists the run whether its Verify agent is between turns or a request of it is under
//! auto mode's review. *Wait for agents to save* waits for the agent past the quiet close's deadline,
//! as long as it works, and ends the session with the run paused; Quit now ends it at once; Cancel
//! keeps the application open. `cargo test -p orchester-desktop --test form_keyboard close_flow`.

use std::{
  fs,
  path::Path,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  connection::AgentProvider,
  run::{RunStatus, review::AutoModeSettings},
  store::ActivityStore,
};
use orchester_desktop::runner::QUIET_STOP_TIMEOUT;
use orchester_store::{ProjectDb, SessionEnd};
use serde_json::json;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, fake_claude, has, init_repository, latest_run, programs, seed_agent},
};

/// A project whose pipeline is a Verify step on Claude Code, which asks before every command but
/// `git`, and with `then_review` a Review step after it.
fn verify_on_claude(repo: &Path, fake: &Path, then_review: bool) {
  seed_agent(repo, &fake_claude(fake), AgentProvider::ClaudeCodeCli, |builder| {
    let mut steps = vec![agent_step(builder, "verify", "Verify", "Append one line.", &[])];
    if then_review {
      steps.push(agent_step(builder, "review", "Review", "Append one line.", &["verify"]));
    }
    steps
  });
}

/// Asks the window to close as the title bar's close control does: the close handler decides.
fn request_close(desktop: &mut Desktop) {
  let answer = desktop.with_mcp_agent(|mut agent| agent.tool("lurq_interact", &json!({"action": "request_close"})));
  let answer: serde_json::Value = serde_json::from_str(&answer).expect("the answer is JSON");
  assert_eq!(
    answer["stayed_open"], true,
    "the close handler holds the close: {answer}"
  );
}

/// The close confirmation lists the run's Verify agent.
fn confirms_with_the_verify_agent(desktop: &mut Desktop) {
  let run = latest_run(desktop.repo()).expect("run").id();
  desktop.wait_for("close-dialog");
  for text in [
    "Close Orchester while agents are working?",
    "Wait for agents to save",
    "Quit now",
  ] {
    assert!(shows(desktop, "close-dialog", text), "{text}");
  }
  assert!(shows(desktop, &format!("close-step-{run}"), "Verify · Builder"));
  assert!(shows(desktop, &format!("close-status-{run}"), "Running"));
  assert_eq!(desktop.lifecycle.session_end(), None);
}

/// The run's agent is at work: its invocation started and has not ended.
fn agent_at_work(repo: &Path, turn_done: bool) -> bool {
  let Some(run) = latest_run(repo) else {
    return false;
  };
  ProjectDb::open(repo)
    .expect("open the project")
    .invocations(run.id())
    .expect("invocations")
    .iter()
    .any(|invocation| invocation.ended_at.is_none() && (!turn_done || invocation.usage.output > 0))
}

#[test]
fn close_flow_a_verify_agent_between_turns_asks_and_quit_now_ends_at_once() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| verify_on_claude(repo, &fake, false));
  create_and_run(&mut desktop, "Work in turns.", "1400000");
  desktop.wait_until("the agent between turns", |desktop| {
    agent_at_work(desktop.repo(), true) && !desktop.runners.active_work().is_empty()
  });

  request_close(&mut desktop);
  confirms_with_the_verify_agent(&mut desktop);
  desktop.click("close-quit-now");
  assert_eq!(desktop.lifecycle.session_end(), Some(SessionEnd::QuitNow));
}

#[test]
fn close_flow_a_request_under_auto_mode_review_asks_and_cancel_keeps_working() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    verify_on_claude(repo, &fake, false);
    let settings = AutoModeSettings {
      new_runs: true,
      reviewer: None,
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .save_auto_mode_settings(&settings)
      .expect("auto mode for new runs");
  });
  create_and_run(&mut desktop, "Run `cargo build`. Reviewer hangs.", "1400000");
  desktop.wait_until("a request under review", |desktop| {
    !ProjectDb::open(desktop.repo())
      .expect("open the project")
      .reviewing_tool_approvals()
      .expect("under review")
      .is_empty()
  });

  request_close(&mut desktop);
  confirms_with_the_verify_agent(&mut desktop);
  desktop.click("close-cancel");
  desktop.wait_until("the confirmation to close", |desktop| !has(desktop, "close-dialog"));
  assert_eq!(desktop.lifecycle.session_end(), None);
  assert!(!desktop.runners.active_work().is_empty());
}

#[test]
fn close_flow_wait_for_agents_waits_as_long_as_the_agent_works_then_closes_paused() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| verify_on_claude(repo, &fake, true));
  create_and_run(&mut desktop, "Finish after release.", "1400000");
  desktop.wait_until("the agent at work", |desktop| {
    agent_at_work(desktop.repo(), false) && !desktop.runners.active_work().is_empty()
  });

  request_close(&mut desktop);
  confirms_with_the_verify_agent(&mut desktop);
  desktop.click("close-wait");
  let run = latest_run(desktop.repo()).expect("run").id();
  desktop.wait_until("the waiting confirmation", |desktop| {
    shows(desktop, "close-dialog", "Saving before closing")
      && shows(desktop, &format!("close-status-{run}"), "Saving at next checkpoint")
      && shows(desktop, "close-waiting", "Closes after 1 more run")
  });
  assert!(!has(&mut desktop, "close-cancel"));
  // Past the quiet close's deadline the window still answers and the agent still works: nothing
  // terminated the runner.
  let past_deadline = Instant::now() + QUIET_STOP_TIMEOUT + Duration::from_secs(2);
  while Instant::now() < past_deadline {
    desktop.tree.tick_futures();
    desktop.pass();
    assert_eq!(desktop.lifecycle.session_end(), None);
    thread::sleep(Duration::from_millis(50));
  }
  assert!(has(&mut desktop, "close-waiting"));
  assert!(agent_at_work(desktop.repo(), false));

  fs::write(desktop.repo().join(".orchester").join("fake-release"), "").expect("release the agent");
  desktop.wait_until("the session end", |desktop| desktop.lifecycle.session_end().is_some());
  assert_eq!(desktop.lifecycle.session_end(), Some(SessionEnd::RunsPaused));
  // Verify finished; the run paused before Review started.
  let run = latest_run(desktop.repo()).expect("run");
  assert!(matches!(run.status(), RunStatus::Paused(_)), "{:?}", run.status());
  let invocations = ProjectDb::open(desktop.repo())
    .expect("open the project")
    .invocations(run.id())
    .expect("invocations");
  assert_eq!(invocations.len(), 1, "{invocations:?}");
}
