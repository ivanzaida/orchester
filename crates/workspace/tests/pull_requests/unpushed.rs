//! Whether a run's pull request would carry commits its remote's base lacks, with real git and local bare
//! repositories as remotes: in sync; ahead by the commits the branch started from (not the ones the base got after
//! it), seen again after a fetch; a branch that merged the base later; a remote that moved on as well; a remote
//! that is not `origin`; a base rewritten or deleted since the run started; `FETCH_HEAD` left alone; and a base the
//! remote doesn't have.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_core::{repository::RemoteName, revision::Revision, run::BranchName};
use orchester_exec::ProcessRunner;
use orchester_workspace::pull_requests::unpushed::{BaseCheck, BaseOnRemote, base_on_remote};

const RUN: &str = "orchester/kon-1-run-1";

fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

/// A bare remote `remote` and a repository on `main` with one commit pushed to it, in a scratch folder of the
/// worktree.
struct Checkout {
  dir: tempfile::TempDir,
  repo: PathBuf,
  remote: RemoteName,
}

impl Checkout {
  fn new() -> Self {
    Self::with_remote("origin")
  }

  fn with_remote(remote: &str) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/pr-unpushed-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("unpushed-")
      .tempdir_in(scratch)
      .expect("temp dir");
    let repo = dir.path().join("repo");
    git(
      dir.path(),
      &["init", "--quiet", "--bare", "--initial-branch=main", "remote.git"],
    );
    git(dir.path(), &["init", "--quiet", "--initial-branch=main", "repo"]);
    for (key, value) in [
      ("user.name", "Test"),
      ("user.email", "test@example.com"),
      ("commit.gpgsign", "false"),
    ] {
      git(&repo, &["config", key, value]);
    }
    let bare = dir.path().join("remote.git");
    git(&repo, &["remote", "add", remote, bare.to_str().expect("a path")]);
    let checkout = Self {
      dir,
      repo,
      remote: RemoteName::new(remote).expect("a remote"),
    };
    checkout.commit("first");
    checkout.push();
    checkout
  }

  fn commit(&self, message: &str) {
    git(&self.repo, &["commit", "--quiet", "--allow-empty", "-m", message]);
  }

  fn push(&self) {
    git(&self.repo, &["push", "--quiet", self.remote.as_str(), "main"]);
  }

  /// Cuts the run's branch from `main` and commits on it, then comes back to `main`; the commit it started from.
  fn cut(&self) -> String {
    let start = git(&self.repo, &["rev-parse", "HEAD"]);
    git(&self.repo, &["switch", "--quiet", "-c", RUN]);
    self.commit("work of the run");
    git(&self.repo, &["switch", "--quiet", "main"]);
    start
  }

  fn check(&self, base: &str, start: &str) -> BaseOnRemote {
    let runner = ProcessRunner::new(Duration::from_secs(60));
    let (base, head) = (
      BranchName::new(base).expect("branch"),
      BranchName::new(RUN).expect("branch"),
    );
    let start = Revision::new(start).expect("revision");
    let check = BaseCheck {
      remote: &self.remote,
      base: &base,
      head: &head,
      start: &start,
    };
    base_on_remote(&runner, (Path::new("git"), &self.repo), check).expect("git tells")
  }
}

#[test]
fn unpushed_base_in_sync_counts_nothing() {
  let checkout = Checkout::new();
  let start = checkout.cut();
  assert_eq!(checkout.check("main", &start), BaseOnRemote::InSync);
}

#[test]
fn unpushed_base_ahead_counts_the_commits_the_branch_carries() {
  let checkout = Checkout::new();
  checkout.commit("local merge 1");
  checkout.commit("local merge 2");
  let start = checkout.cut();
  // main moves on after the branch was cut: those commits are not in the run's pull request.
  checkout.commit("later local merge");
  assert_eq!(checkout.check("main", &start), BaseOnRemote::Ahead { commits: 2 });
  // The person pushes main; a remote-tracking branch left behind is fetched again before the count.
  checkout.push();
  git(&checkout.repo, &["update-ref", "refs/remotes/origin/main", "HEAD~3"]);
  assert_eq!(
    checkout.check("main", &start),
    BaseOnRemote::InSync,
    "the stale remote-tracking branch is fetched again"
  );
}

#[test]
fn unpushed_base_counts_what_a_branch_merged_from_the_base_later() {
  let checkout = Checkout::new();
  checkout.commit("local before the run");
  let start = checkout.cut();
  checkout.commit("local after the run started");
  git(&checkout.repo, &["switch", "--quiet", RUN]);
  git(&checkout.repo, &["merge", "--quiet", "--no-edit", "main"]);
  git(&checkout.repo, &["switch", "--quiet", "main"]);
  checkout.commit("local after the merge");
  assert_eq!(
    checkout.check("main", &start),
    BaseOnRemote::Ahead { commits: 2 },
    "both local commits the branch took in, not the one after its merge"
  );
}

#[test]
fn unpushed_base_counts_its_own_commits_when_the_remote_moved_on_too() {
  let checkout = Checkout::new();
  let other = checkout.dir.path().join("other");
  git(checkout.dir.path(), &["clone", "--quiet", "remote.git", "other"]);
  for (key, value) in [("user.name", "Other"), ("user.email", "other@example.com")] {
    git(&other, &["config", key, value]);
  }
  git(
    &other,
    &["commit", "--quiet", "--allow-empty", "-m", "pushed by someone else"],
  );
  git(&other, &["push", "--quiet", "origin", "main"]);
  checkout.commit("local 1");
  checkout.commit("local 2");
  let start = checkout.cut();
  assert_eq!(checkout.check("main", &start), BaseOnRemote::Ahead { commits: 2 });
}

#[test]
fn unpushed_base_compares_with_the_links_remote_by_its_name() {
  let checkout = Checkout::with_remote("upstream");
  checkout.commit("local");
  let start = checkout.cut();
  assert_eq!(checkout.check("main", &start), BaseOnRemote::Ahead { commits: 1 });
  assert!(
    git(
      &checkout.repo,
      &["rev-parse", "--verify", "--quiet", "refs/remotes/upstream/main"]
    )
    .len()
      == 40,
    "fetched into upstream's remote-tracking branch"
  );
}

#[test]
fn unpushed_base_counts_the_start_after_the_base_was_rewritten_or_deleted() {
  let checkout = Checkout::new();
  checkout.commit("local X");
  let start = checkout.cut();
  // `git reset --hard origin/main` (or a pull that rebased X away): the local base no longer holds X.
  git(&checkout.repo, &["reset", "--quiet", "--hard", "origin/main"]);
  assert_eq!(
    checkout.check("main", &start),
    BaseOnRemote::Ahead { commits: 1 },
    "the branch still carries X"
  );
  git(&checkout.repo, &["switch", "--quiet", "--detach", RUN]);
  git(&checkout.repo, &["branch", "--quiet", "-D", "main"]);
  assert_eq!(
    checkout.check("main", &start),
    BaseOnRemote::Ahead { commits: 1 },
    "without a local base, the start still counts"
  );
}

#[test]
fn unpushed_base_fetch_leaves_fetch_head_alone() {
  let checkout = Checkout::new();
  checkout.commit("local");
  let start = checkout.cut();
  let fetch_head = checkout.repo.join(".git").join("FETCH_HEAD");
  fs::write(&fetch_head, "the person's own fetch\n").expect("FETCH_HEAD");
  assert_eq!(checkout.check("main", &start), BaseOnRemote::Ahead { commits: 1 });
  assert_eq!(
    fs::read_to_string(&fetch_head).expect("FETCH_HEAD"),
    "the person's own fetch\n"
  );
}

#[test]
fn unpushed_base_without_one_on_the_remote_has_no_upstream() {
  let checkout = Checkout::new();
  git(&checkout.repo, &["switch", "--quiet", "-c", "develop"]);
  checkout.commit("develop only");
  let start = git(&checkout.repo, &["rev-parse", "HEAD"]);
  git(&checkout.repo, &["switch", "--quiet", "-c", RUN]);
  checkout.commit("work of the run");
  assert_eq!(checkout.check("develop", &start), BaseOnRemote::NoUpstream);
}
