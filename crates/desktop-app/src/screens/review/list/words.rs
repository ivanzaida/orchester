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
    let (key, color, glyph) = match state {
      ReviewState::CommentsNotSent { .. } => ("review.state.commentsNotSent", PaletteColor::Info, eye()),
      ReviewState::ChangedSinceReview { .. } => ("review.state.changed", PaletteColor::Warning, eye()),
      ReviewState::Ready => ("review.state.ready", PaletteColor::Info, eye()),
      ReviewState::Conflicts { .. } => (
        "review.state.conflicts",
        PaletteColor::Danger,
        (Icon::GitCompareArrows, PaletteColor::Danger),
      ),
      ReviewState::ImplementOn { .. } => (
        "review.state.implementOn",
        PaletteColor::TextSecondary,
        (Icon::LoaderCircle, PaletteColor::TextMuted),
      ),
      ReviewState::ResolvingConflicts { .. } => (
        "review.state.resolving",
        PaletteColor::TextSecondary,
        (Icon::LoaderCircle, PaletteColor::TextMuted),
      ),
      ReviewState::Merged { .. } => (
        "review.state.merged",
        PaletteColor::Success,
        (Icon::GitMerge, PaletteColor::Success),
      ),
      ReviewState::Merging { .. } => (
        "review.state.merging",
        PaletteColor::TextSecondary,
        (Icon::GitMerge, PaletteColor::TextSecondary),
      ),
    };
    Self {
      words: words(ctx, key, state),
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

/// The line's words: `key`'s own, `key.named` with the base branch's name or, as the run recorded none, `key.unnamed`.
fn words(ctx: &Ctx, key: &str, state: &ReviewState) -> Arc<str> {
  match state {
    ReviewState::CommentsNotSent { count } => count_words(ctx, key, *count),
    ReviewState::ImplementOn { comments } => count_words(ctx, key, *comments),
    ReviewState::Conflicts { base, .. }
    | ReviewState::ResolvingConflicts { base }
    | ReviewState::Merged { base }
    | ReviewState::Merging { base } => match base {
      Some(base) => ctx.t_args(&format!("{key}.named"), [("base", base.to_string())]),
      None => ctx.t(&format!("{key}.unnamed")),
    },
    ReviewState::ChangedSinceReview { .. } | ReviewState::Ready => ctx.t(key),
  }
}

/// `key`'s words for `count` things, which the key's `.one` and `.other` tell apart.
fn count_words(ctx: &Ctx, key: &str, count: usize) -> Arc<str> {
  let form = if count == 1 { "one" } else { "other" };
  ctx.t_args(&format!("{key}.{form}"), [("count", count.to_string())])
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
