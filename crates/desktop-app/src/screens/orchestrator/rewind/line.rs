//! Where a rewind went back to (10 `Zm7kq`): the line *Rewound to before your message of 16:04 · 4 messages dropped*
//! (`Rewound` `NMQOp`: a `border_strong` line at each side of a 13 `history` and the words in 11.5 `text_secondary`,
//! 10 apart), and when the Orchestrator had done something in what was dropped, what stays (`Done after this point ·
//! stays` `D6rF7`, 8 apart): a neutral callout with `list-checks` (*What the Orchestrator did after this point stays*)
//! over the event lines of what it did, as an answer lists them.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{ChatEvent, RewoundTo},
  time::Timestamp,
};
use orchester_workspace::display;

use crate::{
  screens::orchestrator::{action, line::list},
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing},
  ui::{
    callout::{Callout, CalloutTone},
    icon::{Icon, icon},
  },
};

/// The line's rules' height (`R4E15` in `NMQOp`).
const RULE_HEIGHT: f32 = 1.0;

/// The row of rewind `event`, kept at `at` in project `project`, named `id`; `None` for another event.
pub(in crate::screens::orchestrator) fn line(
  ctx: &mut Ctx,
  id: &str,
  event: &ChatEvent,
  (at, project): (Timestamp, ProjectId),
) -> Option<Element> {
  let ChatEvent::Rewound { to, dropped, done } = event else {
    return None;
  };
  let (keys, time) = match to {
    RewoundTo::BeforeMessage { at } => (
      (
        "orchestrator.rewind.lineBefore.one",
        "orchestrator.rewind.lineBefore.other",
      ),
      display::clock(*at),
    ),
    RewoundTo::AfterAnswer { at } => (
      (
        "orchestrator.rewind.lineAfter.one",
        "orchestrator.rewind.lineAfter.other",
      ),
      display::clock(*at),
    ),
  };
  let words = super::dialog::plural(ctx, keys, *dropped, &[("time", &time)]);
  let rule = || {
    Row::new()
      .flex(1.0)
      .height(Dimension::Px(RULE_HEIGHT))
      .background(PaletteColor::BorderStrong)
  };
  let rewound = Row::new()
    .id(format!("{id}-line"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(rule())
    .child(icon(Icon::History, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&words)
        .id(format!("{id}-words"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(rule());
  let stays = (!done.is_empty()).then(|| {
    let lines = done
      .iter()
      .enumerate()
      .map(|(index, action)| action::line(ctx, &format!("{id}-done-{index}"), (action, at, true), project))
      .collect();
    let callout = Callout::new(
      format!("{id}-kept"),
      CalloutTone::Neutral,
      ctx.t("orchestrator.rewind.kept.title"),
    )
    .icon(Icon::ListChecks)
    .body(ctx.t("orchestrator.rewind.kept.body"));
    Column::new()
      .id(format!("{id}-stays"))
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Sm)
      .child(callout)
      .child(list(&format!("{id}-done"), lines))
  });
  Some(
    Column::new()
      .id(id.to_owned())
      .describe("dropped", dropped.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(OrchestratorSpacing::Items.role())
      .child(rewound)
      .with_children(stays)
      .into(),
  )
}
