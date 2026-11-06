//! The base notices area of the coordinator link: whether the project tells its working agents that the base branch
//! moved (Project settings), and the notice the desktop queued for a run (`orchester_core::base_move`). Both are asked
//! at each agent start, as they were read before; the notice's delivery is recorded by the coordinator otherwise.

use orchester_core::{base_move::BaseNotice, id::RunId};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answers about the run's moved base.
pub trait BaseNoticeLink {
  /// Whether the project tells its working agents when the base branch moves.
  fn tells_base_moves(&self, request: &TellsBaseMovesRequest) -> Result<bool, LinkError>;

  /// The notice the run of `request` has pending, if it has one.
  fn pending_notice(&self, request: &PendingNoticeRequest) -> Result<Option<BaseNotice>, LinkError>;
}

/// Project settings › tell working agents when the base branch moves: the project is the link's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TellsBaseMovesRequest {}

/// The notice `run` has pending.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingNoticeRequest {
  pub run: RunId,
}

/// Whether the project tells its working agents when the base branch moves, asked through `link`.
pub fn tells_base_moves(link: &(impl BaseNoticeLink + ?Sized)) -> Result<bool, LinkError> {
  link.tells_base_moves(&TellsBaseMovesRequest {})
}

/// The notice `run` has pending, asked through `link`.
pub fn pending_notice(link: &(impl BaseNoticeLink + ?Sized), run: RunId) -> Result<Option<BaseNotice>, LinkError> {
  link.pending_notice(&PendingNoticeRequest { run })
}
