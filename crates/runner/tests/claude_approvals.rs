//! Interactive approvals on Claude Code, end to end: `orchester-fake-claude` starts the permission
//! prompt relay (`orchester-runner --claude-permission-prompt`) from its `--mcp-config` and calls it
//! as Claude Code does; the runner shows the request and answers exactly that call with a person's
//! decision. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test
//! claude_approvals claude_approvals`.

mod support;

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{McpEnvValue, McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    stream::{AgentEvent, InvocationOutcome},
  },
  id::{ConnectionId, IdempotencyKey, RunId, SecretId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::{CommandArg, CommandProgram, StepKey, VariableName},
  run::{Answerer, ApprovalAnswer, PauseReason, PendingToolApproval, RunStatus, StepStatus},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, Setup, TestProject, git, project, reported, scheduler_with, tools::TEST_COMMAND, wait_for,
};

const DOCS_TOKEN: &str = "docs-token-value-7f3a";

fn permissions(commands: CommandPermission, tools: bool) -> AgentPermissions {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off);
  permissions.set_interactive_tool_approvals(tools);
  permissions
}

fn claude(brief: &'static str, permissions: AgentPermissions) -> TestProject {
  project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions),
    ..Setup::new(brief)
  })
}

/// A project whose Builder asks before MCP actions and uses `docs`, which takes a secret.
fn with_docs(brief: &'static str) -> TestProject {
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("DOCS_TOKEN").expect("valid name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::now(),
  );
  let docs = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("docs").expect("valid name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(TEST_COMMAND).expect("valid program"),
      vec![CommandArg::new("--stdio").expect("valid argument")],
      None,
    )),
    McpEnvironment::new(BTreeMap::from([(
      VariableName::new("DOCS_TOKEN").expect("valid name"),
      McpEnvValue::Secret(secret.id()),
    )]))
    .expect("valid variables"),
    Timestamp::now(),
  )
  .expect("a valid server");
  let git = CommandPermission::Allowed(
    AllowedCommands::new(vec![CommandPrefix::new("git").expect("valid prefix")]).expect("valid list"),
  );
  project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(git, true)),
    mcp: vec![docs],
    secrets: vec![(secret, DOCS_TOKEN)],
    ..Setup::new(brief)
  })
}

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db.pending_tool_approvals(run).expect("requests").first()
        && db
          .invocations(run)
          .expect("invocations")
          .iter()
          .any(|invocation| invocation.id == request.invocation && invocation.timing.waiting_since().is_some())
      {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("an approval arrives")
}

fn readme(root: &Path, run: RunId) -> String {
  std::fs::read_to_string(support::run_folder(root, run).join("README.md")).expect("README")
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_run_an_approved_command_once() {
  let project = claude(
    "Ask before publishing.",
    permissions(CommandPermission::AskBefore, false),
  );
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert!(request.command);
  assert_eq!(request.action, "pnpm publish --dry-run");
  assert_eq!(request.scope.as_deref(), Some("Publish a dry run"));
  let worktree = support::run_folder(root, id);
  assert_eq!(request.target.as_deref(), worktree.to_str());
  assert!(request.id.ends_with(":toolu_publish"), "{}", request.id);
  assert_eq!(readme(root, id), "hi\n", "nothing runs before the decision");
  // Waiting for the person is not active agent time.
  tokio::time::sleep(std::time::Duration::from_millis(60)).await;
  assert_eq!(
    scheduler
      .decide_tool_approval(id, "another-call".to_owned(), ApprovalAnswer::Once, Answerer::Person)
      .await,
    Err(Rejection::ToolApprovalNotAwaited)
  );
  scheduler
    .decide_tool_approval(id, request.id.clone(), ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  assert_eq!(
    scheduler
      .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
      .await,
    Err(Rejection::ToolApprovalAnswered),
    "a decision is taken once"
  );
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let head = git(root, &["rev-parse", run.branch().as_str()]);
  assert_eq!(
    git(root, &["show", &format!("{head}:README.md")]),
    "hi\napproved\nhello"
  );
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_publish allow"]);
  assert_eq!(
    reported(&db, id, "permission again: "),
    ["toolu_publish deny"],
    "the approval does not cover the call asked again"
  );
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(
    argv.contains(" --permission-prompt-tool mcp__Orchester__approve "),
    "{argv}"
  );
  assert!(argv.contains(",Bash ") && !argv.contains("Bash("), "{argv}");
  assert!(!db.has_interrupted_tool_approval(id).expect("approvals"));
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert!(invocation.timing.waiting_since().is_none());
  assert!(invocation.timing.inactive_ms() >= 60, "{:?}", invocation.timing);
}

/// How a run is stopped while Claude Code waits for a decision.
enum Stopping {
  Pause,
  Shutdown,
}

/// A run paused at a pending call, as [`stop_at_a_pending_approval`] leaves it.
struct PausedAtApproval {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

/// A pause or a graceful shutdown while a call waits checkpoints the session without granting the
/// call, as for Codex: the run pauses with its step still running and its work saved, and nothing
/// in its activity calls that a failure.
async fn stop_at_a_pending_approval(stopping: Stopping) -> PausedAtApproval {
  let project = claude(
    "Ask before publishing.",
    permissions(CommandPermission::AskBefore, false),
  );
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  let worktree = support::run_folder(root, id);
  std::fs::write(worktree.join("progress.txt"), "keep actual progress").expect("work");
  match stopping {
    Stopping::Pause => tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
      .await
      .expect("answered in time")
      .expect("the pause is recorded"),
    Stopping::Shutdown => tokio::time::timeout(PATIENCE, scheduler.finish_at_checkpoints())
      .await
      .expect("the shutdown settles"),
  }
  let run = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(run.status(), &RunStatus::Paused(PauseReason::Requested));
  let implement = run
    .latest_execution(&StepKey::new("implement").expect("key"))
    .expect("the step ran");
  assert_eq!(implement.status(), StepStatus::Running, "the step continues later");
  let checkpoint = run.last_checkpoint().expect("progress checkpoint");
  assert_eq!(
    git(root, &["show", &format!("{}:progress.txt", checkpoint.revision())]),
    "keep actual progress"
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(db.pending_tool_approvals(id).expect("requests").is_empty());
  assert_eq!(
    scheduler
      .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
      .await,
    Err(Rejection::ToolApprovalNotAwaited),
    "the call was not granted and cannot be later"
  );
  assert!(!readme(root, id).contains("approved"));
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert!(invocation.ended_at.is_some());
  assert!(invocation.timing.waiting_since().is_none());
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Terminated));
  let failures: Vec<AgentEvent> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .filter(|event| matches!(event, AgentEvent::TurnFailed { .. }))
    .collect();
  assert!(failures.is_empty(), "a pause is no failure: {failures:?}");
  PausedAtApproval {
    project,
    scheduler,
    events,
    id,
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_pause_at_a_pending_call_saves_without_granting_it() {
  stop_at_a_pending_approval(Stopping::Pause).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_shutdown_at_a_pending_call_saves_without_granting_it() {
  stop_at_a_pending_approval(Stopping::Shutdown).await;
}

/// Continuing a run paused at a pending call resumes the same session, whose call has no result.
/// Claude Code marks it interrupted, neither granted nor refused (`fake_claude`, `resume`), so
/// nothing ran; the agent asks again in a new call, which a person decides as usual.
#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_resume_after_a_pause_at_a_pending_call_asks_again() {
  let PausedAtApproval {
    project,
    scheduler,
    mut events,
    id,
  } = stop_at_a_pending_approval(Stopping::Pause).await;
  let root = project.repo.path();
  let first = ProjectDb::open(root)
    .expect("open")
    .invocations(id)
    .expect("invocations")[0]
    .clone();
  let session = first.session.clone().expect("the paused session");
  tokio::time::timeout(PATIENCE, scheduler.resume_run(id))
    .await
    .expect("answered in time")
    .expect("the run continues");
  let again = pending(root, &mut events, id).await;
  assert!(again.id.ends_with(":toolu_publish_again"), "{}", again.id);
  assert_ne!(
    again.invocation, first.id,
    "the new call belongs to the resumed invocation"
  );
  assert!(
    !readme(root, id).contains("approved"),
    "the interrupted call did not run"
  );
  scheduler
    .decide_tool_approval(id, again.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "resumed: "), ["toolu_publish interrupted"]);
  assert_eq!(reported(&db, id, "permission: "), ["toolu_publish_again allow"]);
  let argv = reported(&db, id, "argv: ");
  assert_eq!(argv.len(), 2);
  assert!(argv[1].ends_with(&format!("--resume {session}")), "{}", argv[1]);
  assert!(
    argv[1].contains("--permission-prompt-tool mcp__Orchester__approve"),
    "{}",
    argv[1]
  );
  let head = git(root, &["rev-parse", run.branch().as_str()]);
  assert_eq!(
    git(root, &["show", &format!("{head}:README.md")]),
    "hi\napproved\nhello",
    "the action ran once, after the new approval"
  );
}

/// Stopping ends the agent, which takes its permission prompt with it, and the prompt's connection
/// usually closes before the agent's exit is seen (without the fix, every pause here reported the
/// closed prompt as a failed turn). Both happen together and the worker takes whichever it sees
/// first, so the pause is repeated to meet either order.
#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_pause_is_no_failure_when_the_prompt_closes_before_the_exit() {
  for _ in 0..6 {
    stop_at_a_pending_approval(Stopping::Pause).await;
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_ask_even_for_a_command_accept_edits_would_run() {
  let project = claude("Make a folder.", permissions(CommandPermission::AskBefore, false));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert_eq!(request.action, "mkdir build-output");
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_mkdir allow"]);
}

/// Agents ask: a read-only step that asks before commands asks about every one; approved, the
/// command runs once (outside any sandbox, which the approval says).
#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_ask_in_a_read_only_step_too() {
  let mut read_only = permissions(CommandPermission::AskBefore, false);
  read_only.set_repository(RepositoryAccess::ReadOnly);
  let project = claude("Make a folder.", read_only);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert_eq!(request.action, "mkdir build-output");
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_mkdir allow"]);
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  for expected in [
    " --permission-prompt-tool mcp__Orchester__approve ",
    " --permission-mode default ",
    r#" --settings {"permissions":{"ask":["Bash"]}} "#,
    " --disallowedTools Edit Write NotebookEdit ",
  ] {
    assert!(argv.contains(expected), "{expected}: {argv}");
  }
  let launched = db.invocations(id).expect("invocations")[0]
    .permissions
    .clone()
    .expect("recorded");
  assert_eq!(*launched.permissions().commands(), CommandPermission::AskBefore);
  assert_eq!(
    launched.outside_commands(),
    Some(orchester_core::permission::OutsideCommands::PermissionPrompt)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_denial_tells_the_agent_and_the_step_goes_on() {
  let project = claude(
    "Ask before publishing.",
    permissions(CommandPermission::AskBefore, false),
  );
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let implement = run
    .latest_execution(&StepKey::new("implement").expect("key"))
    .expect("the step ran");
  assert_eq!(implement.status(), StepStatus::Passed);
  let readme = readme(root, id);
  assert!(!readme.contains("approved") && readme.contains("went on"), "{readme}");
  let db = ProjectDb::open(root).expect("open");
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Completed));
  let told: Vec<String> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::ToolDecided { decision, .. } => decision.told,
      _ => None,
    })
    .collect();
  assert_eq!(told, ["Denied by the person reviewing this run. Continue without it."]);
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_cancel_while_waiting_leaves_a_stale_decision_unusable() {
  let project = claude(
    "Ask before publishing.",
    permissions(CommandPermission::AskBefore, false),
  );
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  tokio::time::timeout(PATIENCE, scheduler.cancel_run(id))
    .await
    .expect("answered in time")
    .expect("the run is cancelled");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Cancelled);
  let db = ProjectDb::open(root).expect("open");
  assert!(db.pending_tool_approvals(id).expect("requests").is_empty());
  assert_eq!(
    scheduler
      .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
      .await,
    Err(Rejection::ToolApprovalNotAwaited)
  );
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Terminated));
  assert!(invocation.timing.waiting_since().is_none());
  assert!(!readme(root, id).contains("approved"));
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_show_an_mcp_action_without_its_secret() {
  let project = with_docs("Ask before writing docs.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  assert!(!request.command);
  assert_eq!((request.server.as_str(), request.action.as_str()), ("docs", "write"));
  assert_eq!(request.target.as_deref(), Some("docs/<redacted>.md"));
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_docs allow"]);
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(
    !argv.contains("mcp__docs"),
    "the docs tools are not allowed without asking: {argv}"
  );
  let activity = format!("{:?}", db.activity(id, 0).expect("activity"));
  assert!(!activity.contains(DOCS_TOKEN), "the secret stays out of the activity");
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_approvals_deny_at_once_what_no_person_may_approve() {
  let project = claude("Try an edit outside.", permissions(CommandPermission::AskBefore, true));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_outside deny"]);
  assert!(db.pending_tool_approvals(id).expect("requests").is_empty());
}
