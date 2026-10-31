//! The MCP servers of one agent invocation, as each CLI is given them (`docs/design/orchester-workspace.md`,
//! "MCP servers"; checked against Codex CLI 0.139 and Claude Code 2.1.89 on 2026-09-28):
//!
//! - **Codex**: configuration overrides, `-c mcp_servers.<name>.command=… .args=[…] .cwd=… .env.<VAR>=…` for a program,
//!   `-c mcp_servers.<name>.url=…` for an HTTP server; the user's own servers are turned off for the thread
//!   (`crate::connection::app_server_mcp`), and a server named like one of theirs gets another name in Codex
//!   ([`CodexMcp::named_apart`]). A value from a secret never goes into an argument: the variable is set on the Codex
//!   process and listed in `mcp_servers.<name>.env_vars`, which hands it to the server (Codex starts a server with a
//!   few system variables, its `env` and its `env_vars` only), a bearer token goes the same way through
//!   `bearer_token_env_var`, and another header's value through `env_http_headers = { "<header>" = "<VAR>" }` (Codex
//!   0.137 lists it in `codex mcp list --json`, 2026-10-25). `shell_environment_policy.exclude` kept a test variable
//!   out of an agent shell command in a signed-in Codex 0.139 turn (2026-09-29). Because variables pass by name, two
//!   servers cannot take one name from different secrets, and a secret variable cannot be one the CLI itself needs.
//! - **Claude Code**: `--strict-mcp-config --mcp-config <file>`, a JSON file the runner writes for the invocation only
//!   and removes when the CLI has ended ([`ClaudeMcpConfig`]); secret values, a server's `headers` among them, are
//!   written into it, nowhere else. Claude Code ignores a server's `cwd` and starts every server in the worktree, so a
//!   server set to start in another folder is left out.
//!
//! Values are TOML for Codex (`-c` parses them as TOML): strings are written as basic strings, so they need no
//! shell, and pass the `.cmd` launcher only when it is npm's Node.js launcher, which is started directly.

use std::{
  collections::{BTreeMap, BTreeSet},
  fmt,
  sync::Arc,
};

use serde::Serialize;
use zeroize::Zeroizing;

use super::{CodexOrchester, HttpAuthorization, is_agent_variable};
use crate::{
  command::SecretVariable,
  connection::mcp::{ExemptTools, InvocationExemptions, McpServerName, McpServerProblem, McpToolName, McpUrl},
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
    authorization: HttpAuthorization,
  },
}

/// One MCP server as an invocation gives it to its CLI: its program found and its secrets read.
#[derive(Clone, Debug)]
pub struct McpLaunch {
  name: McpServerName,
  transport: Transport,
  exempt: ExemptTools,
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
      exempt: ExemptTools::none(),
    }
  }

  pub const fn http(name: McpServerName, url: McpUrl, authorization: HttpAuthorization) -> Self {
    Self {
      name,
      transport: Transport::Http { url, authorization },
      exempt: ExemptTools::none(),
    }
  }

  /// This server, its tools `exempt` running without asking when the step asks before MCP actions.
  #[must_use]
  pub fn with_exempt_tools(mut self, exempt: ExemptTools) -> Self {
    self.exempt = exempt;
    self
  }

  pub const fn name(&self) -> &McpServerName {
    &self.name
  }

  /// Secret variables the server takes through the CLI's environment, by name; with the bearer token or the
  /// header's value under [`token_variable`].
  fn secret_variables(&self) -> Vec<(String, &Arc<SecretValue>)> {
    match &self.transport {
      Transport::Stdio { environment, .. } => environment
        .iter()
        .filter_map(|(name, value)| match value {
          LaunchValue::Secret(secret) => Some((name.as_str().to_owned(), secret)),
          LaunchValue::Text(_) => None,
        })
        .collect(),
      Transport::Http { authorization, .. } => match authorization {
        HttpAuthorization::None => Vec::new(),
        HttpAuthorization::Bearer(secret) | HttpAuthorization::Header { value: secret, .. } => {
          vec![(token_variable(&self.name), secret)]
        }
      },
    }
  }

  /// Every secret value of the server, for what the agent's activity must not show: its secret variables and, for an
  /// HTTP server, also what the header's value is made of (a Basic password, the bare credential), under
  /// [`token_variable`]. Never handed to a process.
  fn hidden(&self) -> Vec<SecretVariable> {
    match &self.transport {
      Transport::Stdio { .. } => self
        .secret_variables()
        .into_iter()
        .map(|(name, value)| SecretVariable::new(&name, value.clone()))
        .collect(),
      Transport::Http { authorization, .. } => {
        let name = token_variable(&self.name);
        authorization
          .hidden()
          .into_iter()
          .map(|value| SecretVariable::new(&name, value.clone()))
          .collect()
      }
    }
  }
}

/// A server left out of an invocation, and why.
pub type LeftOut = (McpServerName, McpServerProblem);

/// The variable a Codex invocation hands `server`'s bearer token or header value over in.
pub fn token_variable(server: &McpServerName) -> String {
  format!(
    "ORCHESTER_MCP_TOKEN_{}",
    server.as_str().to_ascii_uppercase().replace('-', "_")
  )
}

/// The MCP arguments of a Codex invocation and the secret variables its process starts with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodexMcp {
  /// Each server's settings (`command`, `args`, …, each with its TOML value), whatever name Codex gets it by.
  settings: Vec<(McpServerName, Vec<(String, String)>)>,
  secrets: Vec<SecretVariable>,
  /// Every secret value of the servers, to hide; not the process's variables.
  hidden: Vec<SecretVariable>,
  servers: Vec<McpServerName>,
  /// The servers Codex gets by another name ([`Self::named_apart`]), with that name.
  renamed: BTreeMap<McpServerName, String>,
  exemptions: InvocationExemptions,
  /// Orchester's own server, with the step's Orchester tools.
  orchester: Option<CodexOrchester>,
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
          mcp.settings.push((server.name.clone(), settings(server)));
          mcp.secrets.extend(new);
          mcp.hidden.extend(server.hidden());
          mcp.servers.push(server.name.clone());
        }
        Err(problem) => left_out.push((server.name.clone(), problem)),
      }
    }
    mcp.exemptions = exemptions(servers.iter().filter(|server| mcp.servers.contains(&server.name)));
    (mcp, left_out)
  }

  /// These servers, each one whose name `taken` holds (the MCP servers Codex's own configuration has) given to Codex
  /// as `orchester-<name>`, numbered on (`orchester-<name>-2`, …) while that name is taken too: under its own name
  /// Codex would merge the two servers' tables, so the user's `env`, `args` or `command` would reach the chosen one,
  /// and a `url` of theirs next to a chosen `command` makes Codex refuse its whole configuration (Codex 0.159.2,
  /// `docs/evidence/orc-547-codex-server-names-2026-10-30.md`).
  #[must_use]
  pub fn named_apart(mut self, taken: &BTreeSet<String>) -> Self {
    let mut used: BTreeSet<String> = taken.iter().cloned().collect();
    used.extend(self.servers.iter().map(ToString::to_string));
    used.insert(crate::orchester_tools::SERVER.to_owned());
    for server in self.servers.iter().filter(|server| taken.contains(server.as_str())) {
      let base = format!("orchester-{server}");
      let name = std::iter::once(base.clone())
        .chain((2_u32..).map(|number| format!("{base}-{number}")))
        .find(|name| !used.contains(name))
        .expect("there are more numbered names than taken ones");
      used.insert(name.clone());
      self.renamed.insert(server.clone(), name);
    }
    self
  }

  /// The name Codex gets `server` by: its own unless [`Self::named_apart`] gave it another.
  pub fn codex_name<'a>(&'a self, server: &'a McpServerName) -> &'a str {
    self.renamed.get(server).map_or(server.as_str(), String::as_str)
  }

  /// The servers Codex gets by another name ([`Self::named_apart`]), with that name.
  pub const fn renamed(&self) -> &BTreeMap<McpServerName, String> {
    &self.renamed
  }

  /// What the agent is told of each server Codex gets by another name ([`Self::named_apart`]); `None` when none is.
  pub fn renamed_prompt(&self) -> Option<String> {
    let sentences: Vec<String> = self
      .renamed
      .iter()
      .map(|(server, name)| {
        format!(
          "The MCP server `{server}` of this step is called `{name}` in this Codex session, because Codex's own \
           configuration has another server named `{server}`, turned off for you: use `{name}`'s tools for it."
        )
      })
      .collect();
    (!sentences.is_empty()).then(|| sentences.join(" "))
  }

  /// These servers and Orchester's own, `server`.
  #[must_use]
  pub fn with_orchester(mut self, server: CodexOrchester) -> Self {
    self.secrets.push(server.token().clone());
    self.orchester = Some(server);
    self
  }

  /// Whether Codex gets Orchester's own server.
  pub const fn has_orchester(&self) -> bool {
    self.orchester.is_some()
  }

  /// These servers in a step that does not ask before MCP actions: no tool of theirs is approved.
  #[must_use]
  pub fn without_exemptions(mut self) -> Self {
    self.exemptions = InvocationExemptions::default();
    self
  }

  /// Arguments after the sandbox settings, in order: the servers, each under its name in Codex ([`Self::codex_name`]),
  /// the approval of their exempt tools ([`approved_tools`]), then the secret variables kept out of the agent's shell
  /// commands.
  pub fn args(&self) -> Vec<String> {
    let mut args = Vec::new();
    for (server, settings) in &self.settings {
      let key = format!("mcp_servers.{}", self.codex_name(server));
      for (field, value) in settings {
        args.extend(["-c".to_owned(), format!("{key}.{field}={value}")]);
      }
    }
    for (server, tools) in self.exemptions.servers() {
      args.extend(approved_tools(
        self.codex_name(server),
        tools.tools().iter().map(McpToolName::as_str),
      ));
    }
    if let Some(orchester) = &self.orchester {
      args.extend(orchester.args());
    }
    if !self.secrets.is_empty() {
      let names: Vec<&str> = self.secrets.iter().map(SecretVariable::name).collect();
      args.extend([
        "-c".to_owned(),
        format!("shell_environment_policy.exclude={}", toml_array(&names)),
      ]);
    }
    args
  }

  /// Variables of the Codex process that its servers take their secrets from.
  pub fn secrets(&self) -> &[SecretVariable] {
    &self.secrets
  }

  /// Every secret value of these servers, for what the agent's activity must not show: Codex's App Server hides the
  /// values of [`Self::secrets`], not what a header's value is made of (a Basic password, the bare credential).
  /// Never handed to the process.
  pub fn hidden(&self) -> &[SecretVariable] {
    &self.hidden
  }

  /// The exact server names supplied by this invocation, for App Server inventory checks.
  pub fn servers(&self) -> &[McpServerName] {
    &self.servers
  }

  /// The tools of these servers that run without asking: none after [`Self::without_exemptions`].
  pub const fn exemptions(&self) -> &InvocationExemptions {
    &self.exemptions
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
}

/// `server`'s settings as Codex takes them, each field with its TOML value.
fn settings(server: &McpLaunch) -> Vec<(String, String)> {
  let mut settings = Vec::new();
  let mut set = |field: &str, value: String| settings.push((field.to_owned(), value));
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
    Transport::Http { url, authorization } => {
      set("url", toml_string(url.as_str()));
      let variable = toml_string(&token_variable(&server.name));
      match authorization {
        HttpAuthorization::None => {}
        HttpAuthorization::Bearer(_) => set("bearer_token_env_var", variable),
        HttpAuthorization::Header { name, .. } => {
          set(
            "env_http_headers",
            format!("{{{}={variable}}}", toml_string(name.as_str())),
          );
        }
      }
    }
  }
  settings
}

/// The `--mcp-config` file of a Claude Code invocation: JSON with the servers' secret values in it.
/// `Debug` shows the servers' names only, the memory holding the text is wiped when it is dropped,
/// and the runner writes it to a file only it and the CLI read, removed when the CLI has ended.
pub struct ClaudeMcpConfig {
  contents: Zeroizing<String>,
  servers: Vec<McpServerName>,
  exemptions: InvocationExemptions,
  /// The secret values in the file, by the variable or header they fill, and what a header's value is made of.
  secrets: Vec<SecretVariable>,
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
          authorization: HttpAuthorization::Bearer(token),
          ..
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
      exemptions: exemptions(kept.iter().copied()),
      secrets: kept.iter().flat_map(|server| server.hidden()).collect(),
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

  /// The secret values in the file, to keep them out of what an approval shows.
  pub fn secrets(&self) -> &[SecretVariable] {
    &self.secrets
  }

  /// The tools of these servers that run without asking when the step asks before MCP actions.
  pub const fn exemptions(&self) -> &InvocationExemptions {
    &self.exemptions
  }
}

/// The tools of `servers` that run without asking.
fn exemptions<'a>(servers: impl Iterator<Item = &'a McpLaunch>) -> InvocationExemptions {
  InvocationExemptions::new(servers.map(|server| (server.name.clone(), server.exempt.clone())))
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
    headers: Option<BTreeMap<&'a str, &'a str>>,
  },
}

impl<'a> ClaudeServer<'a> {
  /// `server`, its bearer token, if it has one, as the header's value `bearer`.
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
      Transport::Http { url, authorization } => Self::Http {
        url: url.as_str(),
        headers: match authorization {
          HttpAuthorization::None => None,
          HttpAuthorization::Bearer(_) => Some(BTreeMap::from([("Authorization", bearer)])),
          HttpAuthorization::Header { name, value, .. } => Some(BTreeMap::from([(name.as_str(), value.expose())])),
        },
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

/// The overrides that approve `tools` of `server` (`approval_mode = "approve"`), so that Codex runs them
/// without an approval request; none for no tools. They must be overrides of the process, next to the
/// server's own: Codex 0.159.2 refuses a thread whose configuration (`thread/start`'s `config`) has a
/// table for a server that only overrides define, with *invalid transport in `mcp_servers.<server>`*
/// (checked 2026-10-07; `config/read` lists the approved tools).
pub(super) fn approved_tools<'a>(server: &str, tools: impl IntoIterator<Item = &'a str>) -> Vec<String> {
  let entries: Vec<String> = tools
    .into_iter()
    .map(|tool| format!("{}={{approval_mode=\"approve\"}}", toml_string(tool)))
    .collect();
  if entries.is_empty() {
    return Vec::new();
  }
  vec![
    "-c".to_owned(),
    format!("mcp_servers.{server}.tools={{{}}}", entries.join(",")),
  ]
}

pub(super) fn toml_array(values: &[impl AsRef<str>]) -> String {
  let items: Vec<String> = values.iter().map(|value| toml_string(value.as_ref())).collect();
  format!("[{}]", items.join(","))
}
