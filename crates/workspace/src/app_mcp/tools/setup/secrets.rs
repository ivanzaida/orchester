//! `list_secrets`: a project's active secrets as Secrets lists them, by name, kind and environment, with what
//! uses each: tools, MCP servers, agent connections whose API key it is, and the agent profiles that choose an
//! MCP server using it. A secret's value lives only in the OS keychain; this tool never reads the keychain, so
//! no value can reach its result (`docs/design/orchester-workspace.md`, "Secrets and access").

use orchester_core::{
  id::{ProjectId, SecretId},
  secret::Secret,
};
use rmcp::handler::server::common::schema_for_type;
use serde_json::{Value, json};

use super::super::{
  Spec, Translate,
  across::{InProjects, across},
  parse, wire,
};
use crate::{
  agents::data::ProjectAgents,
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::Link,
  secrets::data::{ProjectSecrets, SecretUseKind},
};

pub(super) const SPECS: &[Spec] = &[Spec {
  name: "list_secrets",
  description: "A project's active secrets, by name and use only: never a value, which stays in the computer's \
                keychain and is not read. Without `project`, every project's, each naming its project \
                (unreadable_projects lists the projects that could not be read, with why). Each: id, name, kind, \
                environment (its scope: a name is unique per environment, written NAME@environment where tools \
                and MCP servers refer to it), when it was added and its value last replaced, what uses it (tools, \
                MCP servers, agent connections whose API key it is, and the agent profiles whose draft or latest \
                version chooses an MCP server that uses it, with that server), each with its link, and the link \
                to Secrets. Adding, replacing and archiving secrets are a person's, in the app.",
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
    "list_secrets" => list_secrets(services, parse(arguments)?, translate).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list_secrets(services: &Services, args: InProjects, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || {
    across(&services, args.project.as_deref(), |_, db| {
      Ok((ProjectSecrets::load(db)?, ProjectAgents::load(db)?))
    })
  })
  .await?;
  Ok(read.json("secrets", translate, |open, (secrets, agents)| {
    secrets
      .in_environment(None)
      .map(|secret| secret_json(secret, (secrets, agents), open.project.id()))
      .collect()
  }))
}

/// `secret` of `project` by its name, kind and environment, with what uses it: no value.
fn secret_json(secret: &Secret, (secrets, agents): (&ProjectSecrets, &ProjectAgents), project: ProjectId) -> Value {
  let used = |link: Link, name: &str, id: String| json!({ "id": id, "name": name, "link": link.path() });
  let (mut tools, mut servers) = (Vec::new(), Vec::new());
  for using in secrets.used_by(secret.id()) {
    match using.kind {
      SecretUseKind::Tool(id) => tools.push(used(Link::Tool(project, id), &using.name, id.to_string())),
      SecretUseKind::McpServer(id) => servers.push(used(Link::Connection(project, id), &using.name, id.to_string())),
    }
  }
  let connections: Vec<Value> = agents
    .connections
    .iter()
    .filter(|connection| connection.api().is_some_and(|api| api.key() == secret.id()))
    .map(|connection| {
      let id = connection.id();
      used(
        Link::Connection(project, id),
        connection.name().as_str(),
        id.to_string(),
      )
    })
    .collect();
  json!({
    "id": secret.id().to_string(),
    "name": secret.name().to_string(),
    "kind": secret.kind().as_str(),
    "environment": secret.environment().as_str(),
    "added_at": wire::time(secret.created_at()),
    "replaced_at": secret.rotated_at().map(wire::time),
    "used_by": {
      "tools": tools,
      "mcp_servers": servers,
      "agent_connections": connections,
      "profiles": profiles_using(agents, secret.id(), project),
    },
    "link": Link::Secrets(project).path(),
  })
}

/// The profiles whose draft or latest version chooses an MCP server that uses `secret`, each with those servers.
fn profiles_using(agents: &ProjectAgents, secret: SecretId, project: ProjectId) -> Vec<Value> {
  agents
    .profiles
    .iter()
    .filter_map(|entry| {
      let through: Vec<&str> = agents
        .mcp
        .iter()
        .filter(|server| entry.profile.mcp_servers().ids().contains(&server.id()) && server.secrets().contains(&secret))
        .map(|server| server.name().as_str())
        .collect();
      (!through.is_empty()).then(|| {
        json!({
          "id": entry.id.to_string(),
          "role": entry.profile.role().as_str(),
          "through_mcp_servers": through,
          "link": Link::Profile(project, entry.id).path(),
        })
      })
    })
    .collect()
}
