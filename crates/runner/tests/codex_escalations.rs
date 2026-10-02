//! Codex commands that need more than the sandbox, end to end against `orchester-fake-codex`: with
//! *Any command in the worktree* every step runs as App Server with the approval policy's
//! `sandbox_approval`, so an escalation the model requests reaches a person once, in writing and
//! read-only steps alike, while a file change outside the writable roots is declined without one.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test
//! codex_escalations codex_escalations`.

mod support;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, OutsideCommands, RepositoryAccess},
  run::{ApprovalAnswer, PendingToolApproval, Run, RunStatus},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use serde_json::json;
use tokio::sync::mpsc;

use self::support::{Setup, TestProject, git, pending_approval, project, reported, scheduler, threads, wait_for};

/// A run of `brief` started on Codex with `repository` access and any command in the worktree.
struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  async fn new(brief: &'static str, repository: RepositoryAccess) -> Self {
    let project = project(&Setup {
      permissions: Some(AgentPermissions::new(
        repository,
        CommandPermission::AnyInWorktree,
        NetworkAccess::Off,
      )),
      ..Setup::new(brief)
    });
    let (scheduler, events) = scheduler(project.repo.path());
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

  async fn pending(&mut self) -> PendingToolApproval {
    pending_approval(self.project.repo.path(), &mut self.events, self.id).await
  }

  async fn decide(&mut self, request: PendingToolApproval, answer: ApprovalAnswer) -> Run {
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, answer)
      .await
      .expect("decided");
    self.ended().await
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  fn worktree(&self) -> std::path::PathBuf {
    self
      .project
      .repo
      .path()
      .join(".orchester/worktrees")
      .join(self.id.to_string())
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_escalations_ask_and_run_once_when_approved() {
  let mut started = Started::new("Escalate `pnpm install`.", RepositoryAccess::Write).await;
  let request = started.pending().await;
  assert!(request.command);
  assert_eq!(request.action, "pnpm install");
  assert_eq!(
    request.scope.as_deref(),
    Some("Do you want to reach the package registry?")
  );
  assert_eq!(request.target.as_deref(), started.worktree().to_str());
  assert_eq!(
    std::fs::read_to_string(started.worktree().join("README.md")).expect("README"),
    "hi\n",
    "nothing runs before the decision"
  );
  let run = started.decide(request, ApprovalAnswer::Once).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = started.db();
  let policy = &threads(&db, started.id)[0]["approvalPolicy"];
  assert_eq!(
    *policy,
    json!({"granular": {"mcp_elicitations": false, "rules": false, "sandbox_approval": true,
      "request_permissions": false, "skill_approval": false}})
  );
  let launched = db.invocations(started.id).expect("invocations")[0]
    .permissions
    .clone()
    .expect("recorded");
  assert_eq!(launched.outside_commands(), Some(OutsideCommands::SandboxApproval));
  assert_eq!(reported(&db, started.id, "escalation: "), ["accept"]);
  let head = git(started.project.repo.path(), &["rev-parse", run.branch().as_str()]);
  assert_eq!(
    git(started.project.repo.path(), &["show", &format!("{head}:README.md")]),
    "hi\nescalated\nhello"
  );
  assert!(!db.has_interrupted_tool_approval(started.id).expect("approvals"));
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_escalations_declined_go_on_without_the_command() {
  let mut started = Started::new("Escalate `pnpm install`.", RepositoryAccess::Write).await;
  let request = started.pending().await;
  let run = started.decide(request, ApprovalAnswer::Deny).await;
  // Codex declines the command, tells its model so, and the turn goes on to the step's result.
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(reported(&started.db(), started.id, "escalation: "), ["decline"]);
  let readme = std::fs::read_to_string(started.worktree().join("README.md")).expect("README");
  assert!(!readme.contains("escalated") && readme.contains("went on"), "{readme}");
  let told: Vec<String> = started
    .db()
    .activity(started.id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      orchester_core::connection::stream::AgentEvent::ToolDecided { decision, .. } => decision.told,
      _ => None,
    })
    .collect();
  assert_eq!(told, [orchester_core::run::codex_denial(true)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_escalations_ask_in_a_read_only_step() {
  let mut started = Started::new("Escalate `pnpm install`.", RepositoryAccess::ReadOnly).await;
  let request = started.pending().await;
  assert_eq!(request.action, "pnpm install");
  let run = started.decide(request, ApprovalAnswer::Deny).await;
  assert!(!matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
  let db = started.db();
  let thread = &threads(&db, started.id)[0];
  assert_eq!(thread["sandbox"], "read-only");
  assert_eq!(thread["approvalPolicy"]["granular"]["sandbox_approval"], true);
  let launched = db.invocations(started.id).expect("invocations")[0]
    .permissions
    .clone()
    .expect("recorded");
  assert_eq!(launched.permissions().repository(), RepositoryAccess::ReadOnly);
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_escalations_decline_a_file_change_outside_without_asking() {
  let mut started = Started::new("Patch outside.", RepositoryAccess::Write).await;
  let run = started.ended().await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = started.db();
  assert_eq!(reported(&db, started.id, "file change: "), ["decline"]);
  assert!(db.pending_tool_approvals(started.id).expect("requests").is_empty());
}
