//! What a person who is not an admin of the team reads where an admin has controls (`Agents · … · member` M1–M6 in
//! `acFkz`; `docs/design/team-configuration.md`, decision 7 and edge case 11): one quiet line under a panel's chips
//! saying that only the admins change the team's part and who they are, the head's *· member*, the filter's hint, and,
//! for a kind with nothing published, one empty state saying an admin adds it. Nothing disabled stands in for what a
//! member lacks: the controls are not drawn, and the workspace refuses their writes as well.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::sharing::team::TeamConfiguration;

use crate::{
  routes::HubFilter,
  team::lock::locked,
  theme::HubText,
  ui::{icon::Icon, state::EmptyState},
};

/// Who changes the team's resources: the project's owners, then the server's admins who are not among them, by their
/// username there.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Admins {
  owners: Arc<[Arc<str>]>,
  server: Arc<[Arc<str>]>,
}

impl Admins {
  pub(super) fn of(team: &TeamConfiguration) -> Self {
    Self {
      owners: team.owners.clone(),
      server: team.admins.clone(),
    }
  }

  /// `dana-k (owner) and olha (server admin)`; `None` while the members are not read or name nobody.
  fn names(&self, ctx: &Ctx) -> Option<String> {
    let named = |key: &str, name: &Arc<str>| ctx.t_args(key, [("name", &**name)]).to_string();
    let all: Vec<String> = self
      .owners
      .iter()
      .map(|name| named("team_hub.member.owner", name))
      .chain(
        self
          .server
          .iter()
          .map(|name| named("team_hub.member.serverAdmin", name)),
      )
      .collect();
    let (last, rest) = all.split_last()?;
    Some(if rest.is_empty() {
      last.clone()
    } else {
      ctx
        .t_args(
          "team_hub.member.and",
          [
            ("first", rest.join(&ctx.t("team_hub.member.separator"))),
            ("last", last.clone()),
          ],
        )
        .to_string()
    })
  }
}

/// *Only team admins change this: dana-k (owner) and olha (server admin).* under the chips of a member's panel; none
/// for an admin.
pub(super) fn lock_line(ctx: &Ctx, may_change: bool, admins: &Admins) -> Option<Row> {
  if may_change {
    return None;
  }
  let words = match admins.names(ctx) {
    Some(names) => ctx.t_args("team_hub.member.lock", [("admins", names)]),
    None => ctx.t("team_hub.member.lockUnnamed"),
  };
  Some(locked("team-hub-member-lock", &words))
}

/// *· member* after the team's address in the head.
pub(super) fn role(ctx: &Ctx) -> Text {
  Text::new(&ctx.t("team_hub.member.role"))
    .id("team-hub-member-role")
    .variant(HubText::Hint.role())
    .nowrap()
}

/// The filter's hint for a member: what the kind is, and that they fill in only what is local.
pub(super) const fn hint_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::All => "team_hub.member.hint.all",
    HubFilter::Profiles => "team_hub.member.hint.profiles",
    HubFilter::McpServers => "team_hub.member.hint.mcpServers",
    HubFilter::Instructions => "team_hub.member.hint.instructions",
    HubFilter::Skills => "team_hub.member.hint.skills",
    HubFilter::Tools => "team_hub.member.hint.tools",
  }
}

/// The glyph, title and detail of a kind with nothing published.
const fn empty_of(filter: HubFilter) -> (Icon, &'static str, &'static str) {
  match filter {
    HubFilter::All => (
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

/// What a member sees where the list and the panel would be when `filter` holds nothing (`M5` `ITxzr`): that an admin
/// adds it, and who the admins are, with no *New* anywhere.
pub(super) fn empty(ctx: &Ctx, filter: HubFilter, admins: &Admins) -> Element {
  let (glyph, title, detail) = empty_of(filter);
  let state = EmptyState::new("team-hub-empty", glyph, ctx.t(title), ctx.t(detail))
    .compact()
    .unframed()
    .centered();
  let state = match admins.names(ctx) {
    Some(names) => state.note(ctx.t_args("team_hub.member.empty.admins", [("admins", names)])),
    None => state,
  };
  Column::new()
    .id("team-hub-member-empty")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(state)
    .into()
}
