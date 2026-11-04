//! The side agent (`crate::run::side_question`) on the Orchestrator's connection and model, isolated as the reviewer
//! is: Claude Code in print mode with no tools at all (`--tools ""`), no MCP server, no settings, skills, slash
//! commands, session or auto memory, the instructions as its system prompt and the answer as structured output; or
//! `codex exec` in its read-only sandbox without network, with its shell tool and web search off. Either runs in an
//! empty folder the runner owns, so it has nothing to read but its message and nothing to run or write with. What it
//! spent is read as a review's is.

use std::path::{Path, PathBuf};

use super::{
  QUIET, ReviewFolder, claude_args, claude_spend, claude_structured,
  codex::{codex_final_text, codex_reviewer_args, codex_spend},
  run_cli,
};
use crate::{
  command::{Command, CommandRunner},
  connection::{AgentProvider, invocation::without_auto_memory},
  resource::ModelId,
  run::{
    review::{DetailRedaction, ReviewError, ReviewSpend},
    side_question::{
      SIDE_INSTRUCTIONS, SIDE_SCHEMA, SideInput, SideReply, parse_side_reply, parse_side_reply_text, side_prompt,
    },
  },
};

/// The side agent's answer, with what asking it spent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideAsked {
  pub reply: Result<SideReply, ReviewError>,
  pub spend: ReviewSpend,
}

/// The side agent on a Claude Code or Codex connection (`provider`): its program, model and effort, the empty folder
/// it runs in (whose instructions and schema files hold [`SIDE_INSTRUCTIONS`] and [`SIDE_SCHEMA`]) and the variables
/// it starts with.
pub struct CliSideAgent {
  pub provider: AgentProvider,
  pub executable: PathBuf,
  pub model: (ModelId, Option<String>),
  pub folder: ReviewFolder,
  pub environment: Vec<(String, String)>,
}

impl CliSideAgent {
  /// The command that answers `input`: Codex for a Codex connection, Claude Code for any other.
  pub fn command(&self, input: &SideInput) -> Command {
    let (model, effort) = (&self.model.0, self.model.1.as_deref());
    match self.provider {
      AgentProvider::CodexCli => Command::new(&self.executable, codex_reviewer_args((model, effort), &self.folder))
        .with_current_dir(&self.folder.work)
        .with_environment(self.environment.clone())
        .with_stdin(format!("{SIDE_INSTRUCTIONS}\n\n{}", side_prompt(input))),
      _ => without_auto_memory(
        Command::new(
          &self.executable,
          claude_side_args((model, effort), &self.folder.instructions),
        )
        .with_current_dir(&self.folder.work)
        .with_environment(self.environment.clone())
        .with_env(QUIET.0, QUIET.1)
        .with_stdin(side_prompt(input)),
      ),
    }
  }

  /// The answer `command` (this agent's [`Self::command`], perhaps on the connection's API provider) gives, run
  /// through `runner`, which stops it when it takes too long. Why it gave none holds none of the command's secrets and
  /// nothing of its prompt.
  pub fn ask(&self, command: &Command, runner: &impl CommandRunner) -> SideAsked {
    let read = match self.provider {
      AgentProvider::CodexCli => read_codex_side,
      _ => read_claude_side,
    };
    let mut asked = match run_cli(runner, command) {
      Ok((stdout, exited)) => {
        let mut asked = read(&stdout);
        if let Err(error) = exited {
          asked.reply = Err(error);
        }
        asked
      }
      Err((error, spend)) => SideAsked {
        reply: Err(error),
        spend,
      },
    };
    asked.reply = asked
      .reply
      .map_err(|error| error.redacted(&DetailRedaction::of(command)));
    asked
  }
}

/// Claude Code's arguments for the side agent: the reviewer's, with no tools at all, and the answer's schema.
pub fn claude_side_args(model: (&ModelId, Option<&str>), instructions: &Path) -> Vec<String> {
  claude_args(model, instructions, SIDE_SCHEMA)
}

/// Claude Code's answer: the structured output of its last `result` line, and what it spent.
fn read_claude_side(stdout: &str) -> SideAsked {
  SideAsked {
    reply: claude_structured(stdout).and_then(parse_side_reply),
    spend: claude_spend(stdout),
  }
}

/// Codex's answer: its last message, the JSON object alone, and what it spent.
fn read_codex_side(stdout: &str) -> SideAsked {
  SideAsked {
    reply: codex_final_text(stdout).and_then(|text| parse_side_reply_text(&text)),
    spend: codex_spend(stdout),
  }
}
