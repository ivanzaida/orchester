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
// Instructions empty state glyph yxIJ7.
const INSTRUCTIONS_GLYPH_TILE: f32 = 40.0;
const EXPLANATION_WIDTH: f32 = 620.0;
/// Explanation of GE9ao/ImhCU in Secrets.
const SECRETS_EXPLANATION_WIDTH: f32 = 640.0;

pub struct EmptyState {
  id: Arc<str>,
  glyph: Icon,
  title: Arc<str>,
  explanation: Arc<str>,
  actions: Vec<Element>,
  more: Vec<Element>,
  wide: bool,
  compact: bool,
  secrets: bool,
  measure: Option<f32>,
  unframed: bool,
  note: Option<Arc<str>>,
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
      wide: false,
      compact: false,
      secrets: false,
      measure: None,
      unframed: false,
      note: None,
    }
  }

  /// Full-width explanation, as the Tools state panel (`yUrNX`) uses.
  pub fn wide(mut self) -> Self {
    self.wide = true;
    self
  }

  /// Compact Tools access rules variant (`b8yzr`): 20 inside, 12 apart, 13/12 text.
  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  /// The Secrets empty panels (GE9ao/ImhCU) use a wider explanation and six-pixel text gap.
  pub fn secrets(mut self) -> Self {
    self.secrets = true;
    self
  }

  /// The widest the explanation runs, where a design sets its own (520 in the Overview's `F7LdL`).
  pub fn measure(mut self, width: f32) -> Self {
    self.measure = Some(width);
    self
  }

  /// The bare Instructions empty state sG2xi, with its repository findings below.
  pub fn unframed(mut self) -> Self {
    self.unframed = true;
    self
  }

  /// A supporting note under the explanation, before the actions.
  pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
    self.note = Some(note.into());
    self
  }

  /// The widest the explanation runs: all of the card when wide, else the design's own measure.
  fn explanation_width(&self) -> Dimension {
    if self.wide {
      Dimension::Auto
    } else if let Some(measure) = self.measure {
      Dimension::Px(measure)
    } else if self.secrets {
      Dimension::Px(SECRETS_EXPLANATION_WIDTH)
    } else {
      Dimension::Px(EXPLANATION_WIDTH)
    }
  }

  fn glyph_tile(&self) -> Row {
    let tile_size = if self.unframed {
      INSTRUCTIONS_GLYPH_TILE
    } else {
      GLYPH_TILE
    };
    Row::new()
      .width(Dimension::Px(tile_size))
      .height(Dimension::Px(tile_size))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::Tile.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(self.glyph, IconSize::Xl, PaletteColor::TextSecondary))
      .id(format!("{}-glyph", self.id))
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
    let tile = empty.glyph_tile();
    let text = Column::new()
      .width(if empty.wide {
        Dimension::Pct(100.0)
      } else {
        Dimension::Auto
      })
      .spacing(if empty.secrets || empty.unframed {
        ExtraSpacing::Snug.role()
      } else {
        SpacingSize::Xs
      })
      .child(
        Text::new(&empty.title)
          .id(format!("{}-title", empty.id))
          .variant(if empty.compact {
            ExtraTypography::Strong.role()
          } else {
            TypographyStyle::Title
          }),
      )
      .child(
        Text::new(&empty.explanation)
          .width(if empty.wide {
            Dimension::Pct(100.0)
          } else {
            Dimension::Auto
          })
          .variant(if empty.compact {
            TypographyStyle::Description
          } else {
            ExtraTypography::Lead.role()
          })
          .max_width(empty.explanation_width()),
      );
    let text = text.with_children(empty.note.map(|note| {
      Text::new(&note)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
    }));
    let actions = (!empty.actions.is_empty()).then(|| {
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(empty.actions)
    });
    let frame = Column::new()
      .id(empty.id.clone())
      .width(Dimension::Pct(100.0))
      .spacing(if empty.compact {
        SpacingSize::Md
      } else {
        ExtraSpacing::Inset.role()
      });
    let frame = if empty.unframed {
      frame
    } else {
      frame
        .padding(if empty.compact {
          ExtraSpacing::Tabs.role()
        } else {
          ExtraSpacing::Roomy.role()
        })
        .background(PaletteColor::SurfacePanel)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Lg)
    };
    frame
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
  let again = ActionButton::new(format!("{id}-retry"), ButtonKind::Primary, ctx.t("common.tryAgain"))
    .icon(Icon::RotateCcw)
    .on_click(retry);
  Notice::new(id, NoticeTone::Danger, title)
    .body(body)
    .action(again)
    .into()
}
