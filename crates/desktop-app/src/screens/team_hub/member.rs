//! What a person who is not an admin of the team reads where an admin has controls (`Agents · … · member` M1–M6 in
//! `acFkz`; `docs/design/team-configuration.md`, decision 7 and edge case 11): one quiet line under a panel's chips
//! saying that only the admins change the team's part and who they are, the head's *· member*, the filter's hint, and,
//! for a kind with nothing published, one empty state saying an admin adds it. Nothing disabled stands in for what a
//! member lacks: the controls are not drawn, and the workspace refuses their writes as well.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::sharing::team::TeamConfiguration;

use crate::{
  routes::HubFilter,
  screens::hub::team::admins,
  team::lock::flat,
  theme::{ExtraSpacing, HubText},
  ui::{icon::Icon, state::glyph_tile},
};

/// *Only team admins change this: dana-k (Admin) and olha (Server admin).* under the chips of a member's panel; none
/// for an admin.
pub(super) fn lock_line(ctx: &Ctx, team: &TeamConfiguration) -> Option<Row> {
  if team.may_change {
    return None;
  }
  let words = ctx.t_args("team_hub.member.lock", [("admins", admins(ctx, team))]);
  Some(flat("team-hub-member-lock", &words))
}

/// *· member* after the team's address in the head.
pub(super) fn role(ctx: &Ctx) -> Text {
  Text::new(&ctx.t("team_hub.member.role"))
    .id("team-hub-member-role")
    .variant(HubText::Hint.role())
    .nowrap()
}

/// The glyph, title and detail of a kind with nothing published.
const fn empty_of(filter: HubFilter) -> (Icon, &'static str, &'static str) {
  match filter {
    HubFilter::All | HubFilter::Archived => (
      Icon::Users,
      "team_hub.member.empty.all.title",
      "team_hub.member.empty.all.detail",
    ),
    HubFilter::Profiles => (
      Icon::Bot,
      "team_hub.member.empty.profiles.title",
      "team_hub.member.empty.profiles.detail",
    ),
    HubFilter::McpServers => (
      Icon::Plug,
      "team_hub.member.empty.mcpServers.title",
      "team_hub.member.empty.mcpServers.detail",
    ),
    HubFilter::Instructions => (
      Icon::FileText,
      "team_hub.member.empty.instructions.title",
      "team_hub.member.empty.instructions.detail",
    ),
    HubFilter::Skills => (
      Icon::Sparkles,
      "team_hub.member.empty.skills.title",
      "team_hub.member.empty.skills.detail",
    ),
    HubFilter::Tools => (
      Icon::Wrench,
      "team_hub.member.empty.tools.title",
      "team_hub.member.empty.tools.detail",
    ),
  }
}

/// What a member sees where the list and the panel would be when `filter` holds nothing (`Body` `lNzIn`, `Empty state`
/// `Z3nzdX` in M5 `ITxzr`): in the outline the list and the panel stand in, that an admin adds it and who the admins
/// are, with no *New* anywhere.
pub(super) fn empty(ctx: &Ctx, filter: HubFilter, team: &TeamConfiguration) -> Element {
  let (glyph, title, detail) = empty_of(filter);
  let line = |id: &str, words: &str, role: HubText| {
    Text::new(words)
      .id(id.to_owned())
      .variant(role.role())
      .text_align(TextAlign::Center)
  };
  Column::new()
    .id("team-hub-member-empty")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(
      Column::new()
        .id("team-hub-empty")
        .describe("state", "empty")
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(glyph_tile("team-hub-empty", glyph))
        .child(line("team-hub-empty-title", &ctx.t(title), HubText::EmptyTitle))
        .child(line("team-hub-empty-detail", &ctx.t(detail), HubText::EmptyDetail))
        .child(line(
          "team-hub-empty-admins",
          &ctx.t_args("team_hub.member.empty.admins", [("admins", admins(ctx, team))]),
          HubText::Hint,
        )),
    )
    .into()
}
