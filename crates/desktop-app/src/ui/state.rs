//! What a list or a panel shows instead of its content. Empty (`Empty project` `LeIOm`): a card,
//! `surface_panel` in a `border` stroke, radius 10, 28 inside, its parts 14 apart: a 36 tile (radius 8,
//! `surface_raised` in a `border` stroke) with an 18 `text_secondary` glyph, the title in 15/600 with its
//! explanation 4 under it in 12.5/1.5 `text_secondary`, then the actions 8 apart. Loading (`Reading`
//! `O8pgq`): the turning 14 loader and what is being read in 12 `text_muted`, 8 apart, centered with 56
//! over and under them. A failure to read (`Tasks cannot be read` `DNeq9`) is a raised danger notice
//! with its Try again; loading and failure never look like an empty list (handover §5.6).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
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
  /// The glyph says something is wrong: in `warning` (`07 · Browser not found` `YEv2c`).
  warning: bool,
  title: Arc<str>,
  explanation: Arc<str>,
  actions: Vec<Element>,
  more: Vec<Element>,
  wide: bool,
  compact: bool,
  secrets: bool,
  measure: Option<f32>,
  unframed: bool,
  centered: bool,
  gap: Option<SpacingSize>,
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
      warning: false,
      title: title.into(),
      explanation: explanation.into(),
      actions: Vec::new(),
      more: Vec::new(),
      wide: false,
      compact: false,
      secrets: false,
      measure: None,
      unframed: false,
      centered: false,
      gap: None,
      note: None,
    }
  }

  /// Full-width explanation, as the Tools state panel (`yUrNX`) uses.
  pub fn wide(mut self) -> Self {
    self.wide = true;
    self
  }

  /// The glyph in `warning`: what the state says is wrong (`07 · Browser not found` `YEv2c`).
  pub fn warning(mut self) -> Self {
    self.warning = true;
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

  /// Every part centered under the glyph, the explanation's lines too (`04 · Modules · none yet` `yJdJ9`), the
  /// explanation's top 25 under the title's.
  pub fn centered(mut self) -> Self {
    self.centered = true;
    self
  }

  /// Its parts `gap` apart where the design sets its own (16 in `yJdJ9`, 12 in the Modules' `jIxzJ`).
  pub fn gap(mut self, gap: SpacingSize) -> Self {
    self.gap = Some(gap);
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
    let color = if self.warning {
      PaletteColor::Warning
    } else {
      PaletteColor::TextSecondary
    };
    sized_tile(&self.id, (self.glyph, color), tile_size)
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

impl EmptyState {
  /// The title over its explanation and the note, each line centered when the state is.
  fn text(&self) -> Column {
    let line = |text: Text| {
      if self.centered {
        text.text_align(TextAlign::Center)
      } else {
        text
      }
    };
    let text = Column::new()
      .width(if self.wide {
        Dimension::Pct(100.0)
      } else {
        Dimension::Auto
      })
      .spacing(if self.centered {
        // The design's 19 high title and 6 put the explanation 25 under it; the app's title line is 20 high.
        ExtraSpacing::Pair.role()
      } else if self.secrets || self.unframed {
        ExtraSpacing::Snug.role()
      } else {
        SpacingSize::Xs
      })
      .child(line(Text::new(&self.title).id(format!("{}-title", self.id)).variant(
        if self.compact {
          ExtraTypography::Strong.role()
        } else {
          TypographyStyle::Title
        },
      )))
      .child(line(
        Text::new(&self.explanation)
          .width(if self.wide {
            Dimension::Pct(100.0)
          } else {
            Dimension::Auto
          })
          .variant(if self.compact {
            TypographyStyle::Description
          } else {
            ExtraTypography::Lead.role()
          })
          .max_width(self.explanation_width()),
      ))
      .with_children(self.note.as_ref().map(|note| {
        line(
          Text::new(note)
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        )
      }));
    if self.centered {
      text.align_items(Alignment::Center)
    } else {
      text
    }
  }
}

impl From<EmptyState> for Element {
  fn from(empty: EmptyState) -> Self {
    let tile = empty.glyph_tile();
    let text = empty.text();
    let centered = empty.centered;
    let actions = (!empty.actions.is_empty()).then(|| {
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(empty.actions)
    });
    let frame =
      Column::new()
        .id(empty.id.clone())
        .width(Dimension::Pct(100.0))
        .spacing(match (empty.gap, empty.compact) {
          (Some(gap), _) => gap,
          (None, true) => SpacingSize::Md,
          (None, false) => ExtraSpacing::Inset.role(),
        });
    let frame = if centered {
      frame.align_items(Alignment::Center)
    } else {
      frame
    };
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

/// An empty state's glyph tile (`q4tTW`), for a state laid out on its own (`X0l2I6` in Project memory).
pub fn glyph_tile(id: &str, glyph: Icon) -> Row {
  sized_tile(id, (glyph, PaletteColor::TextSecondary), GLYPH_TILE)
}

fn sized_tile(id: &str, (glyph, color): (Icon, PaletteColor), size: f32) -> Row {
  Row::new()
    .width(Dimension::Px(size))
    .height(Dimension::Px(size))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Xl, color))
    .id(format!("{id}-glyph"))
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

/// Reading failed: a raised danger notice saying `title` and `body` with a secondary Try again under the
/// words at its end (`Notice · danger` `b3htnP` in `DNeq9`), which runs `retry`.
pub fn load_error(
  ctx: &Ctx,
  id: &str,
  (title, body): (&str, &str),
  retry: impl Fn() + Send + Sync + 'static,
) -> Element {
  let again = ActionButton::new(format!("{id}-retry"), ButtonKind::Secondary, ctx.t("common.tryAgain"))
    .icon(Icon::RotateCcw)
    .on_click(retry);
  Notice::new(id, NoticeTone::Danger, title)
    .body(body)
    .raised()
    .action(again)
    .into()
}
