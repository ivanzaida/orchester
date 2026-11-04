//! What the attempt page says of a stage and an attempt (09b `PVZKL`): a stage's name and what it touches (`Where` in
//! `n8L7kz`), how it went (`Result`), and an attempt's row: when, who, from where and how it ended (`ZBKaG`).

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  integration::{
    MergeKind,
    attempt::{AttemptOutcome, AttemptStage, CauseKind, MergeAsker, MergeAttempt, MergeVia, OsCause, StageResult},
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
    AttemptOutcome::Refused { .. } => category(ctx, attempt).unwrap_or_else(|| ctx.t("merges.page.result.refused")),
    AttemptOutcome::Conflicts { files } if files.len() == 1 => ctx.t("merges.page.result.conflictsOne"),
    AttemptOutcome::Conflicts { files } => ctx.t_args(
      "merges.page.result.conflictsOther",
      [("count", files.len().to_string())],
    ),
    AttemptOutcome::NotRecorded { .. } => ctx.t("merges.page.result.notRecorded"),
    AttemptOutcome::GitStopped { .. } => match silent_for(attempt) {
      Some(seconds) => ctx.t_args("merges.page.result.noAnswerFor", [("time", silent_short(ctx, seconds))]),
      None => ctx.t("merges.page.result.noAnswer"),
    },
    AttemptOutcome::LeftChanged { .. } => ctx.t("merges.page.result.stoppedPartWay"),
    AttemptOutcome::Interrupted { .. } => ctx.t("merges.page.result.interrupted"),
    AttemptOutcome::Moved { .. } => ctx.t("merges.page.result.moved"),
    _ => ctx.t("merges.page.result.refused"),
  }
}

/// The category of what refused `attempt`, as its record keeps it: *Used by another program*. An attempt recorded
/// before the category was kept names only the integration lock another operation held, by its cause's words.
fn category(ctx: &Ctx, attempt: &MergeAttempt) -> Option<Arc<str>> {
  let key = match attempt.kind() {
    Some(CauseKind::HeldByOperation) => "merges.page.result.held",
    Some(CauseKind::Os { what, .. }) => match what {
      OsCause::UsedByAnotherProgram => "merges.page.result.usedByAnotherProgram",
      OsCause::AccessDenied => "merges.page.result.accessDenied",
      OsCause::NotFound => "merges.page.result.notFound",
      OsCause::Other => "merges.page.result.osRefused",
    },
    Some(CauseKind::Git) => "merges.page.result.gitRefused",
    Some(CauseKind::Checkout) => "merges.page.result.checkout",
    Some(CauseKind::Unanswered { .. }) => return None,
    None if attempt.cause() == Some(HELD) => "merges.page.result.held",
    None => return None,
  };
  Some(ctx.t(key))
}

/// How long Git wrote nothing before it was stopped, when the attempt's record keeps it.
pub(super) fn silent_for(attempt: &MergeAttempt) -> Option<u64> {
  match attempt.kind() {
    Some(CauseKind::Unanswered { seconds }) => Some(seconds),
    _ => None,
  }
}

/// `seconds` as whole minutes, when they are.
fn whole_minutes(seconds: u64) -> Option<u64> {
  (seconds >= 60 && seconds.is_multiple_of(60)).then_some(seconds / 60)
}

/// *2 min*, or *45 s* for a time that is not whole minutes.
fn silent_short(ctx: &Ctx, seconds: u64) -> String {
  let (key, count) = match whole_minutes(seconds) {
    Some(minutes) => ("merges.page.silent.minutes", minutes),
    None => ("merges.page.silent.seconds", seconds),
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}

/// *2 minutes*, *1 second*: how long Git wrote nothing, as a sentence says it.
pub(super) fn silent_long(ctx: &Ctx, seconds: u64) -> String {
  let (key, count) = match (whole_minutes(seconds), seconds) {
    (Some(1), _) => ("merges.page.silent.minuteOne", 1),
    (Some(minutes), _) => ("merges.page.silent.minutesOther", minutes),
    (None, 1) => ("merges.page.silent.secondOne", 1),
    (None, _) => ("merges.page.silent.secondsOther", seconds),
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
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
pub(super) fn made(ctx: &Ctx, revision: &Revision, kind: MergeKind) -> Arc<str> {
  let key = match kind {
    MergeKind::MergeCommit => "merges.merged.mergeCommit",
    MergeKind::Squash => "merges.merged.squash",
    MergeKind::FastForward | MergeKind::AlreadyMerged => "merges.merged.fastForward",
  };
  ctx.t_args(key, [("revision", short_revision(revision.as_str()))])
}

/// An attempt's row says how it ended, in its color: landed in `success`, else in `warning`. A refusal names its stage
/// and the OS's error number, else its category, as its record keeps them; else its cause.
pub(super) fn attempt_outcome(ctx: &Ctx, attempt: &MergeAttempt, branch: &str) -> (Arc<str>, PaletteColor) {
  let stage = |at: &AttemptStage| stage_name(ctx, *at).to_string();
  let text = match attempt.outcome() {
    AttemptOutcome::Merged { revision, .. } => {
      let text = ctx.t_args(
        "merges.page.attempt.merged",
        [("revision", short_revision(revision.as_str()))],
      );
      return (text, PaletteColor::Success);
    }
    AttemptOutcome::Refused { stage: at } => match attempt.kind() {
      Some(CauseKind::Os { code: Some(code), .. }) => ctx.t_args(
        "merges.page.attempt.osError",
        [("stage", stage(at)), ("code", code.to_string())],
      ),
      _ => {
        let why = category(ctx, attempt).unwrap_or_else(|| cause(ctx, attempt.cause()));
        ctx.t_args(
          "merges.page.attempt.refusedAt",
          [("stage", stage(at)), ("cause", why.to_string())],
        )
      }
    },
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
