//! Auto mode's *approve everything in each run* (ORC-199) end to end with `orchester-fake-claude` as the agent and the
//! reviewer: in a run that uses auto mode with the setting on, a worktree command and an MCP call are answered from
//! the run's first request without a pending approval, recorded as auto-approved; a network command still goes to
//! the reviewer, and waits for the person when it asks; with the setting off, the same command is reviewed. Needs
//! `git`. Run with `cargo test -p orchester-runner --features test-agent --test approve_everything approve_everything`.

mod support;

use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    stream::AgentEvent,
  },
  id::{ConnectionId, IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{CommandArg, CommandProgram},
  run::{
    Answerer, ApprovalAnswer, ApproveEverything, DecidedBy, Run, RunStatus, ToolDecision,
    review::{AutoModeSettings, Review},
  },
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler_with, tools::TEST_COMMAND, wait_for};

/// `pencil`, with no tool listed to run without asking.
fn pencil() -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("pencil").expect("valid name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(TEST_COMMAND).expect("valid program"),
      vec![CommandArg::new("--stdio").expect("valid argument")],
      None,
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("a valid server")
}

/// A Claude Code Builder that asks before every command and MCP action, with `pencil`.
fn asking(brief: &'static str) -> TestProject {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  permissions.set_interactive_tool_approvals(true);
  project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions),
    mcp: vec![pencil()],
    ..Setup::new(brief)
  })
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  /// A run of `project`'s task, its new runs using auto mode, with *approve everything in each run* as `setting`.
  async fn new(project: TestProject, setting: ApproveEverything) -> Self {
    let mut db = ProjectDb::open(project.repo.path()).expect("open");
    db.save_auto_mode_settings(&AutoModeSettings {
      new_runs: true,
      reviewer: None,
    })
    .expect("auto mode for new runs");
    db.save_approve_everything(setting).expect("approve everything");
    let (scheduler, events) = scheduler_with(project.repo.path(), project.vault.clone());
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    Self {
      project,
      scheduler,
      events,
      id,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  /// How the run's requests were answered, as the activity records them.
  fn decisions(&self) -> Vec<ToolDecision> {
    self
      .db()
      .activity(self.id, 0)
      .expect("activity")
      .into_iter()
      .filter_map(|entry| match entry.event {
        AgentEvent::ToolDecided { decision, .. } => Some(decision),
        _ => None,
      })
      .collect()
  }

  /// The first waiting request once auto mode left it to the person, with what auto mode did.
  async fn left_to_person(&mut self) -> (String, Review) {
    let root = self.project.repo.path().to_path_buf();
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(&root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && let Some(review) = db.tool_approval_review(id, &request.id).expect("review")
        {
          return (request.id.clone(), review);
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("auto mode leaves a request to the person")
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn approve_everything_answers_a_worktree_command_and_an_mcp_call_without_a_card() {
  for brief in ["Run `cargo test -p core` twice.", "Change the design twice."] {
    let mut started = Started::new(asking(brief), ApproveEverything::on(Answerer::Person)).await;
    let run = started.ended().await;
    assert_eq!(run.status(), &RunStatus::Succeeded, "{brief}");
    let decisions = started.decisions();
    assert_eq!(decisions.len(), 2, "{brief}: {decisions:?}");
    for decision in &decisions {
      // Orchester answered as the request came: nobody waited, so the run's active time never stopped.
      assert_eq!(decision.decided_by, DecidedBy::AutoApproved, "{brief}");
      assert_eq!(decision.waited_ms, None, "{brief}");
      assert_eq!(decision.review, None, "{brief}: no reviewer judged it");
    }
    let counts = started.db().decision_counts(started.id).expect("counts");
    assert_eq!(
      (counts.auto_approved, counts.by_you, counts.by_reviewer),
      (2, 0, 0),
      "{brief}"
    );
    let invocations = started.db().invocations(started.id).expect("invocations");
    assert!(
      invocations
        .iter()
        .all(|invocation| invocation.timing.waiting_since().is_none()),
      "{brief}"
    );
    // The run's own choice stays the person's to make.
    assert_eq!(
      started.db().run_auto_approve(started.id).expect("read"),
      None,
      "{brief}"
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn approve_everything_leaves_a_network_command_to_the_reviewer_and_the_person() {
  let mut started = Started::new(
    asking("Run `git fetch origin`. Reviewer asks."),
    ApproveEverything::on(Answerer::Person),
  )
  .await;
  let (request, review) = started.left_to_person().await;
  assert!(matches!(review, Review::Asked { .. }), "{review:?}");
  started
    .scheduler
    .decide_tool_approval(started.id, request, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  started.ended().await;
  let decided: Vec<DecidedBy> = started.decisions().iter().map(|decision| decision.decided_by).collect();
  assert_eq!(decided, [DecidedBy::PersonDeny]);
}

#[tokio::test(flavor = "multi_thread")]
async fn approve_everything_off_leaves_a_worktree_command_to_the_reviewer() {
  let mut started = Started::new(asking("Run `cargo test -p core`."), ApproveEverything::OFF).await;
  started.ended().await;
  let decided: Vec<DecidedBy> = started.decisions().iter().map(|decision| decision.decided_by).collect();
  assert_eq!(decided, [DecidedBy::Reviewer]);
}
