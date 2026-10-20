//! Returning the work of an ended run to a step (the project task ORC-44 · Return a finished run to an earlier step)
//! as the task page reads it: what a return to resolve a merge's conflicts is for is kept with it, and while its step
//! works on them the run's row in *Runs of this task* says when and with which branch; a run that still works, one
//! merged, one whose worktree is gone and one a merge waits in are refused.

mod support;

use orchester_core::{
  integration::{MergeKind, RunMerge, resolve::Resolution},
  revision::Revision,
  run::{Answerer, BranchName, MergeConflicts},
  store::MergeStore,
};
use orchester_workspace::tasks::{
  detail::TaskDetail,
  resolve::Worktree,
  return_work::{self, ReturnRefusal},
};

fn conflicts() -> MergeConflicts {
  MergeConflicts {
    base: BranchName::new("main").expect("a branch"),
    files: vec!["src/a.rs".to_owned()],
  }
}

fn present(merge: Option<Resolution>) -> Worktree {
  Worktree { present: true, merge }
}

#[test]
fn return_work_the_conflict_note_names_no_file() {
  let note = return_work::conflict_note(&BranchName::new("main").expect("a branch"));
  assert!(
    note.as_str().starts_with("Merge main into this branch"),
    "{}",
    note.as_str()
  );
  assert!(note.as_str().contains("conflicts listed below"), "{}", note.as_str());
}

#[test]
fn return_work_the_conflict_note_takes_the_callers_note_after_its_own_words() {
  let main = BranchName::new("main").expect("a branch");
  let page = return_work::conflict_note(&main);
  let note = return_work::conflict_note_with(&main, Some("  Keep both retry settings.  "));
  assert!(note.as_str().starts_with(page.as_str()), "{}", note.as_str());
  assert!(
    note.as_str().ends_with("\n\nKeep both retry settings."),
    "{}",
    note.as_str()
  );
  for empty in [None, Some(""), Some("   ")] {
    assert_eq!(return_work::conflict_note_with(&main, empty), page, "{empty:?}");
  }
}

#[test]
fn return_work_while_the_step_resolves_the_run_row_says_when_and_with_what() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(task, support::at(20));
  fixture.succeed(&mut run, support::at(30));
  let step = run.ended_return_targets().remove(0);
  let note = return_work::conflict_note(&conflicts().base);
  let events = run
    .return_ended(&step, (&note, Some(&conflicts())), Answerer::Person, support::at(40))
    .expect("returned");
  fixture.record(&run, &events);
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, support::at(41)).expect("started");
  fixture.record(&run, &events);
  let (resolving, resolved) = return_work::resolving_conflicts(&run).expect("the step resolves");
  assert_eq!(resolving, &step);
  assert_eq!(resolved, &conflicts());
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
fn return_work_is_refused_while_the_run_works_once_merged_without_its_worktree_and_in_a_merge() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(task, support::at(20));
  let root = fixture.repo.path().to_path_buf();
  let read = |root: &std::path::Path| TaskDetail::load_in(root, task).expect("detail");
  let detail = read(&root);
  let view = detail.run.as_ref().expect("its run");
  assert_eq!(
    return_work::refusal(&detail, view, &present(None)),
    Some(ReturnRefusal::NotEnded)
  );

  fixture.succeed(&mut run, support::at(30));
  let detail = read(&root);
  let view = detail.run.as_ref().expect("its run");
  let gone = Worktree {
    present: false,
    merge: None,
  };
  assert_eq!(
    return_work::refusal(&detail, view, &gone),
    Some(ReturnRefusal::WorktreeRemoved)
  );
  assert_eq!(
    return_work::refusal(&detail, view, &present(Some(Resolution { files: Vec::new() }))),
    Some(ReturnRefusal::InMerge)
  );
  assert_eq!(return_work::refusal(&detail, view, &present(None)), None);

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
  assert_eq!(
    return_work::refusal(&detail, view, &present(None)),
    Some(ReturnRefusal::Merged)
  );
}
