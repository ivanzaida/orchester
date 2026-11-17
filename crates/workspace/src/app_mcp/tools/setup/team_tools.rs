//! A team project's tools through the tools (`docs/design/local-and-team-parity.md`): they are the team's resources,
//! read from the team's configuration this session read, in the team's form (a secret by its name). `list_tools` and
//! `get_tool` read them, `save_tool_draft` publishes a command tool's next version on the team server at once (without
//! `tool`, the team's new command tool as its version 1), checked as a local save is, its program found on this
//! computer, and `publish_tool` answers it as it stands: the team keeps no draft on a computer. Making a tool, or
//! widening what it runs, is a person's decision as for a local tool ([`widening::allowed`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::{CommandAction, ResourceKind, TeamTool, TeamToolAction},
  store::SettingsStore,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{
  created::{create_team, refused as edit_refused},
  given_by::Givers,
  profiles::state_json,
  project_tools::{tool_json, used_by_steps},
  team_profiles::TeamProject,
  team_resources::{Sent, TeamEntry, based_on, config_read, entries, marked, named, pipelines},
  tool_input::{SaveToolDraft, command, new_name, not_a_command_tool, refused, widenings},
  widening,
};
use crate::{
  agents::{data::VersionState, team_body_edit},
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::Link,
  messages::Message,
  sharing::team_edit::{NewTeamResource, Publication},
  tasks::outside::Outside,
  team_config::TeamConfig,
  tools::data::{ProjectTools, ToolEntry, uses_in},
  workspace::OpenProject,
};

const KIND: (ResourceKind, Missing) = (ResourceKind::Tool, Missing::Tool);

/// The team's command tools in `config`, as this computer runs them, and the steps of the team's pipelines that call
/// any of its tools.
fn tools(config: &TeamConfig) -> ProjectTools {
  let commands = entries::<TeamTool>(config, ResourceKind::Tool)
    .into_iter()
    .filter_map(|team| {
      Some(ToolEntry {
        tool: team.body.command_tool()?,
        id: team.id,
        name: team.name,
        state: VersionState::Published(team.number),
      })
    })
    .collect();
  let uses = pipelines(config)
    .iter()
    .flat_map(|(named, body)| uses_in(named.clone(), body).collect::<Vec<_>>())
    .collect();
  ProjectTools {
    tools: Arc::new(commands),
    uses: Arc::new(uses),
  }
}

/// The team's tools of the project at `open`, each with its id, as `list_tools` lists them; none for a local project
/// and while the team's configuration was not read this session.
pub(super) fn listed(
  services: &Services,
  (open, db): (&OpenProject, &ProjectDb),
) -> Result<Vec<(ResourceId, Value)>, ToolError> {
  let Some(config) = config_read(services, (open, db))? else {
    return Ok(Vec::new());
  };
  let (tools, givers) = (tools(&config), Givers::team(&config));
  Ok(
    entries::<TeamTool>(&config, ResourceKind::Tool)
      .iter()
      .map(|entry| (entry.id, shown((&tools, &givers), entry, open.project.id())))
      .collect(),
  )
}

/// `entry` as `get_tool` shows it, with the profiles that give it to their agents and its command line, marked as the
/// team's. An HTTP or database tool shows nothing of the secret it uses but its kind and description.
fn shown((tools, givers): (&ProjectTools, &Givers), entry: &TeamEntry<TeamTool>, project: ProjectId) -> Value {
  let state = VersionState::Published(entry.number);
  let mut value = match tools.tool(entry.id) {
    Some(command) => tool_json(tools, command, project),
    None => json!({
      "id": entry.id.to_string(),
      "name": &*entry.name,
      "kind": entry.body.kind(),
      "description": entry.body.description(),
      "state": state_json(state),
      "runnable": entry.body.kind().runnable() && entry.body.access().is_none(),
      "link": Link::Tool(project, entry.id).path(),
      "command": null,
      "used_by_steps": used_by_steps(tools, entry.id),
    }),
  };
  value["used_by"] = givers.of(entry.id);
  value["command_line"] = json!(entry.body.command().map(CommandAction::command_line));
  marked(value, entry.number)
}

/// The team tool `named` of the team project `team`, when the team has one by that id, name or slug.
pub(super) fn of(team: &TeamProject, named: &str) -> Option<ResourceId> {
  self::named::<TeamTool>(&team.config, KIND, named)
    .ok()
    .map(|entry| entry.id)
}

/// Team tool `id` of `open`'s project as `get_tool` answers it, read now.
pub(super) async fn shown_now(services: &Services, open: &OpenProject, id: ResourceId) -> Result<Value, ToolError> {
  let (services, open) = (services.clone(), open.clone());
  blocking(move || {
    let config = services.workspace.team_read(&open.repo_root).loaded()?;
    let entry = named::<TeamTool>(&config, KIND, &id.to_string())?;
    let read = (&tools(&config), &Givers::team(&config));
    Ok(json!({ "tool": shown(read, &entry, open.project.id()) }))
  })
  .await
}

/// Publishes the command tool `args` give in the team project `team` as its next version, or as the team's new tool;
/// the tool as `get_tool` shows it after.
pub(super) async fn save(services: &Services, team: &TeamProject, args: SaveToolDraft) -> Result<Value, ToolError> {
  let (reading, config, root) = (services.clone(), team.config.clone(), team.open.repo_root.clone());
  let sent = blocking(move || prepared(&reading, (&root, &config), &args)).await?;
  let root = &team.open.repo_root;
  let id = match sent {
    Sent::Next(id, base, tool) => {
      team_body_edit::save_team_tool(&services.servers, root, (id, base), &tool)
        .await
        .map_err(|error| edit_refused(&error))?;
      id
    }
    Sent::New(new) => create_team(services, root, new).await?,
  };
  services.changes.record(team.open.project.id(), Outside::Tools);
  let mut shown = shown_now(services, &team.open, id).await?;
  shown["tool"]["draft_kept"] = json!(false);
  Ok(shown)
}

/// What a save of `args` sends, checked as a local save is, and refused where it makes a tool or widens what one runs
/// without a person's leave. Blocks on SQLite.
fn prepared(
  services: &Services,
  (root, config): (&Path, &TeamConfig),
  args: &SaveToolDraft,
) -> Result<Sent<TeamTool>, ToolError> {
  let project = ProjectDb::open(root)?.project_environment()?;
  let existing = args
    .tool
    .as_deref()
    .map(|tool| named::<TeamTool>(config, KIND, tool))
    .transpose()?;
  let Some(entry) = existing else {
    if args.expected_revision.is_some() {
      return Err(ToolError::InvalidArguments(
        "expected_revision names a tool's version; a new tool has none".into(),
      ));
    }
    let action = command(args, None, &project, root)?;
    let taken = config.resources(ResourceKind::Tool);
    let name = new_name(taken.iter().map(|tool| tool.name.as_str()), args.name.as_deref())?;
    // The tool's first version runs a program with the person's rights: a person's decision, as for a local tool.
    widening::allowed(services, vec![Message::new("appMcp.projectTools.widens.newTool")])?;
    let tool = command_tool(args.description.as_deref().unwrap_or_default(), action, None);
    let publication = Publication::of_team_tool(&tool).map_err(|error| edit_refused(&error))?;
    return Ok(Sent::New(NewTeamResource {
      id: ResourceId::generate(),
      kind: ResourceKind::Tool,
      name,
      publication,
    }));
  };
  if args.name.is_some() {
    return Err(refused(
      "name_not_editable",
      Message::new("appMcp.projectTools.errors.nameNotEditable"),
    ));
  }
  based_on((entry.id, entry.number), args.expected_revision.as_deref())?;
  let Some(before) = entry.body.command() else {
    return Err(not_a_command_tool(&entry.name));
  };
  let action = command(args, Some(before), &project, root)?;
  widening::allowed(services, widenings(before, &action))?;
  let description = args.description.as_deref().unwrap_or(entry.body.description());
  let tool = command_tool(description, action, entry.body.access().cloned());
  Ok(Sent::Next(entry.id, entry.number, tool))
}

fn command_tool(
  description: &str,
  action: CommandAction,
  access: Option<orchester_core::resource::AccessPolicy>,
) -> TeamTool {
  TeamTool::new(description, TeamToolAction::Command(action), access).expect("a command tool needs no access policy")
}
