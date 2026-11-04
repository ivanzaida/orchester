//! The runner's side of the Orchestrator chat's git in a run's worktree (`orchester_core::orchestrator::run_git`):
//! runs one request in the worktree at `dir`, the same way the desktop runs git in the person's checkout. Whether
//! the request may run there at all, the runner decides before ([`RunGitRequest::allowed_before_end`]).
//!
//! [`RunGitRequest::allowed_before_end`]: orchester_core::orchestrator::run_git::RunGitRequest::allowed_before_end

use std::{fs, path::Path, time::Duration};

use orchester_core::{
  command::{CommandError, CommandOutput},
  orchestrator::run_git::{GitInvocation, PathFacts, RunGitAnswer, RunGitCall, chat_git_command},
};

use crate::programs::Programs;

/// Carries out `call` in the worktree at `dir` with `programs`. Blocks on git.
pub fn serve(programs: &Programs, dir: &Path, call: &RunGitCall) -> RunGitAnswer {
  match call {
    RunGitCall::Git(git) => run(programs, dir, git),
    RunGitCall::Paths { paths } => RunGitAnswer::Paths {
      facts: paths.iter().map(|path| path_facts(&dir.join(path))).collect(),
    },
  }
}

fn run(programs: &Programs, dir: &Path, git: &GitInvocation) -> RunGitAnswer {
  let args: Vec<&str> = git.args.iter().map(String::as_str).collect();
  let cap = git.cap.map(|cap| usize::try_from(cap).unwrap_or(usize::MAX));
  let ran = chat_git(
    programs,
    dir,
    (&args, git.input.as_deref(), git.ssh.as_deref()),
    (Duration::from_millis(git.timeout_ms), cap),
  );
  match ran {
    Ok((output, cut)) => RunGitAnswer::Ran { output, cut },
    Err(error) => RunGitAnswer::NotRun { error },
  }
}

/// Runs `git <args>` in `dir` as the chat's git tools run it ([`chat_git_command`]), with `input` and `ssh`, stopped
/// after `timeout`; with `cap`, at most that many bytes of each output kept, read as git writes. Its output and
/// whether some was left out. Blocks on git.
pub fn chat_git(
  programs: &Programs,
  dir: &Path,
  (args, input, ssh): (&[&str], Option<&str>, Option<&str>),
  (timeout, cap): (Duration, Option<usize>),
) -> Result<(CommandOutput, bool), CommandError> {
  let command = chat_git_command(dir, args, input, ssh);
  match cap {
    Some(cap) => programs.capped(&command, timeout, cap),
    None => programs.timed(timeout).run(&command).map(|output| (output, false)),
  }
}

/// What is at `path`: anything at all, and a file's size.
pub fn path_facts(path: &Path) -> PathFacts {
  match fs::metadata(path) {
    Ok(metadata) => PathFacts {
      exists: true,
      file_len: metadata.is_file().then_some(metadata.len()),
    },
    Err(_) => PathFacts {
      exists: false,
      file_len: None,
    },
  }
}
