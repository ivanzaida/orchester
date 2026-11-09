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

/// *Team*, before it *✓ Draft saved* when `saved`. `unreached`: the team server did not
/// answer the last read, which the line says in place of *Team* (`Publish while offline` A16 `x3ywL`).
pub(super) fn status(ctx: &Ctx, team: &TeamConfiguration, (unreached, saved): (bool, bool)) -> Vec<Element> {
  let (glyph, color, words) = if unreached {
    (Icon::CloudOff, PaletteColor::Warning, "team_hub.unreached")
  } else {
    (Icon::Users, PaletteColor::TextMuted, "team_hub.team")
  };
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
