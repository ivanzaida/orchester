//! Tools that run without asking, end to end with `orchester-fake-claude` and `orchester-fake-codex`: in a
//! step that asks before MCP actions, a tool the server lists runs without a request and its activity says
//! so, while another tool of the same server still waits for a person. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test mcp_exempt_tools mcp_exempt`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{ExemptTools, McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    stream::AgentEvent,
  },
  id::{ConnectionId, IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{CommandArg, CommandProgram},
  run::{PendingToolApproval, RunStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, reported, scheduler_with, tools::TEST_COMMAND, wait_for};

/// `pencil`, whose `get_screenshot` runs without asking.
fn pencil() -> McpServer {
  let mut server = McpServer::new(
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
  server.set_exempt_tools(ExemptTools::parse("get_editor_state\nget_screenshot").expect("valid tools"));
  server
}

/// A Builder on `provider` that asks before MCP actions and gets `pencil`.
fn asking(provider: AgentProvider, brief: &'static str) -> TestProject {
  let commands = if provider == AgentProvider::CodexCli {
    CommandPermission::AnyInWorktree
  } else {
    CommandPermission::AskBefore
  };
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off);
  permissions.set_interactive_tool_approvals(true);
  project(&Setup {
    provider,
    permissions: Some(permissions),
    mcp: vec![pencil()],
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
    .decide_tool_approval(id, request.id, true)
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
    .decide_tool_approval(id, request.id, true)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "unasked: "), ["pencil/get_screenshot"]);
  let thread = reported(&db, id, "thread: ").pop().expect("thread parameters");
  let thread: serde_json::Value = serde_json::from_str(&thread).expect("JSON");
  assert_eq!(
    thread["config"]["mcp_servers"]["pencil"],
    serde_json::json!({"tools": {
      "get_editor_state": {"approval_mode": "approve"},
      "get_screenshot": {"approval_mode": "approve"},
    }})
  );
  assert_eq!(not_asked(&db, id), listed());
}
