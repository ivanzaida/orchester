//! An MCP server of the project as `list_connections` and the MCP server tools show it.

use std::path::Path;

use orchester_core::{
  connection::mcp::{McpAuthorization, McpEnvValue, McpServer, McpTransport},
  id::{ConnectionId, ProjectId, SecretId},
};
use serde_json::{Value, json};

use crate::{
  app_mcp::error::{Missing, ToolError},
  connections::data::ProjectConnections,
  links::Link,
};

/// The MCP servers of `project`, as `list_connections` lists them, each with its link: its inspector in
/// Connections.
pub(super) fn servers_json(listed: &ProjectConnections, project: ProjectId) -> Vec<Value> {
  listed
    .mcp
    .iter()
    .map(|server| {
      let mut value = server_json(listed, server);
      value["link"] = json!(Link::Connection(project, server.id()).path());
      value
    })
    .collect()
}

/// `server` with the fields Edit MCP server shows, and the roles of the profiles that choose it. A secret is named
/// (`NAME@environment`), never its value; a variable's plain value and the values of the endpoint's query are left
/// out, since a person may have pasted a token there: automation sees that they are set, not what they hold.
fn server_json(listed: &ProjectConnections, server: &McpServer) -> Value {
  let secrets = &listed.secrets;
  let secret_named = |id| {
    secrets
      .iter()
      .find(|secret| secret.id() == id)
      .map(|secret| format!("{}@{}", secret.name(), secret.environment().as_str()))
  };
  let transport = match server.transport() {
    McpTransport::Stdio(stdio) => json!({
      "kind": "stdio",
      "program": stdio.program().as_str(),
      "args": stdio.args().iter().map(|arg| arg.as_str()).collect::<Vec<_>>(),
      "working_directory": stdio.working_directory().map(|folder| folder.as_str()),
    }),
    McpTransport::Http(http) => json!({
      "kind": "http",
      "url": without_query_values(http.url().as_str()),
      "authorization": authorization_json(http.authorization(), secret_named),
    }),
  };
  let environment: Vec<Value> = server
    .environment()
    .iter()
    .map(|(name, value)| match value {
      McpEnvValue::Text(_) => json!({ "name": name.to_string(), "kind": "text" }),
      McpEnvValue::Secret(id) => json!({
        "name": name.to_string(),
        "kind": "secret",
        "secret": secret_named(*id),
      }),
    })
    .collect();
  let used_by: Vec<&str> = listed.used_by(server.id()).map(|used| &*used.role).collect();
  json!({
    "id": server.id().to_string(),
    "name": server.name().as_str(),
    "enabled": server.enabled(),
    "transport": transport,
    "environment": environment,
    "exempt_tools": server.exempt_tools().tools().iter().map(|tool| tool.as_str()).collect::<Vec<_>>(),
    "used_by": used_by,
  })
}

/// `url` with each query value replaced by `…`, its names kept, and a fragment's text left out.
pub(super) fn without_query_values(url: &str) -> String {
  let Some((base, query)) = url.split_once('?') else {
    return url.to_owned();
  };
  let (query, fragment) = query.split_once('#').map_or((query, false), |(query, _)| (query, true));
  let names: Vec<String> = query
    .split('&')
    .map(|pair| match pair.split_once('=') {
      Some((name, _)) => format!("{name}=…"),
      None => pair.to_owned(),
    })
    .collect();
  let fragment = if fragment { "#…" } else { "" };
  format!("{base}?{}{fragment}", names.join("&"))
}

pub(super) fn server_result(repo_root: &Path, id: ConnectionId) -> Result<Value, ToolError> {
  let listed = ProjectConnections::load_in(repo_root)?;
  let server = listed
    .mcp_server(id)
    .ok_or_else(|| ToolError::not_found(Missing::McpServer, id.to_string()))?;
  Ok(json!({ "server": server_json(&listed, server) }))
}

/// `authorization` as `list_connections` shows it: its scheme, the user name or header name, and its secret named.
fn authorization_json(authorization: &McpAuthorization, secret_named: impl Fn(SecretId) -> Option<String>) -> Value {
  let secret = authorization.secret().and_then(secret_named);
  match authorization {
    McpAuthorization::None => json!({ "scheme": "none" }),
    McpAuthorization::Bearer { .. } => json!({ "scheme": "bearer", "secret": secret }),
    McpAuthorization::Basic { username, .. } => {
      json!({ "scheme": "basic", "username": username.as_str(), "secret": secret })
    }
    McpAuthorization::BasicCredential { .. } => json!({ "scheme": "basic_credential", "secret": secret }),
    McpAuthorization::Header { name, .. } => json!({ "scheme": "header", "name": name.as_str(), "secret": secret }),
  }
}
