//! The input of the MCP server tools: the fields of a server as Add MCP server and Edit MCP server have them, and the
//! arguments of the tools that add, change, remove and switch one.

use rmcp::schemars::JsonSchema;
use serde::Deserialize;

/// A variable the server's program gets.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Variable {
  pub name: String,
  /// The value as written; `${secret:NAME@environment}` takes the value of the project secret NAME of that
  /// environment when an agent starts.
  pub value: String,
}

/// What a remote server is sent with every request. Each `secret` is an active project secret named
/// `NAME@environment`, whose value is read when an agent starts; no tool takes a secret's value.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "scheme", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub enum Authorization {
  /// Nothing.
  None,
  /// `Authorization: Bearer <the secret's value>`.
  Bearer { secret: String },
  /// `Authorization: Basic <base64 of username:password>`, the password the secret's value.
  Basic { username: String, secret: String },
  /// `Authorization: Basic <credential>`, the secret's value a Basic credential pasted whole: its base64 part, or
  /// the header's whole value starting with `Basic `.
  BasicCredential { secret: String },
  /// `<name>: <the secret's value>`, for a server that takes another header or scheme (`X-API-Key`).
  Header { name: String, secret: String },
}

/// The fields of a server, as Add MCP server and Edit MCP server have them.
#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ServerFields {
  /// Small letters, digits, - and _, starting with a letter; agents know the server and its tools by it.
  pub name: Option<String>,
  /// A program on this computer, which the agent's CLI starts with each agent: an absolute path to an
  /// existing file. Not with `url`.
  pub program: Option<String>,
  /// The program's arguments, passed as written; none may be empty or hold a line break.
  pub args: Option<Vec<String>>,
  /// Where the program starts: a folder inside the worktree or an absolute path; empty: the worktree.
  pub working_directory: Option<String>,
  /// The program's variables.
  pub environment: Option<Vec<Variable>>,
  /// A remote server the agent's CLI reaches over HTTP: its http:// or https:// URL. Not with `program`.
  pub url: Option<String>,
  /// What a remote server is sent to let the agent in; scheme none sends nothing.
  pub authorization: Option<Authorization>,
  /// Tools of the server that run without asking, as the server names them, for profiles that ask
  /// before MCP actions.
  pub exempt_tools: Option<Vec<String>>,
  /// Team MCP servers only: the names of the variables the program needs; each computer gives their values. A team
  /// server holds no value, so `environment` is refused in a team project. With `program`, not with `url`.
  pub variable_names: Option<Vec<String>>,
  /// Team MCP servers only: the names of the headers a remote server is sent a secret in (`Authorization` for a
  /// bearer or Basic credential, `X-API-Key`); each computer gives the secrets. A team server holds no secret, so
  /// `authorization` is refused in a team project. With `url`, not with `program`.
  pub secret_headers: Option<Vec<String>>,
}

impl ServerFields {
  /// The first field only a team project's MCP server has, if one is given.
  pub(super) const fn team_only(&self) -> Option<&'static str> {
    if self.variable_names.is_some() {
      Some("variable_names")
    } else if self.secret_headers.is_some() {
      Some("secret_headers")
    } else {
      None
    }
  }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct AddMcpServer {
  /// The project's id, or its name.
  pub project: String,
  #[serde(flatten)]
  pub fields: ServerFields,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct UpdateMcpServer {
  /// The project's id, or its name.
  pub project: String,
  /// The server's id, or its name.
  pub server: String,
  /// The fields to change; one left out keeps its value.
  #[serde(flatten)]
  pub fields: ServerFields,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InServer {
  /// The project's id, or its name.
  pub project: String,
  /// The server's id, or its name.
  pub server: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetMcpServerEnabled {
  /// The project's id, or its name.
  pub project: String,
  /// The server's id, or its name.
  pub server: String,
  /// On: every profile that chooses it gives it to its agents from their next start; off: none does.
  pub enabled: bool,
}
