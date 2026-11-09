//! The body of a team project's hub when its team server could not be reached and nothing of the team's configuration
//! was read this session (`A25 · Agents · team server unreachable` `Zhcif`): the team's resources live only on the
//! server, so nothing older is shown; the tile, *Can't reach the team server*, which server did not answer, the reason
//! with when it was tried, and *Try again*, which asks the server again now.

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{display, sharing::team::TeamConfiguration, team_config::Unreached};

use crate::{
  theme::{ExtraSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    state::warning_tile,
  },
};

/// The widest the line under the title runs (`Line` `ubt70`).
const LINE_WIDTH: f32 = 520.0;
/// The room between the reason and *Try again*, over the column's own gap (`Action gap` `G33P4K`).
const ACTION_GAP: f32 = 4.0;

/// The body, in the frame every hub's body has; `again` asks the team server again.
pub(super) fn body(
  ctx: &Ctx,
  team: &TeamConfiguration,
  unreached: &Unreached,
  again: impl Fn() + Send + Sync + 'static,
) -> Element {
  let line = match &team.server {
    Some((_, host)) => ctx.t_args("team_hub.cantReach.line", [("host", host.as_ref())]),
    None => ctx.t("team_hub.cantReach.lineNoHost"),
  };
  let tried = display::clock(unreached.at);
  let reason = ctx.t_args(
    "team_hub.cantReach.reason",
    [("reason", message_text(ctx, &unreached.reason).as_ref()), ("time", tried.as_str())],
  );
  let action = ActionButton::new("team-hub-unreachable-again", ButtonKind::Secondary, ctx.t("common.tryAgain"))
    .icon(Icon::RotateCcw)
    .on_click(again);
  let state = Column::new()
    .id("team-hub-unreachable")
    .describe("state", "unreachable")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(warning_tile("team-hub-unreachable", Icon::CloudOff))
    .child(
      Text::new(&ctx.t("team_hub.cantReach.title"))
        .id("team-hub-unreachable-title")
        .variant(HubText::EmptyTitle.role()),
    )
    .child(
      Text::new(&line)
        .id("team-hub-unreachable-line")
        .variant(HubText::EmptyDetail.role())
        .text_align(TextAlign::Center)
        .width(Dimension::Px(LINE_WIDTH)),
    )
    .child(
      Text::new(&reason)
        .id("team-hub-unreachable-reason")
        .variant(HubText::Reason.role()),
    )
    .child(Row::new().height(Dimension::Px(ACTION_GAP)))
    .child(action);
  crate::screens::hub::agents::framed("team-hub-unreachable-body", state.into())
}
