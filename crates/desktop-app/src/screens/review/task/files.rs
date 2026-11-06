//! The run's files beside its diff (`Files` `qCQrx` in 09i): 240 wide, its head (*Files* in 13/600, their count, and
//! *3 of 7 viewed* at its end), then a 30-high row per file, 10 at its sides: a mark (*Viewed*'s check in `success`, a
//! file with open threads `info`'s speech mark, else the file's own), its name in 12, and its added and removed lines
//! in Geist Mono 11 in `success` and `danger`. The chosen file's row is on `hover` in `text_primary`; choosing one
//! scrolls the diff to it and unfolds it. A row's tooltip names the file's path and its threads (*2 comments · 1
//! question*).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::diff::Diff;

use super::{Env, ThreadCounts, state::Held, words::counted};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, file_icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// The list's width and a file's row (`qCQrx`, `niUdh`).
pub(super) const FILES_WIDTH: f32 = 240.0;
pub(super) const ROW_HEIGHT: f32 = 30.0;

/// The list of `diff`'s files.
pub(super) fn files(ctx: &mut Ctx, env: &Env, diff: &Diff) -> Element {
  let total = diff.files().len();
  let viewed = diff
    .files()
    .iter()
    .filter(|file| env.viewed.contains(file.path()))
    .count();
  let chosen = env
    .state
    .chosen
    .get()
    .0
    .or_else(|| diff.files().first().map(|file| Arc::from(file.path())));
  let head = head(ctx, (viewed, total));
  let rows: Vec<Element> = diff
    .files()
    .iter()
    .enumerate()
    .map(|(at, file)| {
      let path: Arc<str> = file.path().into();
      let shown = Shown {
        chosen: chosen.as_deref() == Some(file.path()),
        viewed: env.viewed.contains(file.path()),
        counts: env.counts.get(&path).copied().unwrap_or_default(),
        lines: (file.additions(), file.deletions()),
      };
      row(ctx, env, (at, path), &shown)
    })
    .collect();
  Column::new()
    .id("review-files")
    .width(Dimension::Px(FILES_WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(head)
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Tight.role())
          .with_children(rows),
      )
      .id("review-files-list")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0)),
    )
    .into()
}

/// *Files* with their count, and *3 of 7 viewed* at the end.
fn head(ctx: &Ctx, (viewed, total): (usize, usize)) -> Row {
  let muted = |id: &str, words: &str| {
    Text::new(words)
      .id(id.to_owned())
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap()
  };
  let count = ctx.t_args(
    "review_page.files.viewed",
    [("viewed", viewed.to_string()), ("total", total.to_string())],
  );
  Row::new()
    .id("review-files-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role())
        .bottom(ExtraSpacing::Snug.role()),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("review_page.files.title"))
        .variant(ExtraTypography::FormSection.role())
        .nowrap(),
    )
    .child(muted("review-files-count", &total.to_string()))
    .child(Row::new().flex(1.0))
    .child(muted("review-files-viewed", &count))
}

/// What a file's row shows.
struct Shown {
  chosen: bool,
  viewed: bool,
  counts: ThreadCounts,
  /// Its added and removed lines.
  lines: (u32, u32),
}

fn row(ctx: &mut Ctx, env: &Env, (at, path): (usize, Arc<str>), shown: &Shown) -> Element {
  let id = format!("review-files-{at}");
  let open = shown.counts.comments + shown.counts.questions > 0;
  let (glyph, color) = if shown.viewed {
    (Icon::CircleCheck, PaletteColor::Success)
  } else if open {
    (Icon::MessageSquare, PaletteColor::Info)
  } else {
    (file_icon(&path), PaletteColor::TextMuted)
  };
  let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
  let (added, removed) = shown.lines;
  let state = env.state.clone();
  let chosen = path.clone();
  let mut row = pressable(move || {
    state.fold(chosen.clone(), false);
    state.chosen.set(Held(Some(chosen.clone())));
  })
  .id(id.clone())
  .describe("chosen", shown.chosen.to_string())
  .describe("viewed", shown.viewed.to_string())
  .width(Dimension::Pct(100.0))
  .height(Dimension::Px(ROW_HEIGHT))
  .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
  .spacing(SpacingSize::Sm)
  .align_items(Alignment::Center)
  .rounded(RadiusSize::Sm)
  .child(icon(glyph, IconSize::Base, color))
  .child(
    Text::new(&name)
      .id(format!("{id}-name"))
      .variant(TypographyStyle::Description)
      .color(if shown.chosen {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      })
      .nowrap()
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0)),
  )
  .child(Row::new().flex(1.0))
  .child(
    Text::new(&ctx.t_args("review_page.files.added", [("count", added.to_string())]))
      .variant(ExtraTypography::MonoNote.role())
      .color(PaletteColor::Success)
      .nowrap(),
  )
  .child(
    Text::new(&ctx.t_args("review_page.files.removed", [("count", removed.to_string())]))
      .variant(ExtraTypography::MonoNote.role())
      .color(PaletteColor::Danger)
      .nowrap(),
  );
  if shown.chosen {
    row = row.background(ExtraColor::Hover.role());
  }
  Tooltip::new(format!("{id}-tip"), tip(ctx, &path, shown.counts)).wrap(ctx, row)
}

/// The file's path, and its threads when it has some: *crates/webhooks/src/retry.rs · 2 comments · 1 question*.
fn tip(ctx: &Ctx, path: &str, counts: ThreadCounts) -> Arc<str> {
  let mut parts: Vec<Arc<str>> = vec![path.into()];
  if counts.comments > 0 {
    parts.push(counted(
      ctx,
      ("review_page.diff.commentsOne", "review_page.diff.commentsOther"),
      counts.comments,
    ));
  }
  if counts.questions > 0 {
    parts.push(counted(
      ctx,
      ("review_page.diff.questionsOne", "review_page.diff.questionsOther"),
      counts.questions,
    ));
  }
  Arc::from(parts.join(&*ctx.t("review_page.separator")))
}
