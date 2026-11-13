//! Values of the Pipelines hub (`Pipelines · v2` `pCzIS`) that are its own: why the pipelines could not be read, a
//! pipeline's slug, the fade of the bones and the list's width in a narrow window. The rest of the hub is drawn with
//! the Agents hub's roles (`HubText`, `HubSpacing`).
use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// The Pipelines page's list rows and step cards in bones: the first two whole, then fading (`Item bone` `vaAc2`, `Step
/// bone` `FfeE8` in P5 `TF9O3`).
pub const PIPELINES_BONE_FADE: [f32; 4] = [1.0, 1.0, 0.7, 0.45];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelinesText {
  /// 12 `text_secondary`: the store's own words for why the pipelines couldn't be read (`Reason` `v3szq` in P9
  /// `H4fLj`, `Pipelines · v2`).
  ReadReason,
  /// Geist Mono 12 `text_primary`, 1.3: a pipeline's slug in its settings (`Slug` `tb7tA` in P17 `LzGvn`,
  /// `Pipelines · v2`).
  Slug,
}

impl PipelinesText {
  pub const ALL: [Self; 2] = [Self::ReadReason, Self::Slug];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ReadReason => "pipelines_read_reason",
      Self::Slug => "pipelines_slug",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::ReadReason => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Slug => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
    }
  }
}

/// The Pipelines hub's list in a window under the `xl` breakpoint (1280 logical), where the board's 440 would leave
/// the panel too narrow for its rows, cards and bar: the list gives way first (PV2-17; no design frame below the 1440
/// board; the list's rows cut their lines with an ellipsis).
pub const PIPELINE_HUB_LIST_NARROW: f32 = 260.0;

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  PipelinesText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
/// v1nZD's dashed outline; explicit segments preserve dashes in lurq's SVG tessellator.
pub fn slot_outline() -> &'static str {
  r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="74" viewBox="0 0 240 74"><path d="M0.5 8 A7.5 7.5 0 0 1 8 0.5 M232 0.5 A7.5 7.5 0 0 1 239.5 8 M239.5 66 A7.5 7.5 0 0 1 232 73.5 M8 73.5 A7.5 7.5 0 0 1 0.5 66 M8 0.5h4 M8 73.5h4 M16 0.5h4 M16 73.5h4 M24 0.5h4 M24 73.5h4 M32 0.5h4 M32 73.5h4 M40 0.5h4 M40 73.5h4 M48 0.5h4 M48 73.5h4 M56 0.5h4 M56 73.5h4 M64 0.5h4 M64 73.5h4 M72 0.5h4 M72 73.5h4 M80 0.5h4 M80 73.5h4 M88 0.5h4 M88 73.5h4 M96 0.5h4 M96 73.5h4 M104 0.5h4 M104 73.5h4 M112 0.5h4 M112 73.5h4 M120 0.5h4 M120 73.5h4 M128 0.5h4 M128 73.5h4 M136 0.5h4 M136 73.5h4 M144 0.5h4 M144 73.5h4 M152 0.5h4 M152 73.5h4 M160 0.5h4 M160 73.5h4 M168 0.5h4 M168 73.5h4 M176 0.5h4 M176 73.5h4 M184 0.5h4 M184 73.5h4 M192 0.5h4 M192 73.5h4 M200 0.5h4 M200 73.5h4 M208 0.5h4 M208 73.5h4 M216 0.5h4 M216 73.5h4 M224 0.5h4 M224 73.5h4 M0.5 8v4 M239.5 8v4 M0.5 16v4 M239.5 16v4 M0.5 24v4 M239.5 24v4 M0.5 32v4 M239.5 32v4 M0.5 40v4 M239.5 40v4 M0.5 48v4 M239.5 48v4 M0.5 56v4 M239.5 56v4 M0.5 64v4 M239.5 64v4" fill="none" stroke="black" stroke-width="1"/></svg>"#
}
