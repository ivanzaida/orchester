//! Which kinds of attachment an agent's connection takes, so a message an agent could not read is refused before it
//! counts as delivered:
//!
//! - Claude Code (`claude -p`, stream-json input) takes images and PDFs as content blocks and text as text blocks,
//!   as the Anthropic Messages API it sends them on does; every current Claude model reads both.
//! - Codex (App Server) takes images as `localImage` input items and text as text items; it has no input for a PDF,
//!   and its agent cannot read one from a file either.
//! - Native agents take what their API does: Anthropic Messages image, document and text blocks; OpenAI Responses
//!   `input_image`, `input_file` and text.

use super::AttachmentKind;
use crate::connection::AgentProvider;

/// Whether an agent on `provider` takes an attachment of `kind`.
pub const fn takes(provider: AgentProvider, kind: AttachmentKind) -> bool {
  match provider {
    AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => true,
    AgentProvider::CodexCli => !matches!(kind, AttachmentKind::Pdf),
  }
}
