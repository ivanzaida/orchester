//! Values of the Pipelines screens (`Pipelines · redesign` `evQ4U`): the page's sides and top (`Header` `Jf1Gu`,
//! `Body` `mifhW`), the flow's words (`On task start` `xXtB2`, `Run in parallel` `kgGb2`) and the step heading's
//! number (`o06df`).
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelinesText {
  /// Geist Mono 10.5 `text_muted`: *ON TASK START* and *RUN IN PARALLEL* in the flow (`zPT6x`, `zOhok`).
  FlowLabel,
  /// Geist Mono 11.5 `text_muted`: a step card's number (`V1JR5`).
  CardNumber,
  /// Geist Mono 13 `text_muted`: the chosen step's number beside its name (`o06df`).
  StepNumber,
  /// 12/500 `text_secondary`: the version on the header's status line (`J4YQR`).
  Status,
}

impl PipelinesText {
  pub const ALL: [Self; 4] = [Self::FlowLabel, Self::CardNumber, Self::StepNumber, Self::Status];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FlowLabel => "pipelines_flow_label",
      Self::CardNumber => "pipelines_card_number",
      Self::StepNumber => "pipelines_step_number",
      Self::Status => "pipelines_status",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::FlowLabel => mono(style(10.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::CardNumber => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::StepNumber => mono(style(13.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::Status => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelinesSpacing {
  /// 20: above the header and the body (`Jf1Gu`, `mifhW`).
  PageTop,
  /// 21: a flow edge's line in from the cards' start, under their numbers (`p9sTT`).
  EdgeIndent,
}

impl PipelinesSpacing {
  pub const ALL: [Self; 2] = [Self::PageTop, Self::EdgeIndent];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PageTop => "pipelines_page_top",
      Self::EdgeIndent => "pipelines_edge_indent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PageTop => 20.0,
      Self::EdgeIndent => 21.0,
    }
  }
}

pub fn install(theme: &Theme) {
  for spacing in PipelinesSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  PipelinesText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
/// v1nZD's dashed outline; explicit segments preserve dashes in lurq's SVG tessellator.
pub fn slot_outline() -> &'static str {
  r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="74" viewBox="0 0 240 74"><path d="M0.5 8 A7.5 7.5 0 0 1 8 0.5 M232 0.5 A7.5 7.5 0 0 1 239.5 8 M239.5 66 A7.5 7.5 0 0 1 232 73.5 M8 73.5 A7.5 7.5 0 0 1 0.5 66 M8 0.5h4 M8 73.5h4 M16 0.5h4 M16 73.5h4 M24 0.5h4 M24 73.5h4 M32 0.5h4 M32 73.5h4 M40 0.5h4 M40 73.5h4 M48 0.5h4 M48 73.5h4 M56 0.5h4 M56 73.5h4 M64 0.5h4 M64 73.5h4 M72 0.5h4 M72 73.5h4 M80 0.5h4 M80 73.5h4 M88 0.5h4 M88 73.5h4 M96 0.5h4 M96 73.5h4 M104 0.5h4 M104 73.5h4 M112 0.5h4 M112 73.5h4 M120 0.5h4 M120 73.5h4 M128 0.5h4 M128 73.5h4 M136 0.5h4 M136 73.5h4 M144 0.5h4 M144 73.5h4 M152 0.5h4 M152 73.5h4 M160 0.5h4 M160 73.5h4 M168 0.5h4 M168 73.5h4 M176 0.5h4 M176 73.5h4 M184 0.5h4 M184 73.5h4 M192 0.5h4 M192 73.5h4 M200 0.5h4 M200 73.5h4 M208 0.5h4 M208 73.5h4 M216 0.5h4 M216 73.5h4 M224 0.5h4 M224 73.5h4 M0.5 8v4 M239.5 8v4 M0.5 16v4 M239.5 16v4 M0.5 24v4 M239.5 24v4 M0.5 32v4 M239.5 32v4 M0.5 40v4 M239.5 40v4 M0.5 48v4 M239.5 48v4 M0.5 56v4 M239.5 56v4 M0.5 64v4 M239.5 64v4" fill="none" stroke="black" stroke-width="1"/></svg>"#
}
