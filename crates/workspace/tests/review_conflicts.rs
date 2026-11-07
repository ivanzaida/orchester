//! A merge that conflicts on the review page (ORC-807): the conflicting files and lines with what the base has there,
//! *Ask the agent to resolve*, *Try merge again*, and the Review list around it. `cargo test -p orchester-workspace
//! --test review_conflicts`.

#[allow(dead_code, reason = "the review tests' fixture: these tests use a part of it")]
#[path = "review_threads/support.rs"]
mod support;

use std::fs;

use orchester_core::{
  integration::{
    MergeError,
    attempt::{AttemptOutcome, AttemptStage, MergeAsker, MergeAttempt, MergeVia},
  },
  pull_request::feedback::DiffSide,
  resource::StepKey,
  run::{Answerer, MergeConflicts, ReviewNote},
  store::RunStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::Command;
use orchester_workspace::tasks::{
  data::ProjectTasks,
  integration::{IntegrationProblem, merge_in},
  resolve::RunWorktrees,
  review_list::{ReviewState, review_list},
  review_threads::{
    ReviewRefusal,
    approve::{Approved, approve_and_merge},
    conflicts::{self, BaseLine},
    page::{ReviewPage, review_page},
  },
  worktree_removal::WorktreeRemover,
};
use support::{Repo, at, git};

fn remover(repo: &Repo) -> WorktreeRemover {
  WorktreeRemover::here_for_test(repo.root(), Programs::processes())
}

fn worktrees(repo: &Repo) -> RunWorktrees {
  RunWorktrees::here_for_test(repo.root(), Programs::processes())
}

/// The base branch changes the line the run added first, and the approved merge into it conflicts.
fn conflict_with_main(repo: &Repo) {
  fs::write(repo.root().join("src/lib.rs"), "fn one() { main }\n").expect("write");
  git(repo.root(), &["commit", "-am", "main changed it too"]);
  let problem = merge_in(&remover(repo), &Programs::processes(), (repo.run, None)).expect_err("conflicts");
  assert!(
    matches!(problem, IntegrationProblem::Merge(MergeError::Conflict { .. })),
    "{problem:?}"
  );
}

fn page(repo: &Repo) -> Option<ReviewPage> {
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(repo.root(), &programs).expect("the tasks read");
  let key = tasks.tasks[0].key().clone();
  review_page(&tasks, (repo.root(), &programs), (&key, repo.stored().number())).expect("the page reads")
}

/// The run's work goes back to Implement to resolve the conflicts of its merge, and Implement starts.
fn return_to_resolve(repo: &Repo) {
  let mut db = repo.db();
  let mut run = repo.stored();
  let step = StepKey::new("implement").expect("key");
  let conflicts = MergeConflicts {
    base: run.base_branch().cloned().expect("a base branch"),
    files: vec!["src/lib.rs".to_owned()],
  };
  let note = ReviewNote::new("Resolve.").expect("note");
  let events = run
    .return_ended(&step, (&note, Some(&conflicts)), None, Answerer::Person, at(30))
    .expect("returned");
  db.append_events(&run, &events).expect("events");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(31)).expect("started");
  db.append_events(&run, &events).expect("events");
}

fn later() -> Timestamp {
  Timestamp::from_unix_millis(Timestamp::now().unix_millis() + 60_000)
}

#[test]
fn review_conflicts_a_merge_that_conflicted_opens_the_review_with_its_files_and_the_base_lines() {
  let repo = Repo::new();
  assert!(
    page(&repo).expect("the page").conflicts.is_none(),
    "nothing was tried yet"
  );
  conflict_with_main(&repo);
  let page = page(&repo).expect("an approved run whose merge conflicted is reviewed again");
  let conflicts = page.conflicts.expect("the conflicts");
  assert_eq!(conflicts.files, ["src/lib.rs"]);
  assert_eq!(conflicts.base.as_ref().map(|base| base.as_str()), Some("main"));
  assert!(conflicts.conflicts_in("src/lib.rs") && !conflicts.conflicts_in("src/other.rs"));
  let [region] = conflicts.regions.as_slice() else {
    panic!("one region: {:?}", conflicts.regions);
  };
  assert_eq!(
    (region.path.as_str(), region.side, region.lines),
    ("src/lib.rs", DiffSide::New, (1, 3))
  );
  assert_eq!(
    region.base_lines,
    [BaseLine {
      number: 1,
      text: "fn one() { main }".to_owned()
    }]
  );
  assert!(page.review_in.is_some(), "it waits for review");
}

#[test]
fn review_conflicts_a_merge_that_was_refused_leaves_the_run_in_review_and_the_conflicts_as_they_were() {
  let repo = Repo::new();
  conflict_with_main(&repo);
  let refused = MergeAttempt::new(
    (repo.run, Timestamp::now()),
    (MergeAsker::Person, MergeVia::TaskPage),
    AttemptOutcome::Refused {
      stage: AttemptStage::ProjectDatabase,
    },
    Some("the project's database is busy"),
  );
  repo.db().record_merge_attempt(&refused).expect("recorded");
  let page = page(&repo).expect("the run is still reviewed");
  assert_eq!(page.conflicts.expect("the conflicts stand").files, ["src/lib.rs"]);
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(repo.root(), &programs).expect("tasks");
  let list = review_list(&tasks, (repo.root(), &programs), Timestamp::now()).expect("list");
  assert!(
    matches!(list.waiting[0].state, ReviewState::Conflicts { files: 1, .. }),
    "{:?}",
    list.waiting[0].state
  );
}

#[test]
fn review_conflicts_a_run_whose_merge_was_only_refused_is_the_attempt_pages() {
  let repo = Repo::new();
  let refused = MergeAttempt::new(
    (repo.run, Timestamp::now()),
    (MergeAsker::Person, MergeVia::TaskPage),
    AttemptOutcome::Refused {
      stage: AttemptStage::ProjectDatabase,
    },
    Some("the project's database is busy"),
  );
  repo.db().record_merge_attempt(&refused).expect("recorded");
  assert!(
    page(&repo).is_none(),
    "no conflicts: the attempt page tells a refused merge"
  );
}

#[test]
fn review_conflicts_work_after_the_merge_clears_them_and_the_run_is_reviewed_afresh() {
  let repo = Repo::new();
  conflict_with_main(&repo);
  assert!(page(&repo).expect("page").conflicts.is_some());
  return_to_resolve(&repo);
  let working = page(&repo).expect("the run Implement works on is shown");
  assert!(
    working.conflicts.is_none() && working.review_in.is_none(),
    "{:?}",
    working.conflicts
  );
  repo.implement(
    "fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\n",
    "Resolved.",
    later(),
  );
  let back = page(&repo).expect("it is back for review");
  assert!(back.conflicts.is_none(), "the old merge no longer stands");
  assert!(back.review_in.is_some());
}

#[test]
fn review_conflicts_ask_the_agent_returns_the_run_to_implement_with_the_conflicts_as_data() {
  let repo = Repo::new();
  let refused =
    conflicts::resolve::begin(repo.root(), &worktrees(&repo), repo.run, None).expect_err("nothing conflicts");
  assert_eq!(refused, ReviewRefusal::NoConflicts);
  conflict_with_main(&repo);
  let command = conflicts::resolve::begin(repo.root(), &worktrees(&repo), repo.run, None).expect("the command");
  let Command::ReturnRun(returned) = command else {
    panic!("a return of the run's work: {command:?}");
  };
  assert_eq!(returned.run, repo.run);
  assert_eq!(returned.step, StepKey::new("implement").expect("key"));
  assert_eq!(returned.by, Answerer::Person);
  let data = returned.conflicts.expect("the conflicts go as data");
  assert_eq!(data.files, ["src/lib.rs"]);
  assert_eq!(data.base.as_str(), "main");
  assert!(returned.note.as_str().contains("main"), "{:?}", returned.note);
}

#[test]
fn review_conflicts_the_task_is_with_the_agent_while_it_resolves() {
  let repo = Repo::new();
  conflict_with_main(&repo);
  return_to_resolve(&repo);
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(repo.root(), &programs).expect("tasks");
  let list = review_list(&tasks, (repo.root(), &programs), Timestamp::now()).expect("list");
  assert!(list.waiting.is_empty(), "{:?}", list.waiting);
  let [row] = list.with_agent.as_slice() else {
    panic!("one row with the agent: {:?}", list.with_agent);
  };
  assert!(
    matches!(&row.state, ReviewState::ResolvingConflicts { base: Some(base) } if base.as_str() == "main"),
    "{:?}",
    row.state
  );
}

#[test]
fn review_conflicts_the_list_stops_saying_conflicts_once_the_agent_committed_again() {
  let repo = Repo::new();
  conflict_with_main(&repo);
  let programs = Programs::processes();
  let read = || {
    let tasks = ProjectTasks::load(repo.root(), &programs).expect("tasks");
    review_list(&tasks, (repo.root(), &programs), Timestamp::now()).expect("list")
  };
  assert!(matches!(
    read().waiting[0].state,
    ReviewState::Conflicts { files: 1, .. }
  ));
  return_to_resolve(&repo);
  repo.implement(
    "fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\n",
    "Resolved.",
    later(),
  );
  let after = read();
  assert_eq!(after.waiting.len(), 1);
  assert!(
    !matches!(after.waiting[0].state, ReviewState::Conflicts { .. }),
    "{:?}",
    after.waiting[0].state
  );
}

#[test]
fn review_conflicts_try_merge_again_conflicts_again_until_the_base_moved_away() {
  let repo = Repo::new();
  conflict_with_main(&repo);
  let again = approve_and_merge(
    &remover(&repo),
    &Programs::processes(),
    (repo.run, &repo.head),
    Timestamp::now(),
  )
  .expect_err("the base still has its change");
  assert!(
    matches!(
      again,
      ReviewRefusal::Merge(IntegrationProblem::Merge(MergeError::Conflict { .. }))
    ),
    "{again:?}"
  );
  assert!(page(&repo).expect("page").conflicts.is_some(), "it still conflicts");
  // The base moves once more: the change that conflicted is taken back.
  git(repo.root(), &["revert", "--no-edit", "HEAD"]);
  let merged = approve_and_merge(
    &remover(&repo),
    &Programs::processes(),
    (repo.run, &repo.head),
    Timestamp::now(),
  )
  .expect("merged");
  assert!(matches!(merged, Approved::Merged(_)));
  let merged = page(&repo).expect("a merged run keeps its read-only review");
  assert!(merged.approved.is_some() && merged.conflicts.is_none());
}
