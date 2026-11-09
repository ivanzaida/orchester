//! *Changed since your review* on the review page (Review v2, `Review - v2` 05 `e6KEO`): when the run's branch has
//! commits after the head the person reviewed, a `warning_muted` strip under the status line (`Since your review`
//! `miJaX`) says how many and when they reviewed, how many of their comments sit on lines those commits changed, and
//! switches the diff between *All changes* and *Since your review*. The choice is kept per task while the window lives
//! (`ProjectFeed::all_changes`); *Since your review* is the one shown first. Where the diff since the review cannot be
//! made (the branch was rewritten, git failed), its reason stands in the diff's place and *All changes* still shows
//! the whole diff.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{project::TaskKey, revision::Revision};
use orchester_workspace::{
  display::clock,
  messages::Message,
  tasks::{
    changes::{Checkpointed, ReadChanges},
    review_threads::page::ReviewPage,
  },
};

use super::{Env, read::SinceRead};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, IconSize, ReviewText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::Focus,
    state::{EmptyState, loading},
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// What the diff column shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Shown {
  /// The run's whole diff.
  Whole,
  /// The diff since the review is being read.
  Reading,
  Since(Arc<ReadChanges>),
  /// The commits since the review leave the files as they were.
  Nothing,
  /// The diff since the review is larger than Orchester reads: its files.
  TooLarge(u32),
  /// Why there is no diff since the review.
  Failed(Message),
}

impl Shown {
  /// What `read` of the diff since the review makes of the column; `None`: the whole diff.
  pub fn of(read: Option<SinceRead>) -> Self {
    match read {
      None => Self::Whole,
      Some(SinceRead::Reading) => Self::Reading,
      Some(SinceRead::Failed(reason)) => Self::Failed(reason),
      Some(SinceRead::Ready(read)) => match &*read {
        Checkpointed::Read { changes, .. } if !changes.diff.files().is_empty() => Self::Since(changes.clone()),
        Checkpointed::TooLarge { size, .. } => Self::TooLarge(size.files),
        Checkpointed::Read { .. } | Checkpointed::NoCheckpoint => Self::Nothing,
      },
    }
  }

  /// How many files the diff shown changes, and the lines it adds and removes; `None` while it is not known.
  pub fn size(&self, page: &ReviewPage) -> Option<(usize, u32, u32)> {
    match self {
      Self::Whole => super::head::size(&page.changes),
      Self::Since(changes) => Some((
        changes.diff.files().len(),
        changes.diff.additions(),
        changes.diff.deletions(),
      )),
      Self::Reading | Self::Nothing | Self::TooLarge(_) | Self::Failed(_) => None,
    }
  }
}

/// The head that was reviewed, when the run's branch has commits after it.
pub(super) fn moved(page: &ReviewPage) -> Option<&Revision> {
  let reviewed = page.reviewed.as_ref()?;
  (page.review.head.as_ref() != Some(reviewed) && !page.since_review.is_empty()).then_some(reviewed)
}

/// Whether the review of `key` shows the diff since the review, which it does unless the person chose *All changes*.
pub(super) fn wanted(project: &ShownProject, key: &TaskKey) -> bool {
  !project.feed.all_changes.get().contains(key)
}

/// What stands in the diff's place while it is not the diff shown.
pub(super) fn instead(ctx: &Ctx, shown: &Shown) -> Option<Element> {
  match shown {
    Shown::Whole | Shown::Since(_) => None,
    Shown::Reading => Some(loading("review-since-reading", &ctx.t("review_page.changed.reading")).into()),
    Shown::Nothing => Some(
      EmptyState::new(
        "review-since-nothing",
        Icon::History,
        ctx.t("review_page.changed.nothing"),
        ctx.t("review_page.changed.nothingBody"),
      )
      .wide()
      .into(),
    ),
    Shown::TooLarge(files) => Some(
      EmptyState::new(
        "review-since-too-large",
        Icon::FileDiff,
        ctx.t("review_page.changed.tooLarge"),
        ctx.t_args("review_page.changed.tooLargeBody", [("files", files.to_string())]),
      )
      .wide()
      .into(),
    ),
    Shown::Failed(reason) => Some(
      EmptyState::new(
        "review-since-failed",
        Icon::History,
        ctx.t("review_page.changed.since"),
        message_text(ctx, reason),
      )
      .wide()
      .warning()
      .into(),
    ),
  }
}

/// The strip, when the branch moved after the head reviewed; a merge that conflicts shows no strip (rule 10).
pub(super) fn strip(ctx: &Ctx, env: &Env) -> Option<Element> {
  if env.page.conflicts.is_some() {
    return None;
  }
  moved(&env.page)?;
  let showing_all = matches!(env.shown, Shown::Whole);
  let key = env.page.run.key.clone();
  let segment = |id: &str, label: &str, all: bool| {
    let (feed, key) = (env.project.feed.clone(), key.clone());
    Segment::new(
      id.to_owned(),
      SegmentFace::Text(ctx.t(label)),
      showing_all == all,
      move || {
        feed.all_changes.update(|shown| shown.choose(&key, all));
      },
    )
  };
  let switch = segmented(
    "review-since-switch",
    [
      segment("review-since-all", "review_page.changed.all", true),
      segment("review-since-only", "review_page.changed.since", false),
    ],
    Focus::Keyboard,
  );
  Some(
    Row::new()
      .id("review-since")
      .describe("view", if showing_all { "all" } else { "since" })
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::WarningMuted)
      .rounded(RadiusSize::Md)
      .child(icon(Icon::History, IconSize::Md, PaletteColor::Warning))
      .child(
        Text::new(&words(ctx, env))
          .id("review-since-text")
          .variant(ReviewText::Strip.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(switch)
      .into(),
  )
}

/// *2 commits since you reviewed at 12:31. 1 of your comments is on a line they changed.*
fn words(ctx: &Ctx, env: &Env) -> Arc<str> {
  let (count, at) = (env.page.since_review.len(), env.page.reviewed_at);
  let args = |time: Option<String>| {
    let mut args = vec![("count", count.to_string())];
    args.extend(time.map(|time| ("time", time)));
    args
  };
  let commits = match (count == 1, at) {
    (true, Some(at)) => ctx.t_args("review_page.changed.sinceOne", args(Some(clock(at)))),
    (true, None) => ctx.t("review_page.changed.sinceOneUntimed"),
    (false, Some(at)) => ctx.t_args("review_page.changed.sinceOther", args(Some(clock(at)))),
    (false, None) => ctx.t_args("review_page.changed.sinceOtherUntimed", args(None)),
  };
  let outdated: usize = env.counts.values().map(|counts| counts.outdated).sum();
  let comments = match outdated {
    0 => return commits,
    1 => ctx.t("review_page.changed.outdatedOne"),
    many => ctx.t_args("review_page.changed.outdatedOther", [("count", many.to_string())]),
  };
  ctx.t_args(
    "review_page.changed.strip",
    [("commits", commits.to_string()), ("comments", comments.to_string())],
  )
}
