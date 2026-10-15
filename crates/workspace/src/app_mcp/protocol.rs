//! The protocol revisions every Orchester MCP server speaks.

use std::borrow::Cow;

use rmcp::model::ProtocolVersion;

/// The revisions up to 2025-11-25, which Orchester's servers implement. rmcp would also advertise 2026-07-28,
/// whose stateless lifecycle (no `initialize`, no sessions, `ttlMs` and `cacheScope` on list results) they do not
/// speak: a client asking for it, as Claude Code's `server/discover` probe does, gets the unsupported-version error
/// and falls back to `initialize`. Each server's `ServerHandler::supported_protocol_versions` returns this.
pub fn supported_protocol_versions() -> Cow<'static, [ProtocolVersion]> {
  Cow::Borrowed(ProtocolVersion::known_up_to(&ProtocolVersion::V_2025_11_25))
}
