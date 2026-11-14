//! *Follow* or *Lead to the end*, chosen in a /watch picker (`Mode` `GA2bV` in `L03 · Choosing · Follow or Lead to the
//! end` `xE361`, board `Orchestrator autonomy · frames to approve`; `docs/plan/orchestrator-autonomy/OA-10-…`).
//!
//! Above the picker's keys, under a `border` line, 10 over and 8 at its sides and under it: *What it does with them*
//! in 11.5 `text_muted` and, 10 after it, a control of two ways, 26 high in a `border` stroke, radius md, 2 inside
//! and between them. A way is 20 high, radius sm, 9 at its sides, an 11 icon and its name 5 apart: *Follow* with an
//! `eye`, *Lead to the end* with a `flag`, in `text_muted` and 11.5 `text_secondary`; the chosen one on `hover`, its
//! icon and its name (11.5/500) in `text_primary`. 8 under them, one line in 11.5/1.45 `text_muted` says what the
//! chosen way means. Tab switches the way while the picker is open, a press on a way chooses it, and the composer's
//! badge names the way chosen (`ui::slash`). *Follow* is where a command chosen anew starts.
//!
//! Choosing is the person's alone, as applying the command is: leading to the end lets the Orchestrator act without
//! them, so synthetic input (an MCP client's) is refused outside a QA copy (`mcp::qa_person`) and the way stays.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::panel::Picker;
use crate::{
  mcp::qa_person::QaPerson,
  theme::{ControlHeight, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, LeadingSpacing, LeadingText},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
    slash::{Badge, OtherWay},
  },
};

/// How one /watch command takes its targets, as its picker chose: kept for as long as the screen shows, so the
/// picker, the badge and the command's apply read the same choice.
#[derive(Clone)]
pub(in crate::screens::orchestrator) struct LeadChoice {
  lead: Signal<bool>,
  qa_person: QaPerson,
}

impl LeadChoice {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      lead: ctx.signal(false),
      qa_person: QaPerson::of(ctx),
    }
  }

  /// What the command is applied with: whether it leads its targets to the end. The choice goes back to *Follow*,
  /// where the next command starts.
  pub(super) fn take(&self) -> bool {
    let lead = self.lead.get_untracked();
    if lead {
      self.lead.set(false);
    }
    lead
  }

  /// The person chooses to lead to the end (`lead`) or to follow. Synthetic input chooses nothing.
  fn choose(&self, lead: bool) {
    if self
      .qa_person
      .refuses("choose whether the Orchestrator follows what it watches or leads it to the end")
    {
      return;
    }
    if self.lead.get_untracked() != lead {
      self.lead.set(lead);
    }
  }

  /// The command's other way for the composer: its badge reads *Lead to the end* with a `flag` while it is chosen,
  /// and Tab in the open picker switches.
  pub(super) fn other_way(&self, ctx: &Ctx) -> OtherWay {
    let this = self.clone();
    OtherWay {
      chosen: self.lead.clone(),
      badge: Badge {
        icon: Icon::Flag,
        verb: ctx.t("orchestrator.lead.toTheEnd"),
      },
      switch: Arc::new(move || this.choose(!this.lead.get_untracked())),
    }
  }

  /// The row over the keys of composer `id`'s `picker`, named `{id}-{prefix}-mode`: the two ways, and what the chosen
  /// one means. Read tracked, so the picker follows the choice.
  pub(super) fn row(&self, ctx: &Ctx, (id, picker): (&str, Picker)) -> Element {
    let lead = self.lead.get();
    let id = format!("{id}-{}-mode", picker.prefix());
    let way = |leads: bool| {
      let this = self.clone();
      let (name, glyph, words) = if leads {
        ("lead", Icon::Flag, ctx.t("orchestrator.lead.toTheEnd"))
      } else {
        ("follow", Icon::Eye, ctx.t("orchestrator.lead.follow"))
      };
      segment(
        &format!("{id}-{name}"),
        (glyph, &words),
        lead == leads,
        Press::run(move || this.choose(leads)),
      )
    };
    let ways = Row::new()
      .id(format!("{id}-ways"))
      .describe("chosen", if lead { "lead" } else { "follow" })
      .flex_shrink(0.0)
      .height(Dimension::Px(ControlHeight::Snug.value()))
      .padding(ExtraSpacing::Tight.role())
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(way(false))
      .child(way(true));
    let choice = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t("orchestrator.lead.what"))
          .id(format!("{id}-what"))
          .variant(ExtraTypography::Note.role())
          .nowrap(),
      )
      .child(ways);
    let meaning = ctx.t(if lead {
      "orchestrator.lead.meaning.lead"
    } else {
      "orchestrator.lead.meaning.follow"
    });
    Column::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Loose.role())
          .right(SpacingSize::Sm)
          .bottom(SpacingSize::Sm)
          .left(SpacingSize::Sm),
      )
      .spacing(SpacingSize::Sm)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(choice)
      .child(
        Text::new(&meaning)
          .id(format!("{id}-meaning"))
          .variant(LeadingText::Meaning.role())
          .width(Dimension::Pct(100.0)),
      )
      .into()
  }
}

/// One way of the control, named `id`: its icon and `words`, on `hover` in `text_primary` while it is `chosen`. The
/// pointer's only: Tab is the picker's key for it, and a press leaves the focus in the picker's search.
fn segment(id: &str, (glyph, words): (Icon, &str), chosen: bool, press: Press) -> Element {
  let (tone, style) = if chosen {
    (PaletteColor::TextPrimary, ExtraTypography::NoteStrong.role())
  } else {
    (PaletteColor::TextMuted, LeadingText::Way.role())
  };
  let way = press
    .button()
    .id(id.to_owned())
    .describe("a11y_name", words.to_owned())
    .describe("chosen", chosen.to_string())
    .height(Dimension::Px(ControlHeight::Xs.value()))
    .padding(Padding::horizontal(LeadingSpacing::WaySide.role()))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Xs, tone))
    .child(Text::new(words).variant(style).nowrap());
  if chosen {
    way.background(ExtraColor::Hover.role()).into()
  } else {
    way.into()
  }
}
