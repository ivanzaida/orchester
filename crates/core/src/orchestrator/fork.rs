//! *Fork from here* in the Orchestrator chat (ORC-334, `docs/design/conversation-rewind.md`): a new conversation holds
//! copies of a conversation's entries up to a point, and the original stays as it was. The point is a
//! [`RewindPoint`]: before the person's message, whose words go to the fork's composer, or after an answer. The fork
//! opens with its line ([`ChatEvent::Forked`]) and, when the original had done something after the point, ends its
//! copies with what stays there ([`ChatEvent::MadeInOriginal`]). It has no session: its first turn starts a new one,
//! told the copied conversation ([`super::rewind::primed_prompt`]), on Claude Code and Codex alike. A fork may be made
//! while the Orchestrator answers: it takes what was kept so far.

use super::{
  chat::{ChatEntry, ChatEvent, EntryKind},
  rewind::RewindPoint,
};

/// The first entry of `entries` a fork from `point` leaves out; `None` when the point is not in them.
pub fn cut(entries: &[ChatEntry], point: RewindPoint) -> Option<i64> {
  match point {
    RewindPoint::Before(message) => entries.iter().find_map(|entry| match &entry.kind {
      EntryKind::Person { message: kept, .. } if *kept == message => Some(entry.seq),
      _ => None,
    }),
    RewindPoint::After { last, .. } => entries.iter().any(|entry| entry.seq == last).then_some(last + 1),
  }
}

/// Whether a fork takes a copy of `kind`: not a message that still waits for its turn, which the agent never heard,
/// nor a compaction or a new session's line, which are about the original's session.
pub const fn takes(kind: &EntryKind) -> bool {
  match kind {
    EntryKind::Person { turn, .. } => !turn.is_waiting(),
    EntryKind::Event(ChatEvent::Compaction { .. } | ChatEvent::NewSession { .. }) => false,
    _ => true,
  }
}
