//! *Deny with a message* (ORC-29), end to end with `orchester-fake-claude` and `orchester-fake-codex`: a denial's words
//! reach the agent as the denial's reason through Claude Code's permission prompt; Codex, whose decline carries no
//! words, declines the command or the MCP call and gets the words as input of the turn that asked; the decision record
//! keeps who denied and the words. A native agent's denial carries no words, so it is refused with one. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test denial_messages denial_messages`.

mod support;

use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    stream::AgentEvent,
  },
  id::{ConnectionId, IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::{CommandArg, CommandProgram},
  run::{Answerer, ApprovalAnswer, DecidedBy, DenialNote, PendingToolApproval, Run, RunStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  Setup, TestProject, pending_approval, project, reported, scheduler_with, settled, tools::TEST_COMMAND, wait_for,
};

const WORDS: &str = "Use the workspace's serde_json instead.";

fn words() -> ApprovalAnswer {
  ApprovalAnswer::DenyWithMessage {
    message: DenialNote::new(WORDS).expect("words"),
  }
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  async fn new(setup: &Setup) -> Self {
    let project = project(setup);
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

  async fn pending(&mut self) -> PendingToolApproval {
    pending_approval(self.project.repo.path(), &mut self.events, self.id).await
  }

  /// Denies the waiting request with [`WORDS`], `by` a person or automation, and waits for the run to settle: the agent
  /// goes on without the call.
  async fn deny_with_words(&mut self, by: Answerer) -> Run {
    let request = self.pending().await;
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, words(), by)
      .await
      .expect("denied");
    wait_for(self.project.repo.path(), &mut self.events, self.id, settled).await
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  fn reported(&self, prefix: &str) -> Vec<String> {
    reported(&self.db(), self.id, prefix)
  }

  /// Who denied each decided request, and the words it kept.
  fn recorded(&self) -> Vec<(Option<DecidedBy>, Option<String>)> {
    self
      .db()
      .decided_tool_approvals(self.id)
      .expect("decided")
      .into_iter()
      .map(|decided| (decided.decided_by, decided.note.map(|note| note.as_str().to_owned())))
      .collect()
  }

  /// What the activity says each denial told the agent.
  fn told(&self) -> Vec<String> {
    self
      .db()
      .activity(self.id, 0)
      .expect("activity")
      .into_iter()
      .filter_map(|entry| match entry.event {
        AgentEvent::ToolDecided { decision, .. } => decision.told,
        _ => None,
      })
      .collect()
  }
}

/// A Claude Code step that runs `git` and asks about any other command.
fn claude(brief: &'static str) -> Setup {
  let listed = AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("list");
  Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(listed),
      NetworkAccess::Off,
    )),
    ..Setup::new(brief)
  }
}

/// A Codex step that runs any command in the worktree and asks before MCP actions, with `pencil`.
fn codex(brief: &'static str) -> Setup {
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  permissions.set_interactive_tool_approvals(true);
  let pencil = McpServer::new(
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
  .expect("a valid server");
  Setup {
    permissions: Some(permissions),
    mcp: vec![pencil],
    ..Setup::new(brief)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn denial_messages_reach_claude_code_as_the_denial() {
  let mut started = Started::new(&claude("Run `pnpm build`.")).await;
  let run = started.deny_with_words(Answerer::Person).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let told = format!("Denied by the person reviewing this run: {WORDS}");
  assert_eq!(started.reported("told: "), std::slice::from_ref(&told));
  assert_eq!(started.told(), [told]);
  assert_eq!(
    started.recorded(),
    [(Some(DecidedBy::PersonDeny), Some(WORDS.to_owned()))]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn denial_messages_of_automation_name_automation() {
  let mut started = Started::new(&claude("Run `pnpm build`.")).await;
  started.deny_with_words(Answerer::Automation).await;
  assert_eq!(
    started.reported("told: "),
    [format!("Denied by automation through Orchester's MCP server: {WORDS}")]
  );
  assert_eq!(
    started.recorded(),
    [(Some(DecidedBy::AutomationDeny), Some(WORDS.to_owned()))]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn denial_messages_follow_a_codex_command_decline() {
  let mut started = Started::new(&codex("Escalate `pnpm install`.")).await;
  let run = started.deny_with_words(Answerer::Person).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(started.reported("escalation: "), ["decline"]);
  assert_eq!(
    started.reported("steered: "),
    [format!("Denied by the person reviewing this run: {WORDS}")]
  );
  // Codex tells its model of the decline in its own words; the words follow as input of the turn.
  assert_eq!(started.told(), [orchester_core::run::codex_denial(true)]);
  assert_eq!(
    started.recorded(),
    [(Some(DecidedBy::PersonDeny), Some(WORDS.to_owned()))]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn denial_messages_follow_a_codex_mcp_decline() {
  let mut started = Started::new(&codex("Use the design tools.")).await;
  let request = started.pending().await;
  assert!(!request.command);
  assert_eq!(
    (request.server.as_str(), request.action.as_str()),
    ("pencil", "get_screenshot")
  );
  started
    .scheduler
    .decide_tool_approval(started.id, request.id, words(), Answerer::Person)
    .await
    .expect("denied");
  let run = wait_for(started.project.repo.path(), &mut started.events, started.id, settled).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    started.reported("steered: "),
    [format!(
      "The person reviewing this run denied pencil get_screenshot: {WORDS}"
    )]
  );
  assert_eq!(started.told(), [orchester_core::run::codex_denial(false)]);
  assert_eq!(
    started.recorded(),
    [(Some(DecidedBy::PersonDeny), Some(WORDS.to_owned()))]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn denial_messages_without_words_steer_nothing() {
  let mut started = Started::new(&codex("Escalate `pnpm install`.")).await;
  let request = started.pending().await;
  started
    .scheduler
    .decide_tool_approval(started.id, request.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  wait_for(started.project.repo.path(), &mut started.events, started.id, settled).await;
  assert!(started.reported("steered: ").is_empty());
  assert_eq!(started.recorded(), [(Some(DecidedBy::PersonDeny), None)]);
}
