//! What the chat is read from besides the task: the run's transcript, read again as the run changes, one
//! read at a time (a change that comes while a read is under way waits for it and is read next, so a run
//! that writes faster than it is read still shows), and its tool requests and questions, read after each
//! read of the transcript and again as soon as the project's waiting requests change (ORC-74). The chat
//! shows a read of the transcript only with the requests read after it, so a request's card never waits
//! beside what its agent did after the answer. Reads wait while QA holds them (`project::hold`).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureState},
  core::Signal,
};
use orchester_core::id::RunId;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::{
    approvals::ShownApproval,
    data::ProjectTasks,
    timeline::load::{Loaded, requests_in, transcript_in},
    transcript::Entry,
  },
};
use parking_lot::Mutex;

use crate::project::hold::ReadHold;

/// The run's transcript, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Transcript(pub(super) Arc<Vec<Entry>>);

impl lurq::app::component::DevtoolsInspectable for Transcript {}

/// Which change of the run the transcript is read at, and the read's state once it was asked for.
#[derive(Default)]
pub(super) struct TranscriptRead {
  at: Mutex<u64>,
  state: Mutex<Option<Signal<FutureState<Transcript, Message>>>>,
}

impl TranscriptRead {
  /// `run`'s transcript, read again when `revision`, the run's changes, moved on since the last read and
  /// that read is done, and on `retry`.
  pub(super) fn read(
    &self,
    ctx: &mut Ctx,
    (repo_root, run, hold): (PathBuf, RunId, ReadHold),
    (revision, retry): (u64, u64),
  ) -> FutureState<Transcript, Message> {
    let reading = self
      .state
      .lock()
      .as_ref()
      .is_some_and(|state| state.get_untracked().is_pending());
    let at = {
      let mut at = self.at.lock();
      if !reading {
        *at = revision;
      }
      *at
    };
    let load = ctx.future((repo_root, run, at, retry), move |(repo_root, run, ..)| {
      tracing::trace!(target: crate::app_log::READ_LOG, read = "transcript");
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || transcript_in(&repo_root, run))
          .await?
          .map(Transcript)
          .map_err(|error| {
            tracing::error!(%error, "cannot read the run's transcript");
            messages::store_error(&error)
          })
      }
    });
    let state = load.state();
    let read = state.get();
    *self.state.lock() = Some(state);
    read
  }
}

/// The run's requests that wait for the person, and those auto mode's reviewer is judging, as the
/// project's waiting requests list them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Waiting {
  pending: Vec<String>,
  reviewing: Vec<String>,
}

impl Waiting {
  pub(super) fn of(tasks: &ProjectTasks, run: RunId) -> Self {
    let ids = |list: &[ShownApproval]| {
      list
        .iter()
        .filter(|shown| shown.approval.run == run)
        .map(|shown| shown.approval.id.clone())
        .collect()
    };
    Self {
      pending: ids(&tasks.pending_tool_approvals),
      reviewing: ids(&tasks.reviewing_tool_approvals),
    }
  }
}

/// A read of the transcript, told from another by the read rather than by its lines: each read asks for
/// the requests again, so that they are at least as new as it.
#[derive(Clone, Debug)]
struct Read(Option<Arc<Vec<Entry>>>);

impl PartialEq for Read {
  fn eq(&self, other: &Self) -> bool {
    match (&self.0, &other.0) {
      (Some(read), Some(other)) => Arc::ptr_eq(read, other),
      (read, other) => read.is_none() && other.is_none(),
    }
  }
}

/// The timeline's sources of `run`: `transcript`, the latest read of its transcript, with its requests
/// read after it; read again when its `waiting` requests change and on `retry`. Until the requests of a
/// new read are read, the read before it is shown with its own; `None` until the transcript was read.
pub(super) fn with_requests(
  ctx: &mut Ctx,
  (repo_root, run, hold): (PathBuf, RunId, ReadHold),
  (transcript, waiting, retry): (Option<Arc<Vec<Entry>>>, Option<Waiting>, u64),
) -> FutureState<Option<Loaded>, Message> {
  let deps = (repo_root, run, waiting, Read(transcript), retry);
  let load = ctx.future(deps, move |(repo_root, run, _, Read(transcript), _)| {
    if transcript.is_some() {
      tracing::trace!(target: crate::app_log::READ_LOG, read = "requests");
    }
    let hold = hold.clone();
    async move {
      let Some(entries) = transcript else {
        return Ok(None);
      };
      hold.released().await;
      background::run(move || requests_in(&repo_root, run))
        .await?
        .map(|requests| Some(Loaded::of(entries, requests)))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the run's tool requests");
          messages::store_error(&error)
        })
    }
  });
  load.state().get()
}
