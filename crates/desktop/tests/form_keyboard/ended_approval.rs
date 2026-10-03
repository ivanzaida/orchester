//! A request whose agent can no longer receive an answer, headlessly with the real runner: the step's
//! agent (`orchester-fake-claude`) asks twice, and a request recorded for the same invocation that the
//! agent does not wait on is answered. The runner ends it and refuses the answer; the dialog drops it
//! at once for the agent's own request, says the previous one ended, and Needs you counts one less.

use orchester_core::{connection::AgentProvider, run::PendingToolApproval};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, fake_claude, has, init_repository, programs, seed_agent},
};

const DIALOG: &str = "tool-approval-dialog";

/// The waiting requests of the project, oldest first.
fn waiting(desktop: &Desktop) -> Vec<PendingToolApproval> {
  ProjectDb::open(desktop.repo())
    .expect("open")
    .pending_tool_approvals_all()
    .expect("requests")
}

#[test]
fn ended_approval_leaves_the_dialog_for_the_next_and_needs_you_counts_one_less() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(
        builder,
        "implement",
        "Implement",
        "Run `cargo add serde` twice.",
        &[],
      )]
    });
  });
  create_and_run(&mut desktop, "Add serde.", "1400000");
  desktop.wait_for(DIALOG);
  let first = waiting(&desktop).remove(0);
  // Recorded for the agent's invocation, but not the call it waits on: no agent can take its answer.
  let unheld = PendingToolApproval {
    id: format!("{}:unheld", first.invocation),
    action: "pnpm unheld-check".to_owned(),
    ..first.clone()
  };
  ProjectDb::open(desktop.repo())
    .expect("open")
    .request_tool_approval(&unheld)
    .expect("recorded");
  // Approving the agent's first call reads the requests again; the agent then asks a second time.
  desktop.press("tool-approval-approve-once");
  desktop.wait_until("the unheld request and the agent's second", |desktop| {
    has(desktop, DIALOG) && shows(desktop, "tool-approval-position", "1 of 2 waiting")
  });
  assert!(shows(&mut desktop, DIALOG, "pnpm unheld-check"));
  assert!(shows(&mut desktop, "status-needs-you", "2"));

  desktop.press("tool-approval-approve-once");
  desktop.wait_until("the dialog moves to the agent's request", |desktop| {
    has(desktop, DIALOG) && shows(desktop, "tool-approval-position", "1 of 1 waiting")
  });
  // The fake's second call adds another package.
  assert!(shows(&mut desktop, DIALOG, "cargo add tokio"));
  assert!(!shows(&mut desktop, DIALOG, "unheld-check"));
  assert!(shows(
    &mut desktop,
    "tool-approval-actions",
    "The previous request ended without an answer"
  ));
  desktop.wait_until("Needs you counts the agent's request only", |desktop| {
    shows(desktop, "status-needs-you", "1")
  });
  let left = waiting(&desktop);
  assert_eq!(left.len(), 1);
  assert_ne!(left[0].id, unheld.id);
  let decided = ProjectDb::open(desktop.repo())
    .expect("open")
    .decided_tool_approvals(first.run)
    .expect("decided");
  let ended = decided
    .iter()
    .find(|decided| decided.request.id == unheld.id)
    .expect("recorded");
  assert_eq!(ended.decision, None, "it ended without an answer");

  // Esc closes the dialog whatever happened before.
  desktop.key("Escape", "Escape");
  desktop.wait_until("Esc closes the dialog", |desktop| !has(desktop, DIALOG));
}
