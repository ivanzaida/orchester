//! Orchester's own MCP server as Codex App Server starts it (`crate::orchester_tools`): the runner's relay
//! (`orchester-runner --orchester-tools`), which reaches the runner at a loopback address with the
//! invocation's token. The address is a configuration value; the token is a variable of the Codex process,
//! handed to the relay by name (`env_vars`) and kept out of the agent's shell commands like every MCP
//! secret ([`super::CodexMcp`]). The tools' approval modes are set in the thread's configuration
//! (`crate::orchester_tools::StepTools::codex_tools`).

use super::mcp::{toml_array, toml_string};
use crate::{
  command::SecretVariable,
  connection::claude_approval::{ADDRESS_VARIABLE, TOKEN_VARIABLE},
  orchester_tools::SERVER,
  resource::CommandTimeout,
};

/// How long Codex waits for one call: the longest a project tool may run, and a minute for the rest.
/// Codex's own default (60 seconds) would end a build or a test run halfway.
const TOOL_TIMEOUT_SECS: u64 = CommandTimeout::MAX_HOURS * 3600 + 60;

/// The relay Codex starts as the server [`SERVER`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexOrchester {
  program: String,
  args: Vec<String>,
  address: String,
  token: SecretVariable,
}

impl CodexOrchester {
  /// `program` with `args`, reaching the runner at `address` with `token` (the variable
  /// [`TOKEN_VARIABLE`]).
  pub fn new(program: &str, args: &[&str], address: &str, token: SecretVariable) -> Self {
    Self {
      program: program.to_owned(),
      args: args.iter().map(|arg| (*arg).to_owned()).collect(),
      address: address.to_owned(),
      token,
    }
  }

  /// The secret variable the Codex process starts with.
  pub const fn token(&self) -> &SecretVariable {
    &self.token
  }

  /// The configuration overrides that add the server.
  pub(super) fn args(&self) -> Vec<String> {
    let key = format!("mcp_servers.{SERVER}");
    [
      format!("{key}.command={}", toml_string(&self.program)),
      format!("{key}.args={}", toml_array(&self.args)),
      format!("{key}.env.{ADDRESS_VARIABLE}={}", toml_string(&self.address)),
      format!("{key}.env_vars={}", toml_array(&[TOKEN_VARIABLE])),
      format!("{key}.tool_timeout_sec={TOOL_TIMEOUT_SECS}"),
    ]
    .into_iter()
    .flat_map(|setting| ["-c".to_owned(), setting])
    .collect()
  }
}
