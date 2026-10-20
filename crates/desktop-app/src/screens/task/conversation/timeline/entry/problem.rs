//! What a runner problem says: what went wrong and, where it helps, what the agent did instead or how to fix
//! it; and a denied command as a problem row (`Problem row` `IxIMv`: *Command denied · cargo publish
//! --dry-run*, *Builder was told: “Denied by the person reviewing this run. Continue without it.”*).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  code_index::RipgrepFallback,
  connection::stream::{CommandStatus, MemoryNotSavedReason, QaSlotHolder, QuestionRefusal, RunnerProblem},
  run::DecidedBy,
  tokens::compact_tokens,
};
use orchester_workspace::tasks::transcript::{CommandEntry, CommandMark, Entry, EntryKind, command_preview};

use super::{mcp::subject_key, native, shown::Shown};
use crate::ui::icon::Icon;

/// A problem row that reads better titled than as its transcript entry: a command denied (`agent` was told
/// why; `answered`: who denied the run's request to run it, when one did), a request that ended without a
/// decision, a question Orchester did not put to the person.
pub(super) fn titled(ctx: &Ctx, entry: &Entry, (agent, answered): (&str, Option<DecidedBy>)) -> Option<Shown> {
  match &entry.kind {
    EntryKind::Command(command) if command.status == Some(CommandStatus::Declined) => Some(match &command.native {
      Some(call) => native::call(ctx, command, call, false),
      None => denied_command(ctx, command, (agent, answered)),
    }),
    EntryKind::Problem(found @ (RunnerProblem::DecisionInterrupted { .. } | RunnerProblem::QuestionRefused { .. })) => {
      let (title, body) = text(ctx, found);
      Some(Shown::plain(Icon::CircleAlert, title, "".into(), body))
    }
    _ => None,
  }
}

/// *Command denied · cargo publish --dry-run*: who denied it, as its mark or the run's `answered` request
/// says, and what `agent` was told; without either, the run's permissions did.
fn denied_command(ctx: &Ctx, command: &CommandEntry, (agent, answered): (&str, Option<DecidedBy>)) -> Shown {
  let decided = match &command.mark {
    Some(CommandMark::Decided(mark)) => Some(mark),
    _ => None,
  };
  let line = command_preview(&command.command);
  let title = match decided.and_then(|mark| mark.decision.category.map(|category| (category, mark.decision.secrets))) {
    Some((category, secrets)) => ctx.t_args(
      "conversation.problem.categoryDenied",
      [
        ("subject", &*ctx.t(subject_key(category, secrets))),
        ("command", &*line),
      ],
    ),
    None => ctx.t_args("conversation.problem.commandDenied", [("command", &*line)]),
  };
  let denial = decided.filter(|_| command.mark.as_ref().is_some_and(CommandMark::is_denial));
  let told = denial.and_then(|mark| mark.decision.told.as_deref());
  let denied_by = denial.map(|mark| mark.decision.decided_by).or(answered);
  let body = match (denied_by, told) {
    (Some(_), Some(told)) => ctx.t_args("conversation.problem.told", [("agent", agent), ("message", told)]),
    (Some(by), None) if by.by_automation() => ctx.t("conversation.problem.deniedByAutomation"),
    (Some(_), None) => ctx.t("conversation.problem.deniedByYou"),
    (None, _) => ctx.t("conversation.problem.notAllowed"),
  };
  Shown {
    failed: true,
    ..Shown::plain(Icon::Ban, title, "".into(), Some(body))
  }
}

/// The problem's title and, if it has one, what it means.
pub(super) fn text(ctx: &Ctx, problem: &RunnerProblem) -> (Arc<str>, Option<Arc<str>>) {
  let titled = |key: &str, detail: &str| (ctx.t(key), Some(Arc::from(detail)));
  let explained = |key: &str, detail: &str| (ctx.t(key), Some(ctx.t(detail)));
  let alone = |key: &str, name: &str, value: &str| (ctx.t_args(key, [(name, value)]), None);
  match problem {
    RunnerProblem::UnavailableVariable { variable } => {
      alone("conversation.problem.unavailableVariable", "variable", variable)
    }
    RunnerProblem::CouldNotStart { detail } => titled("conversation.problem.couldNotStart", detail),
    RunnerProblem::CheckpointFailed { detail } => titled("conversation.problem.checkpointFailed", detail),
    RunnerProblem::MergeConflict { branch, files } => (
      ctx.t_args("conversation.problem.mergeConflict", [("branch", branch.as_str())]),
      Some(files.join("\n").into()),
    ),
    RunnerProblem::StepBudgetReached { budget } => alone(
      "conversation.problem.stepBudgetReached",
      "budget",
      &compact_tokens(*budget),
    ),
    RunnerProblem::PermissionPromptUnreachable => explained(
      "conversation.problem.promptUnreachable",
      "conversation.problem.promptUnreachableDetail",
    ),
    RunnerProblem::DecisionInterrupted { server, action, .. } => decision_interrupted(ctx, server, action),
    RunnerProblem::RunnerFailed { .. } => explained(
      "conversation.problem.runnerFailed",
      "conversation.problem.runnerFailedDetail",
    ),
    RunnerProblem::CliTooOld {
      model,
      installed,
      required,
    } => cli_too_old(
      ctx,
      [("model", model), ("installed", installed), ("required", required)],
    ),
    RunnerProblem::ProgramNotFound { program } => program_problem(ctx, program, false),
    RunnerProblem::ProgramMoved { program } => program_problem(ctx, program, true),
    RunnerProblem::CodebaseIndexUnavailable { detail } => {
      titled("conversation.problem.codebaseIndexUnavailable", detail)
    }
    RunnerProblem::RipgrepNotFound { fallback } => ripgrep(ctx, *fallback, None),
    RunnerProblem::RipgrepUnavailable { detail, fallback } => ripgrep(ctx, *fallback, Some(detail)),
    RunnerProblem::MemoryNotSaved { reason } => (
      ctx.t("conversation.problem.memoryNotSaved"),
      Some(memory_not_saved(ctx, reason)),
    ),
    RunnerProblem::MemoryScanFailed { detail } => titled("conversation.problem.memoryScanFailed", detail),
    RunnerProblem::MemoryUnreadable { records } => counted(
      ctx,
      *records,
      (
        "conversation.problem.memoryUnreadable.one",
        "conversation.problem.memoryUnreadable.other",
      ),
      "conversation.problem.memoryUnreadableDetail",
    ),
    RunnerProblem::CodexSettingsIgnored { folder } => (
      ctx.t("conversation.problem.codexSettingsIgnored"),
      Some(ctx.t_args(
        "conversation.problem.codexSettingsIgnoredDetail",
        [("folder", folder.as_str())],
      )),
    ),
    RunnerProblem::StderrLeftOut { lines } => counted(
      ctx,
      *lines,
      (
        "conversation.problem.stderrLeftOut.one",
        "conversation.problem.stderrLeftOut.other",
      ),
      "conversation.problem.stderrLeftOutDetail",
    ),
    RunnerProblem::QuestionRefused { question, reason } => question_refused(ctx, question, *reason),
    RunnerProblem::ContextCompacted { .. }
    | RunnerProblem::ModelRetrying { .. }
    | RunnerProblem::ThinkingStripped
    | RunnerProblem::WaitingForQaSlot { .. } => note(ctx, problem),
  }
}

/// A command tool's program that did not start: not found, or `moved` into the project's repository or off this
/// computer.
fn program_problem(ctx: &Ctx, program: &str, moved: bool) -> (Arc<str>, Option<Arc<str>>) {
  let (title, detail) = if moved {
    (
      "conversation.problem.programMoved",
      "conversation.problem.programMovedDetail",
    )
  } else {
    (
      "conversation.problem.programNotFound",
      "conversation.problem.programNotFoundDetail",
    )
  };
  (ctx.t_args(title, [("program", program)]), Some(ctx.t(detail)))
}

/// A model the provider's API refused to the installed CLI, and the version it needs.
fn cli_too_old(ctx: &Ctx, versions: [(&str, &String); 3]) -> (Arc<str>, Option<Arc<str>>) {
  let title = ctx.t_args(
    "conversation.problem.cliTooOld",
    versions.map(|(name, value)| (name, value.as_str())),
  );
  (title, Some(ctx.t("conversation.problem.cliTooOldDetail")))
}

/// A title that counts `count` things, and what it means.
fn counted(ctx: &Ctx, count: u64, (one, other): (&str, &str), detail: &str) -> (Arc<str>, Option<Arc<str>>) {
  let key = if count == 1 { one } else { other };
  let title = ctx.t_args(key, [("count", count.to_string().as_str())]);
  (title, Some(ctx.t(detail)))
}

/// Who holds the QA slots a start waits for: *2 QA apps are open (ORC-62 run #39, ORC-18 run #32). This one starts
/// when one closes.*, or that every slot is taken when the runner could not name them.
fn qa_slot_holders(ctx: &Ctx, holders: &[QaSlotHolder]) -> Arc<str> {
  if holders.is_empty() {
    return ctx.t("conversation.problem.waitingForQaSlotDetail");
  }
  let named: Vec<Arc<str>> = holders
    .iter()
    .map(|holder| {
      ctx.t_args(
        "conversation.problem.qaSlotHolder",
        [("task", holder.task.as_str()), ("run", holder.run.to_string().as_str())],
      )
    })
    .collect();
  let key = if holders.len() == 1 {
    "conversation.problem.qaSlotsHeld.one"
  } else {
    "conversation.problem.qaSlotsHeld.other"
  };
  ctx.t_args(
    key,
    [
      ("count", holders.len().to_string().as_str()),
      ("runs", named.join(&*ctx.t("conversation.list.separator")).as_str()),
    ],
  )
}

/// What happened around the agent that is no failure: a native conversation with its model compacted, a request
/// retried, its earlier reasoning dropped; or a start that waits for a QA slot.
fn note(ctx: &Ctx, problem: &RunnerProblem) -> (Arc<str>, Option<Arc<str>>) {
  match problem {
    RunnerProblem::WaitingForQaSlot { holders } => (
      ctx.t("conversation.problem.waitingForQaSlot"),
      Some(qa_slot_holders(ctx, holders)),
    ),
    RunnerProblem::ContextCompacted { before, after } => (
      ctx.t("conversation.problem.contextCompacted"),
      Some(ctx.t_args(
        "conversation.problem.contextCompactedDetail",
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
        "conversation.problem.modelRetrying",
        [
          ("seconds", wait_seconds.to_string().as_str()),
          ("attempt", attempt.to_string().as_str()),
        ],
      ),
      Some(error.as_str().into()),
    ),
    _ => (
      ctx.t("conversation.problem.thinkingStripped"),
      Some(ctx.t("conversation.problem.thinkingStrippedDetail")),
    ),
  }
}

/// A request that ended without a decision, and the request: its command on one line, or its MCP server
/// and tool.
fn decision_interrupted(ctx: &Ctx, server: &str, action: &str) -> (Arc<str>, Option<Arc<str>>) {
  let request: Arc<str> = if server == "command" {
    command_preview(action).into()
  } else {
    ctx.t_args("conversation.decision.tool", [("server", server), ("tool", action)])
  };
  (ctx.t("conversation.problem.decisionInterrupted"), Some(request))
}

/// A question Orchester did not put to the person: what it asked, and why it was declined.
fn question_refused(ctx: &Ctx, question: &str, reason: QuestionRefusal) -> (Arc<str>, Option<Arc<str>>) {
  let (title, detail) = match reason {
    QuestionRefusal::Secret => (
      "conversation.problem.secretQuestion",
      "conversation.problem.secretQuestionDetail",
    ),
    QuestionRefusal::NoOptions => (
      "conversation.problem.questionWithoutOptions",
      "conversation.problem.questionWithoutOptionsDetail",
    ),
  };
  (ctx.t_args(title, [("question", question)]), Some(ctx.t(detail)))
}

/// Why a step's handoff was not kept as task memory, naming a secret but never its value.
fn memory_not_saved(ctx: &Ctx, reason: &MemoryNotSavedReason) -> Arc<str> {
  match reason {
    MemoryNotSavedReason::ContainsSecret { secret } => ctx.t_args(
      "conversation.problem.memoryContainsSecret",
      [("secret", secret.as_str())],
    ),
    MemoryNotSavedReason::KeychainUnavailable { secret } => ctx.t_args(
      "conversation.problem.memoryKeychainUnavailable",
      [("secret", secret.as_str())],
    ),
    MemoryNotSavedReason::StorageFailed { detail } => ctx.t_args(
      "conversation.problem.memoryStorageFailed",
      [("detail", detail.as_str())],
    ),
  }
}

/// ripgrep was not found (`detail` `None`) or could not be given to the agent, and how it searched instead.
fn ripgrep(ctx: &Ctx, fallback: RipgrepFallback, detail: Option<&str>) -> (Arc<str>, Option<Arc<str>>) {
  let fallback = ctx.t(fallback_key(fallback));
  match detail {
    None => (ctx.t("conversation.problem.ripgrepNotFound"), Some(fallback)),
    Some(detail) => (
      ctx.t("conversation.problem.ripgrepUnavailable"),
      Some(ctx.t_args(
        "conversation.problem.ripgrepUnavailableDetail",
        [("fallback", &*fallback), ("detail", detail)],
      )),
    ),
  }
}

/// How an agent without ripgrep searched.
const fn fallback_key(fallback: RipgrepFallback) -> &'static str {
  match fallback {
    RipgrepFallback::Unknown => "conversation.problem.ripgrepFallback.unknown",
    RipgrepFallback::IndexWithGitGrep => "conversation.problem.ripgrepFallback.indexWithGitGrep",
    RipgrepFallback::IndexWithOwnTools => "conversation.problem.ripgrepFallback.indexWithOwnTools",
    RipgrepFallback::GitGrep => "conversation.problem.ripgrepFallback.gitGrep",
    RipgrepFallback::OwnTools => "conversation.problem.ripgrepFallback.ownTools",
  }
}
