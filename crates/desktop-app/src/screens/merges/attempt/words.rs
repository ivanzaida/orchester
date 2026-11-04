//! What the attempt page says of a stage and an attempt (09b `PVZKL`): a stage's name and what it touches (`Where` in
//! `n8L7kz`), how it went (`Result`), and an attempt's row: when, who, from where and how it ended (`ZBKaG`).

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  integration::{
    MergeKind,
    attempt::{AttemptOutcome, AttemptStage, MergeAsker, MergeAttempt, MergeVia, StageResult},
    retry::RETRIES,
  },
  revision::Revision,
  time::Timestamp,
};
use orchester_workspace::display::{self, short_revision};

use super::super::words::cause;

/// The cause a preflight's integration lock gives when another operation holds it.
const HELD: &str = "held by another operation";

pub(super) fn stage_name(ctx: &Ctx, stage: AttemptStage) -> Arc<str> {
  ctx.t(match stage {
    AttemptStage::Policy => "merges.page.stage.policy",
    AttemptStage::ProjectDatabase => "merges.page.stage.projectDatabase",
    AttemptStage::IntegrationLock => "merges.page.stage.integrationLock",
    AttemptStage::ScratchFolder => "merges.page.stage.scratchFolder",
    AttemptStage::CheckoutIdentity => "merges.page.stage.checkoutIdentity",
    AttemptStage::GitMetadata => "merges.page.stage.gitMetadata",
    AttemptStage::ExistingResult => "merges.page.stage.existingResult",
    AttemptStage::Merge => "merges.page.stage.merge",
    AttemptStage::Record => "merges.page.stage.record",
  })
}

/// What `stage` touches: a file, a command, the branches it compares.
pub(super) fn stage_where(ctx: &Ctx, stage: AttemptStage, (base, branch): (&str, &str)) -> Arc<str> {
  let key = match stage {
    AttemptStage::Policy => "merges.page.where.policy",
    AttemptStage::ProjectDatabase => "merges.page.where.projectDatabase",
    AttemptStage::IntegrationLock => "merges.page.where.integrationLock",
    AttemptStage::ScratchFolder => "merges.page.where.scratchFolder",
    AttemptStage::CheckoutIdentity => "merges.page.where.checkoutIdentity",
    AttemptStage::GitMetadata => "merges.page.where.gitMetadata",
    AttemptStage::ExistingResult => "merges.page.where.existingResult",
    AttemptStage::Merge => "merges.page.where.merge",
    AttemptStage::Record => "merges.page.where.record",
  };
  ctx.t_args(key, [("base", base), ("branch", branch)])
}

/// How `stage` went in `attempt`, in its color.
pub(super) fn stage_result(
  ctx: &Ctx,
  attempt: &MergeAttempt,
  (stage, result): (AttemptStage, StageResult),
) -> (Arc<str>, PaletteColor) {
  match result {
    StageResult::NotReached => (ctx.t("merges.page.result.notReached"), PaletteColor::TextMuted),
    StageResult::Refused => (refused_result(ctx, attempt), PaletteColor::Warning),
    StageResult::Passed => (passed_result(ctx, attempt, stage), PaletteColor::TextMuted),
  }
}

fn refused_result(ctx: &Ctx, attempt: &MergeAttempt) -> Arc<str> {
  match attempt.outcome() {
    AttemptOutcome::Refused { .. } if attempt.cause() == Some(HELD) => ctx.t("merges.page.result.held"),
    AttemptOutcome::Conflicts { files } if files.len() == 1 => ctx.t("merges.page.result.conflictsOne"),
    AttemptOutcome::Conflicts { files } => ctx.t_args(
      "merges.page.result.conflictsOther",
      [("count", files.len().to_string())],
    ),
    AttemptOutcome::NotRecorded { .. } => ctx.t("merges.page.result.notRecorded"),
    AttemptOutcome::GitStopped { .. } => ctx.t("merges.page.result.noAnswer"),
    AttemptOutcome::LeftChanged { .. } => ctx.t("merges.page.result.stoppedPartWay"),
    AttemptOutcome::Interrupted { .. } => ctx.t("merges.page.result.interrupted"),
    AttemptOutcome::Moved { .. } => ctx.t("merges.page.result.moved"),
    _ => ctx.t("merges.page.result.refused"),
  }
}

/// A stage that passed; the merge says the commit it made, the record that it is recorded.
fn passed_result(ctx: &Ctx, attempt: &MergeAttempt, stage: AttemptStage) -> Arc<str> {
  match (stage, attempt.outcome()) {
    (AttemptStage::Merge, AttemptOutcome::Merged { revision, kind }) => made(ctx, revision, *kind),
    (AttemptStage::Merge, AttemptOutcome::NotRecorded { revision }) => made(ctx, revision, MergeKind::MergeCommit),
    (AttemptStage::Record, AttemptOutcome::Merged { .. }) => ctx.t("merges.page.result.recorded"),
    _ => ctx.t("merges.page.result.passed"),
  }
}

/// *Merge commit d08e094*: how Git brought the run in.
fn made(ctx: &Ctx, revision: &Revision, kind: MergeKind) -> Arc<str> {
  let key = match kind {
    MergeKind::MergeCommit => "merges.merged.mergeCommit",
    MergeKind::Squash => "merges.merged.squash",
    MergeKind::FastForward | MergeKind::AlreadyMerged => "merges.merged.fastForward",
  };
  ctx.t_args(key, [("revision", short_revision(revision.as_str()))])
}

/// An attempt's row says how it ended, in its color: landed in `success`, else in `warning`.
pub(super) fn attempt_outcome(ctx: &Ctx, attempt: &MergeAttempt, branch: &str) -> (Arc<str>, PaletteColor) {
  let why = cause(ctx, attempt.cause()).to_string();
  let stage = |at: &AttemptStage| stage_name(ctx, *at).to_string();
  let text = match attempt.outcome() {
    AttemptOutcome::Merged { revision, .. } => {
      let text = ctx.t_args(
        "merges.page.attempt.merged",
        [("revision", short_revision(revision.as_str()))],
      );
      return (text, PaletteColor::Success);
    }
    AttemptOutcome::Refused { stage: at } => {
      ctx.t_args("merges.page.attempt.refusedAt", [("stage", stage(at)), ("cause", why)])
    }
    AttemptOutcome::Stopped { words } => ctx.t_args("merges.page.attempt.queueStopped", [("cause", words.as_str())]),
    AttemptOutcome::Conflicts { files } if files.len() == 1 => ctx.t("merges.page.attempt.conflictsOne"),
    AttemptOutcome::Conflicts { files } => ctx.t_args(
      "merges.page.attempt.conflictsOther",
      [("count", files.len().to_string())],
    ),
    AttemptOutcome::NotRecorded { revision } => ctx.t_args(
      "merges.page.attempt.notRecorded",
      [("revision", short_revision(revision.as_str()))],
    ),
    AttemptOutcome::GitStopped { stage: at } => ctx.t_args("merges.page.attempt.gitStopped", [("stage", stage(at))]),
    AttemptOutcome::LeftChanged { files } => {
      let merge = stage(&AttemptStage::Merge);
      if files.len() == 1 {
        ctx.t_args("merges.page.attempt.leftChangedOne", [("stage", merge)])
      } else {
        ctx.t_args(
          "merges.page.attempt.leftChangedOther",
          [("stage", merge), ("count", files.len().to_string())],
        )
      }
    }
    AttemptOutcome::Interrupted { stage: at } => ctx.t_args("merges.page.attempt.interrupted", [("stage", stage(at))]),
    AttemptOutcome::Moved { .. } => ctx.t_args("merges.page.attempt.moved", [("branch", branch)]),
  };
  (text, PaletteColor::Warning)
}

/// Where an attempt came from, after who asked: *task page*, *merge_run*, *delivering run #8*, *retry 2 of 3*.
pub(super) fn attempt_via(ctx: &Ctx, attempt: &MergeAttempt, run: u32) -> Arc<str> {
  if let MergeAsker::Retry { number } = attempt.asked_by() {
    return ctx.t_args(
      "merges.page.attempt.retry",
      [("number", number.get().to_string()), ("of", RETRIES.to_string())],
    );
  }
  match attempt.via() {
    MergeVia::TaskPage => ctx.t("merges.page.attempt.taskPage"),
    MergeVia::MergeRun => ctx.t("merges.page.attempt.mergeRun"),
    MergeVia::MergeQueue => ctx.t_args("merges.page.attempt.mergeQueue", [("run", run.to_string())]),
  }
}

/// *by you from the task page*: who asked for an attempt, and from where.
pub(super) fn asked(ctx: &Ctx, attempt: &MergeAttempt) -> Arc<str> {
  let via = ctx.t(match attempt.via() {
    MergeVia::TaskPage => "merges.page.via.taskPage",
    MergeVia::MergeRun => "merges.page.via.mergeRun",
    MergeVia::MergeQueue => "merges.page.via.mergeQueue",
  });
  match attempt.asked_by() {
    MergeAsker::Person => ctx.t_args("merges.page.by.person", [("via", via)]),
    MergeAsker::Automation => ctx.t_args("merges.page.by.automation", [("via", via)]),
    MergeAsker::MergeQueue => ctx.t("merges.page.by.mergeQueue"),
    MergeAsker::Retry { number } => ctx.t_args(
      "merges.page.by.orchester",
      [("number", number.get().to_string()), ("of", RETRIES.to_string())],
    ),
  }
}

/// When an attempt was made, as its row and the page say it: with the seconds when `seconds`, for attempts a minute
/// holds more than one of; today's without the date.
pub(super) fn attempt_time(ctx: &Ctx, at: Timestamp, seconds: bool) -> Arc<str> {
  if seconds && at.local_day_start() == Timestamp::now().local_day_start() {
    return Arc::from(display::clock_seconds(at));
  }
  super::super::words::when(ctx, at)
}
