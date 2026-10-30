//! Runs executed end to end against `orchester-fake-codex`, a stand-in for the Codex CLI that
//! replays recorded output (see `tests/support/fake_codex.rs`); the real CLI never runs in tests.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test execution`.

mod support;

use std::fs;

use orchester_core::{
  connection::{
    AgentConnection, CatalogSource, CheckResult, ConnectionCheck, ConnectionStatus, ModelCatalog, SignIn,
    stream::{AgentEvent, CommandStatus, InvocationOutcome, OutputStream},
  },
  id::{IdempotencyKey, RunId, TaskId},
  integration::base::{BaseChoice, RunBaseSource, TaskBase},
  module::{ModuleDetails, ModuleState},
  name::Name,
  resource::StepKey,
  revision::Revision,
  run::{BranchName, InterruptCause, PreflightProblem, RunStatus, StepStatus},
  store::{ActivityStore, ConnectionStore, RunStore, TaskStore},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, git, project, scheduler, wait_for, wait_for_turns};

#[tokio::test(flavor = "multi_thread")]
async fn execution_runs_a_step_on_the_cli_and_commits_its_changes() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let key = IdempotencyKey::generate();
  let id = scheduler.start_run(key, project.task).await.expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.branch().as_str(), "orchester/orc-1-run-1");
  let checkpoint = run.last_checkpoint().expect("the step's changes were committed");
  let head = git(root, &["rev-parse", "orchester/orc-1-run-1"]);
  assert_eq!(checkpoint.revision().as_str(), head);
  assert_eq!(git(root, &["show", &format!("{head}:README.md")]), "hi\nhello");
  assert_eq!(git(root, &["log", "-1", "--format=%an", &head]), "Orchester Test");
  // The user's checkout is untouched.
  assert_eq!(fs::read_to_string(root.join("README.md")).expect("readme"), "hi\n");

  // Repeating the request with the same key returns the same run and starts nothing.
  assert_eq!(scheduler.start_run(key, project.task).await, Ok(id));
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.runs_of_task(project.task).expect("runs").len(), 1);

  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 1);
  let invocation = &invocations[0];
  assert_eq!(invocation.session.as_deref(), Some("fake-thread-1"));
  assert_eq!(invocation.usage.counted(), 12_166 - 2_432 + 5);
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Completed));
  assert!(invocation.ended_at.is_some());
  assert_transcript(&db, id);
}

/// What the runner asked Codex App Server for: the model, the sandbox and approval policy of the
/// thread, the effort and the result schema of the turn.
fn assert_thread_and_turn(stderr: &[&str]) {
  let reported = |prefix: &str| {
    stderr
      .iter()
      .find_map(|line| line.strip_prefix(prefix))
      .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON"))
      .expect("reported")
  };
  let thread = reported("thread: ");
  assert_eq!(thread["model"], "gpt-5.5");
  // The user's own MCP server, read from `config/read`, is turned off for the thread by its dotted key, and
  // nothing else is there: an `mcp_servers` table would replace the servers of Codex's arguments,
  // Orchester's among them.
  assert_eq!(
    thread["config"],
    serde_json::json!({"mcp_servers.user-global.enabled": false})
  );
  assert_eq!(thread["sandbox"], "workspace-write");
  // Commands that need more than the sandbox ask a person; nothing else does.
  assert_eq!(
    thread["approvalPolicy"],
    serde_json::json!({"granular": {"mcp_elicitations": false, "rules": false, "sandbox_approval": true,
      "request_permissions": false, "skill_approval": false}})
  );
  let turn = reported("turn: ");
  assert_eq!(turn["effort"], "low");
  assert!(
    turn["outputSchema"].is_object(),
    "a new session reports its result in the schema"
  );
}

fn assert_transcript(db: &ProjectDb, run: RunId) {
  let entries: Vec<AgentEvent> = db
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .collect();
  let stderr: Vec<&str> = entries
    .iter()
    .filter_map(|event| match event {
      AgentEvent::Raw {
        stream: OutputStream::Stderr,
        line,
      } => Some(line.as_str()),
      _ => None,
    })
    .collect();
  let argv = stderr
    .iter()
    .find_map(|line| line.strip_prefix("argv: "))
    .expect("argv reported");
  // What the user's own Codex setup would add is turned off first: notify, plugins, apps, the permission requests
  // that could stay granted and the user's own hooks; then Codex's sub-agents, which would not get the step's tools.
  // The fake's `config/read` lists the user's hooks (`hooks.state` entries) as the person's Codex did, and the step
  // still runs to the end, since they are off.
  let isolation = "app-server --stdio -c notify=[] -c features.plugins=false -c features.apps=false -c \
                   features.exec_permission_approvals=false -c features.request_permissions_tool=false -c \
                   features.hooks=false -c features.multi_agent=false -c features.multi_agent_v2=false ";
  // The profile has network off, which Codex cannot enforce on Windows: network stays on there.
  let expected = if cfg!(windows) {
    "-c windows.sandbox=elevated -c sandbox_workspace_write.network_access=true -c sandbox_workspace_write.writable_roots=["
  } else {
    "-c sandbox_workspace_write.network_access=false -c sandbox_workspace_write.writable_roots=["
  };
  // Then the worktree and its repository are untrusted, so the repository's `.codex` never loads.
  assert!(argv.starts_with(&format!("{isolation}-c projects={{")), "{argv}");
  assert!(
    argv.contains(&format!("={{trust_level=\"untrusted\"}}}} {expected}")),
    "{argv}"
  );
  assert_thread_and_turn(&stderr);
  let environment = stderr
    .iter()
    .find_map(|line| line.strip_prefix("env: "))
    .expect("environment reported");
  assert!(
    environment
      .split(',')
      .all(|name| !name.starts_with("CARGO") && !name.starts_with("RUST")),
    "{environment}"
  );
  assert!(stderr.contains(&"prompt: # Your role: Builder"), "{stderr:?}");
  // After its Orchester tools, the agent is told that its sub-agents are off, so it does the work with them itself.
  assert!(
    stderr.iter().any(|line| line.starts_with("prompt| Use the shell")
      && line.ends_with(orchester_core::connection::invocation::CODEX_SUB_AGENTS_PROMPT)),
    "{stderr:?}"
  );
  assert!(entries.contains(&AgentEvent::Message {
    text: "Appending the line.".into()
  }));
  assert!(entries.iter().any(|event| matches!(
    event,
    AgentEvent::CommandFinished {
      exit_code: Some(0),
      status: CommandStatus::Completed,
      ..
    }
  )));
  assert!(
    entries
      .iter()
      .any(|event| matches!(event, AgentEvent::FileChanged { .. }))
  );
  assert!(
    entries
      .iter()
      .any(|event| matches!(event, AgentEvent::TurnCompleted { .. }))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn execution_stops_the_agent_and_interrupts_the_run() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let db = ProjectDb::open(root).expect("open");
  wait_for_turns(root, &mut events, id, 1).await;
  assert_eq!(scheduler.active_runs(), [id]);
  tokio::time::timeout(PATIENCE, scheduler.stop_all(InterruptCause::QuitNow))
    .await
    .expect("stops in time");
  let run = db.run(id).expect("read").expect("run");
  assert_eq!(run.status(), &RunStatus::Interrupted(InterruptCause::QuitNow));
  let execution = &run.current_attempt().expect("attempt").executions()[0];
  assert_eq!(execution.status(), StepStatus::Running);
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Terminated));
}

#[tokio::test(flavor = "multi_thread")]
async fn execution_rejects_what_the_preflight_finds_and_tasks_that_are_not_queued() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let stored = db.connections().expect("connections").remove(0);
  let mut connection = AgentConnection::try_from(stored).expect("an agent connection");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in: SignIn::SignedOut,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("an agent check fits a CLI connection");
  db.save_connection(&connection.into_connection()).expect("save");
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::ConnectionNotUsable {
        step: StepKey::new("implement").expect("key"),
        status: ConnectionStatus::SignInRequired,
      }]
    })
  );
  assert_eq!(
    scheduler
      .start_run(IdempotencyKey::generate(), TaskId::generate())
      .await,
    Err(Rejection::TaskNotFound)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn run_base_resolution_records_the_branch_the_checkout_has_when_the_run_starts() {
  let on_main = project(&Setup::new("Append one line."));
  let run = run_to_the_end(on_main.repo.path(), on_main.task).await;
  assert_eq!(run.base_branch().map(|branch| branch.as_str()), Some("main"));
  assert_eq!(run.base_source(), RunBaseSource::CheckedOut);
  assert_eq!(run.base().as_str(), git(on_main.repo.path(), &["rev-parse", "main"]));
}

/// A detached checkout leaves a task with no base branch of its own or its module's nothing to deliver into: the start
/// is refused with the commit `HEAD` is at, and no run is recorded. A base branch set on the task starts it from that
/// branch all the same.
#[tokio::test(flavor = "multi_thread")]
async fn run_base_resolution_a_detached_checkout_refuses_a_run_with_no_base_branch_set() {
  let detached = project(&Setup::new("Append one line."));
  let root = detached.repo.path();
  git(root, &["switch", "--quiet", "--detach", "main"]);
  let head = Revision::new(&git(root, &["rev-parse", "HEAD"])).expect("revision");
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), detached.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::NoBranchCheckedOut { head }]
    })
  );
  assert!(
    ProjectDb::open(root)
      .expect("open")
      .runs_of_task(detached.task)
      .expect("runs")
      .is_empty(),
    "no run was recorded"
  );

  let main = BranchName::new("main").expect("branch");
  set_task_base(root, detached.task, &TaskBase::Branch(main.clone()));
  let run = run_to_the_end(root, detached.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.base_branch(), Some(&main));
  assert_eq!(run.base_source(), RunBaseSource::Task);
}

/// Run again is a new run, which resolves its base again: after the task got a base branch of its own, the next run
/// starts from it, while the run before keeps the checked-out branch it recorded.
#[tokio::test(flavor = "multi_thread")]
async fn run_base_resolution_run_again_resolves_the_base_again() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  git(root, &["switch", "--quiet", "-c", "staging"]);
  git(root, &["commit", "--quiet", "--allow-empty", "-m", "staging work"]);
  git(root, &["switch", "--quiet", "main"]);
  let (scheduler, mut events) = scheduler(root);
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let failed = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  assert_eq!(failed.base_source(), RunBaseSource::CheckedOut);

  let staging = BranchName::new("staging").expect("branch");
  set_task_base(root, project.task, &TaskBase::Branch(staging.clone()));
  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, None)
    .await
    .expect("the task runs again");
  let db = ProjectDb::open(root).expect("open");
  let next = db.run(again).expect("read").expect("run");
  assert_eq!(next.base_branch(), Some(&staging));
  assert_eq!(next.base_source(), RunBaseSource::Task);
  assert_eq!(next.base().as_str(), git(root, &["rev-parse", "staging"]));
  let before = db.run(first).expect("read").expect("run");
  assert_eq!(before.base_branch().map(BranchName::as_str), Some("main"));
  assert_eq!(before.base_source(), RunBaseSource::CheckedOut, "the run before keeps its base");
  wait_for(root, &mut events, again, |run| run.status().is_terminal()).await;
}

fn set_task_base(root: &std::path::Path, task: TaskId, base: &TaskBase) {
  let mut db = ProjectDb::open(root).expect("open");
  let read = db.task(task).expect("read").expect("task");
  db.set_task_base(task, read.revision(), base)
    .expect("set")
    .saved()
    .expect("saved on the revision read");
}

/// A base branch set (ORC-81), here the task's module's: a new run starts from that branch's tip while the checkout is
/// on another branch, records it as its base with its source, and leaves the checkout where it was; a branch set on the
/// task that is gone refuses the run, naming it, and no run starts from the checkout instead.
#[tokio::test(flavor = "multi_thread")]
async fn run_base_resolution_starts_from_the_base_branch_set_wherever_the_checkout_is() {
  let on_main = project(&Setup::new("Append one line."));
  let root = on_main.repo.path();
  git(root, &["switch", "--quiet", "-c", "staging"]);
  git(root, &["commit", "--quiet", "--allow-empty", "-m", "staging work"]);
  git(root, &["switch", "--quiet", "main"]);
  let staging = BranchName::new("staging").expect("branch");
  let module = {
    let mut db = ProjectDb::open(root).expect("open");
    let details = ModuleDetails::new(
      Name::new("Webhooks v2").expect("name"),
      "",
      (None, None),
      ModuleState::Open,
    )
    .expect("details");
    let module = db.create_module(details, Timestamp::now()).expect("module").id();
    db.set_module_base(module, &BaseChoice::CheckedOut, BaseChoice::Branch(staging.clone()))
      .expect("set");
    db.set_task_module(on_main.task, Some(module), ChangeActor::Person, Timestamp::now())
      .expect("moved");
    module
  };
  let run = run_to_the_end(root, on_main.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.base_branch(), Some(&staging));
  assert_eq!(run.base_source(), RunBaseSource::Module { module });
  assert_eq!(run.base().as_str(), git(root, &["rev-parse", "staging"]));
  assert_ne!(
    run.base().as_str(),
    git(root, &["rev-parse", "main"]),
    "not the checkout's HEAD"
  );
  assert_eq!(
    git(root, &["symbolic-ref", "--short", "HEAD"]),
    "main",
    "the checkout stays on main"
  );

  // On a detached checkout too: the branch set is what is missing, not a checked-out one.
  let gone = project(&Setup::new("Append one line."));
  git(gone.repo.path(), &["switch", "--quiet", "--detach", "main"]);
  let branch = BranchName::new("gone").expect("branch");
  set_task_base(gone.repo.path(), gone.task, &TaskBase::Branch(branch.clone()));
  let (scheduler, _events) = scheduler(gone.repo.path());
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), gone.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::BaseBranchMissing { branch }]
    })
  );
  assert!(
    ProjectDb::open(gone.repo.path())
      .expect("open")
      .runs_of_task(gone.task)
      .expect("runs")
      .is_empty(),
    "no run was recorded"
  );
}

/// Codex refusing the thread stops the step with Codex's reason and code on one line, the secrets of the
/// invocation redacted; the step restarts once, then waits for a person.
#[tokio::test(flavor = "multi_thread")]
async fn execution_says_why_codex_refused_the_thread() {
  let project = project(&Setup {
    model: Some("fake-refused-model"),
    ..Setup::new("Append one line.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::WaitingForPerson(_))
  })
  .await;
  let invocations = ProjectDb::open(root)
    .expect("open")
    .invocations(id)
    .expect("invocations");
  assert_eq!(invocations.len(), 2, "one fresh restart");
  let expected = "Codex App Server rejected thread/start or resume (code -32600): model fake-refused-model is \
    refused for the token <redacted>";
  for invocation in invocations {
    assert_eq!(
      invocation.outcome,
      Some(InvocationOutcome::TurnFailed {
        message: expected.to_owned()
      })
    );
  }
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

async fn run_to_the_end(root: &std::path::Path, task: TaskId) -> orchester_core::run::Run {
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await
}
