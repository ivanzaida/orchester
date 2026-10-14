//! The title and detail of a runner problem entry in the Activity tab: what went wrong and, where
//! it helps, what the agent did instead or how to fix it; and a denied command as a problem row of
//! the timeline (`OlLGD`): what was denied as its title, the command and what the agent was told in
//! the explanation under it.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  code_index::RipgrepFallback,
  connection::stream::{CommandStatus, MemoryNotSavedReason, QuestionRefusal, RunnerProblem},
  run::DecidedBy,
  tokens::compact_tokens,
};

use super::{entry::Shown, mcp_call::subject_key, native_call};
use crate::{
  tasks::transcript::{CommandEntry, CommandMark, Entry, EntryKind},
  ui::{command_row::command_preview, icon::Icon},
};

/// A problem row that reads as a titled row with an explanation rather than as its transcript entry:
/// a command that did not run because it was denied; a request that ended without a decision, with
/// what happened to it beside its time, as a denial has.
pub(super) fn titled(ctx: &Ctx, entry: &Entry) -> Option<Shown> {
  match &entry.kind {
    EntryKind::Command(command) if command.status == Some(CommandStatus::Declined) => Some(match &command.native {
      Some(call) => native_call::call(ctx, command, call),
      None => denied_command(ctx, command),
    }),
    EntryKind::Problem(problem @ RunnerProblem::DecisionInterrupted { .. }) => {
      let (title, body) = problem_text(ctx, problem);
      let kind = ctx.t("runs.conversation.problem.kind.decisionInterrupted");
      Some(Shown::plain(Icon::AlertCircle, title, kind, body))
    }
    EntryKind::Problem(problem @ RunnerProblem::QuestionRefused { .. }) => {
      let (title, body) = problem_text(ctx, problem);
      let kind = ctx.t("runs.conversation.problem.kind.questionRefused");
      Some(Shown::plain(Icon::AlertCircle, title, kind, body))
    }
    _ => None,
  }
}

/// Whether `entry` is an agent's question for a secret that Orchester refused, under which the row offers
/// the project's Secrets.
pub(super) fn asks_for_secret(entry: &Entry) -> bool {
  matches!(
    &entry.kind,
    EntryKind::Problem(RunnerProblem::QuestionRefused {
      reason: QuestionRefusal::Secret,
      ..
    })
  )
}

/// *Outside write denied* · *permission_denied*: `apply_patch ../shared/auth.ts did not run: …`.
fn denied_command(ctx: &Ctx, command: &CommandEntry) -> Shown {
  let decided = match &command.mark {
    Some(CommandMark::Decided(mark)) => Some(mark),
    _ => None,
  };
  let title = match decided.and_then(|mark| mark.decision.category.map(|category| (category, mark.decision.secrets))) {
    Some((category, secrets)) => ctx.t_args(
      "runs.conversation.problem.categoryDenied",
      [("subject", &*ctx.t(subject_key(category, secrets)))],
    ),
    None => ctx.t("runs.conversation.problem.commandDenied"),
  };
  let line = command_preview(&command.command);
  let denied = decided
    .filter(|_| command.mark.as_ref().is_some_and(CommandMark::is_denial))
    .map(|mark| Denial::by(mark.decision.decided_by));
  let told = decided.and_then(|mark| mark.decision.told.as_deref());
  let (kind, body) = match (denied, told) {
    (Some(denial), Some(told)) => (
      denial.kind,
      ctx.t_args(denial.told, [("command", &*line), ("message", told)]),
    ),
    (Some(denial), None) => (denial.kind, ctx.t_args(denial.text, [("command", &*line)])),
    (None, _) => (
      "runs.conversation.problem.kind.permissionDenied",
      ctx.t_args("runs.conversation.problem.notAllowed", [("command", &*line)]),
    ),
  };
  Shown::plain(Icon::ShieldAlert, title, ctx.t(kind), Some(body))
}

/// The keys a denial is told with: by the person, or by automation through Orchester's MCP server.
struct Denial {
  kind: &'static str,
  text: &'static str,
  told: &'static str,
}

impl Denial {
  const fn by(decided_by: DecidedBy) -> Self {
    if decided_by.by_automation() {
      Self {
        kind: "runs.conversation.problem.kind.deniedByAutomation",
        text: "runs.conversation.problem.deniedByAutomation",
        told: "runs.conversation.problem.deniedByAutomationTold",
      }
    } else {
      Self {
        kind: "runs.conversation.problem.kind.deniedByYou",
        text: "runs.conversation.problem.deniedByYou",
        told: "runs.conversation.problem.deniedByYouTold",
      }
    }
  }
}

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
    RunnerProblem::PermissionPromptUnreachable => prompt_unreachable(ctx),
    RunnerProblem::DecisionInterrupted { server, action, .. } => decision_interrupted(ctx, server, action),
    RunnerProblem::RunnerFailed { detail } => runner_failed(ctx, detail),
    RunnerProblem::CliTooOld {
      model,
      installed,
      required,
    } => cli_too_old(ctx, model, installed, required),
    RunnerProblem::ProgramNotFound { program } => program_problem(ctx, program, false),
    RunnerProblem::ProgramMoved { program } => program_problem(ctx, program, true),
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
    RunnerProblem::CodexSettingsIgnored { folder } => codex_settings_ignored(ctx, folder),
    RunnerProblem::StderrLeftOut { lines } => stderr_left_out(ctx, *lines),
    RunnerProblem::QuestionRefused { question, reason } => question_refused(ctx, question, *reason),
    RunnerProblem::ContextCompacted { .. }
    | RunnerProblem::ModelRetrying { .. }
    | RunnerProblem::ThinkingStripped
    | RunnerProblem::WaitingForQaSlot => native_problem(ctx, problem),
  }
}

/// What happened around the agent that is no failure: a native conversation with its model compacted, a request
/// retried, its earlier reasoning dropped; or a start that waits for the QA slot.
fn native_problem(ctx: &Ctx, problem: &RunnerProblem) -> (Arc<str>, Option<Arc<str>>) {
  match problem {
    RunnerProblem::ContextCompacted { before, after } => (
      ctx.t("runs.activity.problem.contextCompacted"),
      Some(ctx.t_args(
        "runs.activity.problem.contextCompactedDetail",
        [
          ("before", compact_tokens(*before).as_str()),
          ("after", compact_tokens(*after).as_str()),
        ],
      )),
    ),
    RunnerProblem::ModelRetrying {
      attempt,
      wait_seconds,
      error,
    } => (
      ctx.t_args(
        "runs.activity.problem.modelRetrying",
        [
          ("seconds", wait_seconds.to_string().as_str()),
          ("attempt", attempt.to_string().as_str()),
        ],
      ),
      Some(error.as_str().into()),
    ),
    RunnerProblem::WaitingForQaSlot => (
      ctx.t("runs.activity.problem.waitingForQaSlot"),
      Some(ctx.t("runs.activity.problem.waitingForQaSlotDetail")),
    ),
    _ => (
      ctx.t("runs.activity.problem.thinkingStripped"),
      Some(ctx.t("runs.activity.problem.thinkingStrippedDetail")),
    ),
  }
}

/// Claude Code's permission prompt never reached the runner.
fn prompt_unreachable(ctx: &Ctx) -> (Arc<str>, Option<Arc<str>>) {
  (
    ctx.t("runs.activity.problem.promptUnreachable"),
    Some(ctx.t("runs.activity.problem.promptUnreachableDetail")),
  )
}

/// The runner could not record the run, which was interrupted, and its error.
fn runner_failed(ctx: &Ctx, detail: &str) -> (Arc<str>, Option<Arc<str>>) {
  (
    ctx.t("runs.activity.problem.runnerFailed"),
    Some(ctx.t_args("runs.activity.problem.runnerFailedDetail", [("detail", detail)])),
  )
}

/// A request that ended without a decision: what happened as the title, which keeps its words at any width,
/// and the request under it, by its command on one line or by its MCP server and tool.
fn decision_interrupted(ctx: &Ctx, server: &str, action: &str) -> (Arc<str>, Option<Arc<str>>) {
  let request: Arc<str> = if server == "command" {
    command_preview(action).into()
  } else {
    ctx.t_args("runs.activity.decision.tool", [("server", server), ("tool", action)])
  };
  (
    ctx.t("runs.activity.problem.decisionInterrupted"),
    Some(ctx.t_args(
      "runs.activity.problem.decisionInterruptedDetail",
      [("request", &*request)],
    )),
  )
}

/// A question Orchester did not put to the person: what it asked, and why it was declined.
fn question_refused(ctx: &Ctx, question: &str, reason: QuestionRefusal) -> (Arc<str>, Option<Arc<str>>) {
  let (title, detail) = match reason {
    QuestionRefusal::Secret => (
      "runs.activity.problem.secretQuestion",
      "runs.activity.problem.secretQuestionDetail",
    ),
    QuestionRefusal::NoOptions => (
      "runs.activity.problem.questionWithoutOptions",
      "runs.activity.problem.questionWithoutOptionsDetail",
    ),
  };
  (ctx.t_args(title, [("question", question)]), Some(ctx.t(detail)))
}

/// Lines of the CLI's standard error the activity did not keep.
fn stderr_left_out(ctx: &Ctx, lines: u64) -> (Arc<str>, Option<Arc<str>>) {
  let key = if lines == 1 {
    "runs.activity.problem.stderrLeftOut.one"
  } else {
    "runs.activity.problem.stderrLeftOut.other"
  };
  (
    ctx.t_args(key, [("count", lines.to_string().as_str())]),
    Some(ctx.t("runs.activity.problem.stderrLeftOutDetail")),
  )
}

/// The repository's `.codex` folder Codex left out, and what still applies.
fn codex_settings_ignored(ctx: &Ctx, folder: &str) -> (Arc<str>, Option<Arc<str>>) {
  (
    ctx.t("runs.activity.problem.codexSettingsIgnored"),
    Some(ctx.t_args("runs.activity.problem.codexSettingsIgnoredDetail", [("folder", folder)])),
  )
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

/// A command tool's program that did not start: not found, or `moved` into the project's repository or off this
/// computer.
fn program_problem(ctx: &Ctx, program: &str, moved: bool) -> (Arc<str>, Option<Arc<str>>) {
  let (title, detail) = if moved {
    (
      "runs.activity.problem.programMoved",
      "runs.activity.problem.programMovedDetail",
    )
  } else {
    (
      "runs.activity.problem.programNotFound",
      "runs.activity.problem.programNotFoundDetail",
    )
  };
  (ctx.t_args(title, [("program", program)]), Some(ctx.t(detail)))
}
