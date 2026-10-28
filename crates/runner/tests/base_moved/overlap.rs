//! Only a base move that overlaps the run's changes interrupts a writing step (ORC-151): one that does not lets its
//! step pass, and the runner merges it itself before Check; a merge of the runner's that conflicts is undone and the
//! notice goes to the next writing step; pending moves combine, and the combined one is delivered once any of its
//! files overlap.

use std::path::Path;

use orchester_core::{
  base_move::{BaseNotice, LandedCommit, NoticeDelivery},
  id::{NoticeId, RunId, TaskId},
  revision::Revision,
  run::{BranchName, Run, RunStatus},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  NOTICE, delivery, ended, execution_of, implement, invocations, notices, release,
  support::{Setup, StepSpec, TestProject, git, project, reported, scheduler, wait_for, wait_for_turns},
  verify,
};

/// Commits `file` (in folders of its own when its path has them) on `main` in the checkout as `subject`, as a
/// person's pull would bring it; its head.
fn landed_file(root: &Path, file: &str, subject: &str) -> String {
  let path = root.join(file);
  std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
  std::fs::write(&path, "landed\n").expect("written");
  git(root, &["add", "--", file]);
  git(root, &["commit", "--quiet", "-m", subject]);
  git(root, &["rev-parse", "main"])
}

/// Queues a notice for `run` of `task` that `main` moved to `head`, deliverable now, with `commits` (revision and
/// subject, newest first) and the files the desktop found both sides changed, as it does.
fn queue_moved(root: &Path, (run, task): (RunId, TaskId), head: &str, commits: &[(&str, &str)], overlapping: &[&str]) {
  let notice = BaseNotice {
    id: NoticeId::generate(),
    run,
    task,
    branch: BranchName::new("main").expect("branch"),
    revision: Revision::new(head).expect("revision"),
    commits: commits
      .iter()
      .map(|(revision, subject)| LandedCommit {
        revision: Revision::new(revision).expect("revision"),
        subject: (*subject).to_owned(),
      })
      .collect(),
    tasks: Vec::new(),
    overlapping: overlapping.iter().map(|path| (*path).to_owned()).collect(),
    noticed_at: Timestamp::now(),
    deliver_after: Timestamp::now(),
    delivery: NoticeDelivery::Pending,
  };
  ProjectDb::open(root)
    .expect("open")
    .queue_base_notice(&notice)
    .expect("queued");
}

/// `setup`'s project and its run, once its first agent's turn started: `moved` changes the base then, and the run is
/// released and followed to its end.
async fn run_while_moved(setup: &Setup, moved: impl FnOnce(&Path, (RunId, TaskId))) -> (TestProject, Run) {
  let project = project(setup);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  moved(root, (id, project.task));
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  (project, run)
}

fn worktree_of(root: &Path, run: &Run) -> std::path::PathBuf {
  root.join(".orchester").join("worktrees").join(run.id().to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_without_overlap_implement_passes_and_the_runner_merges_before_check() {
  let setup = delivery(
    implement("Wait for release."),
    verify("Compare the screen. Change nothing."),
  );
  let mut head = String::new();
  let (project, run) = run_while_moved(&setup, |root, ids| {
    head = landed_file(root, "landed.md", "ORC-7: Land a page");
    queue_moved(root, ids, &head, &[(&head, "ORC-7: Land a page")], &[]);
  })
  .await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    invocations(root, &run),
    4,
    "Implement was not interrupted; Check, Review and Verify ran once"
  );
  let log = git(root, &["log", "--format=%s", run.branch().as_str()]);
  assert!(!log.contains("saved to merge the moved base"), "{log}");

  let check = execution_of(&run, "check");
  let told = notices(root, run.id());
  assert_eq!(told.len(), 1, "{told:?}");
  let NoticeDelivery::Merged {
    execution, revision, ..
  } = &told[0].delivery
  else {
    panic!("the runner merged it: {:?}", told[0].delivery);
  };
  assert_eq!(*execution, check, "before Check started");
  let parents = git(root, &["rev-list", "--parents", "-n", "1", revision.as_str()]);
  assert_eq!(
    parents.split(' ').count(),
    3,
    "a merge commit, never a fast-forward: {parents}"
  );
  assert!(parents.ends_with(&head), "{parents}");
  let subject = git(root, &["log", "-1", "--format=%s", revision.as_str()]);
  assert!(subject.contains("merge the moved base main"), "{subject}");
  let checkpoint = run.last_checkpoint().expect("the merge's checkpoint");
  assert_eq!(checkpoint.revision(), revision, "Check started on the merged tree");
  assert_eq!(checkpoint.execution(), check);
  assert!(worktree_of(root, &run).join("landed.md").is_file());
  let prompt = reported(&ProjectDb::open(root).expect("open"), run.id(), "prompt| ");
  assert!(!prompt.iter().any(|line| line.starts_with(NOTICE)), "{prompt:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_with_overlap_in_the_worktree_the_notice_reaches_implement() {
  let setup = delivery(
    implement("Wait for release."),
    verify("Compare the screen. Change nothing."),
  );
  let (project, run) = run_while_moved(&setup, |root, ids| {
    std::fs::write(root.join("README.md"), "hi\nfrom the base\n").expect("written");
    git(root, &["commit", "--quiet", "-am", "ORC-7: Reword the readme"]);
    let head = git(root, &["rev-parse", "main"]);
    // The desktop saw no overlap: the run had committed nothing yet; its agent's README.md is uncommitted.
    queue_moved(root, ids, &head, &[(&head, "ORC-7: Reword the readme")], &[]);
  })
  .await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let implement = execution_of(&run, "implement");
  let told = notices(root, run.id());
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { execution, .. } if execution == implement),
    "{:?}",
    told[0].delivery
  );
  assert_eq!(told[0].overlapping, ["README.md"], "kept as found");
  assert_eq!(invocations(root, &run), 5, "Implement went on to read it");
  let log = git(root, &["log", "--format=%s", run.branch().as_str()]);
  assert!(log.contains("saved to merge the moved base"), "{log}");
  assert!(
    !log.contains("merge the moved base main"),
    "the runner merged nothing: {log}"
  );
  let prompt = reported(&ProjectDb::open(root).expect("open"), run.id(), "prompt| ");
  assert!(prompt.iter().any(|line| line == "- `README.md`"), "{prompt:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_a_runner_merge_that_conflicts_is_undone_and_goes_to_the_next_writing_step() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Wait for release. Write own file.", &[]),
      StepSpec::new("check", "Check", "Check the work. Change nothing.", &["implement"]).retry("implement", 2),
      StepSpec::new("document", "Document", "Document the work.", &["check"]),
    ],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_while_moved(&setup, |root, ids| {
    // A folder where Implement writes its file: no path in common, yet the merge conflicts.
    let head = landed_file(root, "implement.txt/landed.md", "ORC-7: Land a folder");
    queue_moved(root, ids, &head, &[(&head, "ORC-7: Land a folder")], &[]);
  })
  .await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let document = execution_of(&run, "document");
  let told = notices(root, run.id());
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { execution, .. } if execution == document),
    "Implement was not interrupted, the runner's merge before Check conflicted, and Document took it: {:?}",
    told[0].delivery
  );
  assert!(!told[0].overlapping.is_empty(), "the files it conflicted in");
  assert_eq!(invocations(root, &run), 3, "no step was held to read it");
  let log = git(root, &["log", "--format=%s", run.branch().as_str()]);
  assert!(
    !log.contains("merge the moved base main"),
    "the merge was undone: {log}"
  );
  assert!(!log.contains("saved to merge the moved base"), "{log}");
  let worktree = worktree_of(root, &run);
  assert!(
    worktree.join("implement.txt").is_file(),
    "Implement's file is as it was"
  );
  assert!(
    !worktree.join("implement.txt~HEAD").exists(),
    "nothing of the merge is left"
  );
  assert_eq!(git(&worktree, &["status", "--porcelain"]), "");
}

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_pending_moves_combine_and_the_combined_one_is_delivered_once_it_overlaps() {
  let setup = delivery(
    implement("Wait for release."),
    verify("Compare the screen. Change nothing."),
  );
  let mut subjects = Vec::new();
  let (project, run) = run_while_moved(&setup, |root, ids| {
    let first = landed_file(root, "landed.md", "ORC-7: Land a page");
    queue_moved(root, ids, &first, &[(&first, "ORC-7: Land a page")], &[]);
    std::fs::write(root.join("README.md"), "hi\nfrom the base\n").expect("written");
    git(root, &["commit", "--quiet", "-am", "ORC-8: Reword the readme"]);
    let second = git(root, &["rev-parse", "main"]);
    // The newer move replaces the pending one with what the run's branch lacks of both.
    let commits = [
      (second.as_str(), "ORC-8: Reword the readme"),
      (first.as_str(), "ORC-7: Land a page"),
    ];
    queue_moved(root, ids, &second, &commits, &[]);
    subjects.extend(commits.map(|(revision, subject)| format!("- {} {subject}", &revision[..10])));
  })
  .await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let told = notices(root, run.id());
  assert_eq!(told.len(), 1, "one combined notice: {told:?}");
  let implement = execution_of(&run, "implement");
  assert!(
    matches!(told[0].delivery, NoticeDelivery::Delivered { execution, .. } if execution == implement),
    "the README.md of the newer move overlaps Implement's: {:?}",
    told[0].delivery
  );
  let prompt = reported(&ProjectDb::open(root).expect("open"), run.id(), "prompt| ");
  for subject in &subjects {
    assert!(prompt.contains(subject), "{subject} in {prompt:?}");
  }
  assert!(prompt.iter().any(|line| line == "- `README.md`"), "{prompt:?}");
}
