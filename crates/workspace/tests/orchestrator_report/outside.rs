//! Work git proves merged into its base branch without Orchester (ORC-111): the report reads it as `get_run` does,
//! merged by outside, so it is not ready; with no time of its own, it is not the day's merge either. Work merged in
//! Orchester and work merged nowhere read as before. Against a real repository.

use std::{fs, path::Path, process};

use orchester_core::run::Run;

use super::{at, merge, report, support::Fixture};

/// `git <args>` in `repo`, which must succeed; its trimmed output.
fn git(repo: &Path, args: &[&str]) -> String {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The fixture's folder as a repository on `main` with one commit, which it returns.
fn repository(fixture: &Fixture) -> String {
  let repo = fixture.repo.path();
  fs::remove_dir_all(repo.join(".git")).expect("the fixture's stand-in .git");
  git(repo, &["init", "--quiet", "--initial-branch=main"]);
  git(repo, &["config", "user.name", "Orchester Test"]);
  git(repo, &["config", "user.email", "test@orchester.invalid"]);
  git(repo, &["config", "commit.gpgsign", "false"]);
  fs::write(repo.join(".git").join("info").join("exclude"), "/.orchester/\n").expect("exclude .orchester");
  fs::write(repo.join("README.md"), "hello\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", "initial"]);
  git(repo, &["rev-parse", "HEAD"])
}

/// A succeeded run of a new task `title` from `base` on `main`, whose branch holds one commit of its own, `file`.
fn succeeded_with_work(fixture: &mut Fixture, base: &str, (title, file): (&str, &str)) -> Run {
  let task = fixture.task(title, at(8));
  let mut run = fixture.started_run_from(task, (base, "main"), at(9));
  fixture.succeed(&mut run, at(11));
  let repo = fixture.repo.path();
  git(repo, &["checkout", "--quiet", "-b", run.branch().as_str()]);
  fs::write(repo.join(file), "work\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", file]);
  git(repo, &["checkout", "--quiet", "main"]);
  run
}

#[test]
fn report_leaves_work_merged_without_orchester_out_of_ready_and_merged() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let by_hand = succeeded_with_work(&mut fixture, &base, ("Write docs", "docs.md"));
  succeeded_with_work(&mut fixture, &base, ("Add sign-out", "sign_out.rs"));
  let in_orchester = succeeded_with_work(&mut fixture, &base, ("Fix zoom", "zoom.rs"));
  merge(&mut fixture, &in_orchester, at(12));
  let read = report(&fixture, at(18));
  assert_eq!(
    read.ready.iter().map(|run| run.task.as_str()).collect::<Vec<_>>(),
    ["ORC-1", "ORC-2"],
    "before the merge by hand, both runs Orchester did not merge wait"
  );

  git(
    fixture.repo.path(),
    &[
      "merge",
      "--quiet",
      "--no-ff",
      "-m",
      "merged by hand",
      by_hand.branch().as_str(),
    ],
  );
  let read = report(&fixture, at(18));
  assert_eq!(
    read.ready.iter().map(|run| run.task.as_str()).collect::<Vec<_>>(),
    ["ORC-2"],
    "work main holds is merged, as get_run tells it; work merged nowhere still waits"
  );
  assert_eq!(
    read.merged.iter().map(|task| task.task.as_str()).collect::<Vec<_>>(),
    ["ORC-3"],
    "Orchester's merge is the day's as before; a merge by hand has no time and is not"
  );
}
