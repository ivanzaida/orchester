//! Whether a run's pull request would carry commits of the local base its remote lacks, with real git and a local
//! bare repository as `origin`: in sync, ahead by the commits the branch shares with the local base (not the ones the
//! base got after it), seen again after a fetch, and a base the remote doesn't have.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_core::{repository::RemoteName, run::BranchName};
use orchester_exec::ProcessRunner;
use orchester_workspace::pull_requests::unpushed::{BaseCheck, BaseOnRemote, base_on_remote};

fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

/// A bare `origin` and a clone of it on `main` with one pushed commit, in a scratch folder of the worktree.
struct Checkout {
  _dir: tempfile::TempDir,
  repo: PathBuf,
}

impl Checkout {
  fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/pr-unpushed-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("unpushed-")
      .tempdir_in(scratch)
      .expect("temp dir");
    let (origin, repo) = (dir.path().join("origin.git"), dir.path().join("repo"));
    git(
      dir.path(),
      &["init", "--quiet", "--bare", "--initial-branch=main", "origin.git"],
    );
    git(dir.path(), &["init", "--quiet", "--initial-branch=main", "repo"]);
    for (key, value) in [
      ("user.name", "Test"),
      ("user.email", "test@example.com"),
      ("commit.gpgsign", "false"),
    ] {
      git(&repo, &["config", key, value]);
    }
    git(&repo, &["remote", "add", "origin", origin.to_str().expect("a path")]);
    let clone = Self { _dir: dir, repo };
    clone.commit("first");
    git(&clone.repo, &["push", "--quiet", "origin", "main"]);
    clone
  }

  fn commit(&self, message: &str) {
    git(&self.repo, &["commit", "--quiet", "--allow-empty", "-m", message]);
  }

  /// Cuts `name` from the checked-out branch and commits on it, then comes back to `main`.
  fn branch(&self, name: &str) {
    git(&self.repo, &["switch", "--quiet", "-c", name]);
    self.commit(&format!("work on {name}"));
    git(&self.repo, &["switch", "--quiet", "main"]);
  }

  fn check(&self, base: &str, head: &str) -> BaseOnRemote {
    let runner = ProcessRunner::new(Duration::from_secs(60));
    let (base, head) = (
      BranchName::new(base).expect("branch"),
      BranchName::new(head).expect("branch"),
    );
    let check = BaseCheck {
      remote: &RemoteName::origin(),
      base: &base,
      head: &head,
    };
    base_on_remote(&runner, (Path::new("git"), &self.repo), check).expect("git tells")
  }
}

#[test]
fn unpushed_base_in_sync_counts_nothing() {
  let clone = Checkout::new();
  clone.branch("orchester/kon-1-run-1");
  assert_eq!(clone.check("main", "orchester/kon-1-run-1"), BaseOnRemote::InSync);
}

#[test]
fn unpushed_base_ahead_counts_the_commits_the_branch_carries() {
  let clone = Checkout::new();
  clone.commit("local merge 1");
  clone.commit("local merge 2");
  clone.branch("orchester/kon-1-run-1");
  // main moves on after the branch was cut: those commits are not in the run's pull request.
  clone.commit("later local merge");
  assert_eq!(
    clone.check("main", "orchester/kon-1-run-1"),
    BaseOnRemote::Ahead { commits: 2 }
  );
  // The person pushes main; a remote-tracking branch left behind is fetched again before the count.
  git(&clone.repo, &["push", "--quiet", "origin", "main"]);
  git(&clone.repo, &["update-ref", "refs/remotes/origin/main", "HEAD~3"]);
  assert_eq!(
    clone.check("main", "orchester/kon-1-run-1"),
    BaseOnRemote::InSync,
    "the stale remote-tracking branch is fetched again"
  );
}

#[test]
fn unpushed_base_without_one_on_the_remote_has_no_upstream() {
  let clone = Checkout::new();
  git(&clone.repo, &["switch", "--quiet", "-c", "develop"]);
  clone.commit("develop only");
  clone.branch("orchester/kon-2-run-1");
  assert_eq!(
    clone.check("develop", "orchester/kon-2-run-1"),
    BaseOnRemote::NoUpstream
  );
}
