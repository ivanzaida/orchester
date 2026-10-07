//! The run's tool requests in the chat (ORC-74): read apart from its transcript, and read again as soon
//! as the project's waiting requests change, which the dialog, *Needs you* and the status bar follow.
//! A decision, by the person in the dialog or the card, by auto mode, by automation, or a request that
//! ended, then shows on its card, the rail and what the chat reveals at once. The chat shows a read of
//! the transcript only with the requests read after it, so it never shows what the agent did after a
//! request while the request's card still waits: the card holds the chat until it is decided, and the
//! rows that came after its answer would push it aside (ORC-71). A request Orchester answered itself
//! never waited; its card comes with the transcript read that brings its answer.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureState};
use orchester_core::id::RunId;

use crate::{
  background,
  messages::{self, Message},
  screens::project::pages::Shown,
  tasks::{
    approvals::ShownApproval,
    timeline::load::{Loaded, requests_in},
    transcript::Entry,
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

/// A read of the transcript, told from another by the read rather than by its lines: each read asks
/// for the requests again, so that they are at least as new as it.
#[derive(Clone, Debug)]
struct Transcript(Option<Arc<Vec<Entry>>>);

impl PartialEq for Transcript {
  fn eq(&self, other: &Self) -> bool {
    match (&self.0, &other.0) {
      (Some(read), Some(other)) => Arc::ptr_eq(read, other),
      (read, other) => read.is_none() && other.is_none(),
    }
  }
}

/// The timeline's sources of `run`: `transcript`, the latest read of its transcript, with its requests
/// read after it; read again when the run's waiting requests change and on `retry`. Until the requests
/// of a new read of the transcript are read, the read before it is shown with its own; `None` until the
/// transcript was read.
pub(super) fn with_requests(
  ctx: &mut Ctx,
  (repo_root, run): (PathBuf, Option<RunId>),
  (transcript, retry): (Option<Arc<Vec<Entry>>>, u64),
) -> FutureState<Option<Loaded>, Message> {
  let waiting = run.and_then(|run| Waiting::of(ctx, run));
  let deps = (repo_root, run, waiting, Transcript(transcript), retry);
  let load = ctx.future(deps, |(repo_root, run, _, Transcript(transcript), _)| async move {
    let (Some(run), Some(entries)) = (run, transcript) else {
      return Ok(None);
    };
    background::run(move || requests_in(&repo_root, run))
      .await?
      .map(|requests| Some(Loaded::of(entries, requests)))
      .map_err(|error| {
        tracing::error!(%error, "cannot read the run's tool requests");
        messages::store_error(&error)
      })
  });
  load.state().get()
}
