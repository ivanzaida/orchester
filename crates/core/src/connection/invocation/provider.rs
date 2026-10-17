//! How an agent CLI starts on an API provider (`docs/design/api-providers.md`): the provider's base URL and the
//! step's model as plain variables or configuration overrides, the key as a secret variable only, so that it reaches
//! the CLI's process and the runner's redaction set and nothing else.
//!
//! - Claude Code (`code.claude.com/docs/en/llm-gateway-connect`, `env-vars`, checked 2026-10-18):
//!   `ANTHROPIC_BASE_URL`, the key in `ANTHROPIC_AUTH_TOKEN` (sent as `Authorization: Bearer`, used at once over a
//!   saved login), and the step's model in `ANTHROPIC_MODEL`, the three `ANTHROPIC_DEFAULT_*_MODEL` aliases and
//!   `CLAUDE_CODE_SUBAGENT_MODEL`, so that aliases, background work (the Haiku slot) and subagents never ask the
//!   provider for a Claude model it does not serve. `ANTHROPIC_API_KEY` is dropped: Claude Code would send it too, in
//!   `x-api-key`, to the provider's host.
//! - Codex (`learn.chatgpt.com/docs/config-file/config-reference`, checked 2026-10-18): a model provider of its own
//!   through configuration overrides, `model_provider` naming it and `model_providers.<id>` with the base URL, the
//!   variable holding the key (`env_key`) and `wire_api = "responses"`, the only wire current Codex takes. The runner
//!   names the provider again in `thread/start` and checks that Codex took it ([`CODEX_PROVIDER_ID`]).

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
/// Claude Code's variables that name a model, each set to the step's model.
const CLAUDE_MODEL_VARIABLES: [&str; 5] = [
  "ANTHROPIC_MODEL",
  "ANTHROPIC_DEFAULT_OPUS_MODEL",
  "ANTHROPIC_DEFAULT_SONNET_MODEL",
  "ANTHROPIC_DEFAULT_HAIKU_MODEL",
  "CLAUDE_CODE_SUBAGENT_MODEL",
];
/// A credential Claude Code would also send to the provider's host.
const CLAUDE_OTHER_KEY: &str = "ANTHROPIC_API_KEY";

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
      .with_env_dropped(CLAUDE_OTHER_KEY)
      .with_env_replaced("ANTHROPIC_BASE_URL", self.provider.base_url().as_str());
    let command = CLAUDE_MODEL_VARIABLES
      .iter()
      .fold(command, |command, name| command.with_env_replaced(name, self.model.as_str()));
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
  ]
}
