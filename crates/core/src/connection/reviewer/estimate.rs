//! The story-point estimator ([`crate::task::estimate`]) on auto mode's reviewer's connection and model, isolated as
//! the reviewer is: the same Claude Code and Codex arguments with the estimate's instructions and schema, or one
//! request without tools on a native connection. Every failure gives no points; what a request spent is read as a
//! review's is.

use std::path::{Path, PathBuf};

use super::{
  QUIET, ReviewFolder, claude_args, claude_spend, claude_structured,
  codex::{codex_final_text, codex_reviewer_args, codex_spend},
  native::{NativeSpend, native_text, one_request},
  run_cli,
};
use crate::{
  command::{Command, CommandRunner},
  native::model::{ModelError, ModelRequest, ModelResponse},
  resource::ModelId,
  run::review::{ReviewError, ReviewSpend},
  task::estimate::{
    ESTIMATE_INSTRUCTIONS, ESTIMATE_SCHEMA, EstimateAnswer, EstimateInput, estimate_prompt, parse_estimate,
    parse_estimate_text,
  },
};

/// The estimator's answer about one task, with what asking it spent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estimated {
  pub answer: Result<EstimateAnswer, ReviewError>,
  pub spend: ReviewSpend,
}

/// Which CLI estimates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstimatorCli {
  ClaudeCode,
  Codex,
}

/// The estimator on a Claude Code or Codex connection: its program, model and effort, the empty folder it runs in
/// (whose instructions and schema files hold [`ESTIMATE_INSTRUCTIONS`] and [`ESTIMATE_SCHEMA`]) and the variables it
/// starts with.
pub struct CliEstimator {
  pub cli: EstimatorCli,
  pub executable: PathBuf,
  pub model: (ModelId, Option<String>),
  pub folder: ReviewFolder,
  pub environment: Vec<(String, String)>,
}

impl CliEstimator {
  /// The command that estimates `input`.
  pub fn command(&self, input: &EstimateInput) -> Command {
    let (model, effort) = (&self.model.0, self.model.1.as_deref());
    match self.cli {
      EstimatorCli::ClaudeCode => Command::new(&self.executable, claude_estimate_args((model, effort), &self.folder.instructions))
        .with_current_dir(&self.folder.work)
        .with_environment(self.environment.clone())
        .with_env(QUIET.0, QUIET.1)
        .with_stdin(estimate_prompt(input)),
      EstimatorCli::Codex => Command::new(&self.executable, codex_reviewer_args((model, effort), &self.folder))
        .with_current_dir(&self.folder.work)
        .with_environment(self.environment.clone())
        .with_stdin(format!("{ESTIMATE_INSTRUCTIONS}\n\n{}", estimate_prompt(input))),
    }
  }

  /// The estimate of `input`, run through `runner`, which stops it when it takes too long.
  pub fn estimate(&self, input: &EstimateInput, runner: &impl CommandRunner) -> Estimated {
    let read = match self.cli {
      EstimatorCli::ClaudeCode => read_claude_estimate,
      EstimatorCli::Codex => read_codex_estimate,
    };
    match run_cli(runner, &self.command(input)) {
      Ok((stdout, exited)) => {
        let mut estimated = read(&stdout);
        if let Err(error) = exited {
          estimated.answer = Err(error);
        }
        estimated
      }
      Err((error, spend)) => Estimated {
        answer: Err(error),
        spend,
      },
    }
  }
}

/// Claude Code's arguments for an estimate: the reviewer's, with the estimate's schema.
pub fn claude_estimate_args(model: (&ModelId, Option<&str>), instructions: &Path) -> Vec<String> {
  claude_args(model, instructions, ESTIMATE_SCHEMA)
}

/// Claude Code's answer: the structured output of its last `result` line, and what it spent.
pub fn read_claude_estimate(stdout: &str) -> Estimated {
  Estimated {
    answer: claude_structured(stdout).and_then(parse_estimate),
    spend: claude_spend(stdout),
  }
}

/// Codex's answer: its last message, the JSON object alone, and what it spent.
pub fn read_codex_estimate(stdout: &str) -> Estimated {
  Estimated {
    answer: codex_final_text(stdout).and_then(|text| parse_estimate_text(&text)),
    spend: codex_spend(stdout),
  }
}

/// The one request that estimates `input` on a native connection with `model` and `effort`.
pub fn native_estimate_request(input: &EstimateInput, model: &ModelId, effort: Option<String>) -> ModelRequest {
  one_request((ESTIMATE_INSTRUCTIONS, estimate_prompt(input)), model, effort)
}

impl NativeSpend {
  /// The model's answer as an estimate, at what it spent.
  pub fn estimated(self, response: &ModelResponse) -> Estimated {
    Estimated {
      answer: native_text(response).and_then(|text| parse_estimate_text(&text)),
      spend: self.answered_spend(response),
    }
  }

  /// An estimate the client gave up on with `error`, at what it spent.
  pub fn not_estimated(self, error: &ModelError) -> Estimated {
    Estimated {
      answer: Err(ReviewError::CouldNotRun(error.to_string())),
      spend: self.unanswered_spend(error),
    }
  }
}
