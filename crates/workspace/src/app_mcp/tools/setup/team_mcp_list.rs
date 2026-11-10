//! `list_team_mcp_servers`: the team's MCP servers of a team project, each with the version this computer uses, how it
//! launches and where it stands here (`readiness_json`): switched on, off, or waiting for a person, with what is
//! missing. The team's servers are resources, not the project's own rows `list_connections` lists, so nothing else
//! lists them. A local project has none. Nothing here names a program's path or a secret's value.

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamMcpServer},
};
use rmcp::handler::server::common::schema_for_type;
use serde_json::{Value, json};

use super::{
  super::{
    Spec, Translate,
    across::{InProjects, across},
    parse, readiness_json,
  },
  team_mcp_body as body,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  readiness::ProjectReadiness,
  team_config::TeamConfig,
};

pub(super) const SPECS: &[Spec] = &[Spec {
  name: "list_team_mcp_servers",
  description: "The team's MCP servers of a team project (a local project has none; list_connections lists the \
                project's own), each: id, name, version (the one this computer uses), how it launches (program: its \
                name, arguments, folder and variable names; or remote: its address and secret headers by name), \
                and readiness here: ready_here, group (ready, needs_you_here when nobody decided on it here or \
                something is missing, off_here when it was switched off here) and the missing parts in the order to \
                fix them, each with the tool that fixes it and who may use it (kind mcp_off: set_mcp_server_enabled, \
                on is a person's alone; mcp_program: the program's path on this computer, a person's in the app; \
                secret: a team secret's value, a person's in the app; undecided: what cannot be known yet). A \
                program's path and a secret's value are never listed. Without `project`, every project's, each \
                naming its project (unreadable_projects lists the projects that could not be read, with why).",
  schema: schema_for_type::<InProjects>,
  read_only: true,
}];

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_team_mcp_servers" => list(services, parse(arguments)?, translate).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list(services: &Services, args: InProjects, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || {
    across(&services, args.project.as_deref(), |open, db| {
      if db.sharing()?.is_none() {
        return Ok(None);
      }
      let readiness = readiness_json::read(&services, db, &open.repo_root)?;
      let read = services.workspace.team_read(&open.repo_root);
      let servers = read
        .config()
        .map_or_else(Vec::new, |config| servers_of(config, &readiness));
      Ok(Some((readiness, servers)))
    })
  })
  .await?;
  Ok(read.json("mcp_servers", translate, |_, found| {
    found.as_ref().map_or_else(Vec::new, |(readiness, servers)| {
      servers.iter().map(|server| server_json(server, readiness)).collect()
    })
  }))
}

/// A team MCP server with the body of the version this computer uses.
struct Listed {
  id: ResourceId,
  version: u32,
  server: Value,
}

/// The team's MCP servers readiness lists, each with the body of its latest version in `config`, the team's
/// configuration as it was read this session.
fn servers_of(config: &TeamConfig, readiness: &ProjectReadiness) -> Vec<Listed> {
  readiness
    .resources()
    .iter()
    .filter(|found| found.kind() == ResourceKind::McpServer)
    .filter_map(|found| {
      let Some(Ok(body)) = config.resource(found.resource())?.body::<TeamMcpServer>() else {
        return None;
      };
      Some(Listed {
        id: found.resource(),
        version: found.number().get(),
        server: body::server_json(&body),
      })
    })
    .collect()
}

fn server_json(listed: &Listed, readiness: &ProjectReadiness) -> Value {
  let mut server = listed.server.clone();
  server["id"] = json!(listed.id.to_string());
  server["team"] = json!(true);
  server["version"] = json!(listed.version);
  server["readiness"] = readiness_json::resource_of(readiness, listed.id);
  server
}
