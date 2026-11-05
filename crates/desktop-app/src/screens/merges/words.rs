//! What a row of Review › Merges says (`Cjprb`, *Merges rows · every outcome*): a run's last attempt as one line, its
//! stage and cause in ORC-360's words; while Orchester retries, its next try (ORC-365); who asked and when; a day's
//! heading. A cause is the words of the OS, Git or Orchester as the attempt kept them, a message of Orchester's told in
//! the person's language.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  integration::{
    MergeKind,
    attempt::{AttemptOutcome, AttemptStage, MergeAsker, MergeAttempt, MergeVia},
    retry::RETRIES,
  },
  time::Timestamp,
};
use orchester_workspace::{
  display::{self, MONTH_KEYS, WEEKDAY_KEYS, short_revision},
  i18n::NAMESPACE,
  tasks::merge_attempts::{
    merges::{MergedRun, NotMergedRun},
    read::Retrying,
  },
};

/// The base branch a line names when the run did not record its own.
const NO_BRANCH: &str = "—";
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

/// The branch the run merges into, as a line names it.
pub(super) fn branch(run: &NotMergedRun) -> &str {
  run.run.base.as_ref().map_or(NO_BRANCH, |branch| branch.as_str())
}

/// The run's last attempt in one line: where it stopped and why. A run Orchester tries again has its own line
/// ([`retrying_line`]).
pub(super) fn outcome(ctx: &Ctx, run: &NotMergedRun) -> Arc<str> {
  attempt_line(ctx, &run.not_merged.attempt.attempt, branch(run))
}

/// `attempt` into `base` in one line, as a row of Review › Merges and the task page's top card say it (ORC-731).
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

/// *Retrying by itself · attempt N of 3 in Ns*, the next try after the one `last` asked for; the merge queue's own
/// retry has no set time.
pub(super) fn retrying_line(ctx: &Ctx, last: MergeAsker, retrying: Retrying, why: Arc<str>) -> Arc<str> {
  let Some(next) = retrying.next else {
    return ctx.t_args("merges.outcome.queueRetries", [("cause", why)]);
  };
  let attempt = match last {
    MergeAsker::Retry { number } => number.get().saturating_add(1).min(RETRIES),
    _ => 1,
  };
  let seconds = ((next.unix_millis() - Timestamp::now().unix_millis()).max(0) + 999) / 1000;
  let (attempt, of) = (attempt.to_string(), RETRIES.to_string());
  if seconds == 0 {
    return ctx.t_args(
      "merges.outcome.retryingNow",
      [("attempt", attempt), ("of", of), ("cause", why.to_string())],
    );
  }
  ctx.t_args(
    "merges.outcome.retrying",
    [
      ("attempt", attempt),
      ("of", of),
      ("seconds", seconds.to_string()),
      ("cause", why.to_string()),
    ],
  )
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
pub(super) fn joined(ctx: &Ctx, parts: &[Arc<str>]) -> Arc<str> {
  let separator = ctx.t("merges.separator");
  Arc::from(parts.iter().map(|part| &**part).collect::<Vec<_>>().join(&separator))
}

/// *You · 12:41 · 2 attempts*: who asked for the last attempt, when, and how many there were; while Orchester tries
/// again by itself, without the count, which its line already says (*attempt N of 3*, `Cjprb`).
pub(super) fn not_merged_meta(ctx: &Ctx, run: &NotMergedRun) -> Arc<str> {
  let recorded = &run.not_merged.attempt;
  let mut parts = vec![who(ctx, recorded.attempt.asked_by()), when(ctx, recorded.attempt.at())];
  if recorded.sequence > 1 && run.not_merged.retrying.is_none() {
    parts.push(ctx.t_args("merges.attempts", [("count", recorded.sequence.to_string())]));
  }
  joined(ctx, &parts)
}

/// How a merged run landed: the commit, what was cleared after it, and the attempts before it.
pub(super) fn merged_line(ctx: &Ctx, run: &MergedRun) -> Arc<str> {
  let revision = short_revision(run.merge.revision.as_str()).to_owned();
  let landed = match run.merge.kind {
    MergeKind::MergeCommit => ctx.t_args("merges.merged.mergeCommit", [("revision", revision)]),
    MergeKind::Squash => ctx.t_args("merges.merged.squash", [("revision", revision)]),
    MergeKind::FastForward => ctx.t_args("merges.merged.fastForward", [("revision", revision)]),
    MergeKind::AlreadyMerged => ctx.t_args(
      "merges.merged.alreadyMerged",
      [("branch", run.merge.into.as_str().to_owned()), ("revision", revision)],
    ),
  };
  let mut parts = vec![landed];
  if run.worktree_removed {
    parts.push(ctx.t("merges.merged.worktreeRemoved"));
  }
  if run.branch_deleted {
    parts.push(ctx.t("merges.merged.branchDeleted"));
  }
  match run.refused_before {
    0 => {}
    1 => parts.push(ctx.t("merges.merged.afterOne")),
    count => parts.push(ctx.t_args("merges.merged.afterOther", [("count", count.to_string())])),
  }
  joined(ctx, &parts)
}

/// *Merge queue · 11:02*: who asked for the merge that landed, and when it landed.
pub(super) fn merged_meta(ctx: &Ctx, run: &MergedRun) -> Arc<str> {
  let at = when(ctx, run.merge.merged_at);
  match run.asked_by {
    Some(asked_by) => joined(ctx, &[who(ctx, asked_by), at]),
    None => at,
  }
}

/// A day's heading: *Today*, *Yesterday*, then the weekday and the date.
pub(super) fn day(ctx: &Ctx, day: Timestamp) -> Arc<str> {
  let today = Timestamp::now().local_day_start();
  if day == today {
    return ctx.t("merges.section.today");
  }
  // A day before today's midnight by less than a day and a little (a DST change) is yesterday.
  let yesterday = Timestamp::from_unix_millis(today.unix_millis() - 1).local_day_start();
  if day == yesterday {
    return ctx.t("merges.section.yesterday");
  }
  let (month, date) = display::month_day(day);
  ctx.t_args(
    "merges.section.day",
    [
      (
        "weekday",
        ctx.t_ns(NAMESPACE, WEEKDAY_KEYS[display::weekday(day)]).to_string(),
      ),
      ("month", ctx.t_ns(NAMESPACE, MONTH_KEYS[month]).to_string()),
      ("day", date.to_string()),
    ],
  )
}
