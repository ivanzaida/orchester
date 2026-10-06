//! A request's card in the Conversation tab follows its decision at once (ORC-74), headlessly with the
//! real runner: the step's agent (`orchester-fake-claude`) asks to run a command and, once answered, is
//! held (`fake-hold`), so nothing but the decision changes the run. Answered in the approval dialog, in
//! the card itself or by automation over Orchester's MCP server, or ended because its agent left, the
//! card reads as decided, without its answers, while the tab stays open. The card follows the run's
//! requests, not its transcript: it does so also while the transcript cannot be read again.
//! `cargo test -p orchester-desktop --test form_keyboard decided_card`.

use std::{fs, path::Path};

use orchester_core::connection::AgentProvider;
use orchester_store::ProjectDb;
use rusqlite::{Connection, params};

use crate::{
  app_mcp::{
    agent,
    approvals::{answer, let_automation_answer},
    ok,
  },
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, fake_claude, has, init_repository, latest_run, programs, seed_agent},
};

const DIALOG: &str = "tool-approval-dialog";
const WAITING: &str = "Waiting for your answer";

/// The project's folder of Orchester's own files, where the fake looks for `fake-hold`.
fn orchester(repo: &Path) -> std::path::PathBuf {
  repo.join(".orchester")
}

/// A run whose agent asks to run `cargo add serde` and, told how, does what `then` says; its
/// Conversation tab open after Decide later, with Orchester's MCP server when `mcp`. The card's id.
fn conversation_of_waiting(then: &str, mcp: bool) -> (Desktop, String) {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let instructions = format!("Run `cargo add serde`. {then}");
  let seed = |repo: &Path| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(builder, "implement", "Implement", &instructions, &[])]
    });
  };
  let mut desktop = if mcp {
    Desktop::with_runner_and_app_mcp(runner, init_repository, seed)
  } else {
    Desktop::with_runner(runner, init_repository, seed)
  };
  fs::write(orchester(desktop.repo()).join("fake-hold"), "hold").expect("hold the agent");
  create_and_run(&mut desktop, "Add serde.", "1400000");
  desktop.wait_for(DIALOG);
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-approval-always");
  let waiting = ProjectDb::open(desktop.repo())
    .expect("open")
    .pending_tool_approvals_all()
    .expect("requests");
  let card = format!("conversation-approval-{}", waiting[0].id);
  assert!(shows(&mut desktop, &card, WAITING));
  (desktop, card)
}

/// Waits until `card` says `state` and offers no answer.
fn decided(desktop: &mut Desktop, card: &str, state: &str) {
  desktop.wait_until(state, |desktop| has(desktop, card) && shows(desktop, card, state));
  assert!(!shows(desktop, card, WAITING));
  assert!(!has(desktop, "conversation-approval-approve"));
}

/// Appends a transcript line no event can be read from, as a damaged database would hold, so the run's
/// transcript cannot be read again.
fn unreadable(repo: &Path) {
  let run = latest_run(repo).expect("the run").id().to_string();
  let db = Connection::open(orchester(repo).join("orchester.db")).expect("open the database");
  db.execute(
    "INSERT INTO agent_events (run_id, sequence, step_execution_id, invocation_id, at, kind, payload) \
     SELECT run_id, sequence + 1000, step_execution_id, invocation_id, at, 'unreadable', '{}' \
     FROM agent_events WHERE run_id = ?1 ORDER BY sequence DESC LIMIT 1",
    params![run],
  )
  .expect("append the line");
}

#[test]
fn decided_card_follows_an_answer_given_in_the_dialog() {
  let (mut desktop, card) = conversation_of_waiting("Hold after the answer.", false);
  unreadable(desktop.repo());
  // The card's Always allow… opens the request in the dialog; Back returns to its answers.
  desktop.click("conversation-approval-always");
  desktop.wait_for("tool-approval-always-title");
  desktop.press("tool-approval-back");
  desktop.wait_for("tool-approval-approve-once");
  desktop.press("tool-approval-approve-once");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  decided(&mut desktop, &card, "Approved");
}

#[test]
fn decided_card_follows_its_own_answer() {
  let (mut desktop, card) = conversation_of_waiting("Hold after the answer.", false);
  desktop.click("conversation-approval-deny");
  decided(&mut desktop, &card, "Denied");
}

#[test]
fn decided_card_follows_an_answer_given_by_automation() {
  let (mut desktop, card) = conversation_of_waiting("Hold after the answer.", true);
  let mut agent = agent(&desktop);
  let_automation_answer(&mut desktop, true);
  let id = card.trim_start_matches("conversation-approval-");
  ok(&mut agent, "decide_approval", &answer(id, "deny"));
  decided(&mut desktop, &card, "Denied by automation");
}

#[test]
fn decided_card_follows_a_request_that_ended() {
  let (mut desktop, card) = conversation_of_waiting("Leave while waiting.", false);
  // The agent's permission prompt closes while the request waits: no answer can reach it.
  fs::write(orchester(desktop.repo()).join("fake-release"), "release").expect("let the agent leave");
  decided(&mut desktop, &card, "Ended without an answer");
}
