//! How a step's agent CLI starts per approval mode (`docs/design/approval-modes.md` › Enforcement), end to end
//! against `orchester-fake-claude` and `orchester-fake-codex`: an Autopilot step starts its CLI with the CLI's own
//! permission prompt off, so every request reaches Orchester; a Manual step starts as before; and a CLI too old to
//! hand its requests to Orchester runs the Autopilot profile as Manual. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test approval_mode_launch approval_mode_launch`.

mod support;

use orchester_core::{
  connection::{AgentProvider, CatalogSource, CheckResult, ConnectionCheck, LastCheck, ModelCatalog, SignIn},
  id::{IdempotencyKey, RunId},
  permission::{
    AgentPermissions, AllowedCommands, ApprovalMode, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, PermissionProblem, Unavailable},
  },
  resource::StepKey,
  run::{Answerer, ApprovalAnswer, PreflightProblem, Run, RunStatus},
  store::{ActivityStore, ConnectionStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use serde_json::json;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, reported, scheduler, settled, threads, wait_for};

const ASK_EVERY_COMMAND: &str = r#"--settings {"permissions":{"ask":["Bash"]}}"#;

fn permissions(commands: CommandPermission, mode: ApprovalMode) -> AgentPermissions {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off);
  permissions.set_approval_mode(mode);
  permissions
}

fn git_only() -> CommandPermission {
  CommandPermission::Allowed(
    AllowedCommands::new(vec![CommandPrefix::new("git").expect("valid prefix")]).expect("valid list"),
  )
}

/// A project whose Builder runs on `provider` with `permissions`, its connection's CLI checked at `version`. The
/// fake Claude Code runs `git status --short`, a listed and read-only command, deciding it as the CLI would.
fn on_version(provider: AgentProvider, permissions: AgentPermissions, version: &str) -> TestProject {
  let brief = match provider {
    AgentProvider::ClaudeCodeCli => "Run `git status --short`.",
    _ => "Append one line.",
  };
  let project = project(&Setup {
    provider,
    permissions: Some(permissions),
    ..Setup::new(brief)
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  let connection = db.connections().expect("connections").remove(0).id();
  let check = ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: version.to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  };
  db.record_connection_check(connection, LastCheck::Agent(check))
    .expect("the check is recorded");
  project
}

async fn start(project: &TestProject) -> (Scheduler, mpsc::UnboundedReceiver<Event>, RunId) {
  let (scheduler, events) = scheduler(project.repo.path());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  (scheduler, events, id)
}

/// The run once it ends, approving each request that reaches Orchester on the way; the requests' actions.
async fn approving(
  project: &TestProject,
  scheduler: &Scheduler,
  events: &mut mpsc::UnboundedReceiver<Event>,
  id: RunId,
) -> (Run, Vec<String>) {
  let root = project.repo.path();
  let mut asked = Vec::new();
  let ended = tokio::time::timeout(PATIENCE, async {
    loop {
      let run = wait_for(root, events, id, |run| {
        settled(run)
          || !ProjectDb::open(root)
            .expect("open")
            .pending_tool_approvals(id)
            .expect("requests")
            .is_empty()
      })
      .await;
      let pending = ProjectDb::open(root)
        .expect("open")
        .pending_tool_approvals(id)
        .expect("requests");
      let Some(request) = pending.first() else {
        return run;
      };
      asked.push(request.action.clone());
      scheduler
        .decide_tool_approval(id, request.id.clone(), ApprovalAnswer::Once, Answerer::Person)
        .await
        .expect("approved");
    }
  })
  .await
  .expect("the run ends in time");
  (ended, asked)
}

fn launched_mode(db: &ProjectDb, id: RunId) -> ApprovalMode {
  db.invocations(id).expect("invocations")[0]
    .permissions
    .as_ref()
    .expect("recorded")
    .permissions()
    .approval_mode()
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_launch_claude_autopilot_hands_every_command_to_orchester() {
  let project = on_version(
    AgentProvider::ClaudeCodeCli,
    permissions(git_only(), ApprovalMode::Autopilot),
    "2.1.301",
  );
  let (scheduler, mut events, id) = start(&project).await;
  let (run, _) = approving(&project, &scheduler, &mut events, id).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  // `git status` is listed and read-only, which Claude Code would run by itself; in Autopilot the CLI hands it to
  // the permission prompt, and Orchester answers it.
  let db = ProjectDb::open(project.repo.path()).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_run allow"]);
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(
    argv.contains(" --permission-prompt-tool mcp__Orchester__approve "),
    "{argv}"
  );
  assert!(argv.contains(ASK_EVERY_COMMAND), "{argv}");
  assert!(!argv.contains("Bash("), "{argv}");
  assert_eq!(launched_mode(&db, id), ApprovalMode::Autopilot);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_launch_claude_manual_starts_as_before() {
  let project = on_version(
    AgentProvider::ClaudeCodeCli,
    permissions(git_only(), ApprovalMode::Manual),
    "2.1.301",
  );
  let (scheduler, mut events, id) = start(&project).await;
  let (run, asked) = approving(&project, &scheduler, &mut events, id).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(asked.is_empty(), "{asked:?}");
  let db = ProjectDb::open(project.repo.path()).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_run allow-by-rule"]);
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(argv.contains(" --allowedTools Bash(git:*) "), "{argv}");
  assert!(!argv.contains("--settings"), "{argv}");
  assert_eq!(launched_mode(&db, id), ApprovalMode::Manual);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_launch_claude_too_old_runs_autopilot_as_manual() {
  let project = on_version(
    AgentProvider::ClaudeCodeCli,
    permissions(git_only(), ApprovalMode::Autopilot),
    "2.1.88",
  );
  let (scheduler, mut events, id) = start(&project).await;
  let (run, asked) = approving(&project, &scheduler, &mut events, id).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(asked.is_empty(), "{asked:?}");
  let db = ProjectDb::open(project.repo.path()).expect("open");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_run allow-by-rule"]);
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(argv.contains(" --allowedTools Bash(git:*) "), "{argv}");
  assert!(!argv.contains("--settings"), "{argv}");
  assert_eq!(launched_mode(&db, id), ApprovalMode::Manual);
}

/// *Any command in the worktree*, which Claude Code cannot confine, never passes preflight in Autopilot: on a CLI too
/// old to hand its requests to Orchester the step would run as Manual, which cannot start it. Refused on a new CLI
/// too, since the fallback depends on a version check that can change.
#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_launch_claude_autopilot_any_command_is_refused_before_it_starts() {
  for version in ["2.1.88", "2.1.301"] {
    let project = on_version(
      AgentProvider::ClaudeCodeCli,
      permissions(CommandPermission::AnyInWorktree, ApprovalMode::Autopilot),
      version,
    );
    let (scheduler, _events) = scheduler(project.repo.path());
    assert_eq!(
      scheduler.start_run(IdempotencyKey::generate(), project.task).await,
      Err(Rejection::Preflight {
        problems: vec![PreflightProblem::PermissionsNotEnforceable {
          step: StepKey::new("implement").expect("key"),
          reason: PermissionProblem::NotEnforceable {
            option: PermissionOption::AnyCommand,
            reason: Unavailable::CannotConfineCommands,
          },
        }]
      }),
      "on {version}"
    );
  }
}

/// The approval policy of the run's first thread.
fn policy(project: &TestProject, id: RunId) -> serde_json::Value {
  let db = ProjectDb::open(project.repo.path()).expect("open");
  threads(&db, id)[0]["approvalPolicy"].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_launch_codex_autopilot_hands_every_request_to_orchester() {
  let project = on_version(
    AgentProvider::CodexCli,
    permissions(CommandPermission::AnyInWorktree, ApprovalMode::Autopilot),
    "0.159.2",
  );
  let (scheduler, mut events, id) = start(&project).await;
  let (run, _) = approving(&project, &scheduler, &mut events, id).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    policy(&project, id),
    json!({"granular": {"mcp_elicitations": true, "rules": true, "sandbox_approval": true,
      "request_permissions": false, "skill_approval": false}})
  );
  let db = ProjectDb::open(project.repo.path()).expect("open");
  assert_eq!(threads(&db, id)[0]["approvalsReviewer"], "user");
  assert_eq!(launched_mode(&db, id), ApprovalMode::Autopilot);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_launch_codex_too_old_runs_autopilot_as_manual() {
  let manual = json!({"granular": {"mcp_elicitations": false, "rules": false, "sandbox_approval": true,
    "request_permissions": false, "skill_approval": false}});
  for (mode, version) in [(ApprovalMode::Autopilot, "0.139.0"), (ApprovalMode::Manual, "0.159.2")] {
    let project = on_version(
      AgentProvider::CodexCli,
      permissions(CommandPermission::AnyInWorktree, mode),
      version,
    );
    let (scheduler, mut events, id) = start(&project).await;
    let (run, asked) = approving(&project, &scheduler, &mut events, id).await;
    assert_eq!(run.status(), &RunStatus::Succeeded, "{mode:?} on {version}");
    assert!(asked.is_empty(), "{asked:?}");
    assert_eq!(policy(&project, id), manual, "{mode:?} on {version}");
    let db = ProjectDb::open(project.repo.path()).expect("open");
    assert_eq!(launched_mode(&db, id), ApprovalMode::Manual);
  }
}
