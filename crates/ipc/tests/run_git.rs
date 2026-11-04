//! The Orchestrator chat's git in a run's worktree (`Command::RunGit`, protocol 45) crosses the connection as it was
//! sent: the request with each kind of call, and each kind of answer.

use orchester_core::{
  command::{CommandError, CommandOutput},
  id::RunId,
  orchestrator::run_git::{GitAccess, GitInvocation, PathFacts, RunGitAnswer, RunGitCall, RunGitRequest},
};
use orchester_ipc::{Command, Event, PROTOCOL_VERSION, Rejection, Reply, Request, RequestId};

fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
  serde_json::from_str(&serde_json::to_string(value).expect("serializes")).expect("reads back")
}

#[test]
fn run_git_requests_and_answers_round_trip_under_protocol_45() {
  assert_eq!(
    PROTOCOL_VERSION, 47,
    "came with protocol 45; resolving by hand raised it to 46, a run's files to 47"
  );
  let run = RunId::generate();
  let calls = [
    RunGitCall::Git(GitInvocation {
      args: vec!["commit".to_owned(), "-F".to_owned(), "-".to_owned()],
      input: Some("A message\n".to_owned()),
      ssh: Some("ssh -o BatchMode=yes".to_owned()),
      timeout_ms: 300_000,
      cap: Some(60_000),
    }),
    RunGitCall::Paths {
      paths: vec![
        "MERGE_HEAD".to_owned(),
        "C:/repo/.git/worktrees/x/REVERT_HEAD".to_owned(),
      ],
    },
  ];
  for (call, access) in calls.into_iter().zip([GitAccess::Write, GitAccess::Read]) {
    let request = Request {
      id: RequestId::new(7),
      command: Command::RunGit(RunGitRequest { run, access, call }),
    };
    assert_eq!(round_trip(&request), request);
  }
  let replies = [
    Reply::RunGit(RunGitAnswer::Ran {
      output: CommandOutput {
        code: Some(1),
        stdout: "out".to_owned(),
        stderr: "error: nope".to_owned(),
      },
      cut: true,
    }),
    Reply::RunGit(RunGitAnswer::NotRun {
      error: CommandError::TimedOut,
    }),
    Reply::RunGit(RunGitAnswer::NotRun {
      error: CommandError::UnsafeScriptArgument { position: 2 },
    }),
    Reply::RunGit(RunGitAnswer::Paths {
      facts: vec![
        PathFacts {
          exists: true,
          file_len: None,
        },
        PathFacts {
          exists: true,
          file_len: Some(12),
        },
      ],
    }),
    Reply::Rejected(Rejection::RunNotEnded),
  ];
  for reply in replies {
    let event = Event::Reply {
      id: RequestId::new(7),
      reply,
    };
    assert_eq!(round_trip(&event), event);
  }
}

#[test]
fn run_git_before_the_end_takes_only_reads() {
  let git = |access, args: &[&str]| RunGitRequest {
    run: RunId::generate(),
    access,
    call: RunGitCall::Git(GitInvocation {
      args: args.iter().map(|arg| (*arg).to_owned()).collect(),
      input: None,
      ssh: None,
      timeout_ms: 1,
      cap: None,
    }),
  };
  for args in [
    &["status", "--porcelain=v2"][..],
    &["log", "-n", "5"],
    &["diff", "--output-indicator-new=>", "HEAD"],
    &["log", "--", "--output=a-file-of-that-name"],
    &["remote", "-v"],
    &["remote"],
    &["config", "--get", "core.sshCommand"],
    &["symbolic-ref", "--quiet", "--short", "HEAD"],
    &["hash-object", "--stdin"],
  ] {
    assert!(git(GitAccess::Read, args).allowed_before_end(), "{args:?}");
    assert!(!git(GitAccess::Write, args).allowed_before_end(), "{args:?}");
  }
  for args in [
    &["commit", "-m", "x"][..],
    &["diff", "--output=stolen.txt"],
    &["show", "--output", "stolen.txt", "HEAD"],
    &["log", "-p", "--output=stolen.txt", "--", "README.md"],
    &["remote", "add", "far", "x"],
    &["config", "user.name", "x"],
    &["symbolic-ref", "HEAD", "refs/heads/x"],
    &["hash-object", "-w", "--stdin"],
    &[],
  ] {
    assert!(!git(GitAccess::Read, args).allowed_before_end(), "{args:?}");
  }
}

/// The most output the runner answers with (`MOST_OUTPUT` in `orchester-runner`'s `execution::executor::run_git`):
/// both of git's outputs at that size, all of them control characters JSON writes with six bytes, still fit a frame.
#[test]
fn run_git_answer_at_the_runners_most_output_fits_a_frame() {
  let most = orchester_ipc::frame::MAX_FRAME_LEN as usize / 16;
  let event = Event::Reply {
    id: RequestId::new(u64::MAX),
    reply: Reply::RunGit(RunGitAnswer::Ran {
      output: CommandOutput {
        code: Some(i32::MIN),
        stdout: "\u{0}".repeat(most),
        stderr: "\u{1}".repeat(most),
      },
      cut: true,
    }),
  };
  let len = serde_json::to_vec(&event).expect("serializes").len();
  assert!(len <= orchester_ipc::frame::MAX_FRAME_LEN as usize, "{len} bytes");
}
