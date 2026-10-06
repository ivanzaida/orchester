//! The run's tool requests in the chat (ORC-74): read apart from its transcript, and read again as soon
//! as the project's waiting requests change, which the dialog, *Needs you* and the status bar follow.
//! A decision, by the person in the dialog or the card, by auto mode, by automation, or a request that
//! ended, then shows on its card, the rail and what the chat reveals at once: the transcript is read
//! again with every line of the project's output, and each line starts its read over. A request
//! Orchester answered itself never waited; its card comes with the transcript read that brings its
//! answer.

use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureState};
use orchester_core::id::RunId;

use crate::{
  background,
  messages::{self, Message},
  screens::project::pages::Shown,
  tasks::{
    approvals::ShownApproval,
    timeline::load::{Requests, requests_in},
  },
};

/// The run's requests that wait for the person, and those auto mode's reviewer is judging, as the
/// project's waiting requests list them; `None` while they are not read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Waiting {
  pending: Vec<String>,
  reviewing: Vec<String>,
}

impl Waiting {
  fn of(ctx: &mut Ctx, run: RunId) -> Option<Self> {
    let shown = ctx.use_context::<Shown>()?;
    let tasks = shown.tasks.ready()?;
    let ids = |list: &[ShownApproval]| {
      list
        .iter()
        .filter(|shown| shown.approval.run == run)
        .map(|shown| shown.approval.id.clone())
        .collect()
    };
    Some(Self {
      pending: ids(&tasks.pending_tool_approvals),
      reviewing: ids(&tasks.reviewing_tool_approvals),
    })
  }
}

/// The requests of `run`, read again when the run's waiting requests change, after each read of its
/// transcript (`transcript`, the entries it read) and on `retry`.
pub(super) fn requests(
  ctx: &mut Ctx,
  (repo_root, run): (PathBuf, Option<RunId>),
  (transcript, retry): (usize, u64),
) -> FutureState<Requests, Message> {
  let waiting = run.and_then(|run| Waiting::of(ctx, run));
  let deps = (repo_root, run, waiting, transcript, retry);
  let load = ctx.future(deps, |(repo_root, run, ..)| async move {
    let Some(run) = run else {
      return Ok(Requests::default());
    };
    background::run(move || requests_in(&repo_root, run))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot read the run's tool requests");
        messages::store_error(&error)
      })
  });
  load.state().get()
}
