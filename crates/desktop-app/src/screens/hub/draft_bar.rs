//! An opened item's draft bar (`Draft bar` `W0sRO`): one bar on every editor of the hub — what it is now (*Draft v5 ·
//! saved 16:40 · Based on Published v4*, *Draft written by automation · Published v3*, *Published v1*) over what
//! changed or what to do next, then Discard draft and Publish. A question about the draft opens in its place.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// Where the item stands, which the bar's glyph says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Standing {
  /// A draft saved, not published yet: a dashed circle in `warning` (`IFCep`).
  Draft,
  /// The draft is older than the latest version, or cannot be published: a warning.
  Attention,
  /// Nothing to publish: the latest version is current.
  Published,
  /// Items being chosen, e.g. what an import creates (`F3ukWF` in 02c).
  Choosing,
}

impl Standing {
  fn glyph(self) -> (Icon, PaletteColor) {
    match self {
      Self::Draft => (Icon::CircleDashed, PaletteColor::Warning),
      Self::Attention => (Icon::TriangleAlert, PaletteColor::Warning),
      Self::Published => (Icon::CircleCheck, PaletteColor::Success),
      Self::Choosing => (Icon::ListChecks, PaletteColor::TextSecondary),
    }
  }
}

pub(crate) struct DraftBar {
  id: Arc<str>,
  standing: Standing,
  status: Arc<str>,
  sub: Option<Arc<str>>,
  actions: Vec<Element>,
}

impl DraftBar {
  pub(crate) fn new(id: impl Into<Arc<str>>, standing: Standing, status: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      standing,
      status: status.into(),
      sub: None,
      actions: Vec::new(),
    }
  }

  pub(crate) fn sub(mut self, sub: impl Into<Arc<str>>) -> Self {
    self.sub = Some(sub.into());
    self
  }

  pub(crate) fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }
}

impl From<DraftBar> for Element {
  fn from(bar: DraftBar) -> Self {
    let (glyph, color) = bar.standing.glyph();
    Row::new()
      .id(bar.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Sm)
          .bottom(SpacingSize::Sm)
          .right(SpacingSize::Sm)
          .left(HubSpacing::Row.role()),
      )
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .describe("standing", format!("{:?}", bar.standing).to_lowercase())
      .child(icon(glyph, IconSize::Md, color).id(format!("{}-icon", bar.id)))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Hair.role())
          .child(
            Text::new(&bar.status)
              .id(format!("{}-status", bar.id))
              .variant(HubText::BarStatus.role())
              .width(Dimension::Pct(100.0)),
          )
          .with_children(bar.sub.map(|sub| {
            Text::new(&sub)
              .id(format!("{}-sub", bar.id))
              .variant(HubText::Hint.role())
              .width(Dimension::Pct(100.0))
          })),
      )
      .with_children(bar.actions)
      .into()
  }
}
