//! The run's transcript in the chat, read again as the runner reports changes, one read at a time: a
//! change that comes while a read is under way waits for it and is read next. Started over with every
//! line instead, a run that writes faster than its transcript is read (a step that fails and starts
//! again) shows nothing until it pauses, and a request that ended in between never shows it on its card
//! (ORC-74).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureState},
  core::Signal,
};
use orchester_core::id::RunId;
use parking_lot::Mutex;

use crate::{
  background,
  messages::{self, Message},
  tasks::{timeline::load::transcript_in, transcript::Entry},
};

/// The run's transcript, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Read(pub(super) Arc<Vec<Entry>>);

impl lurq::app::component::DevtoolsInspectable for Read {}

/// Which change of the run the transcript is read at, and the read's state once it was asked for.
#[derive(Default)]
pub(super) struct TranscriptRead {
  at: Mutex<(u64, u64)>,
  state: Mutex<Option<Signal<FutureState<Read, Message>>>>,
}

impl TranscriptRead {
  /// `run`'s transcript, read again when `revision`, the run's changes, moved on since the last read
  /// and that read is done, and on `retry`.
  pub(super) fn read(
    &self,
    ctx: &mut Ctx,
    (repo_root, run): (PathBuf, Option<RunId>),
    (revision, retry): ((u64, u64), u64),
  ) -> FutureState<Read, Message> {
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
    let load = ctx.future((repo_root, run, at, retry), |(repo_root, run, ..)| async move {
      let Some(run) = run else {
        return Ok(Read(Arc::default()));
      };
      background::run(move || transcript_in(&repo_root, run))
        .await?
        .map(Read)
        .map_err(|error| {
          tracing::error!(%error, "cannot read the run's transcript");
          messages::store_error(&error)
        })
    });
    let state = load.state();
    let read = state.get();
    *self.state.lock() = Some(state);
    read
  }
}
