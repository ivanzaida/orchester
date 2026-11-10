//! The profile the fields make, by the rules a profile is saved by: a local one through the profile editor's own form
//! ([`ProfileForm`]) on its connection, with the access that connection can keep; a team one as the team holds it, on
//! a CLI kind, the edited version's permissions, other settings and secrets kept while its CLI stays.

use std::{collections::BTreeMap, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::{
    REASONING_EFFORT,
    invocation::Platform,
    mcp::{McpSelectionError, McpServerSelection},
  },
  id::ConnectionId,
  name::Name,
  permission::AgentPermissions,
  resource::{ChosenResources, PromptText, ProviderSettings, TeamAgentProfile},
};
use orchester_workspace::agents::{
  form::{ModelSelection, ProfileErrors, ProfileForm, access_for_connection},
  role::{RoleErrors, RoleText},
};

use super::{Form, Given, ListKind, Made, Scope, runs::Runs};
use crate::ui::message::message_text;

/// What the fields make, or why they make nothing, in the words of the first thing wrong.
pub(super) fn made(
  ctx: &Ctx,
  form: &Form<'_>,
  (text, given): (&RoleText, &Given),
  runs: &Runs,
) -> Result<Made, Arc<str>> {
  if let Some(why) = text.errors().as_ref().and_then(RoleErrors::first) {
    return Err(message_text(ctx, why));
  }
  match form.scope {
    Scope::Local => local(ctx, (text, given), runs),
    Scope::Team => team(ctx, form.base, (text, given), runs),
  }
}

fn required(ctx: &Ctx) -> Arc<str> {
  ctx.t("team_new.errors.required")
}

/// The list `kind` of `given`.
fn list(given: &Given, kind: ListKind) -> ChosenResources {
  ChosenResources::new(given.0[kind.at()].clone())
    .expect("a list holds each resource once: choosing one takes it out before it adds it")
}

fn local(ctx: &Ctx, (text, given): (&RoleText, &Given), runs: &Runs) -> Result<Made, Arc<str>> {
  let connection = runs.connection.as_ref().ok_or_else(|| required(ctx))?;
  let model = runs.model.clone().ok_or_else(|| required(ctx))?;
  let servers = McpServerSelection::new(
    given.0[ListKind::McpServers.at()]
      .iter()
      .map(|id| ConnectionId::from_uuid(id.as_uuid()))
      .collect(),
  )
  .map_err(|refused| match refused {
    McpSelectionError::TooMany => ctx.t_args(
      "local_new.profile.serversTooMany",
      [("max", McpServerSelection::MAX.to_string())],
    ),
    McpSelectionError::Duplicate(_) => ctx.t("local_new.profile.serversDuplicate"),
  })?;
  let platform = Platform::current();
  let permissions = AgentPermissions::most_restrictive(connection.provider(), platform);
  // A native connection takes neither CLI approval switches nor MCP servers.
  let (permissions, servers) = access_for_connection(&permissions, &servers, connection.provider(), platform);
  ProfileForm {
    role: &text.role,
    responsibilities: &text.responsibilities,
    instructions: &text.instructions,
    connection: Some(connection),
    model: &ModelSelection::Listed(model),
    custom_model: "",
    effort: runs.effort.as_deref(),
    permissions: &permissions,
    mcp_servers: &servers,
    instruction_resources: &list(given, ListKind::Instructions),
    skills: &list(given, ListKind::Skills),
    tools: &list(given, ListKind::Tools),
  }
  .profile()
  .map(|profile| Made::Local(Box::new(profile)))
  .map_err(|errors| first_error(ctx, &errors))
}

fn first_error(ctx: &Ctx, errors: &ProfileErrors) -> Arc<str> {
  [
    &errors.role,
    &errors.instructions,
    &errors.connection,
    &errors.model,
    &errors.effort,
  ]
  .into_iter()
  .find_map(Option::as_ref)
  .map_or_else(|| required(ctx), |message| message_text(ctx, message))
}

fn team(
  ctx: &Ctx,
  base: Option<&TeamAgentProfile>,
  (text, given): (&RoleText, &Given),
  runs: &Runs,
) -> Result<Made, Arc<str>> {
  let cli = runs.cli.ok_or_else(|| required(ctx))?;
  let model = runs.model.clone().ok_or_else(|| required(ctx))?;
  let role = Name::new(&text.role).expect("a role without errors has a name");
  let instructions = PromptText::new(&text.instructions).expect("a role without errors has a prompt");
  let kept = base.filter(|base| base.cli() == cli);
  let permissions = kept.map_or_else(
    || AgentPermissions::most_restrictive(cli, Platform::current()),
    |base| base.permissions().clone(),
  );
  let secrets = base.map_or_else(Vec::new, |base| base.secrets().to_vec());
  let at = |kind: ListKind| given.0[kind.at()].clone();
  TeamAgentProfile::new(
    role,
    text.responsibilities.trim().to_owned(),
    instructions,
    (cli, model, settings(base, runs.effort.as_deref())),
  )
  .with_permissions(permissions)
  .and_then(|profile| profile.with_mcp_servers(at(ListKind::McpServers)))
  .and_then(|profile| profile.with_tools(at(ListKind::Tools)))
  .and_then(|profile| profile.with_skills(at(ListKind::Skills)))
  .and_then(|profile| profile.with_instruction_resources(at(ListKind::Instructions)))
  .and_then(|profile| profile.with_secrets(secrets))
  .map(|profile| Made::Team(Box::new(profile)))
  .map_err(|_| {
    ctx.t_args(
      "team_hub.edit.tooMany",
      [("max", TeamAgentProfile::MAX_LISTED.to_string())],
    )
  })
}

/// The settings the profile runs with: the edited version's others, and `effort` (`None` leaves it to the CLI).
fn settings(base: Option<&TeamAgentProfile>, effort: Option<&str>) -> ProviderSettings {
  let mut settings: BTreeMap<String, String> = base
    .map(|base| {
      base
        .settings()
        .iter()
        .filter(|(name, _)| *name != REASONING_EFFORT)
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
    })
    .unwrap_or_default();
  if let Some(effort) = effort {
    settings.insert(REASONING_EFFORT.to_owned(), effort.to_owned());
  }
  ProviderSettings::new(settings).expect("a version's settings and the reasoning effort setting have valid names")
}
