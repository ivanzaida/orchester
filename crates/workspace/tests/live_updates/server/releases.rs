//! The fake team server's `DELETE …/tasks/{task}/claim`: the holder releases their claim, as Make local does.

use std::net::TcpStream;

use orchester_api::{ClaimAction, Event, TaskClaim, UserRef};
use orchester_core::id::TaskId;
use serde_json::json;

use super::Shared;
use crate::http::respond;

/// `DELETE …/tasks/{task}/claim`: the task has no claim afterwards.
pub(super) fn release(shared: &Shared, user: &UserRef, id: TaskId, stream: &mut TcpStream) {
  let mut state = shared.state.lock().expect("state");
  let task = state.task(id);
  task.claim = None;
  let confirmed = task.clone();
  let delta = TaskClaim {
    task: id,
    claim: None,
    action: ClaimAction::Released,
  };
  state.append(user, delta, Event::TaskClaim);
  drop(state);
  shared.appended.notify_all();
  respond(stream, 200, &json!({ "task": confirmed }));
}
