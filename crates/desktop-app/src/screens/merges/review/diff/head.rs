//! A file's head in the run's diff (`File head` `HEoFr` in 09i): 38 high on `surface_raised` between `border` lines, 16
//! at its sides and its parts 10 apart: the chevron that folds and unfolds the file, its path in Geist Mono 12, its
//! added and removed lines in `text_muted`, its tags (*2 comments*, *1 question* on `info_muted`, *1 outdated* on
//! `surface_raised`), then *Viewed* at its end. A hunk's heading (`Hunk` `TvvGo`): 24 high on `code_bg`, its range in
//! Geist Mono 12 `text_muted`, 16 from the side. A note says what a file's diff does not show as lines.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::super::{
  Env,
  actions::{Kept, Write},
  tag::{TagLook, tag},
  words::counted,
};
use crate::{
  screens::task::changes::diff::rows::FileNote,
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    toggle::{Toggle, ToggleKind},
  },
};

/// Heights of a file's head and a hunk's heading (`HEoFr`, `TvvGo`).
const HEAD_HEIGHT: f32 = 38.0;
const HUNK_HEIGHT: f32 = 24.0;

/// The head of file `file` of the diff, `width` wide.
pub(super) fn file_head(ctx: &mut Ctx, env: &Env, file: usize, width: Dimension) -> Element {
  let Some(diff) = env.diff.files().get(file) else {
    return Element::new();
  };
  let path: std::sync::Arc<str> = diff.path().into();
  let viewed = env.viewed.contains(diff.path());
  let folded = env.state.is_folded(&path, viewed);
  let counts = env.counts.get(&path).copied().unwrap_or_default();
  let id = format!("review-file-{file}");
  let info = TagLook::new(PaletteColor::InfoMuted, PaletteColor::Info);
  let mut tags: Vec<Element> = Vec::new();
  if counts.comments > 0 {
    let words = counted(
      ctx,
      ("review_page.diff.commentsOne", "review_page.diff.commentsOther"),
      counts.comments,
    );
    tags.push(tag(&format!("{id}-comments"), &words, info.clone()));
  }
  if counts.questions > 0 {
    let words = counted(
      ctx,
      ("review_page.diff.questionsOne", "review_page.diff.questionsOther"),
      counts.questions,
    );
    tags.push(tag(&format!("{id}-questions"), &words, info));
  }
  if counts.outdated > 0 {
    let words = ctx.t_args("review_page.diff.outdated", [("count", counts.outdated.to_string())]);
    let look = TagLook::new(PaletteColor::SurfaceRaised, PaletteColor::TextMuted);
    tags.push(tag(&format!("{id}-outdated"), &words, look));
  }
  let fold = {
    let (state, path) = (env.state.clone(), path.clone());
    pressable(move || state.fold(path.clone(), !folded))
      .id(format!("{id}-fold"))
      .describe("folded", folded.to_string())
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(icon(
        if folded { Icon::ChevronRight } else { Icon::ChevronDown },
        IconSize::Md,
        PaletteColor::TextMuted,
      ))
      .child(
        Text::new(diff.path())
          .id(format!("{id}-path"))
          .variant(TypographyStyle::Mono)
          .nowrap(),
      )
      .child(
        Text::new(&ctx.t_args(
          "review_page.diff.counts",
          [
            ("added", diff.additions().to_string()),
            ("removed", diff.deletions().to_string()),
          ],
        ))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
      )
      .with_children(tags)
  };
  Row::new()
    .id(id.clone())
    .width(width)
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(fold)
    .child(viewed_box(ctx, env, (&id, path), viewed))
    .into()
}

/// *Viewed*: marks the file at the head shown, or takes the mark off.
fn viewed_box(ctx: &Ctx, env: &Env, (id, path): (&str, std::sync::Arc<str>), viewed: bool) -> Element {
  let (writes, head) = (env.writes.clone(), env.head.clone());
  Toggle::new(
    format!("{id}-viewed"),
    ToggleKind::Checkbox,
    viewed,
    ctx.t("review_page.diff.viewed"),
  )
  .label_style(TypographyStyle::Description)
  .on_change(move |on| {
    writes.run(Write::Keep(Kept::Viewed(head.clone(), path.to_string(), on)));
  })
  .into()
}

/// A hunk's heading: `@@ -40,9 +40,15 @@ impl RetryPolicy`.
pub(super) fn hunk(ctx: &Ctx, (old, new, section): (&str, &str, &str), width: Dimension) -> Element {
  let text = if section.is_empty() {
    ctx.t_args("review_page.diff.hunk", [("old", old), ("new", new)])
  } else {
    ctx.t_args(
      "review_page.diff.hunkSection",
      [("old", old), ("new", new), ("section", section)],
    )
  };
  Row::new()
    .id("review-hunk")
    .width(width)
    .height(Dimension::Px(HUNK_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .align_items(Alignment::Center)
    .background(ExtraColor::CodeBackground.role())
    .child(
      Text::new(&text)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// What a file's diff does not show as lines.
pub(super) fn note(ctx: &Ctx, shown: &FileNote, width: Dimension) -> Element {
  let (glyph, text) = match shown {
    FileNote::Binary => (Icon::Info, ctx.t("review_page.diff.binary")),
    FileNote::Mode { old, new } => (
      Icon::Info,
      ctx.t_args("review_page.diff.mode", [("old", &**old), ("new", &**new)]),
    ),
    FileNote::Unchanged { from } => (
      Icon::Info,
      ctx.t_args("review_page.diff.unchanged", [("path", &**from)]),
    ),
    FileNote::Empty => (Icon::Info, ctx.t("review_page.diff.empty")),
    FileNote::Omitted(lines) => (
      Icon::Scissors,
      ctx.t_args("review_page.diff.omitted", [("count", lines.to_string())]),
    ),
  };
  Row::new()
    .id("review-note")
    .width(width)
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(&text).variant(TypographyStyle::Description).nowrap())
    .into()
}
