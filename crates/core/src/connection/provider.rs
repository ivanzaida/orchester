//! The agent providers a connection can run agents on, and how Orchester runs each
//! (`docs/architecture/native-agent.md` §2.1): an agent CLI it starts, or a model API it calls itself
//! as a native agent. Of the native agents only the Anthropic API runs in this version; OpenAI Responses
//! comes with its client (§9, P2) and nothing checks, starts or runs it yet ([`AgentProvider::runs_agents`]).

use serde::{Deserialize, Serialize};

/// The agent providers Orchester knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProvider {
  CodexCli,
  ClaudeCodeCli,
  /// Anthropic's Messages API, run by Orchester itself.
  AnthropicApi,
  /// OpenAI's Responses API, run by Orchester itself.
  #[serde(rename = "openai_api")]
  OpenAiApi,
}

/// How Orchester runs an agent of a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentRuntime {
  /// An agent CLI, by the name of its program on the `PATH`.
  Cli { program: &'static str },
  /// Orchester runs the agent loop over the model API's wire format.
  Native(WireApi),
}

/// The wire format a native agent talks to its model API in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireApi {
  AnthropicMessages,
  OpenAiResponses,
}

impl AgentProvider {
  /// The providers a connection can be added for in this version: the agent CLIs.
  pub const CLI: [Self; 2] = [Self::CodexCli, Self::ClaudeCodeCli];

  pub const fn runtime(self) -> AgentRuntime {
    match self {
      Self::CodexCli => AgentRuntime::Cli { program: "codex" },
      Self::ClaudeCodeCli => AgentRuntime::Cli { program: "claude" },
      Self::AnthropicApi => AgentRuntime::Native(WireApi::AnthropicMessages),
      Self::OpenAiApi => AgentRuntime::Native(WireApi::OpenAiResponses),
    }
  }

  /// Name of the CLI's program on the `PATH`; `None` for a native provider, which has none.
  pub const fn program_name(self) -> Option<&'static str> {
    match self.runtime() {
      AgentRuntime::Cli { program } => Some(program),
      AgentRuntime::Native(_) => None,
    }
  }

  pub const fn is_native(self) -> bool {
    matches!(self.runtime(), AgentRuntime::Native(_))
  }

  /// The API an agent CLI speaks to an API provider it runs on (`docs/design/api-providers.md`): Claude Code
  /// Anthropic Messages, Codex OpenAI Responses (the only wire current Codex takes); `None` for a native provider,
  /// which runs on no API provider.
  pub const fn provider_wire(self) -> Option<WireApi> {
    match self {
      Self::ClaudeCodeCli => Some(WireApi::AnthropicMessages),
      Self::CodexCli => Some(WireApi::OpenAiResponses),
      Self::AnthropicApi | Self::OpenAiApi => None,
    }
  }

  /// Whether this version can run agents on the provider. OpenAI Responses has no client yet (§9, P2):
  /// a step on such a connection does not start, and the app says why.
  pub const fn runs_agents(self) -> bool {
    !matches!(self, Self::OpenAiApi)
  }

  /// Whether an agent at work on the provider takes a person's message while its turn runs (*Send
  /// now*): Codex through App Server's `turn/steer`, Claude Code through its stream-json input.
  /// Checked live 2026-10-02 with Codex 0.159.2 and Claude Code 2.1.286: both take it after the tool
  /// call that is running (`docs/evidence/orc-61-send-now-2026-10-02.md`).
  pub const fn takes_messages_mid_turn(self) -> bool {
    matches!(self, Self::CodexCli | Self::ClaudeCodeCli)
  }

  /// Whether a person can stop an agent's turn on the provider alone (*Stop turn*), its session going on with their
  /// message: the runner ends an agent CLI and resumes its session. A native agent's loop is not stopped so.
  pub const fn stops_turn_alone(self) -> bool {
    matches!(self, Self::CodexCli | Self::ClaudeCodeCli)
  }
}
