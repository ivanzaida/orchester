//! *Profile* of a local profile (`Profile` `Gk3js` in L1): what it runs with (the CLI, the connection, the model and
//! the reasoning effort, the rows every profile form has: [`HeldFields`]), the MCP servers it chooses and the secrets
//! those take, and the project's instructions, skills and tools it gives its agents
//! (`docs/design/team-configuration.md`, "Decided 2026-11-09"). The servers and the three lists are tags with *Add* in
//! place: each choice changes only what the panel holds until *Save* ([`super::edit`]); the secrets are the servers'
//! and only shown.
//!
//! [`HeldFields`]: crate::screens::hub::profile_form::HeldFields

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
};
use orchester_core::id::{ConnectionId, ResourceId};
use orchester_workspace::agents::{
  local_edit::{Edit, GivenKind},
  local_hub::LocalProfile,
};

use super::add_menu;
use crate::screens::{
  hub::{given, profile_form::OnEdit},
  team_hub::rows::{self, Sub},
};

/// What the section needs of the panel.
pub(super) struct Rows<'a> {
  pub profile: &'a LocalProfile,
  /// The profile as it is published, which a tag added since is marked against.
  pub published: &'a LocalProfile,
  /// The rows of what it runs with.
  pub runs: Vec<Row>,
  pub on_edit: OnEdit,
  /// A save is under way: nothing is changed now.
  pub busy: bool,
}

pub(super) fn section(ctx: &mut Ctx, rows: Rows<'_>) -> Column {
  let profile = rows.profile;
  // The lists in the order every profile form has them (A26, L17): instructions, MCP servers, tools, skills, secrets.
  let mut list = vec![
    given(ctx, &rows, GivenKind::Instruction),
    mcp_servers(ctx, &rows),
    given(ctx, &rows, GivenKind::Tool),
    given(ctx, &rows, GivenKind::Skill),
  ];
  if !profile.secrets.is_empty() {
    list.push(rows::row(
      "local-hub-profile-secrets",
      &ctx.t("team_hub.profile.secrets"),
      Some(Sub::quiet(ctx.t("team_hub.profile.secretsSub"))),
      rows::tags(
        "local-hub-profile-secrets-value",
        profile.secrets.iter().map(ToString::to_string),
      ),
    ));
  }
  let mut all = rows.runs;
  all.extend(list);
  rows::section("local-hub-profile", &ctx.t("local_hub.profile.title"), Vec::new(), all)
}

/// *MCP servers*: the ones it chooses in tags, and *Add*, a menu of the project's servers.
fn mcp_servers(ctx: &mut Ctx, rows: &Rows<'_>) -> Row {
  let profile = rows.profile;
  let edit = rows.on_edit.clone();
  let on_toggle: add_menu::Toggle<ConnectionId> = Arc::new(move |server, choose| {
    edit(if choose {
      Edit::AddServer(server)
    } else {
      Edit::RemoveServer(server)
    });
  });
  let list = given::List {
    id: "local-hub-profile-mcp".to_owned(),
    what: "server",
    label: ctx.t("team_hub.profile.mcpServers"),
    sub: None,
    chosen: profile.mcp_servers.clone(),
    before: Some(rows.published.mcp_servers.iter().map(|(id, _)| *id).collect()),
    offered: profile.options.servers.clone(),
    on_toggle,
    locked: rows.busy,
  };
  given::row(ctx, list)
}

/// *Instructions*, *Skills* or *Tools*: the project's ones of `kind` it gives its agents in tags, and *Add*, a menu of
/// the project's others.
fn given(ctx: &mut Ctx, rows: &Rows<'_>, kind: GivenKind) -> Row {
  let profile = rows.profile;
  let edit = rows.on_edit.clone();
  let on_toggle: add_menu::Toggle<ResourceId> = Arc::new(move |resource, choose| {
    edit(if choose {
      Edit::Give(kind, resource)
    } else {
      Edit::TakeBack(kind, resource)
    });
  });
  let (id, what, label) = match kind {
    GivenKind::Instruction => (
      "local-hub-profile-instructions",
      "instruction",
      "local_hub.profile.instructions",
    ),
    GivenKind::Skill => ("local-hub-profile-skills", "skill", "local_hub.profile.skills"),
    GivenKind::Tool => ("local-hub-profile-tools", "tool", "local_hub.profile.tools"),
  };
  let list = given::List {
    id: id.to_owned(),
    what,
    label: ctx.t(label),
    sub: None,
    chosen: profile.given.of(kind).to_vec(),
    before: Some(rows.published.given.of(kind).iter().map(|(id, _)| *id).collect()),
    offered: profile.options.given.of(kind).to_vec(),
    on_toggle,
    locked: rows.busy,
  };
  given::row(ctx, list)
}
