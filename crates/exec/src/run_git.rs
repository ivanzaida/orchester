//! The runner's side of the Orchestrator chat's git in a run's worktree (`orchester_core::orchestrator::run_git`):
//! runs one request in the worktree at `dir`, the same way the desktop runs git in the person's checkout. Whether
//! the request may run there at all, the runner decides before ([`RunGitRequest::allowed_before_end`]).
//!
//! The answer goes to the desktop in one message of a bounded size, so git's output is never kept past `most` bytes
//! of each stream: a request with a smaller cap is cut as asked, and one that wants more of it than that (git's whole
//! output, e.g. `status` of a worktree with a huge number of changes) is answered as git that could not be run
//! ([`too_large`]), never with an output silently cut short.
//!
//! [`RunGitRequest::allowed_before_end`]: orchester_core::orchestrator::run_git::RunGitRequest::allowed_before_end

use std::{fs, path::Path, time::Duration};

use orchester_core::{
  command::{CommandError, CommandOutput},
  orchestrator::run_git::{GitInvocation, PathFacts, RunGitAnswer, RunGitCall, chat_git_command},
};

use crate::programs::Programs;

/// Why git whose output is larger than `most` bytes, the most of each output an answer holds, is answered without it.
pub fn too_large(most: usize) -> CommandError {
  CommandError::Failed(format!(
    "git's output was larger than the runner can send (more than {most} bytes), so its result is not known"
  ))
}

/// Carries out `call` in the worktree at `dir` with `programs`, keeping at most `most` bytes of each of git's outputs.
/// Blocks on git.
pub fn serve(programs: &Programs, dir: &Path, call: &RunGitCall, most: usize) -> RunGitAnswer {
  match call {
    RunGitCall::Git(git) => run(programs, dir, git, most),
    RunGitCall::Paths { paths } => RunGitAnswer::Paths {
      facts: paths.iter().map(|path| path_facts(&dir.join(path))).collect(),
    },
  }
}

fn run(programs: &Programs, dir: &Path, git: &GitInvocation, most: usize) -> RunGitAnswer {
  let args: Vec<&str> = git.args.iter().map(String::as_str).collect();
  let asked = git
    .cap
    .map_or(usize::MAX, |cap| usize::try_from(cap).unwrap_or(usize::MAX));
  let ran = chat_git(
    programs,
    dir,
    (&args, git.input.as_deref(), git.ssh.as_deref()),
    (Duration::from_millis(git.timeout_ms), Some(asked.min(most))),
  );
  match ran {
    Ok((_, true)) if asked > most => RunGitAnswer::NotRun { error: too_large(most) },
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
