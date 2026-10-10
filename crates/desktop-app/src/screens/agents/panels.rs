//! Draft card LruUv, enforcement panel VsylR and pipeline uses uIMpt.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::invocation::Platform,
  permission::capability::{PermissionOption, support},
};
use orchester_workspace::agents::data::{ProjectAgents, VersionState};

use super::{editor::Props, fields::Fields, words};
const PANEL_WIDTH: f32 = 320.0;

fn panel(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Px(PANEL_WIDTH))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
}
pub fn draft(ctx: &Ctx, props: &Props, fields: &Fields) -> Element {
  let version = props
    .entry
    .as_ref()
    .and_then(|e| e.state.published())
    .map_or(0, |v| v.get());
  let mut panel = panel("agent-draft-card").child(
    Text::new(&ctx.t_args("agents.draftCard", [("version", (version + 1).to_string())]))
      .variant(TypographyStyle::Label),
  );
  let based_on = props.entry.as_ref().and_then(|entry| match entry.state {
    VersionState::StaleDraft { based_on, .. } => based_on,
    state => state.published(),
  });
  if let Some(base) = based_on {
    panel = panel.child(words::note(
      &ctx.t_args("agents.basedOn", [("version", base.get().to_string())]),
    ));
  }
  if let Some(entry) = props
    .entry
    .as_ref()
    .and_then(|current| props.listed.published.iter().find(|entry| entry.id == current.id))
  {
    panel = panel.child(words::note(
      &ctx.t_args("agents.changes", [("version", version.to_string())]),
    ));
    let changes = [
      (fields.role.get() != entry.profile.role().as_str(), "agents.role"),
      (
        fields.responsibility.get() != entry.profile.responsibilities(),
        "agents.responsibility",
      ),
      (
        fields.instructions.get() != entry.profile.instructions().as_str(),
        "agents.instructions",
      ),
      (
        fields.permissions.get().0 != *entry.profile.permissions(),
        "agents.permissionsTab",
      ),
      (fields.servers.get().0 != *entry.profile.mcp_servers(), "agents.mcp"),
      (
        fields.model.get() != entry.profile.model().as_str()
          || fields.effort.get()
            != orchester_workspace::agents::form::stored_effort(&entry.profile).unwrap_or_default(),
        "agents.model",
      ),
      (
        fields.connection.get() != entry.profile.connection().to_string(),
        "agents.connection",
      ),
    ];
    for (_, key) in changes.into_iter().filter(|(changed, _)| *changed) {
      panel = panel.child(words::note(&ctx.t(key)));
    }
  }
  panel
    .child(words::note(
      &ctx.t_args("agents.publishNote", [("version", (version + 1).to_string())]),
    ))
    .into()
}
pub fn enforcement(ctx: &Ctx, fields: &Fields, listed: &ProjectAgents) -> Element {
  let Some(connection) = fields.connection(listed) else {
    return panel("agent-enforcement")
      .child(words::note(&ctx.t("agents.chooseConnection")))
      .into();
  };
  let permissions = fields.permissions.get().0;
  let effective = permissions.enforced_on(connection.provider(), Platform::current());
  let title = if effective.is_ok() {
    ctx.t_args("agents.enforced", [("provider", connection.name().as_str())])
  } else {
    ctx.t("agents.notEnforced")
  };
  let mut panel = panel("agent-enforcement")
    .child(Text::new(&title).variant(TypographyStyle::Label))
    .child(words::note(connection.name().as_str()));
  for (key, option, value) in [
    (
      "agents.repository",
      PermissionOption::of_repository(permissions.repository()),
      ctx.t(
        if permissions.repository() == orchester_core::resource::RepositoryAccess::ReadOnly {
          "agents.readOnly"
        } else {
          "agents.writeTitle"
        },
      ),
    ),
    (
      "agents.commands",
      PermissionOption::of_commands(permissions.commands()),
      words::commands(ctx, &permissions),
    ),
    (
      "agents.network",
      PermissionOption::of_network(permissions.network()),
      words::network(ctx, &permissions, Some(connection.provider())),
    ),
  ] {
    let support = effective.as_ref().map_or_else(
      |_| support(connection.provider(), Platform::current(), option),
      |effective| {
        if option == PermissionOption::NetworkOff && effective.network_stays_on().is_some() {
          support(connection.provider(), Platform::current(), option)
        } else {
          orchester_core::permission::capability::Support::Enforced(effective.mechanism(option.setting()))
        }
      },
    );
    let state = match support {
      orchester_core::permission::capability::Support::Enforced(mechanism) => {
        super::enforcement::mechanism(ctx, mechanism, &permissions)
      }
      _ => super::enforcement::unavailable(ctx, Some(connection.provider()), option),
    };
    panel = panel.child(
      Column::new()
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Label))
        .child(words::note(&value))
        .child(words::note(&state)),
    );
  }
  panel
    .child(words::note(&ctx.t("agents.agentFiles")))
    .child(words::note(&ctx.t(if listed.agent_files.is_on() {
      "agents.agentFilesOn"
    } else {
      "agents.agentFilesOff"
    })))
    .child(words::note(&ctx.t("agents.mcpHint")))
    .child(words::note(&ctx.t("agents.localRules")))
    .child(words::note(&ctx.t("agents.enforcementNote")))
    .child(words::note(&ctx.t("agents.newRuns")))
    .into()
}
