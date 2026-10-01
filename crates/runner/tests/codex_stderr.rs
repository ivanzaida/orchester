//! Codex's standard error in a step's activity, end to end against `orchester-fake-codex`: kept as raw
//! lines without colours, without Codex's echo of the user's settings, and bounded per invocation.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test codex_stderr
//! codex_stderr`.

mod support;

use orchester_core::{
  connection::stream::{AgentEvent, OutputStream},
  id::IdempotencyKey,
  run::RunStatus,
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use self::support::{Setup, project, scheduler, wait_for};

#[tokio::test(flavor = "multi_thread")]
async fn codex_stderr_is_bounded_plain_and_without_the_users_settings() {
  let project = project(&Setup::new("Flood standard error."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  let db = ProjectDb::open(root).expect("open");
  let lines: Vec<String> = db
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::Raw {
        stream: OutputStream::Stderr,
        line,
      } => Some(line),
      _ => None,
    })
    .collect();
  assert!(
    lines.iter().any(|line| line.starts_with("argv: ")),
    "what the fake reports is kept"
  );
  for left_out in [
    "unrecognized configuration setting",
    "disable_response_storage",
    "\u{1b}",
  ] {
    assert!(!lines.iter().any(|line| line.contains(left_out)), "{left_out}");
  }
  let long = lines
    .iter()
    .find(|line| line.starts_with("WARN x"))
    .expect("the long line, without its colour");
  assert_eq!(long.chars().count(), 4000);
  assert_eq!(lines.len(), 1000, "an invocation keeps at most 1000 lines");
  assert!(!lines.iter().any(|line| line == "stderr line 1099"));
}
