//! Callout (`Callout · …` in Settings › Orchester, *Connect a server* and Project settings › Sharing: `naoei`,
//! `Gbag9`, `wwtNM`, `tCSh6`): a 16 icon, a title in 13/600 `text_primary` with its explanation 4 under it in 12.5
//! `text_secondary`, 12 apart; radius 10, 14 over and under and 16 at its sides, on its tone's fill with no stroke
//! (the design names a tone stroke without a thickness, which draws none). Information is `info_muted` with an
//! `info` icon (`Gbag9`), a neutral fact `hover` with a `text_muted` icon (`wwtNM`), a warning `warning_muted`
//! (`naoei`), a failure `danger_muted` (`fztvy`) and a success `success_muted` (`ICcO5`), each with its tone's icon.
//! An action goes under the explanation (`Open Orchester Cloud` in `JsrLB`); a quieter line in `text_muted` can follow
//! it (*… didn't answer · 14:31*, `SWg68` in `i8HD8`), and so can parts of its own (*While signed out* `Qkpc2` in
//! `ztigw`).

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalloutTone {
  Info,
  Neutral,
  Warning,
  Danger,
  Success,
}

impl CalloutTone {
  const fn name(self) -> &'static str {
    match self {
      Self::Info => "info",
      Self::Neutral => "neutral",
      Self::Warning => "warning",
      Self::Danger => "danger",
      Self::Success => "success",
    }
  }

  /// The fill and the icon's color.
  fn colors(self) -> (PaletteColor, PaletteColor) {
    match self {
      Self::Info => (PaletteColor::InfoMuted, PaletteColor::Info),
      Self::Neutral => (ExtraColor::Hover.role(), PaletteColor::TextMuted),
      Self::Warning => (PaletteColor::WarningMuted, PaletteColor::Warning),
      Self::Danger => (PaletteColor::DangerMuted, PaletteColor::Danger),
      Self::Success => (PaletteColor::SuccessMuted, PaletteColor::Success),
    }
  }

  /// The icon a callout of this tone shows unless it names its own.
  const fn icon(self) -> Icon {
    match self {
      Self::Info | Self::Neutral => Icon::Info,
      Self::Warning => Icon::TriangleAlert,
      Self::Danger => Icon::CircleAlert,
      Self::Success => Icon::CircleCheck,
    }
  }
}

pub struct Callout {
  id: Arc<str>,
  tone: CalloutTone,
  icon: Icon,
  title: Arc<str>,
  body: Option<Arc<str>>,
  note: Option<Arc<str>>,
  details: Vec<Element>,
  actions: Vec<Element>,
}

impl Callout {
  pub fn new(id: impl Into<Arc<str>>, tone: CalloutTone, title: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      tone,
      icon: tone.icon(),
      title: title.into(),
      body: None,
      note: None,
      details: Vec::new(),
      actions: Vec::new(),
    }
  }

  /// The icon that says what the callout is about, in place of its tone's.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = icon;
    self
  }

  /// What happened and what can be done, under the title.
  pub fn body(mut self, body: impl Into<Arc<str>>) -> Self {
    self.body = Some(body.into());
    self
  }

  /// A quieter line under the explanation, in `text_muted`.
  pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
    self.note = Some(note.into());
    self
  }

  /// A part under the explanation, after the ones added before it.
  pub fn detail(mut self, detail: impl Into<Element>) -> Self {
    self.details.push(detail.into());
    self
  }

  /// An action under the explanation, after the ones added before it.
  pub fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }
}

impl From<Callout> for Element {
  fn from(callout: Callout) -> Self {
    let (fill, mark) = callout.tone.colors();
    let actions = (!callout.actions.is_empty()).then(|| {
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .padding(Padding::new().top(ExtraSpacing::Snug.role()))
        .with_children(callout.actions)
    });
    let text = Column::new()
      .id(format!("{}-text", callout.id))
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&callout.title)
          .id(format!("{}-title", callout.id))
          .variant(ExtraTypography::FormSection.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(callout.body.as_deref().map(|body| {
        Text::new(body)
          .id(format!("{}-body", callout.id))
          .variant(ExtraTypography::Lead.role())
          .width(Dimension::Pct(100.0))
      }))
      .with_children(callout.note.as_deref().map(|note| {
        Text::new(note)
          .id(format!("{}-note", callout.id))
          .variant(ExtraTypography::Lead.role())
          .color(PaletteColor::TextMuted)
          .width(Dimension::Pct(100.0))
      }))
      .with_children(callout.details)
      .with_children(actions);
    Row::new()
      .id(callout.id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .background(fill)
      .rounded(RadiusSize::Lg)
      .describe("tone", callout.tone.name())
      .child(icon(callout.icon, IconSize::Lg, mark).id(format!("{}-icon", callout.id)))
      .child(text)
      .into()
  }
}
