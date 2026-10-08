//! Tools that run without asking, end to end with `orchester-fake-claude` and `orchester-fake-codex`: in a
//! step that asks before MCP actions, a tool the server lists runs without a request and its activity says
//! so, while another tool of the same server still waits for a person; a resumed Codex thread keeps the
//! exemption, a step that does not ask gets none, a Claude Code name two servers could share gets no
//! rule, and an approval Codex asks in a step that does not ask is declined without a person. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test mcp_exempt_tools mcp_exempt`.

mod support;

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{ExemptTools, McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    stream::{AgentEvent, CommandStatus},
  },
  id::{ConnectionId, IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{CommandArg, CommandProgram},
  run::{Answerer, ApprovalAnswer, PendingToolApproval, RunStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Event};
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, Setup, TestProject, killed_mid_run, project, reported, scheduler, scheduler_with, threads,
  tools::TEST_COMMAND, wait_for,
};

/// `pencil`, whose `get_editor_state` and `get_screenshot` run without asking.
fn pencil() -> McpServer {
  server("pencil", "get_editor_state\nget_screenshot")
}

/// A server the CLI starts, whose tools `exempt` run without asking.
fn server(name: &str, exempt: &str) -> McpServer {
  let mut server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(name).expect("valid name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(TEST_COMMAND).expect("valid program"),
      vec![CommandArg::new("--stdio").expect("valid argument")],
      None,
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("a valid server");
  server.set_exempt_tools(ExemptTools::parse(exempt).expect("valid tools"));
  server
}

/// A Builder on `provider` that asks before MCP actions and gets `pencil`.
fn asking(provider: AgentProvider, brief: &'static str) -> TestProject {
  builder(provider, brief, true, vec![pencil()])
}

/// A Builder on `provider` that gets `servers`, asking before MCP actions when `asks`.
fn builder(provider: AgentProvider, brief: &'static str, asks: bool, servers: Vec<McpServer>) -> TestProject {
  let commands = if provider == AgentProvider::CodexCli {
    CommandPermission::AnyInWorktree
  } else {
    CommandPermission::AskBefore
  };
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off);
  permissions.set_interactive_tool_approvals(asks);
  project(&Setup {
    provider,
    permissions: Some(permissions),
    mcp: servers,
    ..Setup::new(brief)
  })
}

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(request) = ProjectDb::open(root)
        .expect("open")
        .pending_tool_approvals(run)
        .expect("requests")
        .first()
      {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("an approval arrives")
}

fn not_asked(db: &ProjectDb, run: RunId) -> Vec<(String, String)> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::McpToolNotAsked { server, tool, .. } => Some((server, tool)),
      _ => None,
    })
    .collect()
}

fn listed() -> Vec<(String, String)> {
  vec![("pencil".to_owned(), "get_screenshot".to_owned())]
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_exempt_claude_tool_runs_without_asking() {
  let project = asking(AgentProvider::ClaudeCodeCli, "Read the design.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_design_read allow-by-rule"]);
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(
    argv.contains(" --allowedTools mcp__pencil__get_editor_state mcp__pencil__get_screenshot "),
    "{argv}"
  );
  assert!(!argv.contains("mcp__pencil "), "no rule for the whole server: {argv}");
  assert_eq!(not_asked(&db, id), listed());
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_exempt_claude_other_tool_still_asks() {
  let project = asking(AgentProvider::ClaudeCodeCli, "Change the design.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert_eq!(
    (request.server.as_str(), request.action.as_str()),
    ("pencil", "batch_design")
  );
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_design_change allow"]);
  assert!(not_asked(&db, id).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_exempt_codex_tool_runs_without_asking_and_another_asks() {
  let project = asking(AgentProvider::CodexCli, "Use the design tools.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert_eq!(
    (request.server.as_str(), request.action.as_str()),
    ("pencil", "batch_design")
  );
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  assert!(
    worktree.join("read-once.txt").exists(),
    "the listed tool ran unasked first"
  );
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "unasked: "), ["pencil/get_screenshot"]);
  assert!(reported(&db, id, "mcp-arg: ").contains(&approved_pencil()));
  for config in thread_configs(root, id) {
    assert_keeps_the_step_servers(&config);
  }
  assert_eq!(not_asked(&db, id), listed());
}

/// The configuration of the thread each Codex start of `run` asked for, in order.
fn thread_configs(root: &Path, run: RunId) -> Vec<serde_json::Value> {
  threads(&ProjectDb::open(root).expect("open"), run)
    .into_iter()
    .map(|thread| thread["config"].clone())
    .collect()
}

/// A thread's configuration turns the user's own server off by its dotted key and has no `mcp_servers`
/// table, which would replace the servers of Codex's arguments, `pencil` and Orchester's, for the thread.
fn assert_keeps_the_step_servers(config: &serde_json::Value) {
  assert_eq!(
    config,
    &serde_json::json!({"mcp_servers.user-global.enabled": false}),
    "{config}"
  );
}

/// The override that approves `pencil`'s listed tools: Codex refuses a thread whose configuration has a
/// table for a server only overrides define, so the approval is one of them.
fn approved_pencil() -> String {
  r#"mcp_servers.pencil.tools={"get_editor_state"={approval_mode="approve"},"get_screenshot"={approval_mode="approve"}}"#
    .to_owned()
}

/// A Codex started again after its runner was killed (`thread/resume`) approves the listed tools again.
#[tokio::test(flavor = "multi_thread")]
async fn mcp_exempt_codex_resumed_thread_keeps_the_exemption() {
  let project = asking(AgentProvider::CodexCli, "Keep working.");
  let root = project.repo.path();
  let run = killed_mid_run(root, project.task).await;
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("above zero"))
    .await
    .expect("attached");
  wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  let started = threads(&ProjectDb::open(root).expect("open"), run);
  assert_eq!(started.len(), 2, "a new thread, then the resumed one");
  assert!(started[1]["threadId"].is_string(), "{}", started[1]);
  let approvals = reported(&ProjectDb::open(root).expect("open"), run, "mcp-arg: ")
    .into_iter()
    .filter(|arg| *arg == approved_pencil())
    .count();
  assert_eq!(approvals, 2, "each Codex start approves them");
}

/// A step that does not ask before MCP actions approves nothing.
#[tokio::test(flavor = "multi_thread")]
async fn mcp_exempt_codex_step_that_does_not_ask_gets_no_exemption() {
  let project = builder(AgentProvider::CodexCli, "Say hello.", false, vec![pencil()]);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let configs = thread_configs(root, id);
  assert_eq!(configs.len(), 1);
  assert_keeps_the_step_servers(&configs[0]);
  let args = reported(&ProjectDb::open(root).expect("open"), id, "mcp-arg: ");
  assert!(
    !args.iter().any(|arg| arg.starts_with("mcp_servers.pencil.tools=")),
    "{args:#?}"
  );
}

/// `a` lists `b__c`, which Claude Code would name as `c` of `a__b` too: it gets no rule, still asks, and
/// the activity says why; `pencil`'s listed tools keep their rules.
#[tokio::test(flavor = "multi_thread")]
async fn mcp_exempt_claude_name_two_servers_could_share_is_not_allowed() {
  let servers = vec![pencil(), server("a", "b__c"), server("a__b", "")];
  let project = builder(AgentProvider::ClaudeCodeCli, "Read the design.", true, servers);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(argv.contains(" mcp__pencil__get_screenshot "), "{argv}");
  assert!(!argv.contains("mcp__a__b__c"), "{argv}");
  let unapplied: Vec<AgentEvent> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .filter(|event| matches!(event, AgentEvent::McpExemptionNotApplied { .. }))
    .collect();
  assert_eq!(
    unapplied,
    [AgentEvent::McpExemptionNotApplied {
      server: "a".to_owned(),
      tool: "b__c".to_owned(),
      other: "a__b".to_owned(),
    }]
  );
}

/// Codex 0.159.2 asks for approval of an MCP call even with `mcp_elicitations` off. In a step that does
/// not ask before MCP actions the runner declines it at once, without a person, records the declined
/// call, and the agent works on.
#[tokio::test(flavor = "multi_thread")]
async fn mcp_codex_approval_in_a_step_that_does_not_ask_is_declined() {
  let project = builder(AgentProvider::CodexCli, "Call a tool unasked.", false, vec![pencil()]);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "mcp answer: "), ["decline"]);
  assert!(db.pending_tool_approvals(id).expect("requests").is_empty());
  let calls: Vec<AgentEvent> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .filter(|event| {
      matches!(
        event,
        AgentEvent::McpToolDeclined { .. } | AgentEvent::McpToolFinished { .. }
      )
    })
    .collect();
  assert_eq!(
    calls,
    [
      AgentEvent::McpToolDeclined {
        item: "mcp-1".to_owned(),
        server: "pencil".to_owned(),
        tool: "batch_design".to_owned(),
      },
      AgentEvent::McpToolFinished {
        item: "mcp-1".to_owned(),
        server: "pencil".to_owned(),
        tool: "batch_design".to_owned(),
        status: CommandStatus::Failed,
        asked: true,
      },
    ]
  );
}
