//! How an agent CLI starts on an API provider (`docs/design/api-providers.md`): the provider's base URL and the
//! step's model as plain variables or configuration overrides, the key as a secret variable only, so that it reaches
//! the CLI's process and the runner's redaction set and nothing else.
//!
//! - Claude Code (`code.claude.com/docs/en/llm-gateway-connect`, `env-vars`, checked 2026-10-18): `ANTHROPIC_BASE_URL`,
//!   the key in `ANTHROPIC_AUTH_TOKEN` (sent as `Authorization: Bearer`, used at once over a saved login), and the
//!   step's model in `ANTHROPIC_MODEL` and every model alias variable ([`CLAUDE_MODEL_VARIABLES`]), so that aliases,
//!   background work (the Haiku slot) and subagents never ask the provider for a Claude model it does not serve. Every
//!   other `ANTHROPIC_*` variable the project's environment sets is dropped (another key, `x-api-key` or custom headers
//!   would reach the provider's host; another base URL would route elsewhere), and so are the switches that route
//!   Claude Code to a cloud provider instead ([`CLAUDE_ROUTING`]). `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1` keeps the key
//!   out of the processes Claude Code starts: checked 2026-10-18 with Claude Code 2.1.301 and no model turn, a stdio
//!   MCP server no longer sees `ANTHROPIC_AUTH_TOKEN` while the variables of its own configuration (Orchester's relay
//!   token among them) stay; that Bash commands and hooks are scrubbed too is the docs' word.
//! - Codex (`learn.chatgpt.com/docs/config-file/config-reference`, checked 2026-10-18): a model provider of its own
//!   through configuration overrides, `model_provider` naming it and `model_providers.<id>` with the base URL, the
//!   variable holding the key (`env_key`) and `wire_api = "responses"`, the only wire current Codex takes. The runner
//!   names the provider again in `thread/start` and checks that Codex took it ([`CODEX_PROVIDER_ID`]).
//!   `shell_environment_policy.ignore_default_excludes=false` keeps the key (its name holds `KEY`) out of the commands
//!   the agent runs; checked 2026-10-18 with Codex 0.159.2 (`codex sandbox`): the key is gone and a user's own
//!   `exclude` list still applies, which an `exclude` override would have replaced. Codex starts stdio MCP servers with
//!   a short list of variables only (checked the same day: no key, no other variable of Codex's environment).

use std::sync::Arc;

use super::mcp::toml_string;
use crate::{
  command::{Command, SecretVariable},
  connection::{AgentProvider, ApiProvider},
  resource::ModelId,
  secret::SecretValue,
};

/// The variable Claude Code reads the provider's key from.
pub const CLAUDE_KEY_VARIABLE: &str = "ANTHROPIC_AUTH_TOKEN";
/// The variable Codex reads the provider's key from (the provider's `env_key`).
pub const CODEX_KEY_VARIABLE: &str = "ORCHESTER_PROVIDER_API_KEY";
/// The id of the model provider Codex runs on; not one of Codex's reserved ids (`openai`, `ollama`, `lmstudio`).
pub const CODEX_PROVIDER_ID: &str = "orchester";
/// The provider's display name in Codex.
const CODEX_PROVIDER_NAME: &str = "Orchester API provider";
/// Claude Code's variables that name a model or what an alias resolves to, each set to the step's model
/// (`ANTHROPIC_SMALL_FAST_MODEL` is deprecated, still read by older versions).
pub const CLAUDE_MODEL_VARIABLES: [&str; 8] = [
  "ANTHROPIC_MODEL",
  "ANTHROPIC_DEFAULT_MODEL",
  "ANTHROPIC_DEFAULT_OPUS_MODEL",
  "ANTHROPIC_DEFAULT_SONNET_MODEL",
  "ANTHROPIC_DEFAULT_HAIKU_MODEL",
  "ANTHROPIC_DEFAULT_FABLE_MODEL",
  "ANTHROPIC_SMALL_FAST_MODEL",
  "CLAUDE_CODE_SUBAGENT_MODEL",
];
/// What makes Claude Code route its requests to a cloud provider, or skip its auth, instead of the base URL; dropped
/// from a provider run's environment, as is every `ANTHROPIC_*` variable Orchester does not set.
pub const CLAUDE_ROUTING: [&str; 12] = [
  "CLAUDE_CODE_USE_BEDROCK",
  "CLAUDE_CODE_USE_VERTEX",
  "CLAUDE_CODE_USE_FOUNDRY",
  "CLAUDE_CODE_USE_MANTLE",
  "CLAUDE_CODE_USE_ANTHROPIC_AWS",
  "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
  "CLAUDE_CODE_SKIP_VERTEX_AUTH",
  "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
  "CLAUDE_CODE_SKIP_MANTLE_AUTH",
  "CLAUDE_CODE_SKIP_ANTHROPIC_AWS_AUTH",
  "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST",
  "CLAUDE_CODE_SUBAGENT_MODEL_FORCE",
];
/// Turns on Claude Code's scrub of credentials from the environments of the processes it starts.
const CLAUDE_SCRUB: &str = "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB";
/// Keeps variables whose names contain KEY, SECRET or TOKEN out of the commands a Codex agent runs.
const CODEX_SHELL_EXCLUDES: &str = "shell_environment_policy.ignore_default_excludes=false";

/// What an agent CLI runs on instead of its own sign-in: the provider, the step's model and the key read for this
/// start. `Debug` shows no key ([`SecretValue`] redacts itself).
#[derive(Debug)]
pub struct OnProvider<'a> {
  pub provider: &'a ApiProvider,
  pub model: &'a ModelId,
  pub key: Arc<SecretValue>,
}

impl OnProvider<'_> {
  /// `command`, the agent CLI's of `cli`, on the provider: its variables set over the environment's and the
  /// project's, and for Codex the overrides that define and choose the provider.
  pub fn apply(&self, command: Command, cli: AgentProvider) -> Command {
    match cli {
      AgentProvider::ClaudeCodeCli => self.claude(command),
      AgentProvider::CodexCli => self.codex(command),
      // No native agent starts a CLI, so none starts one on a provider.
      AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => command,
    }
  }

  /// The key as the variable `cli` reads it from, for the redaction set and the process.
  pub fn key_variable(&self, cli: AgentProvider) -> SecretVariable {
    let name = match cli {
      AgentProvider::CodexCli => CODEX_KEY_VARIABLE,
      _ => CLAUDE_KEY_VARIABLE,
    };
    SecretVariable::new(name, Arc::clone(&self.key))
  }

  fn claude(&self, command: Command) -> Command {
    let command = command
      .with_env_dropped_where(|name| {
        let upper = name.to_ascii_uppercase();
        upper.starts_with("ANTHROPIC_") || CLAUDE_ROUTING.contains(&upper.as_str())
      })
      .with_env_replaced("ANTHROPIC_BASE_URL", self.provider.base_url().as_str())
      .with_env_replaced(CLAUDE_SCRUB, "1");
    let command = CLAUDE_MODEL_VARIABLES.iter().fold(command, |command, name| {
      command.with_env_replaced(name, self.model.as_str())
    });
    command.with_secret_env(self.key_variable(AgentProvider::ClaudeCodeCli))
  }

  fn codex(&self, command: Command) -> Command {
    command
      .with_args(codex_overrides(self.provider))
      .with_secret_env(self.key_variable(AgentProvider::CodexCli))
  }
}

/// The configuration overrides that define Codex's model provider on `provider` and choose it.
pub fn codex_overrides(provider: &ApiProvider) -> Vec<String> {
  let table = format!(
    "model_providers.{CODEX_PROVIDER_ID}={{name={},base_url={},env_key={},wire_api=\"responses\"}}",
    toml_string(CODEX_PROVIDER_NAME),
    toml_string(provider.base_url().as_str()),
    toml_string(CODEX_KEY_VARIABLE),
  );
  vec![
    "-c".to_owned(),
    format!("model_provider={}", toml_string(CODEX_PROVIDER_ID)),
    "-c".to_owned(),
    table,
    "-c".to_owned(),
    CODEX_SHELL_EXCLUDES.to_owned(),
  ]
}
