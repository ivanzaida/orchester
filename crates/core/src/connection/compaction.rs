//! A compaction of an agent session a person asked for (*Compact*, ORC-309), as each CLI carries it out:
//!
//! - Claude Code compacts with its own `/compact` command, which print mode takes as the prompt of a turn that resumes
//!   the session ([`COMPACT_COMMAND`]). It prints `system` with subtype `compact_boundary` and
//!   `compact_metadata.pre_tokens`, the tokens the context held, then a result ([`claude_boundary`]). It reports no
//!   size after the compaction; a message of the main conversation that reports one after the boundary is taken.
//! - Codex App Server compacts the thread with `thread/compact/start`, which runs a turn of its own and reports the
//!   thread's size after it with `thread/tokenUsage/updated`.
//! - A native agent's loop compacts with its own compactor (`crate::native::session`).
//!
//! What the session holds afterwards is only taken when it is less than before ([`compacted_to`]): a reading no
//! smaller is the compaction's own request, not the compacted session.

use serde_json::Value;

use super::context_use::ContextUse;

/// The prompt that makes Claude Code compact the session it resumes.
pub const COMPACT_COMMAND: &str = "/compact";

/// Whether `line` of Claude Code's stream-json output is the boundary its compaction leaves, and if so the tokens the
/// context held before it, when it said; `None` for any other line.
pub fn claude_boundary(line: &str) -> Option<Option<u64>> {
  let value: Value = serde_json::from_str(line).ok()?;
  let boundary = value.get("type").and_then(Value::as_str) == Some("system")
    && value.get("subtype").and_then(Value::as_str) == Some("compact_boundary");
  boundary.then(|| value.pointer("/compact_metadata/pre_tokens").and_then(Value::as_u64))
}

/// The tokens a session holds after its compaction, from `reading`, the reading its CLI reported last, which
/// `earlier` was before the compaction started: taken when it is new and smaller than `before`; `None` otherwise.
pub fn compacted_to(before: Option<u64>, earlier: Option<ContextUse>, reading: Option<ContextUse>) -> Option<u64> {
  let reading = reading.filter(|reading| Some(*reading) != earlier)?;
  let after = reading.used();
  match before {
    Some(before) if after >= before => None,
    _ => Some(after),
  }
}
