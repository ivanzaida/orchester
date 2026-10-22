//! The head the Orchestrator chat (`Header` `nKHIi`) and its settings (`Head` `b8gGj`) share, so switching between
//! them moves neither the breadcrumb, the title nor the content's left edge: 24 over and 40 at its sides (the sides
//! of Settings and Project settings, `ui::settings_page`), 12 apart: the breadcrumb; the title (20/600) over what the
//! page says under it, with the page's actions at their right from the same top; then the page's own lines. Under the
//! head, each page keeps its content 40 in from its sides ([`OrchestratorSpacing::PageSide`]).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::OrchestratorSpacing;

/// A page's head: its breadcrumb, title, actions and lines.
pub(super) struct PageHead {
  id: &'static str,
  crumbs: Element,
  title: (&'static str, Arc<str>),
  sub: Option<Element>,
  actions: Option<Element>,
  lines: Vec<Element>,
  ruled: bool,
}

impl PageHead {
  /// The head with the element id `id`: `crumbs` over the title `text`, whose element id is `title`.
  pub(super) fn new(id: &'static str, crumbs: impl Into<Element>, (title, text): (&'static str, Arc<str>)) -> Self {
    Self {
      id,
      crumbs: crumbs.into(),
      title: (title, text),
      sub: None,
      actions: None,
      lines: Vec::new(),
      ruled: false,
    }
  }

  /// What the page says under its title, beside the actions.
  pub(super) fn sub(mut self, sub: impl Into<Element>) -> Self {
    self.sub = Some(sub.into());
    self
  }

  /// What sits at the right of the title.
  pub(super) fn actions(mut self, actions: Option<Element>) -> Self {
    self.actions = actions;
    self
  }

  /// A line under the title and its actions.
  pub(super) fn line(mut self, line: impl Into<Element>) -> Self {
    self.lines.push(line.into());
    self
  }

  /// Ends the head on a `border` line 12 under its last line.
  pub(super) fn ruled(mut self) -> Self {
    self.ruled = true;
    self
  }
}

impl From<PageHead> for Element {
  fn from(head: PageHead) -> Element {
    let (title_id, title) = head.title;
    let titled = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&title)
          .id(title_id)
          .variant(TypographyStyle::Heading)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .with_children(head.sub);
    let title_row = Row::new()
      .id(format!("{}-title-row", head.id))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Start)
      .child(titled)
      .with_children(head.actions);
    let mut padding = Padding::new()
      .top(SpacingSize::Xl)
      .left(OrchestratorSpacing::PageSide.role())
      .right(OrchestratorSpacing::PageSide.role());
    if head.ruled {
      padding = padding.bottom(SpacingSize::Md);
    }
    let column = Column::new()
      .id(head.id)
      .width(Dimension::Pct(100.0))
      .padding(padding)
      .spacing(SpacingSize::Md)
      .child(head.crumbs)
      .child(title_row)
      .with_children(head.lines);
    if head.ruled {
      column
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .into()
    } else {
      column.into()
    }
  }
}
