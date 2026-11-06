//! A merge that fails for a reason other than the code ends in a state the person can retry from (ORC-365): a git
//! that stops answering is told at its step with its temporary worktree removed and what it wrote put back; files a
//! bring-in left changed are named, put back without touching the person's, and keep every merge out until then; and
//! what a merge the app ended part way left is cleared. `cargo test -p orchester-core --test integration merge_`.

use std::{
  cell::Cell,
  fs,
  panic::{AssertUnwindSafe, catch_unwind},
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  integration::{
    Interrupted, MergeError, MergeKind, MergeOutcome, MergeStep, clear_interrupted, left_by_bring_in, merge_run,
    put_back,
  },
  run::Run,
};

use crate::{
  merge::repo_with_run_work,
  support::{Ending, Processes, RUN_BRANCH, Repo, git_program, now, run, task},
};

const SILENCE: Duration = Duration::from_secs(120);

/// Git as [`Processes`] runs it, except for the commands `fake` answers itself.
struct Faked<F>(F);

impl<F: Fn(&[String]) -> Option<Result<CommandOutput, CommandError>>> CommandRunner for Faked<F> {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    (self.0)(command.args()).unwrap_or_else(|| Processes.run(command))
  }
}

fn has(args: &[String], words: &[&str]) -> bool {
  words.iter().all(|word| args.iter().any(|arg| arg == word))
}

fn failed(stderr: &str) -> Option<Result<CommandOutput, CommandError>> {
  Some(Ok(CommandOutput {
    code: Some(1),
    stdout: String::new(),
    stderr: stderr.to_owned(),
  }))
}

fn merge_with(runner: &impl CommandRunner, repo: &Repo, finished: &Run) -> Result<MergeOutcome, MergeError> {
  merge_run(runner, &git_program(), &repo.root, (finished, &task()), now())
}

/// A repository whose base changes `a.txt` and `b.txt`, and a run that changes both again and adds `new.txt`.
fn repo_with_two_files() -> (Repo, String, Run) {
  let repo = Repo::new();
  repo.commit(&repo.root, "a.txt", "base a\n", "a");
  let base = repo.commit(&repo.root, "b.txt", "base b\n", "b");
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "a.txt", "run a\n", "Implement: a");
  repo.commit(&worktree, "b.txt", "run b\n", "Implement: b");
  repo.commit(&worktree, "new.txt", "run new\n", "Implement: new");
  let finished = run(&base, Some("main"), Ending::Succeeded);
  (repo, base, finished)
}

/// Writes the run's version of `files` into the checkout, as a fast-forward that stopped part way does.
fn wrote_part_way(repo: &Repo, files: &[&str]) {
  for file in files {
    let name = file.trim_end_matches(".txt");
    fs::write(repo.root.join(file), format!("run {name}\n")).expect("git wrote the file");
  }
}

fn read(repo: &Repo, file: &str) -> String {
  fs::read_to_string(repo.root.join(file)).expect("there")
}

fn assert_no_temporary(repo: &Repo) {
  let listed = repo.git(&["worktree", "list", "--porcelain"]);
  assert!(!listed.contains("orchester-merge"), "{listed}");
  let names: Vec<String> = fs::read_dir(repo.root.join(".git"))
    .expect("the git folder")
    .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
    .collect();
  assert!(
    names.iter().all(|name| !name.starts_with("orchester-merge")),
    "{names:?}"
  );
}

#[test]
fn merge_watchdog_a_merge_that_stops_answering_is_told_at_its_step_with_the_temporary_worktree_removed() {
  let (repo, base, _) = repo_with_run_work();
  let moved = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  let finished = run(&base, Some("main"), Ending::Succeeded);
  let hangs =
    Faked(|args: &[String]| has(args, &["merge", "--no-ff"]).then_some(Err(CommandError::Silent { after: SILENCE })));
  let refused = merge_with(&hangs, &repo, &finished);
  let Err(MergeError::Unanswered { step, command, after }) = &refused else {
    panic!("git stopped answering: {refused:?}");
  };
  assert_eq!((*step, *after), (MergeStep::Merge, SILENCE));
  assert!(command.contains("merge") && command.contains("--no-ff"), "{command}");
  let told = refused.expect_err("not merged").to_string();
  assert!(told.contains("merging") && told.contains("120 s"), "{told}");
  assert_eq!(repo.head(), moved);
  assert!(repo.is_clean());
  assert_no_temporary(&repo);
}

#[test]
fn merge_watchdog_a_temporary_worktree_that_stops_answering_is_told_there_and_removed() {
  let (repo, base, _) = repo_with_run_work();
  repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  let finished = run(&base, Some("main"), Ending::Succeeded);
  let hangs =
    Faked(|args: &[String]| has(args, &["reset", "--hard"]).then_some(Err(CommandError::Silent { after: SILENCE })));
  let refused = merge_with(&hangs, &repo, &finished);
  assert!(
    matches!(
      refused,
      Err(MergeError::Unanswered {
        step: MergeStep::TemporaryWorktree,
        ..
      })
    ),
    "{refused:?}"
  );
  assert_no_temporary(&repo);
}

#[test]
fn merge_watchdog_a_bring_in_that_stops_answering_is_put_back() {
  let (repo, base, finished) = repo_with_two_files();
  let hangs = Faked(|args: &[String]| {
    has(args, &["merge", "--ff-only"]).then(|| {
      wrote_part_way(&repo, &["a.txt", "new.txt"]);
      Err(CommandError::Silent { after: SILENCE })
    })
  });
  let refused = merge_with(&hangs, &repo, &finished);
  assert!(
    matches!(
      refused,
      Err(MergeError::Unanswered {
        step: MergeStep::BringIn,
        ..
      })
    ),
    "{refused:?}"
  );
  assert_eq!(repo.head(), base);
  assert_eq!(read(&repo, "a.txt"), "base a\n");
  assert!(!repo.root.join("new.txt").exists(), "a file the base did not have went");
  assert!(repo.is_clean());
  assert_eq!(
    left_by_bring_in(&Processes, (&git_program(), &repo.root), finished.id()),
    Ok(Vec::new()),
    "nothing is left to put back"
  );
}

/// Fast-forwards that write the run's `a.txt` and `b.txt` and stop, and a put-back that cannot write them.
fn left_changed(repo: &Repo, finished: &Run) -> Result<MergeOutcome, MergeError> {
  let stops = Faked(|args: &[String]| {
    if has(args, &["merge", "--ff-only"]) {
      wrote_part_way(repo, &["a.txt", "b.txt"]);
      return failed("error: unable to write new.txt: Input/output error");
    }
    has(args, &["checkout-index"]).then(|| failed("error: a.txt: Input/output error").expect("an answer"))
  });
  merge_with(&stops, repo, finished)
}

#[test]
fn merge_put_back_names_the_files_and_keeps_every_merge_out_until_they_are_back() {
  let (repo, base, finished) = repo_with_two_files();
  let refused = left_changed(&repo, &finished);
  let Err(MergeError::LeftChanged { files, .. }) = &refused else {
    panic!("files were left changed: {refused:?}");
  };
  assert_eq!(files, &["a.txt", "b.txt"]);
  let git = (git_program(), repo.root.clone());
  let git = (git.0.as_path(), git.1.as_path());
  assert_eq!(
    left_by_bring_in(&Processes, git, finished.id()),
    Ok(vec!["a.txt".to_owned(), "b.txt".to_owned()])
  );
  let again = merge_with(&Processes, &repo, &finished);
  assert_eq!(
    again,
    Err(MergeError::NotPutBack {
      run: finished.id(),
      files: vec!["a.txt".to_owned(), "b.txt".to_owned()],
    }),
    "Try again is refused until the files are back"
  );
  assert_eq!(repo.head(), base);

  assert_eq!(put_back(&Processes, git, finished.id()), Ok(()));
  assert_eq!(
    (read(&repo, "a.txt"), read(&repo, "b.txt")),
    ("base a\n".to_owned(), "base b\n".to_owned())
  );
  assert!(repo.is_clean());
  assert_eq!(left_by_bring_in(&Processes, git, finished.id()), Ok(Vec::new()));
  let merged = merge_with(&Processes, &repo, &finished).expect("merged once the files are back");
  assert_eq!(merged.merge.kind, MergeKind::FastForward);
}

#[test]
fn merge_put_back_that_fails_with_a_git_error_keeps_the_files_named_until_they_are_back() {
  let (repo, base, finished) = repo_with_two_files();
  let listed = Cell::new(false);
  let stops = Faked(|args: &[String]| {
    if has(args, &["merge", "--ff-only"]) {
      wrote_part_way(&repo, &["a.txt", "b.txt"]);
      return failed("error: unable to write new.txt: Input/output error");
    }
    // The put-back's first read of the index fails; what it left changed is read after it.
    (has(args, &["ls-files"]) && !listed.replace(true)).then(|| failed("fatal: index file corrupt").expect("an answer"))
  });
  let refused = merge_with(&stops, &repo, &finished);
  let Err(MergeError::LeftChanged { files, detail }) = &refused else {
    panic!("files were left changed: {refused:?}");
  };
  assert_eq!(files, &["a.txt", "b.txt"]);
  assert!(detail.contains("index file corrupt"), "{detail}");
  let git = (git_program(), repo.root.clone());
  let git = (git.0.as_path(), git.1.as_path());
  assert_eq!(
    left_by_bring_in(&Processes, git, finished.id()),
    Ok(vec!["a.txt".to_owned(), "b.txt".to_owned()])
  );
  assert_eq!(
    merge_with(&Processes, &repo, &finished),
    Err(MergeError::NotPutBack {
      run: finished.id(),
      files: vec!["a.txt".to_owned(), "b.txt".to_owned()],
    }),
    "Try again is refused until the files are back"
  );
  assert_eq!(repo.head(), base);

  assert_eq!(put_back(&Processes, git, finished.id()), Ok(()));
  assert!(repo.is_clean());
  merge_with(&Processes, &repo, &finished).expect("merged once the files are back");
}

#[test]
fn merge_put_back_that_cannot_read_the_checkout_names_every_file_the_merge_may_have_written() {
  let (repo, _, finished) = repo_with_two_files();
  let stops = Faked(|args: &[String]| {
    if has(args, &["merge", "--ff-only"]) {
      wrote_part_way(&repo, &["a.txt", "b.txt"]);
      return failed("error: unable to write new.txt: Input/output error");
    }
    has(args, &["ls-files"]).then(|| failed("fatal: index file corrupt").expect("an answer"))
  });
  let refused = merge_with(&stops, &repo, &finished);
  let Err(MergeError::LeftChanged { files, detail }) = &refused else {
    panic!("files were left changed: {refused:?}");
  };
  assert_eq!(files, &["a.txt", "b.txt", "new.txt"]);
  assert!(detail.contains("unknown"), "{detail}");
  let git = (git_program(), repo.root.clone());
  let git = (git.0.as_path(), git.1.as_path());
  assert_eq!(
    left_by_bring_in(&Processes, git, finished.id()),
    Ok(vec!["a.txt".to_owned(), "b.txt".to_owned()]),
    "the record stays"
  );
}

#[test]
fn merge_put_back_never_touches_a_file_the_person_changed_since() {
  let (repo, _, finished) = repo_with_two_files();
  left_changed(&repo, &finished).expect_err("files were left changed");
  fs::write(repo.root.join("a.txt"), "the person's a\n").expect("the person's edit");
  let git = (git_program(), repo.root.clone());
  let git = (git.0.as_path(), git.1.as_path());
  let refused = put_back(&Processes, git, finished.id());
  let Err(MergeError::LeftChanged { files, .. }) = &refused else {
    panic!("the person's file stays: {refused:?}");
  };
  assert_eq!(files, &["a.txt"]);
  assert_eq!(read(&repo, "a.txt"), "the person's a\n", "the person's edit stays");
  assert_eq!(read(&repo, "b.txt"), "base b\n");
  assert!(matches!(
    merge_with(&Processes, &repo, &finished),
    Err(MergeError::NotPutBack { .. })
  ));

  repo.git(&["checkout", "--", "a.txt"]);
  assert_eq!(left_by_bring_in(&Processes, git, finished.id()), Ok(Vec::new()));
  merge_with(&Processes, &repo, &finished).expect("merged once the checkout matches the base");
}

#[test]
fn merge_interrupted_while_merging_leaves_a_temporary_worktree_the_next_start_removes() {
  let (repo, base, _) = repo_with_run_work();
  repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  let finished = run(&base, Some("main"), Ending::Succeeded);
  let ends = Faked(|args: &[String]| -> Option<Result<CommandOutput, CommandError>> {
    assert!(!has(args, &["merge", "--no-ff"]), "the app ended during the merge");
    None
  });
  let ended = catch_unwind(AssertUnwindSafe(|| merge_with(&ends, &repo, &finished)));
  assert!(ended.is_err());
  let common = repo.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
  assert!(fs::metadata(format!("{common}/orchester-merge-{}", finished.id())).is_ok());

  let cleared = clear_interrupted(&Processes, (&git_program(), &repo.root), finished.id());
  assert_eq!(
    cleared,
    Ok(Interrupted {
      left_changed: Vec::new(),
      temporary: None,
    })
  );
  assert_no_temporary(&repo);
  merge_with(&Processes, &repo, &finished).expect("merged after it");
}

#[test]
fn merge_interrupted_while_bringing_in_names_the_files_it_left_changed() {
  let (repo, _, finished) = repo_with_two_files();
  let ends = Faked(|args: &[String]| -> Option<Result<CommandOutput, CommandError>> {
    if has(args, &["merge", "--ff-only"]) {
      wrote_part_way(&repo, &["a.txt"]);
      panic!("the app ended while git brought the merge in");
    }
    None
  });
  let ended = catch_unwind(AssertUnwindSafe(|| merge_with(&ends, &repo, &finished)));
  assert!(ended.is_err());

  let git = (git_program(), repo.root.clone());
  let git = (git.0.as_path(), git.1.as_path());
  let cleared = clear_interrupted(&Processes, git, finished.id()).expect("cleared");
  assert_eq!(cleared.left_changed, ["a.txt"]);
  assert!(matches!(
    merge_with(&Processes, &repo, &finished),
    Err(MergeError::NotPutBack { .. })
  ));
  assert_eq!(put_back(&Processes, git, finished.id()), Ok(()));
  assert_eq!(read(&repo, "a.txt"), "base a\n");
  merge_with(&Processes, &repo, &finished).expect("merged once put back");
}
