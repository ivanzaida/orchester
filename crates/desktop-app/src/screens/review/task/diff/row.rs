//! One row of the run's diff, mounted by the virtualised list: a file's head, a hunk, a line (which keeps whether the
//! pointer is over it, for its **+**), a note, a block the page draws (a thread, a fold, the comment box) or the room
//! for the scrollbar. A row draws again when what it shows changed: a line when the chosen lines did, a head when the
//! files' marks did, a block when the review did.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  core::Signal,
  node::{Element, dimension::Dimension},
};

use super::{
  head::{file_head, hunk, note},
  line::line,
  rows::ReviewRow,
};
use crate::screens::review::task::{Env, thread};

#[derive(Clone)]
pub(super) struct RowProps {
  pub row: ReviewRow,
  /// The widest row of the diff or the pane, whichever is wider: every line's tint reaches across.
  pub width: f32,
  /// The pane's own width, which a block takes.
  pub view_width: f32,
  pub env: Arc<Env>,
}

impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.row == other.row
      && self.width == other.width
      && self.view_width == other.view_width
      && self.env.version == other.env.version
  }
}

impl DevtoolsInspectable for RowProps {}

pub(super) struct RowView {
  hovered: Signal<bool>,
}

impl Component for RowView {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RowProps {
      row,
      width,
      view_width,
      env,
    } = ctx.props::<RowProps>().clone();
    let width = Dimension::Px(width);
    match row {
      ReviewRow::Head { file, .. } => file_head(ctx, &env, file, width),
      ReviewRow::Hunk { old, new, section, .. } => hunk(ctx, (&old, &new, &section), width),
      ReviewRow::Line { file, line: shown, .. } => line(ctx, (&env, file), &shown, (width, &self.hovered)),
      ReviewRow::Note { note: shown, .. } => note(ctx, &shown, width),
      ReviewRow::Block { key, .. } => Row::new()
        .width(Dimension::Px(view_width))
        .child(thread::block(ctx, &env, &key))
        .into(),
      ReviewRow::End { .. } => {
        let scrollbar = ctx.theme().scrollbar().clone();
        let height = scrollbar.width + 2.0 * scrollbar.resolved_edge_inset();
        Row::new()
          .id("review-diff-end")
          .width(width)
          .height(Dimension::Px(height))
          .into()
      }
    }
  }
}
