//! Orchester's typed tools end to end: the relay each CLI starts (`orchester-runner --claude-permission-prompt`
//! for Claude Code, `--orchester-tools` for Codex) lists the step's tools and the runner runs a call in the
//! run's worktree, its output kept within a cap. A tool that asks runs only once approved: through Claude
//! Code's permission prompt, or asked about by the runner when the call arrives (every Codex call, and a
//! call a CLI ran unasked), whether or not the step asks before MCP actions. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test orchester_tools orchester_tools`.

mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  command::Command,
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  orchester_tools::OutputKept,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{Answerer, ApprovalAnswer, PendingToolApproval, RunStatus},
  store::ActivityStore,
};
use orchester_exec::run_capped;
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use serde_json::json;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, git, project, reported, scheduler_with, threads, tools::TEST_COMMAND, wait_for};

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
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "orchester-tools: "),
    [
      "approve,read_file,list_files,search,edit_file,write_file,git_status,git_diff,git_log,git_show,git_add,\
       git_commit,read_outside_file,ask_question"
    ]
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

/// Codex gets Orchester's server in its configuration, with its token only in its environment, and every
/// tool approved in the thread's configuration: the runner asks a person about a read outside the worktree
/// when the call arrives, in a step that does not ask before MCP actions, and runs it once approved.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_serve_codex_and_ask_in_the_runner() {
  let project = project(&Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new("Use Orchester's tools.")
  });
  let root = project.repo.path();
  std::fs::write(root.join(OUTSIDE), "outside line 1\n").expect("a file outside");
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
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "orchester-tools: "),
    ["read_file,list_files,search,git_status,git_diff,git_log,git_show,read_outside_file"]
  );
  assert_eq!(
    reported(&db, id, "orchester-call: "),
    [
      "git_status ok: exit code: 0",
      "read_outside_file ok:      1\toutside line 1"
    ]
  );
  let args = reported(&db, id, "mcp-arg: ");
  assert!(
    args.contains(&r#"mcp_servers.Orchester.args=["--orchester-tools"]"#.to_owned()),
    "{args:#?}"
  );
  assert_eq!(reported(&db, id, "mcp-env: "), ["ORCHESTER_APPROVAL_TOKEN=<redacted>"]);
  let approved = args
    .iter()
    .find_map(|arg| arg.strip_prefix("mcp_servers.Orchester.tools="))
    .expect("Orchester's tools approved");
  for tool in ["read_outside_file", "git_status"] {
    assert!(
      approved.contains(&format!(r#""{tool}"={{approval_mode="approve"}}"#)),
      "{approved}"
    );
  }
  let thread = threads(&db, id).pop().expect("a thread");
  assert!(thread["config"].get("mcp_servers").is_none(), "{thread}");
}

/// A CLI that runs a call it should have asked about gets no answer unasked: the runner asks itself, and a
/// denial reaches the agent as the call's error.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_that_ask_never_run_unapproved() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(RepositoryAccess::Write)),
    ..Setup::new("Use Orchester's tools without asking.")
  });
  let root = project.repo.path();
  std::fs::write(root.join(OUTSIDE), "outside line 1\n").expect("a file outside");
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert_eq!(request.action, "read_outside_file");
  assert!(request.id.contains(":orchester-call-"), "{}", request.id);
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert!(reported(&db, id, "orchester-permission: ").contains(&"read_outside_file skipped".to_owned()));
  let calls = reported(&db, id, "orchester-call: ");
  assert!(
    calls
      .iter()
      .any(|call| call.starts_with("read_outside_file error") && call.contains("denied")),
    "{calls:#?}"
  );
}

/// A program's output is read as it comes and kept within the cap, at the end the tool keeps, whatever it
/// prints.
#[test]
fn orchester_tools_keep_program_output_within_the_cap() {
  let command = Command::new(Path::new(TEST_COMMAND), ["print", "300000"]);
  let limit = 64 * 1024;
  let head = run_capped(&command, Duration::from_secs(60), (limit, OutputKept::Head)).expect("ran");
  assert!(head.stdout.len() <= limit, "{}", head.stdout.len());
  assert!(head.stdout.contains("line 1\n"));
  assert!(head.left_out > 3_000_000, "{}", head.left_out);
  let tail = run_capped(&command, Duration::from_secs(60), (limit, OutputKept::Tail)).expect("ran");
  assert!(tail.stdout.len() <= limit);
  assert!(tail.stdout.trim_end().ends_with("line 300000"));
  assert_eq!(tail.code, Some(0));
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
      .any(|line| line.contains("use the tools of the Orchester MCP server")),
    "the prompt points the agent at the tools"
  );
}
