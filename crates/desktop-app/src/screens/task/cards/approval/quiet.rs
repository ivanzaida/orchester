//! A tool request the run's approval mode approved without the person, as a quiet row (`Approval · Approved by
//! autopilot` `T1Ix39`, `Approval · Allowed by the profile` `G4of0`, `Approval · Approved by the reviewer` `iTpC5` in
//! 04c `vZcoN`): in line with a message's text past its avatar (`vs9mB` in 04 `H6kPd`), a 13 `text_muted` glyph, who
//! approved it in 12 `text_secondary`, what was asked in 12 mono `text_primary`, then in 12 `text_muted` the
//! reviewer's reason and the time, 10 apart. The last row a step's Autopilot rows show says how many more follow
//! (`FHr3B` in `H6kPd`), which a press shows.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::AnswerKind;
use orchester_workspace::{display, tasks::timeline::ApprovalCard};

use super::decided::asked;
use crate::{
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    chat::AVATAR_SIZE,
    icon::{Icon, icon},
    pressable::{Press, pressed},
  },
};

/// The quiet row `id` of `card`, which the run's mode approved ([`ApprovalCard::quiet`]); `more`, how many more
/// Autopilot rows of its step it leaves out and what shows them. Nothing for a card that is no quiet row.
pub(crate) fn line(ctx: &Ctx, id: &str, card: &ApprovalCard, more: Option<(usize, Press)>) -> Element {
  let Some(answer) = card.quiet() else {
    return Row::new().into();
  };
  let (glyph, lead) = match answer.kind {
    AnswerKind::Profile => (Icon::ListChecks, "cards.approval.quiet.profile"),
    AnswerKind::Reviewer => (Icon::ShieldCheck, "cards.approval.quiet.reviewer"),
    AnswerKind::Autopilot | AnswerKind::Person => (Icon::CheckCheck, "cards.approval.quiet.autopilot"),
  };
  let time = card.decided_at.map_or_else(String::new, display::clock);
  let detail = match answer.reason.as_deref().filter(|_| answer.kind == AnswerKind::Reviewer) {
    Some(reason) => ctx.t_args(
      "cards.approval.quiet.reasonAt",
      [("reason", reason), ("time", time.as_str())],
    ),
    None => ctx.t_args("cards.approval.quiet.at", [("time", time.as_str())]),
  };
  let more = more.map(|(count, press)| {
    let count = count.to_string();
    pressed(press).id(format!("{id}-more")).child(
      Text::new(&ctx.t_args("cards.approval.quiet.more", [("count", count.as_str())]))
        .variant(CardsText::Quiet.role())
        .nowrap(),
    )
  });
  // The avatar's room and the chat row's gap after it put the glyph under a message's text.
  Row::new()
    .id(id.to_owned())
    .describe("waiting", "false")
    .describe("answered_as", answer_name(answer.kind))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(Row::new().width(Dimension::Px(AVATAR_SIZE)).flex_shrink(0.0))
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(lead))
        .id(format!("{id}-lead"))
        .variant(CardsText::Lead.role())
        .nowrap(),
    )
    .child(
      Text::new(&asked(ctx, &card.request))
        .id(format!("{id}-asked"))
        .variant(ExtraTypography::PropertyMono.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&detail)
        .id(format!("{id}-detail"))
        .variant(CardsText::Quiet.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(more)
    .into()
}

/// How a row names who approved it, for tooling.
const fn answer_name(kind: AnswerKind) -> &'static str {
  match kind {
    AnswerKind::Autopilot => "autopilot",
    AnswerKind::Profile => "profile",
    AnswerKind::Reviewer => "reviewer",
    AnswerKind::Person => "person",
  }
}
