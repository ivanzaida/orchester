//! Notices that the run's base branch moved (ORC-54) end to end against `orchester-fake-codex`: the desktop queues a
//! notice, and an agent of a writing step that finishes its turn while one is pending continues its session to read
//! it, the newest when a newer move replaced an undelivered one; a read-only step never takes it; a paused run keeps
//! it and its next writing step takes it once the run goes on; one within its move's window, or while the project's
//! switch is off, waits. Every writing agent is told how to merge the base safely. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test base_moved base_moved`.

mod support;

use std::path::Path;

use orchester_core::{
  base_move::{BaseNotice, LandedCommit, LandedTask, NoticeDelivery},
  id::{NoticeId, RunId, TaskId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{RepositoryAccess, StepKey},
  revision::Revision,
  run::{BranchName, Run, RunStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, git, project, reported, scheduler, wait_for, wait_for_turns};

/// The lead of the notice's text, as the agent reads it.
const NOTICE: &str = "Orchester: the base branch `main` moved to ";
/// The start of what every writing agent is told about merging its base.
const MERGE_SAFELY: &str = "When Orchester tells you that the base branch moved, merge it into this worktree";

fn release(root: &Path) {
  std::fs::write(root.join(".orchester").join("fake-release"), "go").expect("release");
}

fn ended(run: &Run) -> bool {
  run.status().is_terminal()
}

/// Commits `subject` on `main` in the checkout, as a person's pull would bring it; its head.
fn landed(root: &Path, subject: &str) -> String {
  git(root, &["commit", "--quiet", "--allow-empty", "-m", subject]);
  git(root, &["rev-parse", "main"])
}

/// Queues a notice for `run` of `task` that `main` moved to `head`, as the desktop does, deliverable from
/// `deliver_after`.
fn queue(root: &Path, (run, task): (RunId, TaskId), (head, subject): (&str, &str), deliver_after: Timestamp) {
  let revision = Revision::new(head).expect("revision");
  let notice = BaseNotice {
    id: NoticeId::generate(),
    run,
    task,
    branch: BranchName::new("main").expect("branch"),
    revision: revision.clone(),
    commits: vec![LandedCommit {
      revision,
      subject: subject.to_owned(),
    }],
    tasks: vec![LandedTask {
      key: "ORC-7".to_owned(),
      title: "Pull the people's change".to_owned(),
    }],
    overlapping: vec!["README.md".to_owned()],
    noticed_at: Timestamp::now(),
    deliver_after,
    delivery: NoticeDelivery::Pending,
  };
  ProjectDb::open(root)
    .expect("open")
    .queue_base_notice(&notice)
    .expect("queued");
}

fn notices(root: &Path, run: RunId) -> Vec<BaseNotice> {
  ProjectDb::open(root)
    .expect("open")
    .run_base_notices(run)
    .expect("notices")
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_a_writing_agent_reads_the_newest_notice_after_its_turn() {
  let project = project(&Setup::new("Wait for release."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  let older = landed(root, "ORC-7: Implement (run 3)");
  queue(
    root,
    (id, project.task),
    (&older, "ORC-7: Implement (run 3)"),
    Timestamp::now(),
  );
  let newer = landed(root, "Merge ORC-7 run #3");
  queue(
    root,
    (id, project.task),
    (&newer, "Merge ORC-7 run #3"),
    Timestamp::now(),
  );
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2, "the step went on to read the notice");
  assert_eq!(
    invocations[1].session, invocations[0].session,
    "after its turn, in its session"
  );
  let prompt = reported(&db, id, "prompt| ");
  assert!(prompt.iter().any(|line| line.contains(MERGE_SAFELY)), "{prompt:?}");
  assert!(
    prompt
      .iter()
      .any(|line| line == &format!("{NOTICE}{newer} while you worked, and this worktree does not have it yet.")),
    "{prompt:?}"
  );
  assert!(
    !prompt.iter().any(|line| line.contains(&older)),
    "the older notice was replaced"
  );
  assert!(
    prompt.iter().any(|line| line == "- ORC-7: Pull the people's change"),
    "{prompt:?}"
  );
  assert!(prompt.iter().any(|line| line == "- `README.md`"), "{prompt:?}");
  let told = notices(root, id);
  assert_eq!(told.len(), 1, "one notice per run: {told:?}");
  assert_eq!(told[0].revision.as_str(), newer);
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { invocation, .. } if invocation == invocations[1].id),
    "{:?}",
    told[0].delivery
  );
  let log = git(root, &["log", "--format=%s", run.branch().as_str()]);
  assert!(log.contains("saved to merge the moved base"), "{log}");
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_a_read_only_step_keeps_the_notice_for_a_writing_step() {
  let project = project(&Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new("Wait for release.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  let head = landed(root, "A change");
  queue(root, (id, project.task), (&head, "A change"), Timestamp::now());
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.invocations(id).expect("invocations").len(),
    1,
    "the read-only step was not held"
  );
  let prompt = reported(&db, id, "prompt| ");
  assert!(!prompt.iter().any(|line| line.starts_with(NOTICE)), "{prompt:?}");
  assert!(
    !prompt.iter().any(|line| line.contains(MERGE_SAFELY)),
    "a read-only step merges nothing"
  );
  assert_eq!(
    notices(root, id)[0].delivery,
    NoticeDelivery::Pending,
    "kept for a writing step"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_a_paused_run_keeps_the_notice_until_it_goes_on() {
  let project = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
    .await
    .expect("answered in time")
    .expect("the pause is requested");
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  let head = landed(root, "A change while paused");
  queue(
    root,
    (id, project.task),
    (&head, "A change while paused"),
    Timestamp::now(),
  );
  assert_eq!(notices(root, id)[0].delivery, NoticeDelivery::Pending);

  scheduler.resume_run(id).await.expect("the run resumes");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let review = run
    .latest_execution(&StepKey::new("review").expect("key"))
    .expect("review ran")
    .id();
  let told = notices(root, id);
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { execution, .. } if execution == review),
    "the step that went on read it: {:?}",
    told[0].delivery
  );
  let prompt = reported(&db, id, "prompt| ");
  assert!(
    prompt.iter().any(|line| line == "# The base branch moved"),
    "{prompt:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_a_notice_within_its_window_or_with_the_switch_off_waits() {
  for (window, off) in [(true, false), (false, true)] {
    let project = project(&Setup::new("Wait for release."));
    let root = project.repo.path();
    if off {
      ProjectDb::open(root)
        .expect("open")
        .save_tells_base_moves(false)
        .expect("off");
    }
    let (scheduler, mut events) = scheduler(root);
    let id = scheduler
      .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    wait_for_turns(root, &mut events, id, 1).await;
    let head = landed(root, "A change");
    let deliver_after = if window {
      Timestamp::from_unix_millis(Timestamp::now().unix_millis() + 60 * 60 * 1000)
    } else {
      Timestamp::now()
    };
    queue(root, (id, project.task), (&head, "A change"), deliver_after);
    release(root);
    let run = wait_for(root, &mut events, id, ended).await;
    assert_eq!(run.status(), &RunStatus::Succeeded);
    let db = ProjectDb::open(root).expect("open");
    assert_eq!(
      db.invocations(id).expect("invocations").len(),
      1,
      "window {window}, off {off}"
    );
    assert_eq!(notices(root, id)[0].delivery, NoticeDelivery::Pending);
  }
}
