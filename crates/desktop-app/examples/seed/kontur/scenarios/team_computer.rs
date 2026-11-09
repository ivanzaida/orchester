//! The agent connections a team scenario's computer has (`team-edit`, `team-archive`, `team-start-*`): what the team's
//! profiles run on and what the hub's create forms offer.

use std::fs;

use orchester_core::connection::{AgentProvider, SignIn};

use crate::kontur::{Kontur, SeedError, io_error as io};

/// The Claude Code connection `Claude · Kontur`, which the team's profiles run on without asking.
pub(super) fn claude_connection(kontur: &mut Kontur) -> Result<(), SeedError> {
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  let program = bin.join("claude");
  fs::create_dir_all(&bin).map_err(io(&bin))?;
  fs::write(&program, "").map_err(io(&program))?;
  let at = kontur.ago(4);
  super::connections::cli(
    kontur,
    ("Claude · Kontur", AgentProvider::ClaudeCodeCli, program),
    ("2.1.89", SignIn::SignedIn, &["claude-sonnet-5-5"]),
    at,
  )
  .map(|_| ())
}

/// The agent connections of the Kontur catalogue, signed in, so the hub's create forms offer a CLI and its models
/// (`Agents · v2` A9): a copy that joins the stand-in team server in a state of the team's resources has no other.
pub(super) fn forms_computer(kontur: &mut Kontur) -> Result<(), SeedError> {
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  fs::create_dir_all(&bin).map_err(io(&bin))?;
  let at = kontur.ago(4);
  for (named, provider, file, models) in [
    (
      "Claude · Kontur",
      AgentProvider::ClaudeCodeCli,
      "claude",
      &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5"][..],
    ),
    (
      "Codex · Kontur",
      AgentProvider::CodexCli,
      "codex",
      &["gpt-5.5", "gpt-5.4-mini"][..],
    ),
  ] {
    let program = bin.join(file);
    fs::write(&program, "").map_err(io(&program))?;
    super::connections::cli(
      kontur,
      (named, provider, program),
      ("2.1.89", SignIn::SignedIn, models),
      at,
    )?;
  }
  Ok(())
}
