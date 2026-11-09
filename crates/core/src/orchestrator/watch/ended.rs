//! Why a subscription stopped leading its target. An ended subscription is kept, with when and why, for its
//! conversation's lines and History; only the ones that have not ended lead.

use super::EndReason;
use crate::id::ChatId;

/// Why a subscription ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchEnd {
  /// It ended by itself: its task or module closed ([`super::ending`]).
  ByItself(EndReason),
  /// The person stopped it.
  Stopped,
  /// `by`, another conversation, took its target over.
  TakenOver { by: ChatId },
  /// Its own conversation watched the target again, with a new order.
  Renewed,
  /// Its conversation was deleted.
  ChatDeleted,
}
