//! What a row says in words: its state line in its tone and the glyph in front of it (`State` `DB295`, `Status`
//! `kDZnw` of `01 · Review` `i6S42`), its files, and when its task came to its group.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::time::Timestamp;
use orchester_workspace::{display, tasks::review_list::ReviewState};

use crate::{screens::tasks::age, ui::icon::Icon};

/// A row's state line.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Phrase {
  pub words: Arc<str>,
  pub color: PaletteColor,
  pub glyph: Icon,
  pub glyph_color: PaletteColor,
}

impl Phrase {
  pub fn of(ctx: &Ctx, state: &ReviewState) -> Self {
    let (keys, color, glyph) = match state {
      ReviewState::CommentsNotSent { .. } => (
        ("review.state.commentsNotSent.one", "review.state.commentsNotSent.other"),
        PaletteColor::Info,
        eye(),
      ),
      ReviewState::ChangedSinceReview { .. } => (
        ("review.state.changed", "review.state.changed"),
        PaletteColor::Warning,
        eye(),
      ),
      ReviewState::Ready => (("review.state.ready", "review.state.ready"), PaletteColor::Info, eye()),
      ReviewState::Conflicts { .. } => (
        ("review.state.conflicts.named", "review.state.conflicts.unnamed"),
        PaletteColor::Danger,
        (Icon::GitCompareArrows, PaletteColor::Danger),
      ),
      ReviewState::ImplementOn { .. } => (
        ("review.state.implementOn.one", "review.state.implementOn.other"),
        PaletteColor::TextSecondary,
        (Icon::LoaderCircle, PaletteColor::TextMuted),
      ),
      ReviewState::ResolvingConflicts { .. } => (
        ("review.state.resolving.named", "review.state.resolving.unnamed"),
        PaletteColor::TextSecondary,
        (Icon::LoaderCircle, PaletteColor::TextMuted),
      ),
      ReviewState::Merged { .. } => (
        ("review.state.merged.named", "review.state.merged.unnamed"),
        PaletteColor::Success,
        (Icon::GitMerge, PaletteColor::Success),
      ),
      ReviewState::Merging { .. } => (
        ("review.state.merging.named", "review.state.merging.unnamed"),
        PaletteColor::TextSecondary,
        (Icon::GitMerge, PaletteColor::TextSecondary),
      ),
    };
    Self {
      words: words(ctx, keys, state),
      color,
      glyph: glyph.0,
      glyph_color: glyph.1,
    }
  }
}

/// The glyph of a run that waits for the person's look.
const fn eye() -> (Icon, PaletteColor) {
  (Icon::Eye, PaletteColor::Info)
}

/// The line's words by its two keys, named in full: the words for one thing and for any other count, the words with
/// the base branch's name and as the run recorded none, or (both the same) its only words.
fn words(ctx: &Ctx, (first, second): (&str, &str), state: &ReviewState) -> Arc<str> {
  match state {
    ReviewState::CommentsNotSent { count } => count_words(ctx, (first, second), *count),
    ReviewState::ImplementOn { comments } => count_words(ctx, (first, second), *comments),
    ReviewState::Conflicts { base, .. }
    | ReviewState::ResolvingConflicts { base }
    | ReviewState::Merged { base }
    | ReviewState::Merging { base } => match base {
      Some(base) => ctx.t_args(first, [("base", base.to_string())]),
      None => ctx.t(second),
    },
    ReviewState::ChangedSinceReview { .. } | ReviewState::Ready => ctx.t(first),
  }
}

/// The words for `count` things: `one`'s for one, `other`'s for any other count.
fn count_words(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> Arc<str> {
  let key = if count == 1 { one } else { other };
  ctx.t_args(key, [("count", count.to_string())])
}

/// `files` files changed.
pub(super) fn files(ctx: &Ctx, files: u32) -> Arc<str> {
  let form = if files == 1 { "one" } else { "other" };
  ctx.t_args(&format!("review.files.{form}"), [("count", files.to_string())])
}

/// When the task came to its group: the clock time today, how long ago on another day.
pub(super) fn since(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let now = Timestamp::now();
  if at.local_day_start() == now.local_day_start() {
    return Arc::from(display::clock(at));
  }
  age(ctx, now, at)
}
