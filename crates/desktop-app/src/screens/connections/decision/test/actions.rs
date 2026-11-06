//! Explicit submission and durable stop, separate from the page's reads.
use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{connection::DecisionConnection, id::IdempotencyKey, resource::ModelId};
use orchester_workspace::{
  background,
  connections::decision::{Writer, test},
  messages::{self, Message},
};

use crate::services::Services;

pub(super) type Start = (PathBuf, IdempotencyKey, DecisionConnection, ModelId, Writer);
pub(super) type Action = FutureAction<Start, (), Message>;
pub(super) type Stop = FutureAction<(PathBuf, IdempotencyKey), (), Message>;

pub(super) fn start(ctx: &mut Ctx) -> Action {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |(root, id, connection, model, writer): Start| {
    let runners = runners.clone();
    async move {
      runners
        .test_decision_connection(&root, id, &connection, model, writer)
        .await
        .map_err(|error| orchester_workspace::connections::native::test_failure(&error))
    }
  })
}

pub(super) fn stop(ctx: &mut Ctx) -> Stop {
  ctx.future_action(|(root, id): (PathBuf, IdempotencyKey)| async move {
    background::run(move || test::stop_in(&root, id))
      .await?
      .map(|_| ())
      .map_err(|error| messages::store_error(&error))
  })
}
