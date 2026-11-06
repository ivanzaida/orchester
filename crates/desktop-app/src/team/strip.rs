//! One line of a team subject's state on its page (`docs/design/team-authority.md`, "Notes for the implementation": the
//! claim line, the module's Work and delivery lines and the authority-lost lines are one strip pattern; `LR58S` in 01c,
//! `hv1Lu` in 02d, `DDQVE` in 02h `xUwYS`, `H1zbi` in 04 `MvniR`): a 14 glyph, 10 from a title in 12.5 `text_primary`
//! over its detail in 12 `text_secondary`, 2 apart, and an action at its end; 10 over and under it and 12 at its sides,
//! radius 6. A plain strip has a 1 `border` outline; an info, success, warning or danger one is on that tone's muted
//! fill, without an outline (`04f` `Wd4G1`).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// How a strip stands out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StripTone {
  /// A `border` outline: a healthy claim, a branch, a delivery not asked for.
  #[default]
  Plain,
  /// On `info_muted`: a delivery the person asked for that waits for an Admin (`Q8rdf` in 04f).
  Info,
  /// On `success_muted`: a delivery an Admin or Automatic merge authorized (`GObEK`, `p0f3TI` in 04f).
  Success,
  /// On `warning_muted`: a claim that is stale or not renewed; a delivery that waits for the person's authorization
  /// (`L3ve0` in 04f).
  Warning,
  /// On `danger_muted`: authority lost.
  Danger,
}

pub struct Strip {
  id: Arc<str>,
  glyph: (Icon, PaletteColor),
  tone: StripTone,
  title: Arc<str>,
  detail: Arc<str>,
  action: Option<Element>,
}

impl Strip {
  /// The strip `id` (its title `<id>-title`, its detail `<id>-detail`) saying `title` over `detail` beside `glyph`.
  pub fn new(id: &str, glyph: (Icon, PaletteColor), title: impl Into<Arc<str>>, detail: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      glyph,
      tone: StripTone::default(),
      title: title.into(),
      detail: detail.into(),
      action: None,
    }
  }

  pub fn tone(mut self, tone: StripTone) -> Self {
    self.tone = tone;
    self
  }

  /// What the strip ends in.
  pub fn action(mut self, action: impl Into<Element>) -> Self {
    self.action = Some(action.into());
    self
  }
}

impl From<Strip> for Element {
  fn from(strip: Strip) -> Self {
    let id = &strip.id;
    let row = Row::new()
      .id(id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md);
    let row = match strip.tone {
      StripTone::Plain => row.border_inside(BorderSize::Sm, PaletteColor::Border),
      StripTone::Info => row.background(PaletteColor::InfoMuted),
      StripTone::Success => row.background(PaletteColor::SuccessMuted),
      StripTone::Warning => row.background(PaletteColor::WarningMuted),
      StripTone::Danger => row.background(PaletteColor::DangerMuted),
    };
    let tone = match strip.tone {
      StripTone::Plain => "plain",
      StripTone::Info => "info",
      StripTone::Success => "success",
      StripTone::Warning => "warning",
      StripTone::Danger => "danger",
    };
    row
      .describe("tone", tone)
      .child(icon(strip.glyph.0, IconSize::Md, strip.glyph.1))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&strip.title)
              .id(format!("{id}-title"))
              .variant(HubText::TeamLine.role())
              .color(PaletteColor::TextPrimary)
              .width(Dimension::Pct(100.0)),
          )
          .child(
            Text::new(&strip.detail)
              .id(format!("{id}-detail"))
              .variant(TypographyStyle::Description)
              .width(Dimension::Pct(100.0)),
          ),
      )
      .with_children(strip.action)
      .into()
  }
}
