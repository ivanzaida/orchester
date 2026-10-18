//! Returning the work of an ended run to a step (ORC-44) as the task page reads it: the note *Ask an agent to resolve*
//! writes reads back as the conflicts it returns, and while the step works on them its row in *Runs of this task*
//! says when and with which branch; a merged run, one whose worktree is gone, and one still at work are refused.

mod support;

use std::fs;

use orchester_core::{
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::{BranchName, ReviewNote},
  store::MergeStore,
};
use orchester_workspace::tasks::{
  changes::run_worktree,
  detail::TaskDetail,
  return_work::{self, ConflictReturn, ReturnRefusal},
};

#[test]
fn return_work_a_conflict_note_reads_back_as_the_conflicts_it_returns() {
  let files = ["src/a.rs".to_owned(), "src/b.rs".to_owned()];
  let note = return_work::conflict_note("main", &files);
  assert_eq!(
    return_work::conflict_return(&note),
    Some(ConflictReturn {
      base: "main".to_owned(),
      files: "src/a.rs, src/b.rs".to_owned(),
    })
  );
  assert_eq!(
    return_work::conflict_return(&ReviewNote::new("Keep the old header.").expect("a note")),
    None
  );
}

#[test]
fn return_work_while_the_step_resolves_the_run_row_says_when_and_with_what() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(task, support::at(20));
  fixture.succeed(&mut run, support::at(30));
  let step = run.ended_return_targets().remove(0);
  let note = return_work::conflict_note("main", &["src/a.rs".to_owned()]);
  let events = run.return_ended(&step, &note, support::at(40)).expect("returned");
  fixture.record(&run, &events);
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, support::at(41)).expect("started");
  fixture.record(&run, &events);
  let (resolving, conflict) = return_work::resolving_conflicts(&run).expect("the step resolves");
  assert_eq!(resolving, &step);
  assert_eq!(conflict.base, "main");
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("detail");
  let row = &detail.history[0];
  assert_eq!(row.detail.key, "runs.history.atWorkResolving", "{:?}", row.detail);
  assert!(
    row
      .detail
      .args
      .iter()
      .any(|(name, value)| *name == "branch" && value == "main"),
    "{:?}",
    row.detail
  );
}

#[test]
fn return_work_is_refused_while_the_run_works_once_merged_and_without_its_worktree() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(task, support::at(20));
  let root = fixture.repo.path().to_path_buf();
  let read = |root: &std::path::Path| TaskDetail::load_in(root, task).expect("detail");
  let detail = read(&root);
  let view = detail.run.as_ref().expect("its run");
  assert_eq!(
    return_work::refusal(&root, &detail, view),
    Some(ReturnRefusal::NotEnded)
  );

  fixture.succeed(&mut run, support::at(30));
  let detail = read(&root);
  let view = detail.run.as_ref().expect("its run");
  assert_eq!(
    return_work::refusal(&root, &detail, view),
    Some(ReturnRefusal::WorktreeRemoved),
    "the fixture's run has no worktree folder"
  );
  fs::create_dir_all(run_worktree(&root, run.id())).expect("the worktree's folder");
  assert_eq!(return_work::refusal(&root, &detail, view), None);

  fixture
    .db
    .save_run_merge(&RunMerge {
      run: run.id(),
      into: BranchName::new("main").expect("branch"),
      revision: Revision::new(&"c".repeat(40)).expect("revision"),
      kind: MergeKind::FastForward,
      merged_at: support::at(35),
    })
    .expect("merged");
  let detail = read(&root);
  let view = detail.run.as_ref().expect("its run");
  assert_eq!(return_work::refusal(&root, &detail, view), Some(ReturnRefusal::Merged));
}
