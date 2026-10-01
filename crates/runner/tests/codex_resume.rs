//! A Codex session recorded before App Server became the only transport (by `codex exec`), resumed
//! end to end against `orchester-fake-codex`: App Server resumes it by its id with `thread/resume`
//! (0.159.2 finds an exec session by id: `thread/read` returned one with `source: exec`), and a session
//! Codex has no history of stops the step with Codex's reason. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test codex_resume codex_resume`.

mod support;

use std::{num::NonZeroU32, path::Path};

use orchester_core::{connection::stream::AgentEvent, id::RunId, run::RunStatus, store::ActivityStore};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;

use self::support::{Setup, killed_mid_run, project, scheduler, threads, wait_for};

/// The id `codex exec` gave a session of an earlier Orchester run.
const EXEC_SESSION: &str = "01a0f41a-a973-7ab3-bd75-813b279d498f";
/// An exec session whose history is gone from the Codex home.
const GONE_SESSION: &str = "01a0f41a-gone-7ab3-bd75-813b279d498f";

/// A run whose runner was killed mid-turn, its agent's session replaced by `session`, as an
/// invocation recorded through `codex exec` holds it, then resumed by a new runner until `done`.
async fn resumed(session: &str, done: fn(&RunStatus) -> bool) -> (support::TestProject, RunId) {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let run = killed_mid_run(root, project.task).await;
  let mut db = ProjectDb::open(root).expect("open");
  let mut invocation = db
    .invocations(run)
    .expect("invocations")
    .pop()
    .expect("the killed agent");
  invocation.session = Some(session.to_owned());
  db.save_invocation(&invocation).expect("save");
  drop(db);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("above zero"))
    .await
    .expect("attached");
  wait_for(root, &mut events, run, |run| done(run.status())).await;
  (project, run)
}

fn resumed_ids(root: &Path, run: RunId) -> Vec<String> {
  let db = ProjectDb::open(root).expect("open");
  threads(&db, run)
    .iter()
    .filter_map(|thread| thread["threadId"].as_str().map(str::to_owned))
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_resume_continues_a_session_codex_exec_recorded() {
  let (project, run) = resumed(EXEC_SESSION, |status| status == &RunStatus::Succeeded).await;
  assert_eq!(resumed_ids(project.repo.path(), run), [EXEC_SESSION]);
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_resume_of_a_session_codex_no_longer_has_says_why() {
  let (project, run) = resumed(GONE_SESSION, |status| matches!(status, RunStatus::Failed(_))).await;
  let root = project.repo.path();
  assert_eq!(resumed_ids(root, run), [GONE_SESSION]);
  let failures: Vec<String> = ProjectDb::open(root)
    .expect("open")
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::TurnFailed { message } => Some(message),
      _ => None,
    })
    .collect();
  let expected =
    format!("Codex could not resume the earlier session {GONE_SESSION}: no rollout found for thread id {GONE_SESSION}");
  assert!(failures.contains(&expected), "{failures:?}");
}
