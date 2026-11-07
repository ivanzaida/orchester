//! A team MCP server's body (core's `TeamMcpServer`: what the team server holds of one) from the fields of the MCP
//! server tools, and back to what they answer. The body holds names only: a program is named, never located, a
//! variable takes the team secret of its own name and a secret header names its secret and a plain-text prefix, whose
//! values each computer fills in. So a path, a value or a local id has no field to go in, and one given is refused with
//! its own code (`team_path_refused`, `team_value_refused`) instead of being dropped. A first version widens nothing by
//! itself ([`widens`]); creating one is gated by its caller.

use orchester_core::{
  connection::mcp::{McpHeaderName, McpServerName, McpTransport},
  resource::{
    HeaderPrefix, TeamMcpLaunch, TeamMcpProgram, TeamMcpRemote, TeamMcpSecretHeader, TeamMcpServer, TeamMcpServerError,
    VariableName,
  },
  secret::SecretName,
  sharing::{VersionWidening, widens},
};
use serde_json::{Value, json};

use super::{
  mcp_server_input::{SecretHeader, ServerFields},
  mcp_server_json::without_query_values,
  mcp_server_text::filled,
};
use crate::{
  app_mcp::error::ToolError,
  connections::mcp::{McpServerText, NAME_TAKEN, TransportChoice, authorization_message},
  messages::Message,
};

/// A new team server from `fields`, its name not one of `taken`.
pub(super) fn new_server(fields: ServerFields, taken: &[McpServerName]) -> Result<TeamMcpServer, ToolError> {
  built(McpServerText::new_server(), fields, (None, taken))
}

/// `before` with the `fields` given put in, as a person would change it. A team server keeps the name it was made
/// with: the resource's name on the server and the body's would otherwise differ.
pub(super) fn changed_server(before: &TeamMcpServer, mut fields: ServerFields) -> Result<TeamMcpServer, ToolError> {
  if fields.name.take().is_some_and(|name| name != before.name().as_str()) {
    return Err(refused(
      "name_not_editable",
      Message::new("appMcp.errors.teamMcpNameNotEditable"),
    ));
  }
  built(text_of(before), fields, (Some(before), &[]))
}

/// Why `after` widens over `before`, the version this computer uses: what a person's gate asks about.
pub(super) fn widenings(before: &TeamMcpServer, after: &TeamMcpServer) -> Vec<Message> {
  let server = || after.name().as_str().to_owned();
  widens(Some(before), after)
    .into_iter()
    .map(|widening| match (widening, before.launch(), after.launch()) {
      (VersionWidening::Command(_), TeamMcpLaunch::Program(was), TeamMcpLaunch::Program(now)) => {
        if was.program() == now.program() {
          Message::new("appMcp.widens.mcpArguments")
        } else {
          Message::new("appMcp.widens.mcpProgram").with("program", now.program().as_str().to_owned())
        }
      }
      (VersionWidening::Address, _, TeamMcpLaunch::Remote(now)) => {
        Message::new("appMcp.widens.mcpEndpoint").with("endpoint", without_query_values(now.url().as_str()))
      }
      (VersionWidening::Access, ..) => {
        let added: Vec<&str> = after
          .exempt_tools()
          .tools()
          .iter()
          .map(|tool| tool.as_str())
          .filter(|tool| !before.exempt_tools().contains(tool))
          .collect();
        Message::new("appMcp.widens.mcpExemptTools").with("tools", added.join(", "))
      }
      _ => Message::new("appMcp.widens.teamMcpServer").with("server", server()),
    })
    .collect()
}

/// `server` as the tools answer: what the team holds of it, with the program named and no path or value.
pub(super) fn server_json(server: &TeamMcpServer) -> Value {
  let launch = match server.launch() {
    TeamMcpLaunch::Program(program) => json!({
      "kind": "program",
      "program": program.program().as_str(),
      "args": program.args().iter().map(|arg| arg.as_str()).collect::<Vec<_>>(),
      "working_directory": program.working_directory().map(|folder| folder.as_str()),
      "variable_names": program.variables().iter().map(VariableName::as_str).collect::<Vec<_>>(),
    }),
    TeamMcpLaunch::Remote(remote) => json!({
      "kind": "remote",
      "url": without_query_values(remote.url().as_str()),
      "secret_headers": remote.secret_headers().iter().map(|header| json!({
        "header": header.header().as_str(),
        "secret": header.secret().as_str(),
        "prefix": header.prefix().map(HeaderPrefix::as_str),
      })).collect::<Vec<_>>(),
    }),
  };
  json!({
    "name": server.name().as_str(),
    "launch": launch,
    "exempt_tools": server.exempt_tools().tools().iter().map(|tool| tool.as_str()).collect::<Vec<_>>(),
  })
}

/// The page's text of `server`: what the team body holds of the fields the page checks.
fn text_of(server: &TeamMcpServer) -> McpServerText {
  let mut text = McpServerText {
    name: server.name().as_str().to_owned(),
    exempt_tools: server
      .exempt_tools()
      .tools()
      .iter()
      .map(|tool| tool.as_str())
      .collect::<Vec<_>>()
      .join("\n"),
    ..McpServerText::new_server()
  };
  match server.launch() {
    TeamMcpLaunch::Program(program) => {
      text.program = program.program().as_str().to_owned();
      text.arguments = program
        .args()
        .iter()
        .map(|arg| arg.as_str())
        .collect::<Vec<_>>()
        .join("\n");
      text.working_directory = program
        .working_directory()
        .map_or_else(String::new, |folder| folder.as_str().to_owned());
    }
    TeamMcpLaunch::Remote(remote) => {
      text.transport = TransportChoice::Http;
      text.url = remote.url().as_str().to_owned();
    }
  }
  text
}

/// The team server `text` describes with the `fields` put in, each checked as the page checks it and as the team body
/// does. The variables or secret headers not given are those of `before`, the server being changed (none for a new
/// one).
fn built(
  text: McpServerText,
  mut fields: ServerFields,
  (before, taken): (Option<&TeamMcpServer>, &[McpServerName]),
) -> Result<TeamMcpServer, ToolError> {
  if let Some(field) = values_given(&fields) {
    return Err(refused(
      "team_value_refused",
      Message::new("appMcp.errors.teamMcpValueRefused").with("field", field.to_owned()),
    ));
  }
  let (variables, headers) = (fields.variable_names.take(), fields.secret_headers.take());
  let text = filled(text, fields, &[])?;
  let draft = text.draft(taken, &[]).map_err(|errors| {
    let code = match &errors.name {
      Some(name) if name.key == NAME_TAKEN => "name_taken",
      _ => "invalid_server",
    };
    let reasons: Vec<Message> = errors.in_order().into_iter().flatten().cloned().collect();
    ToolError::refused(code, reasons)
  })?;
  let launch = match draft.transport {
    McpTransport::Stdio(stdio) => {
      not_here(headers.is_some(), "secret_headers")?;
      let variables = variables.unwrap_or_else(|| kept_variables(before));
      let variables = variables
        .iter()
        .map(|name| {
          VariableName::new(name).map_err(|_| {
            refused(
              "invalid_server",
              Message::new("tools.errors.variableName").with("name", name.clone()),
            )
          })
        })
        .collect::<Result<Vec<_>, _>>()?;
      let program = TeamMcpProgram::new(
        stdio.program().clone(),
        stdio.args().to_vec(),
        stdio.working_directory().cloned(),
        variables,
      );
      TeamMcpLaunch::Program(program.map_err(|error| body_refusal(&error))?)
    }
    McpTransport::Http(http) => {
      not_here(variables.is_some(), "variable_names")?;
      let headers = match headers {
        Some(given) => given.iter().map(secret_header).collect::<Result<Vec<_>, _>>()?,
        None => kept_headers(before),
      };
      TeamMcpLaunch::Remote(TeamMcpRemote::new(http.url().clone(), headers).map_err(|error| body_refusal(&error))?)
    }
  };
  Ok(TeamMcpServer::new(draft.name, launch, draft.exempt_tools))
}

/// The variable names of `server` if it runs a program, else none.
fn kept_variables(server: Option<&TeamMcpServer>) -> Vec<String> {
  match server.map(TeamMcpServer::launch) {
    Some(TeamMcpLaunch::Program(program)) => program
      .variables()
      .iter()
      .map(|name| name.as_str().to_owned())
      .collect(),
    _ => Vec::new(),
  }
}

/// The secret headers of `server` if it is a remote one, else none.
fn kept_headers(server: Option<&TeamMcpServer>) -> Vec<TeamMcpSecretHeader> {
  match server.map(TeamMcpServer::launch) {
    Some(TeamMcpLaunch::Remote(remote)) => remote.secret_headers().clone(),
    _ => Vec::new(),
  }
}

/// The first field that would put a value in the body, if one is given.
const fn values_given(fields: &ServerFields) -> Option<&'static str> {
  if fields.environment.is_some() {
    Some("environment")
  } else if fields.authorization.is_some() {
    Some("authorization")
  } else {
    None
  }
}

/// Refused when `given`: `field` belongs to the other transport than the server's.
fn not_here(given: bool, field: &str) -> Result<(), ToolError> {
  if given {
    return Err(refused(
      "field_not_applicable",
      Message::new("appMcp.errors.mcpFieldNotApplicable").with("field", field.to_owned()),
    ));
  }
  Ok(())
}

/// A secret header as given: its name, the team secret its value comes from and the prefix of the value. Neither
/// refusal repeats the secret or the prefix: a value put there by mistake must not reach the answer.
fn secret_header(given: &SecretHeader) -> Result<TeamMcpSecretHeader, ToolError> {
  let name = header(&given.header)?;
  let secret = SecretName::new(&given.secret).map_err(|_| {
    refused(
      "invalid_server",
      Message::new("appMcp.errors.teamMcpSecretName").with("header", given.header.clone()),
    )
  })?;
  let prefix = given
    .prefix
    .as_deref()
    .map(HeaderPrefix::new)
    .transpose()
    .map_err(|_| {
      refused(
        "invalid_server",
        Message::new("appMcp.errors.teamMcpHeaderPrefix")
          .with("header", given.header.clone())
          .with("max", HeaderPrefix::MAX_CHARS.to_string()),
      )
    })?;
  Ok(TeamMcpSecretHeader::new(name, secret, prefix))
}

/// A secret header's name; `Authorization`, which the page reserves for its bearer and Basic schemes, is the name of
/// those.
fn header(name: &str) -> Result<McpHeaderName, ToolError> {
  if name.trim().eq_ignore_ascii_case("authorization") {
    return Ok(McpHeaderName::authorization());
  }
  McpHeaderName::new(name).map_err(|error| ToolError::refused("invalid_server", vec![authorization_message(&error)]))
}

/// The body's own refusals: a path, an absolute folder or a name listed twice. The rest is checked before.
fn body_refusal(error: &TeamMcpServerError) -> ToolError {
  match error {
    TeamMcpServerError::ProgramPath(program) => refused(
      "team_path_refused",
      Message::new("appMcp.errors.teamMcpProgramIsPath").with("program", program.clone()),
    ),
    TeamMcpServerError::AbsoluteFolder(folder) => refused(
      "team_path_refused",
      Message::new("appMcp.errors.teamMcpFolderIsAbsolute").with("folder", folder.clone()),
    ),
    TeamMcpServerError::UrlQuery => refused(
      "team_value_refused",
      Message::new("appMcp.errors.teamMcpUrlQuery").with("field", "url".to_owned()),
    ),
    TeamMcpServerError::VariableNotSecret(name) => refused(
      "invalid_server",
      Message::new("appMcp.errors.teamMcpVariableNotSecret").with("name", name.clone()),
    ),
    TeamMcpServerError::Duplicate(name) => refused(
      "invalid_server",
      Message::new("appMcp.errors.teamMcpDuplicate").with("name", name.clone()),
    ),
    // Name, program, arguments, folder, URL, header, secret and prefix were checked as the page checks them before the
    // body was made.
    _ => refused("invalid_server", Message::new("appMcp.errors.teamMcpInvalid")),
  }
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
