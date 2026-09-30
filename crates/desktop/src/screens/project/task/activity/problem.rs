//! The title and detail of a runner problem entry in the Activity tab: what went wrong and, where
//! it helps, what the agent did instead or how to fix it.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  code_index::RipgrepFallback,
  connection::stream::{MemoryNotSavedReason, RunnerProblem},
  tokens::compact_tokens,
};

/// The entry's title and, if it has one, its detail for `problem`.
pub(super) fn problem_text(ctx: &Ctx, problem: &RunnerProblem) -> (Arc<str>, Option<Arc<str>>) {
  match problem {
    RunnerProblem::UnavailableVariable { variable } => (
      ctx.t_args(
        "runs.activity.problem.unavailableVariable",
        [("variable", variable.as_str())],
      ),
      None,
    ),
    RunnerProblem::CouldNotStart { detail } => (
      ctx.t("runs.activity.problem.couldNotStart"),
      Some(detail.as_str().into()),
    ),
    RunnerProblem::CheckpointFailed { detail } => (
      ctx.t("runs.activity.problem.checkpointFailed"),
      Some(detail.as_str().into()),
    ),
    RunnerProblem::MergeConflict { branch, files } => (
      ctx.t_args("runs.activity.problem.mergeConflict", [("branch", branch.as_str())]),
      Some(
        files
          .join(
            "
",
          )
          .into(),
      ),
    ),
    RunnerProblem::StepBudgetReached { budget } => (
      ctx.t_args(
        "runs.activity.problem.stepBudgetReached",
        [("budget", compact_tokens(*budget).as_str())],
      ),
      None,
    ),
    RunnerProblem::PermissionPromptUnreachable => (
      ctx.t("runs.activity.problem.promptUnreachable"),
      Some(ctx.t("runs.activity.problem.promptUnreachableDetail")),
    ),
    RunnerProblem::CliTooOld {
      model,
      installed,
      required,
    } => cli_too_old(ctx, model, installed, required),
    RunnerProblem::ProgramNotFound { program } => (
      ctx.t_args("runs.activity.problem.programNotFound", [("program", program.as_str())]),
      Some(ctx.t("runs.activity.problem.programNotFoundDetail")),
    ),
    RunnerProblem::CodebaseIndexUnavailable { detail } => (
      ctx.t("runs.activity.problem.codebaseIndexUnavailable"),
      Some(detail.as_str().into()),
    ),
    RunnerProblem::RipgrepNotFound { fallback } => (
      ctx.t("runs.activity.problem.ripgrepNotFound"),
      Some(ctx.t(fallback_key(*fallback))),
    ),
    RunnerProblem::RipgrepUnavailable { detail, fallback } => (
      ctx.t("runs.activity.problem.ripgrepUnavailable"),
      Some(ctx.t_args(
        "runs.activity.problem.ripgrepUnavailableDetail",
        [
          ("fallback", &*ctx.t(fallback_key(*fallback))),
          ("detail", detail.as_str()),
        ],
      )),
    ),
    RunnerProblem::MemoryNotSaved { reason } => (
      ctx.t("runs.activity.problem.memoryNotSaved"),
      Some(memory_not_saved(ctx, reason)),
    ),
    RunnerProblem::MemoryScanFailed { detail } => (
      ctx.t("runs.activity.problem.memoryScanFailed"),
      Some(detail.as_str().into()),
    ),
    RunnerProblem::MemoryUnreadable { records } => memory_unreadable(ctx, *records),
  }
}

/// A model the provider's API refused to the installed CLI, and the version it needs.
fn cli_too_old(ctx: &Ctx, model: &str, installed: &str, required: &str) -> (Arc<str>, Option<Arc<str>>) {
  let title = ctx.t_args(
    "runs.activity.problem.cliTooOld",
    [("model", model), ("installed", installed), ("required", required)],
  );
  (title, Some(ctx.t("runs.activity.problem.cliTooOldDetail")))
}

/// Memory records that could not be read back.
fn memory_unreadable(ctx: &Ctx, records: u64) -> (Arc<str>, Option<Arc<str>>) {
  let key = if records == 1 {
    "runs.activity.problem.memoryUnreadable.one"
  } else {
    "runs.activity.problem.memoryUnreadable.other"
  };
  (
    ctx.t_args(key, [("count", records.to_string().as_str())]),
    Some(ctx.t("runs.activity.problem.memoryUnreadableDetail")),
  )
}

/// Why a step's handoff was not kept as task memory, naming a secret but never its value.
fn memory_not_saved(ctx: &Ctx, reason: &MemoryNotSavedReason) -> Arc<str> {
  match reason {
    MemoryNotSavedReason::ContainsSecret { secret } => ctx.t_args(
      "runs.activity.problem.memoryContainsSecret",
      [("secret", secret.as_str())],
    ),
    MemoryNotSavedReason::KeychainUnavailable { secret } => ctx.t_args(
      "runs.activity.problem.memoryKeychainUnavailable",
      [("secret", secret.as_str())],
    ),
    MemoryNotSavedReason::StorageFailed { detail } => ctx.t_args(
      "runs.activity.problem.memoryStorageFailed",
      [("detail", detail.as_str())],
    ),
  }
}

/// How an agent without ripgrep searched: with or without the code index, by its CLI.
const fn fallback_key(fallback: RipgrepFallback) -> &'static str {
  match fallback {
    RipgrepFallback::Unknown => "runs.activity.problem.ripgrepFallback.unknown",
    RipgrepFallback::IndexWithGitGrep => "runs.activity.problem.ripgrepFallback.indexWithGitGrep",
    RipgrepFallback::IndexWithOwnTools => "runs.activity.problem.ripgrepFallback.indexWithOwnTools",
    RipgrepFallback::GitGrep => "runs.activity.problem.ripgrepFallback.gitGrep",
    RipgrepFallback::OwnTools => "runs.activity.problem.ripgrepFallback.ownTools",
  }
}
