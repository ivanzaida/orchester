//! The head of Orchestrator settings (`Head` `b8gGj`): 24 over and 40 at its sides (the sides of Settings and Project
//! settings, `ui::settings_page`), 12 apart: the breadcrumb; the title (20/600) over what the page says under it, with
//! the page's actions at their right from the same top; then the page's own lines. Under the head, the page keeps its
//! content 40 in from its sides ([`OrchestratorSpacing::PageSide`]).

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, TypographyStyle},
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
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
    Column::new()
      .id(head.id)
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Xl)
          .left(OrchestratorSpacing::PageSide.role())
          .right(OrchestratorSpacing::PageSide.role()),
      )
      .spacing(SpacingSize::Md)
      .child(head.crumbs)
      .child(title_row)
      .with_children(head.lines)
      .into()
  }
}
