//! Notices that the run's base branch moved (ORC-54) end to end against `orchester-fake-codex`: the desktop queues a
//! notice, and an agent of a writing step that finishes its turn while one is pending continues its session to read
//! it, the newest when a newer move replaced an undelivered one; a read-only step never takes it; a paused run keeps
//! it and its next writing step takes it once the run goes on; one within its move's window, or while the project's
//! switch is off, waits. Only the step that implements the change takes it (ORC-65): in Implement → Check → Review →
//! Verify, where every step writes, a move during Verify stays pending and one during Implement reaches Implement,
//! unless the steps choose otherwise. Only a notice that overlaps the run's changes interrupts a writing step
//! (ORC-151): one that does not lets its step pass and the runner merges it itself before Check; a merge of the
//! runner's that conflicts is undone and the notice goes to the next writing step; pending moves combine, and the
//! combined one is delivered once any of its files overlap. Every writing agent is told how to merge the base safely.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test base_moved base_moved`.

#[path = "base_moved/overlap.rs"]
mod overlap;
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

use self::support::{
  PATIENCE, Setup, StepSpec, TestProject, git, project, reported, scheduler, wait_for, wait_for_turns,
};

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
    steps: vec![
      StepSpec::new("implement", "Implement", "Change README.md.", &[]),
      StepSpec::new("document", "Document", "Change README.md.", &["implement"]),
    ],
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
  let document = run
    .latest_execution(&StepKey::new("document").expect("key"))
    .expect("document ran")
    .id();
  let told = notices(root, id);
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { execution, .. } if execution == document),
    "the writing step that went on read it: {:?}",
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

/// Implement → Check → Review → Verify, each check returning its failures to Implement and every step writing, as a
/// verifier that writes build output and QA files does (ORC-65); `implement` and `verify` are what those two do. An
/// earlier step's prompt lists the first sentence of each later step's instructions, so what a later step's fake agent
/// does comes after a first sentence that asks nothing of it.
fn delivery(implement: StepSpec, verify: StepSpec) -> Setup {
  Setup {
    steps: vec![
      implement,
      StepSpec::new("check", "Check", "Check the work. Change nothing.", &["implement"]).retry("implement", 2),
      StepSpec::new("review", "Review", "Review the work. Change nothing.", &["check"]).retry("implement", 2),
      verify,
    ],
    ..Setup::new("Say hello.")
  }
}

fn implement(instructions: &'static str) -> StepSpec {
  StepSpec::new("implement", "Implement", instructions, &[])
}

fn verify(instructions: &'static str) -> StepSpec {
  StepSpec::new("verify", "Verify", instructions, &["review"]).retry("implement", 2)
}

/// `setup`'s project, whose run was told its base moved once `turns` of its agents' turns started, and that run
/// after it ended.
async fn moved_after_turns(setup: &Setup, turns: usize) -> (TestProject, Run) {
  let project = project(setup);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, turns).await;
  let head = landed(root, "A change");
  queue(root, (id, project.task), (&head, "A change"), Timestamp::now());
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  (project, run)
}

fn execution_of(run: &Run, step: &str) -> orchester_core::id::StepExecutionId {
  run
    .latest_execution(&StepKey::new(step).expect("key"))
    .expect("the step ran")
    .id()
}

fn invocations(root: &Path, run: &Run) -> usize {
  ProjectDb::open(root)
    .expect("open")
    .invocations(run.id())
    .expect("invocations")
    .len()
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_during_verify_the_notice_stays_pending_and_never_reaches_verify() {
  let setup = delivery(
    implement("Change README.md."),
    verify("Compare the screen. Wait for release."),
  );
  let (project, run) = moved_after_turns(&setup, 4).await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    invocations(root, &run),
    4,
    "Verify, which writes, was not held to read it"
  );
  assert_eq!(
    notices(root, run.id())[0].delivery,
    NoticeDelivery::Pending,
    "it waits until the work goes back to Implement"
  );
  let prompt = reported(&ProjectDb::open(root).expect("open"), run.id(), "prompt| ");
  assert!(!prompt.iter().any(|line| line.starts_with(NOTICE)), "{prompt:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_during_implement_the_notice_reaches_implement() {
  let setup = delivery(
    implement("Wait for release."),
    verify("Compare the screen. Change nothing."),
  );
  let (project, run) = moved_after_turns(&setup, 1).await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let implement = execution_of(&run, "implement");
  let told = notices(root, run.id());
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { execution, .. } if execution == implement),
    "{:?}",
    told[0].delivery
  );
  assert_eq!(
    invocations(root, &run),
    5,
    "Implement went on to read it, then Check, Review and Verify ran once"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_the_steps_own_choice_overrides_the_default() {
  let setup = delivery(
    implement("Wait for release.").takes_base_moves(false),
    verify("Compare the screen. Change nothing.").takes_base_moves(true),
  );
  let (project, run) = moved_after_turns(&setup, 1).await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let verify = execution_of(&run, "verify");
  let told = notices(root, run.id());
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { execution, .. } if execution == verify),
    "Implement chose not to take it and Verify chose to: {:?}",
    told[0].delivery
  );
  assert_eq!(
    invocations(root, &run),
    4,
    "Implement was not held to read it; Verify read it as it started"
  );
}

/// What a Verify that writes only git-ignored files (build output, QA files) leaves is none of the run's own work: no
/// checkpoint and no commit of its own, so the step *Ask an agent to resolve* picks (`implementing_step`, the one whose
/// work the last checkpoint holds) and the base the run's work starts from (its own commits) stay Implement's.
#[tokio::test(flavor = "multi_thread")]
async fn base_moved_a_verify_that_writes_only_ignored_files_leaves_no_work_of_its_own() {
  let setup = delivery(
    implement("Change README.md."),
    verify("Compare the screen. Write own file."),
  );
  let project = project(&setup);
  let root = project.repo.path();
  std::fs::write(root.join(".gitignore"), "verify.txt\n").expect("ignore");
  git(root, &["add", ".gitignore"]);
  git(root, &["commit", "--quiet", "-m", "Ignore what Verify writes"]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let worktree = root.join(".orchester").join("worktrees").join(id.to_string());
  assert!(worktree.join("verify.txt").is_file(), "Verify wrote its file");
  let checkpoint = run.last_checkpoint().expect("Implement's checkpoint");
  assert_eq!(checkpoint.execution(), execution_of(&run, "implement"));
  let own = git(
    root,
    &["rev-list", "--count", &format!("main..{}", run.branch().as_str())],
  );
  assert_eq!(own, "1", "only Implement's checkpoint is on the run's branch");
  assert_eq!(git(&worktree, &["status", "--porcelain"]), "", "nothing left to commit");
}
