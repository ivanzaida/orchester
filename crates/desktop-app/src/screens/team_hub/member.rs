//! What a person who is not an admin of the team reads where an admin has controls (`Agents · … · member` M1–M6 in
//! `acFkz`; `docs/design/team-configuration.md`, decision 7 and edge case 11): one quiet line under a panel's chips
//! saying that only the admins change the team's part and who they are, the head's *· member*, and, for a kind with
//! nothing published, the words of the hub's empty state saying an admin adds it. Nothing disabled stands in for what a
//! member lacks: the controls are not drawn, and the workspace refuses their writes as well.

use lurq::{
  app::ctx::Ctx,
  components::{Row, Text},
  layout::text_style::TextAlign,
  node::Element,
};
use orchester_workspace::sharing::team::TeamConfiguration;

use crate::{routes::HubFilter, screens::hub::team::admins, team::lock::flat, theme::HubText};

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

/// The title and the line of a member's empty tab: that an admin adds it.
pub(super) const fn empty_words(filter: HubFilter) -> (&'static str, &'static str) {
  match filter {
    HubFilter::All | HubFilter::Memory | HubFilter::Archived => {
      ("team_hub.member.empty.all.title", "team_hub.member.empty.all.detail")
    }
    HubFilter::Profiles => (
      "team_hub.member.empty.profiles.title",
      "team_hub.member.empty.profiles.detail",
    ),
    HubFilter::McpServers => (
      "team_hub.member.empty.mcpServers.title",
      "team_hub.member.empty.mcpServers.detail",
    ),
    HubFilter::Instructions => (
      "team_hub.member.empty.instructions.title",
      "team_hub.member.empty.instructions.detail",
    ),
    HubFilter::Skills => (
      "team_hub.member.empty.skills.title",
      "team_hub.member.empty.skills.detail",
    ),
    HubFilter::Tools => (
      "team_hub.member.empty.tools.title",
      "team_hub.member.empty.tools.detail",
    ),
  }
}

/// *Admins: dana-k (Admin) and olha (Server admin)* under a member's empty state (`Empty state` `Z3nzdX` in M5).
pub(super) fn admins_line(ctx: &Ctx, team: &TeamConfiguration) -> Element {
  Text::new(&ctx.t_args("team_hub.member.empty.admins", [("admins", admins(ctx, team))]))
    .id("team-hub-empty-admins")
    .variant(HubText::Hint.role())
    .text_align(TextAlign::Center)
    .into()
}
