//! Orchester's typed tools end to end: the relay each CLI starts (`orchester-runner --claude-permission-prompt`
//! for Claude Code, `--orchester-tools` for Codex) lists the step's tools and the runner runs a call in the
//! run's worktree, its answer cut to a size. A tool that runs gets an allow rule (Claude Code) or the approval
//! mode `approve` (Codex); one that reaches outside asks a person, whether or not the step asks before MCP
//! actions. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test orchester_tools
//! orchester_tools`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{ApprovalAnswer, PendingToolApproval, RunStatus},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use serde_json::json;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, git, project, reported, scheduler_with, threads, wait_for};

/// What the fakes read outside the worktree: next to the repository's `.orchester` folder.
const OUTSIDE: &str = "outside-notes.txt";

fn permissions(repository: RepositoryAccess) -> AgentPermissions {
  AgentPermissions::new(
    repository,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
}

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db.pending_tool_approvals(run).expect("requests").first() {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("an approval arrives")
}

/// Claude Code calls the git tools by their allow rules, commits through them, gets a git answer cut to its
/// size, asks a person about a read outside the worktree and gets the file once approved; a path that leaves
/// the worktree is refused.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_serve_claude_code_through_its_permission_prompt() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(RepositoryAccess::Write)),
    ..Setup::new("Use Orchester's tools.")
  });
  let root = project.repo.path();
  std::fs::write(root.join(OUTSIDE), "outside line 1\noutside line 2\n").expect("a file outside");
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert!(!request.command);
  assert_eq!(request.server, "Orchester");
  assert_eq!(request.action, "read_outside_file");
  let path = root.join(OUTSIDE).to_string_lossy().into_owned();
  assert_eq!(request.scope, Some(json!({ "path": path }).to_string()));
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "orchester-tools: "),
    ["approve,git_status,git_diff,git_log,git_show,git_add,git_commit,read_outside_file"]
  );
  assert_eq!(
    reported(&db, id, "orchester-permission: "),
    [
      "git_status allow-by-rule",
      "git_add allow-by-rule",
      "git_commit allow-by-rule",
      "git_show allow-by-rule",
      "read_outside_file allow",
      "git_add allow-by-rule"
    ]
  );
  assert_claude_calls(&reported(&db, id, "orchester-call: "));
  let subjects = git(root, &["log", "--format=%s", run.branch().as_str()]);
  assert!(subjects.contains("Add big.txt; rm -rf / $(whoami)"), "{subjects}");
  assert_claude_shown(&db, id);
}

/// Codex gets Orchester's server in its configuration, with its token only in its environment, and the tools'
/// approval modes in the thread's configuration; the relay serves a read-only step's tools without the
/// permission prompt.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_serve_codex_through_its_configuration() {
  let project = project(&Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new("Use Orchester's tools.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "orchester-tools: "),
    ["git_status,git_diff,git_log,git_show,read_outside_file"]
  );
  let calls = reported(&db, id, "orchester-call: ");
  assert_eq!(calls, ["git_status ok: exit code: 0"]);
  let args = reported(&db, id, "mcp-arg: ");
  assert!(
    args.contains(&r#"mcp_servers.Orchester.args=["--orchester-tools"]"#.to_owned()),
    "{args:#?}"
  );
  assert!(args.contains(&r#"mcp_servers.Orchester.env_vars=["ORCHESTER_APPROVAL_TOKEN"]"#.to_owned()));
  assert_eq!(reported(&db, id, "mcp-env: "), ["ORCHESTER_APPROVAL_TOKEN=<redacted>"]);
  let thread = threads(&db, id).pop().expect("a thread");
  assert_eq!(
    thread["config"]["mcp_servers"]["Orchester"],
    json!({"tools": {
      "git_status": {"approval_mode": "approve"},
      "git_diff": {"approval_mode": "approve"},
      "git_log": {"approval_mode": "approve"},
      "git_show": {"approval_mode": "approve"},
      "read_outside_file": {"approval_mode": "prompt"},
    }})
  );
}

/// Codex asks about a tool whose approval mode is `prompt`; in a step that does not ask before MCP actions the
/// request still reaches a person, since the tool is Orchester's, and an approval answers it.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_that_ask_reach_a_person_from_codex() {
  let project = project(&Setup::new("Read outside unasked."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert_eq!(
    (request.server.as_str(), request.action.as_str()),
    ("Orchester", "read_outside_file")
  );
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "mcp answer: "), ["accept"]);
}

/// What Claude Code's calls were answered: git in the worktree, the commit, a git answer cut to its size, the
/// file outside, and the refusal of a path that leaves the worktree.
fn assert_claude_calls(calls: &[String]) {
  assert_eq!(calls.len(), 6, "{calls:#?}");
  assert!(
    calls[0].starts_with("git_status ok") && calls[0].contains("?? big.txt"),
    "{calls:#?}"
  );
  assert!(
    calls[1].starts_with("git_add ok") && calls[1].contains("exit code: 0"),
    "{calls:#?}"
  );
  assert!(calls[2].starts_with("git_commit ok"), "{calls:#?}");
  assert!(
    calls[3].starts_with("git_show ok") && calls[3].ends_with("bytes of output at the end left out]"),
    "{calls:#?}"
  );
  let shown: usize = calls[3]
    .split_whitespace()
    .nth(2)
    .and_then(|size| size.trim_end_matches(':').parse().ok())
    .expect("a size");
  assert!(shown < 140 * 1024, "{shown}");
  assert_eq!(
    calls[4],
    "read_outside_file ok 44:      1\toutside line 1 |      2\toutside line 2"
  );
  assert!(
    calls[5].starts_with("git_add error") && calls[5].contains("leaves the worktree"),
    "{calls:#?}"
  );
}

/// How Claude Code was told about the tools, and how its calls show in the activity.
fn assert_claude_shown(db: &ProjectDb, id: RunId) {
  let shown_calls: Vec<String> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::CommandFinished { command, .. } if command.starts_with("mcp__Orchester__") => Some(command),
      _ => None,
    })
    .collect();
  assert!(
    shown_calls.contains(&"mcp__Orchester__git_status".to_owned()),
    "{shown_calls:?}"
  );
  let argv = reported(db, id, "argv: ").pop().expect("argv");
  assert!(argv.contains("mcp__Orchester__git_commit"), "{argv}");
  assert!(!argv.contains("mcp__Orchester__read_outside_file"), "{argv}");
  let told = reported(db, id, "prompt| ");
  assert!(
    told
      .iter()
      .any(|line| line.contains("Prefer the tools of the Orchester MCP server")),
    "the prompt points the agent at the tools"
  );
}
