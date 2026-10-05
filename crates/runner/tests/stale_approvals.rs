//! Tool requests whose agent can no longer receive an answer do not stay waiting: the agent ended
//! while one waited, an answer names one no live agent holds (the worker waits on another call, or its
//! step's worker is gone), or an earlier runner left one waiting. Each ends without an answer (recorded
//! interrupted) and an answer to it is refused; an answer to one answered already is refused as such. Against
//! `orchester-fake-claude` and `orchester-fake-codex`; needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test stale_approvals stale_approvals`.

mod support;

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  run::{Answerer, ApprovalAnswer, PendingToolApproval},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Rejection, SessionEnd};
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, pending_approval, project, scheduler, scheduler_with, wait_for};

/// Whether request `id` of `run` waits; panics unless it is recorded either waiting or ended without an
/// answer.
fn waits(root: &Path, run: RunId, id: &str) -> bool {
  let db = ProjectDb::open(root).expect("open");
  if db
    .pending_tool_approvals(run)
    .expect("requests")
    .iter()
    .any(|pending| pending.id == id)
  {
    return true;
  }
  let decided = db.decided_tool_approvals(run).expect("decided");
  let ended = decided
    .iter()
    .find(|decided| decided.request.id == id)
    .expect("recorded");
  assert_eq!(ended.decision, None, "{id} ended without an answer");
  false
}

/// `request` again under the id `id`, as if recorded for a call no agent waits on.
fn recorded(root: &Path, request: &PendingToolApproval, id: &str) -> String {
  let stale = PendingToolApproval {
    id: format!("{}:{id}", request.invocation),
    requested_at: Timestamp::now(),
    ..request.clone()
  };
  ProjectDb::open(root)
    .expect("open")
    .request_tool_approval(&stale)
    .expect("recorded");
  stale.id
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_approvals_end_when_no_live_agent_holds_them() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AskBefore,
      NetworkAccess::Off,
    )),
    ..Setup::new("Ask before publishing.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending_approval(root, &mut events, id).await;

  // A request the agent at work does not wait on ends at its answer; the agent's own keeps waiting.
  let unheld = recorded(root, &request, "unheld");
  let answer = scheduler
    .decide_tool_approval(id, unheld.clone(), ApprovalAnswer::Once, Answerer::Person)
    .await;
  assert_eq!(answer, Err(Rejection::ToolApprovalNotAwaited));
  assert!(!waits(root, id, &unheld));
  assert!(waits(root, id, &request.id));

  // The agent ends while its request waits: the request ends with it.
  scheduler.cancel_run(id).await.expect("cancelled");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert!(run.status().is_terminal());
  assert!(!waits(root, id, &request.id));
  let answer = scheduler
    .decide_tool_approval(id, request.id.clone(), ApprovalAnswer::Once, Answerer::Person)
    .await;
  assert_eq!(answer, Err(Rejection::ToolApprovalNotAwaited));

  // No task executes the run any more: a request recorded for it ends at its answer.
  let left = recorded(root, &request, "left");
  let answer = scheduler
    .decide_tool_approval(id, left.clone(), ApprovalAnswer::Deny, Answerer::Person)
    .await;
  assert_eq!(answer, Err(Rejection::ToolApprovalNotAwaited));
  assert!(!waits(root, id, &left));
  let invocations = ProjectDb::open(root)
    .expect("open")
    .invocations(id)
    .expect("invocations");
  assert!(invocations.iter().all(|invocation| invocation.ended_at.is_some()));
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_approvals_left_by_an_earlier_runner_end_when_the_next_attaches() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (earlier, mut events) = scheduler(root);
  let id = earlier
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  drop(earlier);
  // What the earlier runner left: a request still recorded as waiting on an agent that has ended.
  let mut db = ProjectDb::open(root).expect("open");
  let invocation = db.invocations(id).expect("invocations").remove(0);
  let left = PendingToolApproval {
    id: format!("{}:left", invocation.id),
    run: id,
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "cargo test".to_owned(),
    target: None,
    requested_at: Timestamp::now(),
    native: None,
  };
  db.request_tool_approval(&left).expect("recorded");
  assert!(waits(root, id, &left.id));

  let (next, _events) = scheduler(root);
  next
    .attach(
      AttachSession::Fresh {
        previous_end: SessionEnd::Clean,
      },
      NonZeroU32::new(2).expect("above zero"),
    )
    .await
    .expect("attached");
  assert!(!waits(root, id, &left.id));
  assert!(db.pending_tool_approvals_all().expect("requests").is_empty());
  let answer = next
    .decide_tool_approval(id, left.id, ApprovalAnswer::Once, Answerer::Person)
    .await;
  assert_eq!(answer, Err(Rejection::ToolApprovalNotAwaited));
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_approvals_end_when_their_steps_worker_is_gone_and_an_answered_one_says_so() {
  let listed = AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("list");
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(listed),
      NetworkAccess::Off,
    )),
    steps: vec![
      StepSpec::new("prepare", "Prepare", "Say hello.", &[]),
      StepSpec::new("implement", "Implement", "Run `pnpm build`.", &["prepare"]),
    ],
    ..Setup::new("Add a greeting.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending_approval(root, &mut events, id).await;
  let prepared = ProjectDb::open(root)
    .expect("open")
    .invocations(id)
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.execution != request.execution)
    .expect("Prepare's invocation");
  assert!(prepared.ended_at.is_some(), "Prepare's agent has ended");

  // A request of the step whose agent has ended: the run still works, but no worker of it holds one.
  let gone = PendingToolApproval {
    execution: prepared.execution,
    invocation: prepared.id,
    ..request.clone()
  };
  let gone = recorded(root, &gone, "gone");
  let answer = scheduler
    .decide_tool_approval(id, gone.clone(), ApprovalAnswer::Once, Answerer::Person)
    .await;
  assert_eq!(answer, Err(Rejection::ToolApprovalNotAwaited));
  assert!(!waits(root, id, &gone));
  assert!(waits(root, id, &request.id));

  // A second answer to a request answered already is refused as answered, not as ended.
  scheduler
    .decide_tool_approval(id, request.id.clone(), ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let again = scheduler
    .decide_tool_approval(id, request.id.clone(), ApprovalAnswer::Deny, Answerer::Person)
    .await;
  assert_eq!(again, Err(Rejection::ToolApprovalAnswered));
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert!(run.status().is_terminal());
  let again = scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await;
  assert_eq!(again, Err(Rejection::ToolApprovalAnswered));
}
