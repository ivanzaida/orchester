//! Removing a run's worktrees with the removal recorded first: refused while the store says the
//! run has live work; taken back when git removed nothing; kept once anything went, even when the
//! run's own worktree stayed, so the run never continues without a worktree it may need.

use std::{fs, path::Path};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  id::{RunId, TaskId},
  integration::{
    CleanupError, FolderListing, KeepReason, RecordedCleanupError, RemovalStart, WorktreeOwner, clean_up_recorded,
    clean_up_recorded_as_seen, leftovers,
  },
  store::WorktreeStore,
  time::Timestamp,
};

use crate::{
  cleanup::repo_with_leftovers,
  support::{Ending, Processes, git_program, now},
};

/// The removal record of one run, and whether the run has live work as stored.
#[derive(Default)]
struct Removals {
  live_work: bool,
  recorded: Option<Timestamp>,
}

impl WorktreeStore for Removals {
  type Error = std::io::Error;

  fn save_worktree_removal(&mut self, _run: RunId, at: Timestamp) -> Result<(), Self::Error> {
    self.recorded.get_or_insert(at);
    Ok(())
  }

  fn begin_worktree_removal(&mut self, _run: RunId, at: Timestamp) -> Result<RemovalStart, Self::Error> {
    Ok(match (self.live_work, self.recorded) {
      (true, _) => RemovalStart::LiveWork,
      (false, Some(_)) => RemovalStart::AlreadyRecorded,
      (false, None) => {
        self.recorded = Some(at);
        RemovalStart::Recorded
      }
    })
  }

  fn undo_worktree_removal(&mut self, _run: RunId, at: Timestamp) -> Result<(), Self::Error> {
    if self.recorded == Some(at) {
      self.recorded = None;
    }
    Ok(())
  }

  fn worktree_removal(&self, _run: RunId) -> Result<Option<Timestamp>, Self::Error> {
    Ok(self.recorded)
  }

  fn worktree_removals(&self, _task: TaskId) -> Result<Vec<(RunId, Timestamp)>, Self::Error> {
    Ok(Vec::new())
  }
}

fn own(root: &Path, run: RunId) -> std::path::PathBuf {
  root.join(".orchester/worktrees").join(run.to_string())
}

#[test]
fn recorded_clean_up_is_refused_while_the_run_has_live_work_as_stored() {
  // The page read the run interrupted; it was continued since.
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let mut store = Removals {
    live_work: true,
    ..Removals::default()
  };
  let refused = clean_up_recorded(&mut store, (&Processes, &git_program(), &repo.root), &run, None, now());
  assert!(matches!(
    refused,
    Err(RecordedCleanupError::Cleanup(CleanupError::LiveWork))
  ));
  assert!(own(&repo.root, run.id()).is_dir(), "nothing was removed");
  assert_eq!(store.recorded, None);
}

#[test]
fn recorded_clean_up_takes_the_record_back_when_git_removed_nothing() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  fs::write(own(&repo.root, run.id()).join("draft.txt"), "not committed\n").expect("write");
  let step = repo
    .root
    .join(".orchester/worktrees")
    .join(crate::support::execution(&run, "review").to_string());
  fs::write(step.join("notes.txt"), "not committed\n").expect("write");
  let mut store = Removals::default();
  let cleanup =
    clean_up_recorded(&mut store, (&Processes, &git_program(), &repo.root), &run, None, now()).expect("cleared");
  assert!(cleanup.removed.is_empty(), "no change was confirmed: {cleanup:?}");
  assert_eq!(store.recorded, None, "the run can continue as before");
}

#[test]
fn recorded_clean_up_keeps_the_record_once_a_step_worktree_went_but_its_own_stayed() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  fs::write(own(&repo.root, run.id()).join("draft.txt"), "not committed\n").expect("write");
  let mut store = Removals::default();
  let at = now();
  let cleanup =
    clean_up_recorded(&mut store, (&Processes, &git_program(), &repo.root), &run, None, at).expect("cleared");
  assert_eq!(cleanup.removed.len(), 1);
  assert!(matches!(cleanup.removed[0].owner, WorktreeOwner::Step(_)));
  assert!(own(&repo.root, run.id()).is_dir());
  assert_eq!(
    store.recorded,
    Some(at),
    "a step it would continue from is gone: it never continues"
  );
}

#[test]
fn recorded_clean_up_records_a_removal() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let mut store = Removals::default();
  let at = now();
  let cleanup =
    clean_up_recorded(&mut store, (&Processes, &git_program(), &repo.root), &run, None, at).expect("cleared");
  assert!(cleanup.removed_run_worktree());
  assert_eq!(store.recorded, Some(at));
}

/// Git that refuses `git worktree remove`, as on Windows when a file is held open.
struct RefusingRemove;

impl CommandRunner for RefusingRemove {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let args = command.args();
    if args.len() > 1 && args[0] == "worktree" && args[1] == "remove" {
      return Ok(CommandOutput {
        code: Some(255),
        stdout: String::new(),
        stderr: "error: failed to delete: Invalid argument".to_owned(),
      });
    }
    Processes.run(command)
  }
}

#[test]
fn recorded_clean_up_keeps_the_record_once_git_refused_a_worktree() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let mut store = Removals::default();
  let at = now();
  let cleanup = clean_up_recorded(
    &mut store,
    (&RefusingRemove, &git_program(), &repo.root),
    &run,
    None,
    at,
  )
  .expect("cleared");
  assert!(cleanup.removed.is_empty());
  assert_eq!(cleanup.kept_worktrees.len(), 2);
  assert_eq!(
    store.recorded,
    Some(at),
    "a refusal can come part way: the run no longer continues"
  );
}

#[test]
fn recorded_clean_up_deletes_a_worktree_folder_git_forgot() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let folder = own(&repo.root, run.id());
  // Git forgets the worktree but its folder stays, as when a removal fails part way.
  repo.git(&["worktree", "remove", "--force", folder.to_str().expect("a UTF-8 path")]);
  fs::create_dir_all(folder.join("target")).expect("a folder left behind");
  fs::write(folder.join("target").join("held.bin"), "build output\n").expect("write");
  let left = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  let forgotten: Vec<_> = left.worktrees.iter().filter(|worktree| worktree.forgotten).collect();
  assert_eq!(forgotten.len(), 1);
  assert_eq!(forgotten[0].owner, WorktreeOwner::Run);
  assert!(!left.keeps_run_worktree(), "no agent can continue in it");

  assert_eq!(forgotten[0].uncommitted, ["target/held.bin"], "its files are listed");
  assert_eq!(forgotten[0].listing, FolderListing::Whole, "all of them");

  // A file that appeared after the person saw the folder keeps it.
  fs::write(
    folder.join("later.txt"),
    "later
",
  )
  .expect("write");
  let mut store = Removals::default();
  let cleanup = clean_up_recorded(
    &mut store,
    (&Processes, &git_program(), &repo.root),
    &run,
    Some(&left),
    now(),
  )
  .expect("cleared");
  assert!(!cleanup.removed_run_worktree());
  assert!(folder.join("later.txt").is_file());

  let left = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  let cleanup = clean_up_recorded(
    &mut store,
    (&Processes, &git_program(), &repo.root),
    &run,
    Some(&left),
    now(),
  )
  .expect("cleared");
  assert!(cleanup.removed_run_worktree());
  assert!(!folder.exists());
  assert!(
    repo.root.join(".orchester/worktrees").is_dir(),
    "only the run's folder goes"
  );
  assert!(store.recorded.is_some());
}

/// Git that does not finish `git worktree remove` in time.
struct TimingOut;

impl CommandRunner for TimingOut {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let args = command.args();
    if args.len() > 1 && args[0] == "worktree" && args[1] == "remove" {
      return Err(CommandError::TimedOut);
    }
    Processes.run(command)
  }
}

#[test]
fn recorded_clean_up_keeps_the_record_once_git_did_not_finish_a_removal() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let mut store = Removals::default();
  let at = now();
  let cleanup =
    clean_up_recorded(&mut store, (&TimingOut, &git_program(), &repo.root), &run, None, at).expect("cleared");
  assert!(cleanup.removed.is_empty());
  assert!(
    cleanup
      .kept_worktrees
      .iter()
      .all(|(_, reason)| matches!(reason, KeepReason::Failed(why) if why.contains("did not finish in time")))
  );
  assert_eq!(
    store.recorded,
    Some(at),
    "git may have removed part of it: the run no longer continues"
  );
}

#[test]
fn recorded_clean_up_takes_the_record_back_when_git_cannot_run() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let mut store = Removals::default();
  let missing = repo.root.join("no-such-git");
  let failed = clean_up_recorded(&mut store, (&Processes, &missing, &repo.root), &run, None, now());
  assert!(matches!(
    failed,
    Err(RecordedCleanupError::Cleanup(CleanupError::Git(_)))
  ));
  assert!(own(&repo.root, run.id()).is_dir());
  assert_eq!(
    store.recorded, None,
    "nothing was touched: the run can continue as before"
  );
}

#[test]
fn recorded_clean_up_of_a_run_changed_by_the_time_its_removal_is_recorded_removes_nothing() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let mut store = Removals::default();
  let refused = clean_up_recorded_as_seen(
    &mut store,
    (&Processes, &git_program(), &repo.root),
    &run,
    None,
    (now(), |store: &Removals| {
      // Asked once the removal is recorded: the run can no longer continue and change after the answer.
      assert!(store.recorded.is_some(), "read under the record");
      Ok(false)
    }),
  );
  assert!(matches!(refused, Err(RecordedCleanupError::RunChanged)), "{refused:?}");
  assert!(own(&repo.root, run.id()).is_dir(), "nothing was removed");
  assert_eq!(store.recorded, None, "the record is taken back");
}
