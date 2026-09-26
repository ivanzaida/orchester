//! Requests to the connected runner and their replies, matched by request id.

use std::collections::HashMap;

use orchester_ipc::{Command, Reply, Request, RequestId};
use thiserror::Error;
use tokio::sync::oneshot;

/// Why a request got no reply.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RequestError {
  /// No runner is connected: it is starting, reconnecting, stopped or failed.
  #[error("the runner is not connected")]
  NotConnected,
  /// The connection ended before the runner answered; the request may or may not have been
  /// carried out, so retrying must be safe (idempotent requests).
  #[error("the connection to the runner ended before it answered")]
  Lost,
}

pub(super) type ReplySender = oneshot::Sender<Result<Reply, RequestError>>;

/// Requests sent on one connection that wait for their reply. Dropping it answers the rest with
/// [`RequestError::Lost`].
#[derive(Default)]
pub(super) struct Pending {
  next: u64,
  waiting: HashMap<RequestId, ReplySender>,
}

impl Pending {
  /// The request to send for `command`, remembered so its reply reaches `reply`. `None`
  /// registers nothing: the reply is not needed.
  pub(super) fn request(&mut self, command: Command, reply: Option<ReplySender>) -> Request {
    self.next += 1;
    let id = RequestId::new(self.next);
    if let Some(reply) = reply {
      self.waiting.insert(id, reply);
    }
    Request { id, command }
  }

  /// Delivers the reply to request `id`.
  pub(super) fn answer(&mut self, id: RequestId, reply: Reply) {
    match self.waiting.remove(&id) {
      // The requester may have stopped waiting; the reply no longer matters then.
      Some(sender) => drop(sender.send(Ok(reply))),
      None => tracing::debug!(%id, ?reply, "reply nobody waits for"),
    }
  }

  /// The request could not be sent: its requester learns that at once.
  pub(super) fn fail(&mut self, id: RequestId) {
    if let Some(sender) = self.waiting.remove(&id) {
      drop(sender.send(Err(RequestError::Lost)));
    }
  }
}

impl Drop for Pending {
  fn drop(&mut self) {
    for (_, sender) in self.waiting.drain() {
      drop(sender.send(Err(RequestError::Lost)));
    }
  }
}
