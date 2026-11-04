//! The base notices area of the local link: the project's switch and the run's pending notice, read from the project
//! database at each ask.

use std::ops::Deref;

use orchester_core::base_move::BaseNotice;
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  base_notices::{BaseNoticeLink, PendingNoticeRequest, TellsBaseMovesRequest},
};

impl<D: Deref<Target = ProjectDb>> BaseNoticeLink for LocalLink<D> {
  fn tells_base_moves(&self, _: &TellsBaseMovesRequest) -> Result<bool, LinkError> {
    self.0.tells_base_moves().map_err(|error| read_failed(&error))
  }

  fn pending_notice(&self, request: &PendingNoticeRequest) -> Result<Option<BaseNotice>, LinkError> {
    self
      .0
      .pending_base_notice(request.run)
      .map_err(|error| read_failed(&error))
  }
}
