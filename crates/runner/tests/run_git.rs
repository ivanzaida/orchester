//! The Orchestrator chat's git in a run's worktree, which the runner runs for the desktop
//! (`orchester_ipc::Command::RunGit`, `docs/architecture/remote-runners.md` rule 5): reads while the run has not ended,
//! nothing that writes until it ended, the run's own worktree found here, and output cut at the cap asked. Against
//! `orchester-fake-codex`; needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test run_git`.

mod support;

use orchester_core::{
  id::RunId,
  orchestrator::run_git::{GitAccess, GitInvocation, PathFacts, RunGitAnswer, RunGitCall, RunGitRequest},
};
use orchester_ipc::{Rejection, Reply};

use self::support::{Setup, killed_mid_run, project, scheduler};

fn git(run: RunId, access: GitAccess, args: &[&str], cap: Option<u64>) -> RunGitRequest {
  RunGitRequest {
    run,
    access,
    call: RunGitCall::Git(GitInvocation {
      args: args.iter().map(|arg| (*arg).to_owned()).collect(),
      input: None,
      ssh: None,
      timeout_ms: 30_000,
      cap,
    }),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn run_git_reads_a_working_runs_worktree_and_writes_only_once_it_ended() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let run = killed_mid_run(root, project.task).await;
  let (scheduler, _events) = scheduler(root);

  let Reply::RunGit(RunGitAnswer::Ran { output, cut }) = scheduler
    .run_git(git(run, GitAccess::Read, &["rev-parse", "--show-toplevel"], None))
    .await
  else {
    panic!("a read runs in a working run's worktree");
  };
  assert!(output.success() && !cut, "{output:?}");
  let worktree = root.join(".orchester/worktrees").join(run.to_string());
  assert_eq!(
    std::fs::canonicalize(output.stdout.trim()).expect("git's folder"),
    std::fs::canonicalize(&worktree).expect("the run's worktree"),
  );

  let Reply::RunGit(RunGitAnswer::Ran { output, cut }) = scheduler
    .run_git(git(run, GitAccess::Read, &["log", "--format=%H%n%s"], Some(10)))
    .await
  else {
    panic!("a capped read runs");
  };
  assert!(cut && output.stdout.len() <= 10, "cut at the cap: {output:?}");

  let paths = RunGitRequest {
    run,
    access: GitAccess::Read,
    call: RunGitCall::Paths {
      paths: vec!["README.md".to_owned(), "missing.txt".to_owned()],
    },
  };
  assert_eq!(
    scheduler.run_git(paths).await,
    Reply::RunGit(RunGitAnswer::Paths {
      facts: vec![
        PathFacts {
          exists: true,
          file_len: Some(3)
        },
        PathFacts {
          exists: false,
          file_len: None
        },
      ],
    })
  );

  for request in [
    git(run, GitAccess::Write, &["status"], None),
    git(run, GitAccess::Read, &["commit", "--allow-empty", "-m", "Mine"], None),
    git(run, GitAccess::Read, &["config", "user.name", "Someone"], None),
  ] {
    assert_eq!(
      scheduler.run_git(request.clone()).await,
      Reply::Rejected(Rejection::RunNotEnded),
      "{request:?}"
    );
  }

  scheduler.cancel_run(run).await.expect("the run is cancelled");
  let Reply::RunGit(RunGitAnswer::Ran { output, .. }) = scheduler
    .run_git(git(
      run,
      GitAccess::Write,
      &["commit", "--allow-empty", "-m", "Mine"],
      None,
    ))
    .await
  else {
    panic!("a write runs once the run ended");
  };
  assert!(output.success(), "{output:?}");

  assert_eq!(
    scheduler
      .run_git(git(RunId::generate(), GitAccess::Read, &["status"], None))
      .await,
    Reply::Rejected(Rejection::RunNotFound)
  );
}
