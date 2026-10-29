//! The run's switch to Manual for itself (`Event · switched to Manual` `ARwiU` in 04c `vZcoN`, `wE0eF` in 04b
//! `p6tXIO`): a quiet row 36 in, its 13 `hand` in `text_muted`, *Switched to Manual for this run* in 12
//! `text_secondary`, then who switched it and when in 12 `text_muted` (*by you · 15:20*), with whose profile stays on
//! Autopilot while the run is Manual for itself alone; 10 apart.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{run::Answerer, time::Timestamp};
use orchester_workspace::{display, tasks::timeline::manual_switch::SwitchedToManual};

use crate::{
  theme::{ConversationSpacing, ConversationText, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

pub(super) fn row(ctx: &Ctx, switched: &SwitchedToManual, at: Timestamp) -> Element {
  let time = display::clock(at);
  let detail = match (&switched.agent, switched.by) {
    (Some(agent), Answerer::Person) => ctx.t_args(
      "conversation.approval.switchedBy.personProfile",
      [("time", time.as_str()), ("agent", &**agent)],
    ),
    (Some(agent), Answerer::Automation) => ctx.t_args(
      "conversation.approval.switchedBy.automationProfile",
      [("time", time.as_str()), ("agent", &**agent)],
    ),
    (None, Answerer::Person) => ctx.t_args("conversation.approval.switchedBy.person", [("time", time.as_str())]),
    (None, Answerer::Automation) => {
      ctx.t_args("conversation.approval.switchedBy.automation", [("time", time.as_str())])
    }
  };
  Row::new()
    .id("conversation-switched-to-manual")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(ConversationSpacing::ApprovalRow.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Hand, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("conversation.approval.switched"))
        .id("conversation-switched-to-manual-text")
        .variant(ConversationText::ApprovalLead.role())
        .nowrap(),
    )
    .child(
      Text::new(&detail)
        .id("conversation-switched-to-manual-detail")
        .variant(ConversationText::ApprovalDetail.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
