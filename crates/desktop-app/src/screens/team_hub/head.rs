//! How a team project stands, at the right of the hub's head (`Header` `NRqwv` in `t4mtQL`): *Team*, *· member* for
//! whoever is not an admin, and *✓ Draft saved* while the panel's resource has a draft kept here. The
//! head itself is the frame every hub shares ([`crate::screens::hub::agents`]).

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_workspace::sharing::team::TeamConfiguration;

use super::member;
use crate::{
  theme::{ExtraSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// How the team server stands, which the head says.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Reach {
  Answered,
  /// It did not answer the last read of the hub's other reads (`Publish while offline` A16 `x3ywL`).
  Unanswered,
  /// The team's configuration cannot be read from it now (`Team` `ExtGT` in A25 `Zhcif`).
  Unreachable,
}

/// *Team*, before it *✓ Draft saved* when `saved`; while the team server cannot be reached, the line says so in place
/// of *Team*.
pub(super) fn status(ctx: &Ctx, team: &TeamConfiguration, (reach, saved): (Reach, bool)) -> Vec<Element> {
  let (glyph, color, words) = match reach {
    Reach::Answered => (Icon::Users, PaletteColor::TextMuted, "team_hub.team"),
    Reach::Unanswered => (Icon::CloudOff, PaletteColor::Warning, "team_hub.unreached"),
    Reach::Unreachable => (Icon::CloudOff, PaletteColor::Warning, "team_hub.cantReach.head"),
  };
  let unreached = reach != Reach::Answered;
  let line = Row::new()
    .id("team-hub-team")
    .describe("reached", (!unreached).to_string())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color.clone()))
    .child(
      Text::new(&ctx.t(words))
        .id("team-hub-team-label")
        .variant(HubText::Hint.role())
        .color(if unreached { color } else { PaletteColor::TextMuted })
        .nowrap(),
    )
    .with_children((!team.may_change).then(|| member::role(ctx)));
  let mut status: Vec<Element> = Vec::new();
  if saved {
    status.push(draft_saved(ctx).into());
  }
  status.push(line.into());
  status
}

/// *✓ Draft saved* (`Draft saved` in `Header` of A13 `WMtxm`).
fn draft_saved(ctx: &Ctx) -> Row {
  Row::new()
    .id("team-hub-draft-saved")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Check, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("team_hub.draftSaved"))
        .id("team-hub-draft-saved-label")
        .variant(HubText::Hint.role())
        .nowrap(),
    )
}
