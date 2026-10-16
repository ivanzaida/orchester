//! What a pull request's row and its page both say of it: its state (`State` in `aFRGq`, `Status · Open` in
//! `mVTVX`), what reviews decided, its GitHub checks by outcome, and its counts with their digits grouped.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
};
use orchester_core::{
  pull_request::{PullRequest, PullRequestState, ReviewDecision},
  tokens::grouped as group_digits,
};

use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// `value` with its digits grouped as the locale groups them ("38,214"), as Task › Changes writes counts.
pub(super) fn grouped(ctx: &Ctx, value: u32) -> String {
  group_digits(u64::from(value), &ctx.t("pull_requests.detail.groupSeparator"))
}

/// The state's 13 icon and its word in 12/500, 5 apart.
pub(super) fn state(ctx: &Ctx, pull_request: &PullRequest) -> Row {
  use PaletteColor::{Success, TextMuted, TextPrimary, TextSecondary};
  let (glyph, mark, label, key) = match pull_request.state() {
    PullRequestState::Open => (
      Icon::GitPullRequest,
      TextSecondary,
      TextPrimary,
      "pull_requests.state.open",
    ),
    PullRequestState::Draft => (
      Icon::GitPullRequestDraft,
      TextMuted,
      TextSecondary,
      "pull_requests.state.draft",
    ),
    PullRequestState::Merged => (Icon::GitMerge, Success, TextPrimary, "pull_requests.state.merged"),
    PullRequestState::Closed => (
      Icon::GitPullRequestClosed,
      TextMuted,
      TextSecondary,
      "pull_requests.state.closed",
    ),
  };
  Row::new()
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, mark))
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Label)
        .color(label)
        .nowrap(),
    )
}

/// What reviews decided, its icon and words in its tone and `typography`; *No review required* when the
/// repository asks for none.
pub(super) fn review(ctx: &Ctx, pull_request: &PullRequest, typography: TypographyStyle) -> Row {
  use PaletteColor::{Success, TextMuted, TextSecondary, Warning};
  let (glyph, color, key) = match pull_request.review_decision() {
    Some(ReviewDecision::ReviewRequired) => (Some(Icon::Eye), Warning, "pull_requests.review.required"),
    Some(ReviewDecision::Approved) => (Some(Icon::CircleCheck), Success, "pull_requests.review.approved"),
    Some(ReviewDecision::ChangesRequested) => (
      Some(Icon::FileDiff),
      TextSecondary,
      "pull_requests.review.changesRequested",
    ),
    None => (None, TextMuted, "pull_requests.review.none"),
  };
  Row::new()
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .with_children(glyph.map(|glyph| icon(glyph, IconSize::Sm, color.clone())))
    .child(Text::new(&ctx.t(key)).variant(typography).color(color).nowrap())
}

/// The head's checks by outcome, 10 apart, each a 12 icon and its count in 12; `None` when none ran.
pub(super) fn checks(ctx: &Ctx, pull_request: &PullRequest) -> Option<Row> {
  let checks = pull_request.checks();
  if checks.total() == 0 {
    return None;
  }
  use PaletteColor::{Danger, Success, TextMuted, TextSecondary};
  let counts = [
    (
      checks.passed,
      Icon::CircleCheck,
      Success,
      TextSecondary,
      "pull_requests.checks.passed",
    ),
    (
      checks.pending,
      Icon::LoaderCircle,
      TextSecondary,
      TextSecondary,
      "pull_requests.checks.running",
    ),
    (
      checks.failed,
      Icon::CircleX,
      Danger,
      Danger,
      "pull_requests.checks.failed",
    ),
    (
      checks.skipped,
      Icon::CircleMinus,
      TextMuted,
      TextMuted,
      "pull_requests.checks.skipped",
    ),
  ];
  let counts = counts
    .into_iter()
    .filter(|(count, ..)| *count > 0)
    .map(|(count, glyph, mark, color, key)| {
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Sm, mark))
        .child(
          Text::new(&ctx.t_args(key, [("count", count.to_string())]))
            .variant(TypographyStyle::Description)
            .color(color)
            .nowrap(),
        )
    });
  Some(
    Row::new()
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .with_children(counts),
  )
}
