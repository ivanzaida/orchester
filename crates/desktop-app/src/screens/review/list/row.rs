//! A task's row (`KON-18` `s590ow`, `KON-20` `E9MhFk`, `KON-22` `UvwdB` of `01 · Review` `i6S42`): 40 high, 8 at its
//! sides, its cells 12 apart, radius 6, the `hover` fill under the pointer. The status glyph in its tone, the key (mono
//! 11.5 `text_muted`, 52 wide), the title in 13/500 with the task's tag in 11.5 `text_muted`, the state line in its
//! tone, then in a 150 wide cell, at its end, how many comments are open (a 12 `message-square` and mono 11), the files
//! and the lines added and removed (mono 11 `text_muted`, `success`, `danger`) and the clock time the task came to its
//! group. Under the pointer the small Review button takes the counts' place. The row and its button open the task's
//! review.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, project::TaskKey};

use super::words::Phrase;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::Press,
    tooltip::Truncated,
  },
};

/// The row's height and its cells' widths (`s590ow`): key 52, the counts or the button 150, the time 36.
pub(super) const ROW_HEIGHT: f32 = 40.0;
const KEY_WIDTH: f32 = 52.0;
const COUNTS_WIDTH: f32 = 150.0;
const TIME_WIDTH: f32 = 36.0;

/// What a task's row shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RowProps {
  pub project: ProjectId,
  pub key: TaskKey,
  pub title: Arc<str>,
  pub tag: Option<Arc<str>>,
  pub phrase: Phrase,
  /// The run's comment threads that are not resolved.
  pub comments: usize,
  /// What the run changes, in words: its files, the lines added, the lines removed; none when it cannot be read.
  pub size: Option<(Arc<str>, Arc<str>, Arc<str>)>,
  /// When the task came to its group.
  pub time: Arc<str>,
}

/// The row of `props`, with the pointer over it or not.
pub(super) fn row(ctx: &mut Ctx, props: &RowProps, hovered: &Signal<bool>) -> Element {
  let key = props.key.to_string();
  let open = Navigation::of(ctx).to(Route::Project(
    props.project,
    ProjectPage::TaskReview(props.key.clone()),
  ));
  let trailing = if hovered.get() {
    ActionButton::new(
      format!("review-open-{key}"),
      ButtonKind::Secondary,
      ctx.t("review.open"),
    )
    .icon(Icon::Eye)
    .small()
    .opens(open.clone())
    .into()
  } else {
    counts(ctx, props, &key)
  };
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(props.phrase.glyph, IconSize::Md, props.phrase.glyph_color.clone()).id(format!("review-status-{key}")))
    .child(
      Row::new().width(Dimension::Px(KEY_WIDTH)).flex_shrink(0.0).child(
        Text::new(&key)
          .id(format!("review-key-{key}"))
          .variant(ExtraTypography::MonoKey.role())
          .nowrap(),
      ),
    )
    .child(title(props, &key, ctx))
    .child(
      Truncated::new(
        format!("review-state-{key}"),
        props.phrase.words.clone(),
        TypographyStyle::Description,
      )
      .color(props.phrase.color.clone())
      .render(ctx),
    )
    .child(cell(COUNTS_WIDTH, Justify::End).child(trailing))
    .child(
      cell(TIME_WIDTH, Justify::End).child(
        Text::new(&props.time)
          .id(format!("review-time-{key}"))
          .variant(ExtraTypography::MonoNote.role())
          .nowrap(),
      ),
    );
  let (enter, leave) = (hovered.clone(), hovered.clone());
  Press::Open(open)
    .button()
    .id(format!("review-row-{key}"))
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .on_mouse_enter(move || enter.set(true))
    .on_mouse_leave(move || leave.set(false))
    .child(line)
    .into()
}

/// A cell `width` wide, its content placed by `justify`.
fn cell(width: f32, justify: Justify) -> Row {
  Row::new()
    .width(Dimension::Px(width))
    .flex_shrink(0.0)
    .min_width(Dimension::Px(0.0))
    .justify(justify)
    .align_items(Alignment::Center)
}

/// The title, cut to its room, and the task's tag as a plain word after it.
fn title(props: &RowProps, key: &str, ctx: &mut Ctx) -> Row {
  Row::new()
    .id(format!("review-title-{key}"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Truncated::new(
        format!("review-title-text-{key}"),
        props.title.clone(),
        ExtraTypography::Strong.role(),
      )
      .render(ctx),
    )
    .with_children(props.tag.as_ref().map(|tag| {
      Text::new(tag)
        .id(format!("review-tag-{key}"))
        .variant(ExtraTypography::Note.role())
        .nowrap()
        .flex_shrink(0.0)
    }))
}

/// What the run changes: the open comments, the files and the lines added and removed.
fn counts(ctx: &Ctx, props: &RowProps, key: &str) -> Element {
  let mono = |id: String, text: &str, color: PaletteColor| {
    Text::new(text)
      .id(id)
      .variant(ExtraTypography::MonoNote.role())
      .color(color)
      .nowrap()
  };
  let comments = (props.comments > 0).then(|| {
    Row::new()
      .id(format!("review-comments-{key}"))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .describe(
        "a11y_name",
        ctx.t_args("review.comments", [("count", props.comments.to_string())]),
      )
      .child(icon(Icon::MessageSquare, IconSize::Sm, PaletteColor::TextMuted))
      .child(mono(
        format!("review-comments-count-{key}"),
        &props.comments.to_string(),
        PaletteColor::TextMuted,
      ))
  });
  let size = props.size.as_ref().map(|(files, added, removed)| {
    [
      mono(format!("review-files-{key}"), files, PaletteColor::TextMuted),
      mono(format!("review-added-{key}"), added, PaletteColor::Success),
      mono(format!("review-removed-{key}"), removed, PaletteColor::Danger),
    ]
  });
  Row::new()
    .id(format!("review-counts-{key}"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(comments)
    .with_children(size.into_iter().flatten())
    .into()
}
