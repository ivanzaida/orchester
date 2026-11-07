//! The fields of an MCP server tool put into the page's text (`McpServerText`), as a person would type them: each
//! value refused as the page refuses it, a field the server's transport does not have refused rather than dropped.

use orchester_core::{id::SecretId, secret::Secret};

use super::mcp_server_input::{Authorization, ServerFields};
use crate::{
  app_mcp::error::ToolError,
  connections::mcp::{AuthorizationChoice, AuthorizationText, McpServerText, TransportChoice},
  messages::Message,
};

/// `text` with the `fields` given put in, as a person would type them into the page.
pub(super) fn filled(
  mut text: McpServerText,
  fields: ServerFields,
  secrets: &[Secret],
) -> Result<McpServerText, ToolError> {
  applies(&text, &fields)?;
  match (&fields.program, &fields.url) {
    (Some(_), Some(_)) => return Err(refused("invalid_transport", "appMcp.errors.mcpProgramAndUrl")),
    (Some(_), None) => text.transport = TransportChoice::Stdio,
    (None, Some(_)) => text.transport = TransportChoice::Http,
    (None, None) => {}
  }
  if let Some(name) = fields.name {
    text.name = name;
  }
  if let Some(program) = fields.program {
    // Taken as written: nothing reads the disk before the gate (`widening::allowed`) let the change through.
    text.program = program;
  }
  if let Some(args) = fields.args {
    text.arguments = lines(&args, "appMcp.errors.mcpArgument")?;
  }
  if let Some(folder) = fields.working_directory {
    text.working_directory = folder;
  }
  if let Some(variables) = fields.environment {
    let lines: Vec<String> = variables
      .into_iter()
      .map(|variable| format!("{}={}", variable.name, variable.value))
      .collect();
    text.variables = self::lines(&lines, "appMcp.errors.mcpVariable")?;
  }
  if let Some(url) = fields.url {
    text.url = url;
  }
  if let Some(authorization) = fields.authorization {
    text.authorization = authorization_text(authorization, secrets)?;
  }
  if let Some(tools) = fields.exempt_tools {
    text.exempt_tools = lines(&tools, "appMcp.errors.mcpExemptTool")?;
  }
  Ok(text)
}

/// `values` one per line, none empty and none holding a line break.
fn lines(values: &[String], invalid: &'static str) -> Result<String, ToolError> {
  if let Some(value) = values
    .iter()
    .find(|value| value.trim().is_empty() || value.contains(['\n', '\r']))
  {
    return Err(ToolError::refused(
      "invalid_server",
      vec![Message::new(invalid).with("value", value.clone())],
    ));
  }
  Ok(values.join("\n"))
}

/// The authorization fields `authorization` fills in, its secret named as `secrets` has it.
pub(super) fn authorization_text(
  authorization: Authorization,
  secrets: &[Secret],
) -> Result<AuthorizationText, ToolError> {
  let (choice, secret, username, header) = match authorization {
    Authorization::None => return Ok(AuthorizationText::default()),
    Authorization::Bearer { secret } => (AuthorizationChoice::Bearer, secret, String::new(), String::new()),
    Authorization::Basic { username, secret } => (AuthorizationChoice::Basic, secret, username, String::new()),
    Authorization::BasicCredential { secret } => (
      AuthorizationChoice::BasicCredential,
      secret,
      String::new(),
      String::new(),
    ),
    Authorization::Header { name, secret } => (AuthorizationChoice::Header, secret, String::new(), name),
  };
  Ok(AuthorizationText {
    choice,
    secret: Some(secret_named_by(&secret, secrets)?),
    username,
    header,
  })
}

/// The active secret `NAME@environment` names.
pub(super) fn secret_named_by(named: &str, secrets: &[Secret]) -> Result<SecretId, ToolError> {
  secrets
    .iter()
    .find(|secret| format!("{}@{}", secret.name(), secret.environment().as_str()) == named.trim())
    .map(Secret::id)
    .ok_or_else(|| {
      ToolError::refused(
        "invalid_server",
        vec![Message::new("connections.mcp.errors.unknownSecret").with("secret", named.to_owned())],
      )
    })
}

/// Refuses a field the server's transport (after `fields` chose it) does not have, rather than dropping it.
fn applies(text: &McpServerText, fields: &ServerFields) -> Result<(), ToolError> {
  let transport = match (&fields.program, &fields.url) {
    (Some(_), _) => TransportChoice::Stdio,
    (None, Some(_)) => TransportChoice::Http,
    (None, None) => text.transport,
  };
  let misplaced = match transport {
    TransportChoice::Stdio => vec![("authorization", fields.authorization.is_some())],
    TransportChoice::Http => vec![
      ("args", fields.args.is_some()),
      ("working_directory", fields.working_directory.is_some()),
      ("environment", fields.environment.is_some()),
    ],
  };
  match misplaced.into_iter().find(|(_, given)| *given) {
    Some((field, _)) => Err(ToolError::refused(
      "field_not_applicable",
      vec![Message::new("appMcp.errors.mcpFieldNotApplicable").with("field", field.to_owned())],
    )),
    None => Ok(()),
  }
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}
