//! What a list or a panel shows instead of its content. Empty (`Empty project` `LeIOm`): a card,
//! `surface_panel` in a `border` stroke, radius 10, 28 inside, its parts 14 apart: a 36 tile (radius 8,
//! `surface_raised` in a `border` stroke) with an 18 `text_secondary` glyph, the title in 15/600 with its
//! explanation 4 under it in 12.5/1.5 `text_secondary`, then the actions 8 apart. Loading (`Reading`
//! `O8pgq`): the turning 14 loader and what is being read in 12 `text_muted`, 8 apart, centered with 56
//! over and under them. A failure to read (`Notice · Tasks cannot be read` `kXWCj`) is a danger notice
//! with its Try again; loading and failure never look like an empty list (handover §5.6).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  button::{ActionButton, ButtonKind},
  icon::{Icon, icon},
  notice::{Notice, NoticeTone},
};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

/// An empty state's glyph tile (`q4tTW`) and the widest its explanation runs (`bKD3G`).
const GLYPH_TILE: f32 = 36.0;
const EXPLANATION_WIDTH: f32 = 620.0;

pub struct EmptyState {
  id: Arc<str>,
  glyph: Icon,
  title: Arc<str>,
  explanation: Arc<str>,
  actions: Vec<Element>,
  more: Vec<Element>,
}

impl EmptyState {
  pub fn new(
    id: impl Into<Arc<str>>,
    glyph: Icon,
    title: impl Into<Arc<str>>,
    explanation: impl Into<Arc<str>>,
  ) -> Self {
    Self {
      id: id.into(),
      glyph,
      title: title.into(),
      explanation: explanation.into(),
      actions: Vec::new(),
      more: Vec::new(),
    }
  }

  /// An action in the row under the explanation.
  pub fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }

  /// More of the card after the actions, e.g. what is ready to run (`Ready` `HiAa2`).
  pub fn child(mut self, part: impl Into<Element>) -> Self {
    self.more.push(part.into());
    self
  }
}

impl From<EmptyState> for Element {
  fn from(empty: EmptyState) -> Self {
    let tile = Row::new()
      .width(Dimension::Px(GLYPH_TILE))
      .height(Dimension::Px(GLYPH_TILE))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::Tile.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(empty.glyph, IconSize::Xl, PaletteColor::TextSecondary))
      .id(format!("{}-glyph", empty.id));
    let text = Column::new()
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&empty.title)
          .id(format!("{}-title", empty.id))
          .variant(TypographyStyle::Title),
      )
      .child(
        Text::new(&empty.explanation)
          .variant(ExtraTypography::Lead.role())
          .max_width(Dimension::Px(EXPLANATION_WIDTH)),
      );
    let actions = (!empty.actions.is_empty()).then(|| {
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(empty.actions)
    });
    Column::new()
      .id(empty.id.clone())
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Roomy.role())
      .spacing(ExtraSpacing::Inset.role())
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .describe("state", "empty")
      .child(tile)
      .child(text)
      .with_children(actions)
      .with_children(empty.more)
      .into()
  }
}

/// The turning loader and `what` is being read, centered in the room it was going to fill.
pub fn loading(id: &str, what: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loading.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .describe("state", "loading")
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextMuted).id(format!("{id}-icon")))
    .child(
      Text::new(what)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
}

/// Reading failed: a danger notice saying `title` and `body` with Try again, which runs `retry`.
pub fn load_error(
  ctx: &Ctx,
  id: &str,
  (title, body): (&str, &str),
  retry: impl Fn() + Send + Sync + 'static,
) -> Element {
  let again = ActionButton::new(format!("{id}-retry"), ButtonKind::Primary, ctx.t("ui.tryAgain"))
    .icon(Icon::RotateCcw)
    .on_click(retry);
  Notice::new(id, NoticeTone::Danger, title)
    .body(body)
    .action(again)
    .into()
}
