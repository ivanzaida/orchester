//! How the chat's git tools run git: in one folder, with git's messages in English and without the caller's variables
//! that would point git at another repository, index or settings (`orchester_core::orchestrator::run_git::
//! chat_git_command`); never waiting for a person — no terminal prompt, no credential window, no editor — and stopped
//! after its time. Where git runs, the desktop or the runner, its [`Host`] says. What git said of a failure becomes a
//! refusal with a stable code, its words with any credentials in a URL removed.

use std::time::Duration;

use orchester_core::{
  command::{CommandError, CommandOutput},
  orchestrator::run_git::PathFacts,
  repository::redact_userinfo,
};

use super::host::Host;
use crate::{app_mcp::ToolError, messages::Message};

/// The `git` program, looked up on the `PATH` like the runner does.
const GIT: &str = "git";
/// How long a read of git's whose output is capped may take, and git that only reads or checks.
pub(super) const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// The most of git's output a tool answers with; the rest is cut and the answer says so.
pub(super) const MAX_OUTPUT: usize = 60_000;
/// The most of git's words a card or a refusal keeps.
const MAX_SAID: usize = 2_000;

/// git in one place ([`Host`]), stopped after `timeout`; network git with `ssh` as its ssh command.
pub(super) struct Git<'a> {
  pub(super) at: Host<'a>,
  timeout: Duration,
  pub(super) ssh: Option<String>,
}

impl<'a> Git<'a> {
  pub(super) const fn new(at: Host<'a>, timeout: Duration) -> Self {
    Self { at, timeout, ssh: None }
  }

  /// Runs `git <args>`, reading at most [`MAX_OUTPUT`] bytes as git writes; its output and whether some was left out,
  /// or the refusal of a failure.
  pub(super) fn capped(&self, args: &[&str]) -> Result<(String, bool), ToolError> {
    let (output, cut) = self
      .at
      .git((args, None, self.ssh.as_deref()), READ_TIMEOUT, Some(MAX_OUTPUT))?;
    if output.success() {
      Ok((output.stdout, cut))
    } else {
      Err(refusal(&output, args))
    }
  }

  /// Runs `git <args>`; git that cannot be run or did not finish is refused.
  pub(super) fn run(&self, args: &[&str], input: Option<&str>) -> Result<CommandOutput, ToolError> {
    let (output, _) = self.at.git((args, input, self.ssh.as_deref()), self.timeout, None)?;
    Ok(output)
  }

  /// What is at each of `paths`, as git named them: relative to the folder, or absolute.
  pub(super) fn facts(&self, paths: &[String]) -> Result<Vec<PathFacts>, ToolError> {
    self.at.facts(paths)
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
pub(super) fn not_run(error: &CommandError, args: &[&str]) -> ToolError {
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
