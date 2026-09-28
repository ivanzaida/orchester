//! The MCP servers of one agent invocation, as each CLI is given them (`docs/design/orchester-workspace.md`,
//! "MCP servers"; checked against Codex CLI 0.139 and Claude Code 2.1.89 on 2026-09-28):
//!
//! - **Codex**: configuration overrides next to `--ignore-user-config`, which keeps the user's own servers out: `-c
//!   mcp_servers.<name>.command=… .args=[…] .cwd=… .env.<VAR>=…` for a program, `-c mcp_servers.<name>.url=…` for an
//!   HTTP server. A value from a secret never goes into an argument: the variable is set on the Codex process and
//!   listed in `mcp_servers.<name>.env_vars`, which hands it to the server (Codex starts a server with a few system
//!   variables, its `env` and its `env_vars` only), and a bearer token goes the same way through
//!   `bearer_token_env_var`. `shell_environment_policy.exclude` keeps those variables out of the commands the agent
//!   runs. Because variables pass by name, two servers cannot take one name from different secrets, and a secret
//!   variable cannot be one the CLI itself needs.
//! - **Claude Code**: `--strict-mcp-config --mcp-config <file>`, a JSON file the runner writes for the invocation only
//!   and removes when the CLI has ended ([`ClaudeMcpConfig`]); secret values are written into it, nowhere else. Claude
//!   Code ignores a server's `cwd` and starts every server in the worktree, so a server set to start in another folder
//!   is left out.
//!
//! Values are TOML for Codex (`-c` parses them as TOML): strings are written as basic strings, so they need no
//! shell, and pass the `.cmd` launcher only when it is npm's Node.js launcher, which is started directly.

use std::{collections::BTreeMap, fmt, sync::Arc};

use serde::Serialize;
use zeroize::Zeroizing;

use super::is_agent_variable;
use crate::{
  command::SecretVariable,
  connection::mcp::{McpServerName, McpServerProblem, McpUrl},
  resource::VariableName,
  secret::SecretValue,
};

/// The value a server's variable starts with.
#[derive(Clone, Debug)]
pub enum LaunchValue {
  Text(String),
  /// Read from the keychain for this invocation; `Debug` shows no content.
  Secret(Arc<SecretValue>),
}

#[derive(Clone, Debug)]
enum Transport {
  Stdio {
    program: String,
    args: Vec<String>,
    working_directory: Option<String>,
    environment: Vec<(VariableName, LaunchValue)>,
  },
  Http {
    url: McpUrl,
    bearer: Option<Arc<SecretValue>>,
  },
}

/// One MCP server as an invocation gives it to its CLI: its program found and its secrets read.
#[derive(Clone, Debug)]
pub struct McpLaunch {
  name: McpServerName,
  transport: Transport,
}

impl McpLaunch {
  /// A server the CLI starts: `program` (a file found on the `PATH` or in the worktree) with `args`,
  /// in `working_directory` (`None`: the worktree), with `environment`.
  pub fn stdio(
    name: McpServerName,
    (program, args): (String, Vec<String>),
    working_directory: Option<String>,
    environment: Vec<(VariableName, LaunchValue)>,
  ) -> Self {
    Self {
      name,
      transport: Transport::Stdio {
        program,
        args,
        working_directory,
        environment,
      },
    }
  }

  pub const fn http(name: McpServerName, url: McpUrl, bearer: Option<Arc<SecretValue>>) -> Self {
    Self {
      name,
      transport: Transport::Http { url, bearer },
    }
  }

  pub const fn name(&self) -> &McpServerName {
    &self.name
  }

  /// Secret variables the server takes through the CLI's environment, by name; with a bearer
  /// token under [`bearer_variable`].
  fn secret_variables(&self) -> Vec<(String, &Arc<SecretValue>)> {
    match &self.transport {
      Transport::Stdio { environment, .. } => environment
        .iter()
        .filter_map(|(name, value)| match value {
          LaunchValue::Secret(secret) => Some((name.as_str().to_owned(), secret)),
          LaunchValue::Text(_) => None,
        })
        .collect(),
      Transport::Http { bearer, .. } => bearer
        .iter()
        .map(|secret| (bearer_variable(&self.name), secret))
        .collect(),
    }
  }
}

/// A server left out of an invocation, and why.
pub type LeftOut = (McpServerName, McpServerProblem);

/// The variable a Codex invocation hands `server`'s bearer token over in.
pub fn bearer_variable(server: &McpServerName) -> String {
  format!(
    "ORCHESTER_MCP_TOKEN_{}",
    server.as_str().to_ascii_uppercase().replace('-', "_")
  )
}

/// The MCP arguments of a Codex invocation and the secret variables its process starts with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodexMcp {
  args: Vec<String>,
  secrets: Vec<SecretVariable>,
}

impl CodexMcp {
  /// `servers` as Codex takes them, and the servers left out because their secret variables cannot
  /// reach them.
  pub fn new(servers: &[McpLaunch]) -> (Self, Vec<LeftOut>) {
    let mut mcp = Self::default();
    let mut left_out = Vec::new();
    for server in servers {
      match mcp.shared_secrets(server) {
        Ok(new) => {
          mcp.add(server);
          mcp.secrets.extend(new);
        }
        Err(problem) => left_out.push((server.name.clone(), problem)),
      }
    }
    if !mcp.secrets.is_empty() {
      let names: Vec<&str> = mcp.secrets.iter().map(SecretVariable::name).collect();
      mcp.args.extend([
        "-c".to_owned(),
        format!("shell_environment_policy.exclude={}", toml_array(&names)),
      ]);
    }
    (mcp, left_out)
  }

  /// Arguments after the sandbox settings, in order.
  pub fn args(&self) -> &[String] {
    &self.args
  }

  /// Variables of the Codex process that its servers take their secrets from.
  pub fn secrets(&self) -> &[SecretVariable] {
    &self.secrets
  }

  /// The secret variables `server` adds to the Codex process, or why it cannot have them.
  fn shared_secrets(&self, server: &McpLaunch) -> Result<Vec<SecretVariable>, McpServerProblem> {
    let mut added: Vec<SecretVariable> = Vec::new();
    for (name, value) in server.secret_variables() {
      if is_agent_variable(&name) {
        return Err(McpServerProblem::ReservedVariable { variable: name });
      }
      let taken = self
        .secrets
        .iter()
        .chain(&added)
        .find(|taken| taken.name().eq_ignore_ascii_case(&name));
      match taken {
        Some(taken) if *taken == SecretVariable::new(&name, value.clone()) => {}
        Some(_) => return Err(McpServerProblem::SharedVariable { variable: name }),
        None => added.push(SecretVariable::new(&name, value.clone())),
      }
    }
    Ok(added)
  }

  fn add(&mut self, server: &McpLaunch) {
    let key = format!("mcp_servers.{}", server.name);
    let mut set = |field: &str, value: String| {
      self.args.extend(["-c".to_owned(), format!("{key}.{field}={value}")]);
    };
    match &server.transport {
      Transport::Stdio {
        program,
        args,
        working_directory,
        environment,
      } => {
        set("command", toml_string(program));
        if !args.is_empty() {
          set("args", toml_array(args));
        }
        if let Some(directory) = working_directory {
          set("cwd", toml_string(directory));
        }
        let mut secret_names = Vec::new();
        for (name, value) in environment {
          match value {
            LaunchValue::Text(text) => set(&format!("env.{name}"), toml_string(text)),
            LaunchValue::Secret(_) => secret_names.push(name.as_str()),
          }
        }
        if !secret_names.is_empty() {
          set("env_vars", toml_array(&secret_names));
        }
      }
      Transport::Http { url, bearer } => {
        set("url", toml_string(url.as_str()));
        if bearer.is_some() {
          set("bearer_token_env_var", toml_string(&bearer_variable(&server.name)));
        }
      }
    }
  }
}

/// The `--mcp-config` file of a Claude Code invocation: JSON with the servers' secret values in it.
/// `Debug` shows the servers' names only, the memory holding the text is wiped when it is dropped,
/// and the runner writes it to a file only it and the CLI read, removed when the CLI has ended.
pub struct ClaudeMcpConfig {
  contents: Zeroizing<String>,
  servers: Vec<McpServerName>,
}

impl ClaudeMcpConfig {
  /// `servers` as Claude Code takes them, and the ones left out; `None` when none is left.
  pub fn new(servers: &[McpLaunch]) -> (Option<Self>, Vec<LeftOut>) {
    let (kept, left_out): (Vec<&McpLaunch>, Vec<&McpLaunch>) = servers.iter().partition(|server| {
      !matches!(
        &server.transport,
        Transport::Stdio {
          working_directory: Some(_),
          ..
        }
      )
    });
    let left_out = left_out
      .into_iter()
      .map(|server| (server.name.clone(), McpServerProblem::WorkingDirectoryUnsupported))
      .collect();
    if kept.is_empty() {
      return (None, left_out);
    }
    let bearers: Vec<Zeroizing<String>> = kept
      .iter()
      .map(|server| match &server.transport {
        Transport::Http {
          bearer: Some(token), ..
        } => Zeroizing::new(format!("Bearer {}", token.expose())),
        _ => Zeroizing::new(String::new()),
      })
      .collect();
    let entries = kept
      .iter()
      .zip(&bearers)
      .map(|(server, bearer)| (server.name.as_str(), ClaudeServer::of(server, bearer)))
      .collect();
    let contents =
      serde_json::to_string(&ClaudeFile { servers: entries }).expect("strings and maps of strings serialize to JSON");
    let config = Self {
      contents: Zeroizing::new(contents),
      servers: kept.iter().map(|server| server.name.clone()).collect(),
    };
    (Some(config), left_out)
  }

  /// The file's text, for the one place that writes the file.
  pub fn contents(&self) -> &str {
    &self.contents
  }

  /// The servers in the file, whose tools the invocation allows.
  pub fn servers(&self) -> &[McpServerName] {
    &self.servers
  }
}

impl fmt::Debug for ClaudeMcpConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ClaudeMcpConfig")
      .field("servers", &self.servers)
      .field("contents", &"<redacted>")
      .finish()
  }
}

#[derive(Serialize)]
struct ClaudeFile<'a> {
  #[serde(rename = "mcpServers")]
  servers: BTreeMap<&'a str, ClaudeServer<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClaudeServer<'a> {
  Stdio {
    command: &'a str,
    args: &'a [String],
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<&'a str, &'a str>,
  },
  Http {
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<BTreeMap<&'static str, &'a str>>,
  },
}

impl<'a> ClaudeServer<'a> {
  fn of(server: &'a McpLaunch, bearer: &'a str) -> Self {
    match &server.transport {
      Transport::Stdio {
        program,
        args,
        environment,
        ..
      } => Self::Stdio {
        command: program,
        args,
        env: environment
          .iter()
          .map(|(name, value)| {
            let value = match value {
              LaunchValue::Text(text) => text.as_str(),
              LaunchValue::Secret(secret) => secret.expose(),
            };
            (name.as_str(), value)
          })
          .collect(),
      },
      Transport::Http { url, bearer: token } => Self::Http {
        url: url.as_str(),
        headers: token.as_ref().map(|_| BTreeMap::from([("Authorization", bearer)])),
      },
    }
  }
}

/// `value` as a TOML basic string.
pub(super) fn toml_string(value: &str) -> String {
  let mut quoted = String::with_capacity(value.len() + 2);
  quoted.push('"');
  for c in value.chars() {
    match c {
      '"' => quoted.push_str("\\\""),
      '\\' => quoted.push_str("\\\\"),
      c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", u32::from(c))),
      c => quoted.push(c),
    }
  }
  quoted.push('"');
  quoted
}

pub(super) fn toml_array(values: &[impl AsRef<str>]) -> String {
  let items: Vec<String> = values.iter().map(|value| toml_string(value.as_ref())).collect();
  format!("[{}]", items.join(","))
}
