//! The status line of an approved run's review (`Status` `H6er4v` in `Review - v2` 07 `eidig`): a 14 `git-merge` and
//! *Merged into master* in 13/500 `success`, then who approved it and when, how it landed and when, in 12 `text_muted`:
//! *approved by you 10:58 · merge commit 4f2a91c · 11:02*. While the merge queue still has the run, *Merging into
//! master* with a loader and what the queue does. Whoever approved it is told only as far as Orchester recorded it: a
//! merge automation made, or one recorded without its attempt, says so rather than *approved by you*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::integration::MergeKind;
use orchester_workspace::{
  display::{clock, short_revision},
  tasks::review_threads::approved_run::{Approval, ApprovedRun, Landed},
};

use super::Env;
use crate::{
  screens::merges::words::joined,
  theme::{ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The status line of the approved run `approved`.
pub(super) fn status(ctx: &Ctx, env: &Env, approved: &ApprovedRun) -> Element {
  let base = approved
    .landed
    .as_ref()
    .map(|landed| landed.into.as_str())
    .or_else(|| env.page.run.base.as_ref().map(|base| base.as_str()));
  let (glyph, words, name) = match (&approved.landed, base) {
    (Some(_), Some(base)) => (
      Icon::GitMerge,
      ctx.t_args("review_page.approved.merged", [("branch", base)]),
      "merged",
    ),
    (Some(_), None) => (Icon::GitMerge, ctx.t("review_page.approved.mergedNoBase"), "merged"),
    (None, Some(base)) => (
      Icon::LoaderCircle,
      ctx.t_args("review_page.approved.merging", [("branch", base)]),
      "merging",
    ),
    (None, None) => (
      Icon::LoaderCircle,
      ctx.t("review_page.approved.mergingNoBase"),
      "merging",
    ),
  };
  Row::new()
    .id("review-status")
    .describe("status", name)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, PaletteColor::Success))
    .child(
      Text::new(&words)
        .id("review-status-text")
        .variant(ExtraTypography::Strong.role())
        .color(PaletteColor::Success)
        .nowrap(),
    )
    .child(
      Text::new(&meta(ctx, approved))
        .id("review-status-meta")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// *approved by you 10:58 · merge commit 4f2a91c · 11:02*; while the queue has the run, *approved by you 10:58 · the
/// merge queue merges it in its turn*.
fn meta(ctx: &Ctx, approved: &ApprovedRun) -> Arc<str> {
  let mut parts = vec![match approved.approval {
    Approval::Person(at) => ctx.t_args("review_page.approved.by", [("time", clock(at))]),
    Approval::Automatic => ctx.t("review_page.approved.automatic"),
    Approval::NotRecorded => ctx.t("review_page.approved.notRecorded"),
  }];
  match &approved.landed {
    Some(landed) => {
      parts.push(landed_as(ctx, landed));
      parts.push(clock(landed.at).into());
    }
    None => parts.push(ctx.t("review_page.approved.queue")),
  }
  joined(ctx, &parts)
}

/// How the merge landed: *merge commit 4f2a91c*, *fast-forward to 4f2a91c*, *squashed as 4f2a91c*, or *already in
/// master at 4f2a91c*.
fn landed_as(ctx: &Ctx, landed: &Landed) -> Arc<str> {
  let revision = short_revision(landed.revision.as_str()).to_owned();
  match landed.kind {
    MergeKind::MergeCommit => ctx.t_args("review_page.approved.mergeCommit", [("revision", revision)]),
    MergeKind::Squash => ctx.t_args("review_page.approved.squash", [("revision", revision)]),
    MergeKind::FastForward => ctx.t_args("review_page.approved.fastForward", [("revision", revision)]),
    MergeKind::AlreadyMerged => ctx.t_args(
      "review_page.approved.alreadyMerged",
      [("branch", landed.into.as_str().to_owned()), ("revision", revision)],
    ),
  }
}
