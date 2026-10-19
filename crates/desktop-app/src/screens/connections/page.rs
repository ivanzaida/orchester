//! The frame of Add connection and Add / Edit MCP server, pages in the Connections tab (`Content · Add
//! connection` `VmTji` in `cEJy4`, `M20DGB`): a 720 form centered in a scroll, 28 above it and 40 under it,
//! its parts 28 apart, opened by the page head (`Page head` `ou1ua`: where it is, its title in 20/600 with
//! what it does under it in 12, and its top status at the end); and the action bar pinned under it (`Action
//! bar` `GabxO`: a `border` line over it, 12 over and under, its actions 8 apart at the end of the form's
//! column, and the first reason they cannot be used 6 under them).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  routes::Screen,
  theme::{ExtraSpacing, IconSize, TaskFormSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    unavailable::reason_at_end,
  },
};

/// The form's column (`Form` `QVjmE`), which the action bar's content lines up with (`Inner` `kEsF7`).
pub(super) const FORM_WIDTH: f32 = 720.0;

/// The page head.
pub(super) struct Head {
  pub crumbs: Vec<Crumb>,
  pub title: Arc<str>,
  pub description: Arc<str>,
  /// What the page holds (`Top status` `NwELS`): its icon, color and words.
  pub status: (Icon, PaletteColor, Arc<str>),
}

/// The page of `screen`: the head and `sections` in the scroll, `bar` under it.
pub(super) fn page(ctx: &mut Ctx, screen: Screen, head: Head, sections: Vec<Element>, bar: Element) -> Element {
  let form = Column::new()
    .id("connection-form")
    .width(Dimension::Px(FORM_WIDTH))
    .spacing(ExtraSpacing::Roomy.role())
    .child(head_row(ctx, head))
    .with_children(sections);
  let scrolled = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(TaskFormSpacing::PageTop.role())
        .bottom(TaskFormSpacing::PageBottom.role())
        .left(SpacingSize::Xl)
        .right(SpacingSize::Xl),
    )
    .align_items(Alignment::Center)
    .child(form);
  Column::new()
    .id(format!("screen-{}", screen.name()))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(ScrollVertical::new(scrolled).id("connection-form-scroll").flex(1.0))
    .child(bar)
    .into()
}

fn head_row(ctx: &mut Ctx, head: Head) -> Column {
  let (glyph, color, words) = head.status;
  let status = Row::new()
    .id("connection-form-status")
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("status", words.clone())
    .child(icon(glyph, IconSize::Base, color))
    .child(Text::new(&words).variant(TypographyStyle::Description).nowrap());
  let title = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(&head.title)
            .id("connection-form-title")
            .variant(TypographyStyle::Heading),
        )
        .child(
          Text::new(&head.description)
            .id("connection-form-description")
            .variant(crate::theme::ExtraTypography::Prose.role()),
        ),
    )
    .child(status);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(breadcrumb(
      ctx,
      "connection-form-crumbs",
      CrumbSeparator::Chevron,
      head.crumbs,
    ))
    .child(title)
}

/// `question` in the action bar's place, as wide as the form's column.
pub(super) fn question_bar(question: Element) -> Element {
  Row::new()
    .id("connection-form-question")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(SpacingSize::Xl).right(SpacingSize::Xl))
    .justify(Justify::Center)
    .child(Column::new().width(Dimension::Px(FORM_WIDTH)).child(question))
    .into()
}

/// The action bar: `actions` at the end of the form's column, and `reason` under them when the last
/// cannot be used.
pub(super) fn action_bar(actions: Vec<Element>, reason: Option<&str>) -> Element {
  let right = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End)
    .child(
      Row::new()
        .id("connection-form-actions")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(actions),
    )
    .with_children(reason.map(|reason| reason_at_end("connection-form-reason", reason)));
  Row::new()
    .id("connection-form-bar")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Xl, SpacingSize::Md))
    .justify(Justify::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Px(FORM_WIDTH))
        .justify(Justify::End)
        .child(right),
    )
    .into()
}
