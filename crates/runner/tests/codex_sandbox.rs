//! *Codex sandbox* end to end against `orchester-fake-codex` (ORC-789): with the profile's sandbox off, the step's
//! Codex thread starts in `danger-full-access` without the sandbox's launch settings, everything else of the launch
//! kept, and its commands run; on, it starts in the sandbox its access names; a shared project's agents keep the
//! sandbox on, whatever the profile says. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test codex_sandbox codex_sandbox`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  id::{IdempotencyKey, ProjectId, RunId, ServerId},
  permission::CodexSandbox,
  project::ProjectKey,
  resource::{AgentProfile, Draft, ResourceKind},
  run::{Run, RunStatus},
  sharing::MemberRole,
  store::{ActivityStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink};

use self::support::{Setup, project, reported, scheduler, settled, threads, wait_for};

/// Publishes the Builder profile again with its Codex sandbox `sandbox`.
fn publish_sandbox(root: &Path, sandbox: CodexSandbox) {
  let mut db = ProjectDb::open(root).expect("open");
  let builder = db.resources(ResourceKind::AgentProfile).expect("profiles")[0].id();
  let latest = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  let mut permissions = latest.body().permissions().clone();
  permissions.set_codex_sandbox(sandbox);
  let body = latest.body().clone().with_permissions(permissions);
  let now = Timestamp::now();
  db.save_draft(builder, &Draft::new(body, Some(latest.number()), now))
    .expect("draft");
  db.publish_draft::<AgentProfile>(builder, now).expect("publish");
}

/// Links the project to a server project, as sharing it does, without tasks.
fn share(root: &Path) {
  let now = Timestamp::now();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: now,
    cursor: 0,
    last_synced_at: now,
    removed_at: None,
    address: None,
  };
  let mut db = ProjectDb::open(root).expect("open");
  db.start_sharing(&link, &[], (&[], &Default::default())).expect("share");
}

async fn run(root: &Path, task: orchester_core::id::TaskId) -> (RunId, Run) {
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, settled).await;
  scheduler.wait_for_current_tasks().await;
  (id, run)
}

/// The sandbox the launches of `run` recorded.
fn launched(db: &ProjectDb, run: RunId) -> Vec<CodexSandbox> {
  db.invocations(run)
    .expect("invocations")
    .into_iter()
    .map(|invocation| {
      invocation
        .permissions
        .expect("launch permissions")
        .permissions()
        .codex_sandbox()
    })
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_sandbox_off_runs_the_thread_in_full_access_and_its_commands_run() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  publish_sandbox(root, CodexSandbox::Off);
  let (id, run) = run(root, project.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", run.status());
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ");
  assert!(
    !argv[0].contains("windows.sandbox") && !argv[0].contains("sandbox_workspace_write"),
    "no setting only the sandbox reads: {argv:?}"
  );
  // The rest of the launch stays: the user's setup is kept out, the project layers are untrusted, and Codex keeps its
  // shell, which only the Orchestrator chat takes away.
  for kept in [
    "features.hooks=false",
    "trust_level=\"untrusted\"",
    "mcp_servers.Orchester",
  ] {
    assert!(argv[0].contains(kept), "{kept} in {argv:?}");
  }
  assert!(!argv[0].contains("features.shell_tool=false"), "{argv:?}");
  let thread = &threads(&db, id)[0];
  assert_eq!(thread["sandbox"], "danger-full-access", "{thread}");
  assert_eq!(thread["approvalsReviewer"], "user");
  assert_eq!(
    thread["approvalPolicy"]["granular"]["request_permissions"], false,
    "the approval policy is the step's: {thread}"
  );
  let finished = db.activity(id, 0).expect("activity").into_iter().any(|entry| {
    matches!(
      entry.event,
      AgentEvent::CommandFinished {
        status: CommandStatus::Completed,
        exit_code: Some(0),
        ..
      }
    )
  });
  assert!(finished, "the step's command ran");
  assert_eq!(launched(&db, id), [CodexSandbox::Off]);
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_sandbox_on_runs_the_thread_in_its_sandbox() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (id, run) = run(root, project.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", run.status());
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ");
  assert!(argv[0].contains("sandbox_workspace_write.writable_roots="), "{argv:?}");
  assert_eq!(cfg!(windows), argv[0].contains("windows.sandbox=elevated"), "{argv:?}");
  assert_eq!(threads(&db, id)[0]["sandbox"], "workspace-write");
  assert_eq!(launched(&db, id), [CodexSandbox::On]);
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_sandbox_stays_on_in_a_shared_project() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  publish_sandbox(root, CodexSandbox::Off);
  share(root);
  let (id, run) = run(root, project.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", run.status());
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(threads(&db, id)[0]["sandbox"], "workspace-write");
  assert!(
    reported(&db, id, "argv: ")[0].contains("sandbox_workspace_write.writable_roots="),
    "the sandbox's settings are given"
  );
  assert_eq!(launched(&db, id), [CodexSandbox::On]);
}
