//! An answered decision folded into one quiet line, like a tool line (`Answered line · You answered` `F16aNE` in
//! 05c `hcN1X`): a 13 icon, who answered in 12 `text_secondary`, what was chosen in 12/500 `text_primary`, the question
//! and the time in 12 `text_muted`, and a 13 chevron, 6 apart. A press opens the card as it was; opened, the chevron
//! points down (`gUdeo`). A waiting card folded with ✕ is the same line (`KNcxH`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::{
  theme::{CardsText, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The icon at the start of the line, by who answered or why nobody did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
  /// A person chose: a `success` check (`QAvTi`).
  Chosen,
  /// The Orchestrator answered (`W5TRmd`).
  Orchestrator,
  /// Automation answered through Orchester's MCP server (`aF2rJ`).
  Automation,
  /// The run ended before an answer (`NQ5Y8`).
  Ended,
  /// The agent stopped waiting when the run was interrupted (`xlTR0`).
  Stopped,
  /// The card still waits; it was folded with ✕ (`qr9ur`).
  Waiting,
}

impl Mark {
  fn glyph(self) -> (Icon, PaletteColor) {
    match self {
      Self::Chosen => (Icon::Check, PaletteColor::Success),
      Self::Orchestrator => (Icon::MessagesSquare, PaletteColor::TextMuted),
      Self::Automation => (Icon::Plug, PaletteColor::TextMuted),
      Self::Ended => (Icon::Clock3, PaletteColor::TextMuted),
      Self::Stopped => (Icon::CirclePause, PaletteColor::TextMuted),
      Self::Waiting => (Icon::MessageCircleQuestion, PaletteColor::Warning),
    }
  }
}

/// What the line says: who answered, what was chosen, and the question with the time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnsweredLine {
  mark: Mark,
  lead: Arc<str>,
  chosen: Option<Arc<str>>,
  detail: Arc<str>,
}

impl AnsweredLine {
  /// A line with `mark`, `lead` (*You allowed once*), what was `chosen` and the `detail` after it
  /// (*· Design tools still missing · 15:24*).
  pub fn new(mark: Mark, lead: impl Into<Arc<str>>, chosen: Option<Arc<str>>, detail: impl Into<Arc<str>>) -> Self {
    Self {
      mark,
      lead: lead.into(),
      chosen,
      detail: detail.into(),
    }
  }

  /// *You answered* `chosen`.
  pub fn you(ctx: &Ctx, chosen: impl Into<Arc<str>>, detail: impl Into<Arc<str>>) -> Self {
    Self::new(Mark::Chosen, ctx.t("cards.answered.you"), Some(chosen.into()), detail)
  }

  /// *The Orchestrator answered* `chosen`.
  pub fn orchestrator(ctx: &Ctx, chosen: impl Into<Arc<str>>, detail: impl Into<Arc<str>>) -> Self {
    Self::new(
      Mark::Orchestrator,
      ctx.t("cards.answered.orchestrator"),
      Some(chosen.into()),
      detail,
    )
  }

  /// *Automation answered* `chosen`.
  pub fn automation(ctx: &Ctx, chosen: impl Into<Arc<str>>, detail: impl Into<Arc<str>>) -> Self {
    Self::new(
      Mark::Automation,
      ctx.t("cards.answered.automation"),
      Some(chosen.into()),
      detail,
    )
  }

  /// *Not answered*, with `mark` ([`Mark::Ended`] or [`Mark::Stopped`]) saying why.
  pub fn not_answered(ctx: &Ctx, mark: Mark, detail: impl Into<Arc<str>>) -> Self {
    Self::new(mark, ctx.t("cards.answered.notAnswered"), None, detail)
  }

  /// The line `id` ending in `chevron`; a press runs `toggle`.
  pub(super) fn render(&self, id: &str, chevron: Icon, toggle: impl Fn() + Send + Sync + 'static) -> Element {
    let (glyph, color) = self.mark.glyph();
    pressable(toggle)
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .describe("a11y_name", self.lead.clone())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(glyph, IconSize::Base, color).id(format!("{id}-mark")))
          .child(
            Text::new(&self.lead)
              .id(format!("{id}-lead"))
              .variant(CardsText::Lead.role())
              .nowrap(),
          )
          .with_children(self.chosen.as_ref().map(|chosen| {
            Text::new(chosen)
              .id(format!("{id}-chosen"))
              .variant(CardsText::Chosen.role())
              .nowrap()
          }))
          .child(
            Text::new(&self.detail)
              .id(format!("{id}-detail"))
              .variant(CardsText::Quiet.role())
              .nowrap()
              .text_overflow(TextOverflow::Elipsis)
              .flex(1.0)
              .min_width(Dimension::Px(0.0)),
          )
          .child(icon(chevron, IconSize::Base, PaletteColor::TextMuted).id(format!("{id}-chevron"))),
      )
      .into()
  }
}
