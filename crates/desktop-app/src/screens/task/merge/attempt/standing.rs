//! How a run's last merge attempt left it, as the attempt page's status line, *What stopped it* and its primary action
//! tell it (09b to 09l): merged, tried again by Orchester, stopped with files left changed, and each way a merge that
//! did not land ends.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::integration::attempt::{AttemptOutcome, AttemptStage, MergeVia};
use orchester_workspace::{
  display::short_revision,
  tasks::merge_attempts::{page::AttemptPage, read::Retrying},
};

use super::{
  lines::cause,
  words::{silent_for, silent_long, stage_name},
};
use crate::ui::icon::Icon;

/// The cause a preflight's integration lock gives when another operation holds it.
pub(super) const HELD: &str = "held by another operation";

/// How the run stands after its last attempt, as its page and its task page's Merge row tell it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Standing {
  Merged,
  /// Merged into its base branch without Orchester recording a merge, as git proves it; whatever the attempts were,
  /// they are history.
  MergedOutside,
  Retrying(Retrying),
  Conflicts,
  NotRecorded,
  /// Git stopped part way through bringing the merge in; the files still left changed decide what moves it on.
  LeftChanged,
  GitStopped,
  /// The merge queue stopped it: Try again puts it back in its place there.
  QueueStopped,
  /// The merge queue took it back and waits for a person's approval there, where it merges.
  AwaitingApproval,
  Interrupted,
  Moved,
  /// Refused before Git, or at a stage that ends the merge.
  NotMerged,
}

impl Standing {
  /// How `page`'s run stands; `None` while no merge was tried.
  pub(crate) fn of(page: &AttemptPage) -> Option<Self> {
    if page.merge.is_some() {
      return Some(Self::Merged);
    }
    let last = &page.last()?.attempt;
    if page.delivery.merged_into().is_some() {
      return Some(Self::MergedOutside);
    }
    if let Some(retrying) = page.retrying {
      return Some(Self::Retrying(retrying));
    }
    if !page.left_changed.is_empty() {
      return Some(Self::LeftChanged);
    }
    if page.awaiting_approval {
      return Some(Self::AwaitingApproval);
    }
    Some(match last.outcome() {
      AttemptOutcome::Merged { .. } => Self::Merged,
      AttemptOutcome::Conflicts { .. } => Self::Conflicts,
      AttemptOutcome::NotRecorded { .. } => Self::NotRecorded,
      AttemptOutcome::GitStopped { .. } => Self::GitStopped,
      AttemptOutcome::LeftChanged { .. } => Self::LeftChanged,
      AttemptOutcome::Interrupted { .. } => Self::Interrupted,
      AttemptOutcome::Moved { .. } => Self::Moved,
      AttemptOutcome::Stopped { .. } => Self::QueueStopped,
      AttemptOutcome::Refused { .. } if last.via() == MergeVia::MergeQueue => Self::QueueStopped,
      AttemptOutcome::Refused { .. } => Self::NotMerged,
    })
  }

  /// Whether the run's work is in its base branch.
  pub(crate) const fn is_merged(self) -> bool {
    matches!(self, Self::Merged | Self::MergedOutside)
  }

  /// The status line's glyph and color.
  pub(crate) const fn glyph(self) -> (Icon, PaletteColor) {
    match self {
      Self::Merged | Self::MergedOutside => (Icon::CircleCheck, PaletteColor::Success),
      Self::Retrying(_) => (Icon::RefreshCw, PaletteColor::Info),
      Self::AwaitingApproval => (Icon::CirclePause, PaletteColor::Warning),
      Self::Conflicts => (Icon::GitMerge, PaletteColor::Warning),
      Self::LeftChanged => (Icon::TriangleAlert, PaletteColor::Warning),
      _ => (Icon::CircleAlert, PaletteColor::Warning),
    }
  }

  /// The attempt page's status line's glyph and color: a merge that conflicts is `danger` there (13c `zWlkp`).
  pub(super) const fn page_glyph(self) -> (Icon, PaletteColor) {
    match self {
      Self::Conflicts => (Icon::GitCompareArrows, PaletteColor::Danger),
      _ => self.glyph(),
    }
  }

  /// The glyph of the strip under the status line: what Orchester retries is a warning there, as it stopped.
  pub(super) const fn strip_glyph(self) -> (Icon, PaletteColor) {
    match self {
      Self::Retrying(_) => (Icon::CircleAlert, PaletteColor::Warning),
      _ => self.page_glyph(),
    }
  }

  /// The name of the standing in the page's `standing` description.
  pub(super) const fn name(self) -> &'static str {
    match self {
      Self::Merged => "merged",
      Self::MergedOutside => "merged_outside",
      Self::Retrying(_) => "retrying",
      Self::Conflicts => "conflicts",
      Self::NotRecorded => "not_recorded",
      Self::LeftChanged => "left_changed",
      Self::GitStopped => "git_stopped",
      Self::QueueStopped => "queue_stopped",
      Self::AwaitingApproval => "awaiting_approval",
      Self::Interrupted => "interrupted",
      Self::Moved => "moved",
      Self::NotMerged => "not_merged",
    }
  }

  /// The status line's words, but for Orchester's retry, whose line counts down on its own; the task page's top card
  /// is titled with them too (ORC-731).
  pub(crate) fn status(self, ctx: &Ctx, branch: &str) -> Arc<str> {
    let key = match self {
      Self::Merged => "merges.page.status.merged",
      Self::MergedOutside => "merges.page.status.mergedOutside",
      Self::Retrying(_) => "merges.page.status.queueRetries",
      Self::Conflicts => "merges.page.status.conflicts",
      Self::NotRecorded => "merges.page.status.notRecorded",
      Self::LeftChanged => "merges.page.status.leftChanged",
      Self::GitStopped => "merges.page.status.gitStopped",
      Self::QueueStopped => "merges.page.status.queueStopped",
      Self::AwaitingApproval => "merges.page.status.awaitingApproval",
      Self::Interrupted => "merges.page.status.interrupted",
      Self::Moved => "merges.page.status.moved",
      Self::NotMerged => "merges.page.status.notMerged",
    };
    ctx.t_args(key, [("branch", branch)])
  }
}

/// *What stopped it*, or for a merged run *What happened*, in the words of the stage that ended the last attempt.
pub(super) fn explanation(ctx: &Ctx, page: &AttemptPage, standing: Standing, branch: &str) -> Arc<str> {
  if standing == Standing::Merged {
    return merged(ctx, page, branch);
  }
  let Some(last) = page.last() else {
    return ctx.t("merges.outcome.noCause");
  };
  let attempt = &last.attempt;
  let (key, stage, revision) = match (standing, attempt.outcome()) {
    (Standing::Retrying(Retrying { next: Some(_) }), _) => ("merges.page.explain.retrying", None, None),
    (Standing::Retrying(_), _) => ("merges.page.explain.queueRetries", None, None),
    (Standing::AwaitingApproval, _) => ("merges.page.explain.awaitingApproval", None, None),
    (Standing::LeftChanged, _) if page.left_changed.is_empty() => {
      ("merges.page.explain.leftChangedPutBack", None, None)
    }
    (Standing::LeftChanged, _) => return counted(ctx, "leftChanged", page.left_changed.len(), branch),
    (_, AttemptOutcome::Refused { stage }) if attempt.cause() == Some(HELD) => {
      ("merges.page.explain.held", Some(*stage), None)
    }
    (Standing::QueueStopped, AttemptOutcome::Refused { stage }) => {
      ("merges.page.explain.refusedInQueue", Some(*stage), None)
    }
    (
      _,
      AttemptOutcome::Refused {
        stage: stage @ (AttemptStage::Merge | AttemptStage::Record),
      },
    ) => ("merges.page.explain.refusedAt", Some(*stage), None),
    (_, AttemptOutcome::Refused { stage }) => ("merges.page.explain.refusedBeforeGit", Some(*stage), None),
    (_, AttemptOutcome::Stopped { words }) => {
      return ctx.t_args("merges.page.explain.queueStopped", [("cause", words.as_str())]);
    }
    (_, AttemptOutcome::Conflicts { files }) => return counted(ctx, "conflicts", files.len(), branch),
    (_, AttemptOutcome::LeftChanged { files }) => return counted(ctx, "leftChanged", files.len().max(1), branch),
    (_, AttemptOutcome::NotRecorded { revision }) => ("merges.page.explain.notRecorded", None, Some(revision)),
    (_, AttemptOutcome::GitStopped { stage }) => {
      if let Some(seconds) = silent_for(attempt) {
        return ctx.t_args(
          "merges.page.explain.gitSilent",
          [
            ("time", silent_long(ctx, seconds)),
            ("stage", stage_name(ctx, *stage).to_string()),
            ("branch", branch.to_owned()),
          ],
        );
      }
      ("merges.page.explain.gitStopped", Some(*stage), None)
    }
    (_, AttemptOutcome::Interrupted { stage }) => ("merges.page.explain.interrupted", Some(*stage), None),
    (_, AttemptOutcome::Moved { stage: Some(_) }) => ("merges.page.explain.moved", None, None),
    (_, AttemptOutcome::Moved { stage: None }) => ("merges.page.explain.movedBefore", None, None),
    (_, AttemptOutcome::Merged { .. }) => return merged(ctx, page, branch),
  };
  ctx.t_args(
    key,
    [
      ("branch", branch.to_owned()),
      ("cause", cause(ctx, attempt.cause()).to_string()),
      (
        "stage",
        stage
          .map(|stage| stage_name(ctx, stage).to_string())
          .unwrap_or_default(),
      ),
      (
        "revision",
        revision
          .map(|revision| short_revision(revision.as_str()).to_owned())
          .unwrap_or_default(),
      ),
    ],
  )
}

/// The explanation `what` (`conflicts`, `leftChanged`) of `count` files, worded for one or more.
fn counted(ctx: &Ctx, what: &str, count: usize, branch: &str) -> Arc<str> {
  let form = if count == 1 { "One" } else { "Other" };
  ctx.t_args(
    &format!("merges.page.explain.{what}{form}"),
    [("branch", branch.to_owned()), ("count", count.to_string())],
  )
}

/// *What happened* to a merged run: on which attempt it landed, and the attempts before it.
fn merged(ctx: &Ctx, page: &AttemptPage, branch: &str) -> Arc<str> {
  let revision = page
    .merge
    .as_ref()
    .map(|merge| short_revision(merge.revision.as_str()).to_owned())
    .unwrap_or_default();
  let landed = page
    .attempts
    .iter()
    .position(|recorded| recorded.attempt.outcome().landed());
  let Some(at) = landed else {
    return ctx.t_args(
      "merges.page.explain.recorded",
      [("branch", branch.to_owned()), ("revision", revision)],
    );
  };
  let attempt = page.attempts[at].sequence.to_string();
  let before = page.attempts.len() - at - 1;
  let key = match before {
    0 => "merges.page.explain.merged",
    1 => "merges.page.explain.mergedAfterOne",
    _ => "merges.page.explain.mergedAfterOther",
  };
  ctx.t_args(
    key,
    [
      ("branch", branch.to_owned()),
      ("revision", revision),
      ("attempt", attempt),
      ("count", before.to_string()),
    ],
  )
}
