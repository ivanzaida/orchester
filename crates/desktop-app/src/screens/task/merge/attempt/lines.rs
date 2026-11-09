//! What a merge attempt says in a line (`Cjprb`, *Merges rows · every outcome*), on the attempt page and the task
//! page's top card: a run's last attempt, its stage and cause in ORC-360's words; who asked and when. A cause is the
//! words of the OS, Git or Orchester as the attempt kept them, a message of Orchester's told in the person's language.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  integration::attempt::{AttemptOutcome, AttemptStage, MergeAsker, MergeAttempt, MergeVia},
  time::Timestamp,
};
use orchester_workspace::{display::short_revision, i18n::NAMESPACE};

/// What an unanswered Git's cause says before how long it wrote nothing.
const SILENT: &str = "wrote nothing for ";

/// The stage's name.
pub(crate) fn stage(ctx: &Ctx, stage: AttemptStage) -> Arc<str> {
  ctx.t(match stage {
    AttemptStage::Policy => "merges.stage.policy",
    AttemptStage::ProjectDatabase => "merges.stage.projectDatabase",
    AttemptStage::IntegrationLock => "merges.stage.integrationLock",
    AttemptStage::ScratchFolder => "merges.stage.scratchFolder",
    AttemptStage::CheckoutIdentity => "merges.stage.checkoutIdentity",
    AttemptStage::GitMetadata => "merges.stage.gitMetadata",
    AttemptStage::ExistingResult => "merges.stage.existingResult",
    AttemptStage::Merge => "merges.stage.merge",
    AttemptStage::Record => "merges.stage.record",
  })
}

/// Why the attempt did not land: a message of Orchester's in the person's words, else as it was kept.
pub(super) fn cause(ctx: &Ctx, cause: Option<&str>) -> Arc<str> {
  let Some(cause) = cause.filter(|cause| !cause.trim().is_empty()) else {
    return ctx.t("merges.outcome.noCause");
  };
  let told = ctx.t_ns(NAMESPACE, cause);
  if &*told == cause { Arc::from(cause) } else { told }
}

/// `attempt` into `base` in one line, as the attempt page and the task page's top card say it (ORC-731).
pub(crate) fn attempt_line(ctx: &Ctx, attempt: &MergeAttempt, base: &str) -> Arc<str> {
  let why = cause(ctx, attempt.cause());
  match attempt.outcome() {
    AttemptOutcome::Merged { .. } => why,
    AttemptOutcome::Refused { stage: at } => {
      let key = match (*at, attempt.via()) {
        (AttemptStage::Merge | AttemptStage::Record, _) => "merges.outcome.refusedAt",
        (_, MergeVia::MergeQueue) => "merges.outcome.stoppedInQueue",
        _ => "merges.outcome.refusedBeforeGit",
      };
      ctx.t_args(key, [("stage", stage(ctx, *at)), ("cause", why)])
    }
    AttemptOutcome::Stopped { words } => ctx.t_args("merges.outcome.queueStopped", [("cause", words.as_str())]),
    AttemptOutcome::Conflicts { files } => counted(
      ctx,
      ("merges.outcome.conflictsOne", "merges.outcome.conflictsOther"),
      files.len(),
      base,
    ),
    AttemptOutcome::NotRecorded { revision } => ctx.t_args(
      "merges.outcome.notRecorded",
      [
        ("revision", Arc::from(short_revision(revision.as_str()))),
        ("cause", why),
      ],
    ),
    AttemptOutcome::GitStopped { stage: at } => git_stopped(ctx, (*at, attempt.cause()), base),
    AttemptOutcome::LeftChanged { files } => counted(
      ctx,
      ("merges.outcome.leftChangedOne", "merges.outcome.leftChangedOther"),
      files.len(),
      base,
    ),
    AttemptOutcome::Interrupted { stage: at } => ctx.t_args(
      "merges.outcome.interrupted",
      [("stage", stage(ctx, *at)), ("cause", why)],
    ),
    AttemptOutcome::Moved { stage: Some(_) } => {
      ctx.t_args("merges.outcome.moved", [("branch", Arc::from(base)), ("cause", why)])
    }
    AttemptOutcome::Moved { stage: None } => ctx.t_args("merges.outcome.movedBefore", [("branch", base)]),
  }
}

/// *Git stopped answering at Merge · stopped after 2 min · checkout and master untouched*: the stage, how long Git
/// wrote nothing when the cause says it, and what it did not touch, as `Cjprb` draws it; the cause stays on the
/// attempt page.
fn git_stopped(ctx: &Ctx, (at, cause): (AttemptStage, Option<&str>), base: &str) -> Arc<str> {
  let stage = stage(ctx, at);
  let Some(seconds) = cause.and_then(silent_for) else {
    return ctx.t_args(
      "merges.outcome.gitStopped",
      [("stage", stage), ("branch", Arc::from(base))],
    );
  };
  let (minutes, seconds) = (seconds / 60, seconds % 60);
  let time = match (minutes, seconds) {
    (0, seconds) => ctx.t_args("merges.outcome.seconds", [("seconds", seconds.to_string())]),
    (minutes, 0) => ctx.t_args("merges.outcome.minutes", [("minutes", minutes.to_string())]),
    (minutes, seconds) => ctx.t_args(
      "merges.outcome.minutesSeconds",
      [("minutes", minutes.to_string()), ("seconds", seconds.to_string())],
    ),
  };
  ctx.t_args(
    "merges.outcome.gitStoppedAfter",
    [("stage", stage), ("time", time), ("branch", Arc::from(base))],
  )
}

/// How many seconds Git wrote nothing before it was stopped, as an unanswered Git's cause says it (`… wrote nothing
/// for 120 s and was stopped`, `GitError::Unanswered` and `MergeError::Unanswered`).
fn silent_for(cause: &str) -> Option<u64> {
  let (_, after) = cause.split_once(SILENT)?;
  let (seconds, _) = after.split_once(" s")?;
  seconds.trim().parse().ok()
}

/// A line with `count` files and the branch, worded for one or more.
fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize, base: &str) -> Arc<str> {
  if count == 1 {
    ctx.t_args(one, [("branch", base)])
  } else {
    ctx.t_args(other, [("branch", base.to_owned()), ("count", count.to_string())])
  }
}

/// Who asked for a merge.
pub(super) fn who(ctx: &Ctx, asked_by: MergeAsker) -> Arc<str> {
  ctx.t(match asked_by {
    MergeAsker::Person => "merges.who.person",
    MergeAsker::Automation => "merges.who.automation",
    MergeAsker::MergeQueue => "merges.who.mergeQueue",
    MergeAsker::Retry { .. } => "merges.who.orchester",
  })
}

/// When, as a row says it: the time alone today, else the date and time.
pub(crate) fn when(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  crate::screens::task::summary::runs::line::day_and_clock(ctx, at)
}

/// `parts` one after another, as a row's line runs them.
pub(crate) fn joined(ctx: &Ctx, parts: &[Arc<str>]) -> Arc<str> {
  let separator = ctx.t("merges.separator");
  Arc::from(parts.iter().map(|part| &**part).collect::<Vec<_>>().join(&separator))
}
