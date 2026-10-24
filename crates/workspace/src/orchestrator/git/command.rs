//! How the chat's git tools run git: in one folder, with git's messages in English and without the caller's variables
//! that would point git at another repository, index or settings (`orchester_core::diff::git::local_git_command`);
//! never waiting for a person — no terminal prompt, no credential window, no editor — and stopped after its runner's
//! time. What git said of a failure becomes a refusal with a stable code, its words with any credentials in a URL
//! removed.

use std::{path::Path, time::Duration};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  diff::git::local_git_command,
  repository::redact_userinfo,
};
use orchester_exec::programs::Programs;

use crate::{app_mcp::ToolError, messages::Message};

/// The `git` program, looked up on the `PATH` like the runner does.
const GIT: &str = "git";
/// How long a read of git's whose output is capped may take.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// The most of git's output a tool answers with; the rest is cut and the answer says so.
pub(super) const MAX_OUTPUT: usize = 60_000;
/// The most of git's words a card or a refusal keeps.
const MAX_SAID: usize = 2_000;

/// git in one folder, run by `runner`; network git with `ssh` as its ssh command.
pub(super) struct Git<'a> {
  pub(super) runner: &'a dyn CommandRunner,
  pub(super) dir: &'a Path,
  pub(super) ssh: Option<String>,
}

impl<'a> Git<'a> {
  pub(super) const fn new(runner: &'a dyn CommandRunner, dir: &'a Path) -> Self {
    Self { runner, dir, ssh: None }
  }

  /// `git <args>`, with `input` on its standard input.
  pub(super) fn command(&self, args: &[&str], input: Option<&str>) -> Command {
    let settings = ["-c", "color.ui=never", "-c", "core.quotepath=false"];
    let command = local_git_command(Path::new(GIT), self.dir, settings.iter().chain(args).copied())
      // Nobody answers a prompt: git fails at once where it would ask for a user name, a password or an editor's text.
      .with_env("GIT_TERMINAL_PROMPT", "0")
      .with_env("GCM_INTERACTIVE", "never")
      .with_env("GIT_EDITOR", ":")
      .with_env("GIT_SEQUENCE_EDITOR", ":")
      .with_env("GIT_MERGE_AUTOEDIT", "no")
      // A path the chat names is a path, never a pathspec with magic (`:(top)`, globs).
      .with_env("GIT_LITERAL_PATHSPECS", "1");
    let command = match &self.ssh {
      Some(ssh) => command.with_env("GIT_SSH_COMMAND", ssh),
      None => command,
    };
    match input {
      Some(text) => command.with_stdin(text.to_owned()),
      None => command,
    }
  }

  /// Runs `git <args>` with `programs`, reading at most [`MAX_OUTPUT`] bytes as git writes; its output and whether
  /// some was left out, or the refusal of a failure.
  pub(super) fn capped(&self, programs: &Programs, args: &[&str]) -> Result<(String, bool), ToolError> {
    let (output, cut) = programs
      .capped(&self.command(args, None), READ_TIMEOUT, MAX_OUTPUT)
      .map_err(|error| not_run(&error, args))?;
    if output.success() {
      Ok((output.stdout, cut))
    } else {
      Err(refusal(&output, args))
    }
  }

  /// Runs `git <args>`; git that cannot be run or did not finish is refused.
  pub(super) fn run(&self, args: &[&str], input: Option<&str>) -> Result<CommandOutput, ToolError> {
    self
      .runner
      .run(&self.command(args, input))
      .map_err(|error| not_run(&error, args))
  }

  /// Runs `git <args>` and answers its output; a failure is refused as [`refusal`] reads it.
  pub(super) fn ok(&self, args: &[&str]) -> Result<String, ToolError> {
    self.ok_with(args, None)
  }

  pub(super) fn ok_with(&self, args: &[&str], input: Option<&str>) -> Result<String, ToolError> {
    let output = self.run(args, input)?;
    if output.success() {
      Ok(output.stdout)
    } else {
      Err(refusal(&output, args))
    }
  }

  /// Whether `git <args>` succeeds, e.g. a check of a ref; a git that cannot be run is refused.
  pub(super) fn succeeds(&self, args: &[&str]) -> Result<bool, ToolError> {
    Ok(self.run(args, None)?.success())
  }
}

/// The git command `args` as people read them, without the settings every call carries.
pub(super) fn shown(args: &[&str]) -> String {
  let mut words = vec![GIT.to_owned()];
  words.extend(args.iter().map(|arg| {
    if arg.is_empty() || arg.contains(char::is_whitespace) {
      format!("\"{arg}\"")
    } else {
      (*arg).to_owned()
    }
  }));
  redact_userinfo(&words.join(" "))
}

/// What git wrote of a failure: its error output, or its output when it wrote its reason there; credentials in URLs
/// removed and cut to what a card keeps.
pub(super) fn said(output: &CommandOutput) -> String {
  let error = output.stderr.trim();
  let text = if error.is_empty() { output.stdout.trim() } else { error };
  cut(&redact_userinfo(text), MAX_SAID).0
}

/// What git wrote of a write that worked, for its card: both of its outputs, credentials removed, cut.
pub(super) fn said_of_success(output: &CommandOutput) -> String {
  let text = format!("{}\n{}", output.stdout.trim(), output.stderr.trim());
  cut(&redact_userinfo(text.trim()), MAX_SAID).0
}

/// `text` cut to at most `max` bytes at a character's start, and whether it was cut.
fn cut(text: &str, max: usize) -> (String, bool) {
  if text.len() <= max {
    return (text.to_owned(), false);
  }
  let end = (0..=max).rev().find(|end| text.is_char_boundary(*end)).unwrap_or(0);
  (text[..end].to_owned(), true)
}

/// Why git that could not be run, or did not finish, did not do `args`.
fn not_run(error: &CommandError, args: &[&str]) -> ToolError {
  match error {
    CommandError::NotFound => refused("git_not_found", Message::new("orchestrator.git.gitMissing")),
    CommandError::TimedOut => refused(
      "timed_out",
      Message::new("orchestrator.git.timedOut").with("command", shown(args)),
    ),
    other => refused(
      "git_failed",
      Message::new("orchestrator.git.gitFailed")
        .with("command", shown(args))
        .with("said", redact_userinfo(&other.to_string())),
    ),
  }
}

/// The refusal of `git <args>`, which failed as `output` says: `not_a_repository`, `auth_failed` with a hint,
/// `would_lose_work` where git kept local changes it would have overwritten, `author_unknown`, else `git_failed` in
/// git's words.
pub(super) fn refusal(output: &CommandOutput, args: &[&str]) -> ToolError {
  let said = said(output);
  let lowered = said.to_lowercase();
  let command = shown(args);
  if lowered.contains("not a git repository") {
    return refused("not_a_repository", Message::new("orchestrator.git.notRepository"));
  }
  if AUTH.iter().any(|sign| lowered.contains(sign)) {
    return refused(
      "auth_failed",
      Message::new("orchestrator.git.authFailed")
        .with("command", command)
        .with("said", said),
    );
  }
  if LOSES.iter().any(|sign| lowered.contains(sign)) {
    return refused(
      "would_lose_work",
      Message::new("orchestrator.git.wouldLoseWork")
        .with("command", command)
        .with("said", said),
    );
  }
  if IDENTITY.iter().any(|sign| lowered.contains(sign)) {
    return refused(
      "author_unknown",
      Message::new("orchestrator.git.authorUnknown").with("said", said),
    );
  }
  refused(
    "git_failed",
    Message::new("orchestrator.git.gitFailed")
      .with("command", command)
      .with("said", said),
  )
}

pub(super) fn refused(code: &str, message: Message) -> ToolError {
  ToolError::refused(code, vec![message])
}

/// What git says, lowered, when a remote wants credentials it was not given or refused the ones it was.
const AUTH: [&str; 9] = [
  "authentication failed",
  "could not read username",
  "could not read password",
  "terminal prompts disabled",
  "permission denied (publickey",
  "host key verification failed",
  "access denied",
  "the requested url returned error: 401",
  "the requested url returned error: 403",
];

/// What git says, lowered, when it keeps local changes or files a command would overwrite.
const LOSES: [&str; 3] = [
  "would be overwritten",
  "would be removed by",
  "please commit your changes or stash them",
];

/// What git says, lowered, when it does not know who a commit's author is.
const IDENTITY: [&str; 3] = [
  "please tell me who you are",
  "author identity unknown",
  "empty ident name",
];
