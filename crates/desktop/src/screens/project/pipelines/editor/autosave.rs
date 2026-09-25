//! Saving the edited steps as the draft while the user edits. One save runs at a time; edits
//! made while it runs wait in a single slot, newest wins, and the running save stores them
//! next. The follow-up save is chained inside the running future, never started from a watch
//! of the save's own state: lurq calls a watch while it holds that signal, so starting the
//! action there (which sets the same signal) never returns.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  id::ResourceId,
  resource::{Draft, DraftSaved, Pipeline},
  time::Timestamp,
};
use parking_lot::Mutex;

use super::EditorHandle;
use crate::{
  activity::{Activity, OperationSpec},
  background,
  messages::{self, Message},
  pipelines::data::save_draft_in,
};

/// A draft revision stored at a time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Saved {
  pub revision: u64,
  pub at: Timestamp,
  pub outcome: DraftSaved,
}

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// One revision of the steps to store as the draft.
pub(super) struct SaveJob {
  pub repo_root: PathBuf,
  pub id: ResourceId,
  pub revision: u64,
  pub draft: Draft<Pipeline>,
  pub spec: OperationSpec,
}

pub(super) type SaveAction = FutureAction<(SaveJob, DraftQueue), Saved, Message>;

#[derive(Default)]
struct Queue {
  /// The newest revision waiting for the running save.
  next: Option<SaveJob>,
  /// A save is running and takes `next` before it finishes.
  running: bool,
  /// The newest revision handed to a save; older or equal offers are already covered.
  offered: u64,
}

/// The revisions waiting to be saved, shared by the editor and its running save.
#[derive(Clone, Default)]
pub(super) struct DraftQueue(Arc<Mutex<Queue>>);

impl DraftQueue {
  /// Queues `job`; returns it when no save is running and the caller has to start one.
  pub fn offer(&self, job: SaveJob) -> Option<SaveJob> {
    let mut queue = self.0.lock();
    if job.revision <= queue.offered {
      return None;
    }
    queue.offered = job.revision;
    if queue.running {
      queue.next = Some(job);
      return None;
    }
    queue.running = true;
    Some(job)
  }

  /// The revision to save next; `None` ends the running save in the same critical section, so
  /// an offer made afterwards starts a new one.
  fn next(&self) -> Option<SaveJob> {
    let mut queue = self.0.lock();
    let next = queue.next.take();
    queue.running = next.is_some();
    next
  }

  /// A save failed: nothing waits any more, and the next offer, even of the same revision,
  /// starts a new save.
  fn failed(&self) {
    let mut queue = self.0.lock();
    *queue = Queue::default();
  }
}

pub(super) fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(first, queue): (SaveJob, DraftQueue)| {
    let activity = activity.clone();
    async move {
      let mut job = first;
      loop {
        let saved = match save(&activity, job).await {
          Ok(saved) => saved,
          Err(error) => {
            queue.failed();
            return Err(error);
          }
        };
        match queue.next() {
          Some(next) => job = next,
          None => return Ok(saved),
        }
      }
    }
  })
}

async fn save(activity: &Activity, job: SaveJob) -> Result<Saved, Message> {
  let SaveJob {
    repo_root,
    id,
    revision,
    draft,
    spec,
  } = job;
  let work = async move {
    let at = draft.updated_at();
    let saved = background::run(move || save_draft_in(&repo_root, id, &draft)).await?;
    saved.map(|outcome| Saved { revision, at, outcome }).map_err(|error| {
      tracing::error!(%error, "cannot save the pipeline draft");
      messages::store_error(&error)
    })
  };
  activity.track(spec, work).await
}

/// Records what a finished save stored. It never starts a save: the running save already
/// stored every revision queued while it ran, and a failed one waits for the next edit.
pub(super) fn watch_saves(ctx: &mut Ctx, save: &SaveAction, handle: &EditorHandle) {
  let state = handle.state.clone();
  ctx.watch(&save.state(), move |action| {
    if action.is_pending() {
      return;
    }
    if let Some(saved) = &action.data {
      state.update(|editor| {
        if let Some(editor) = editor {
          editor.saved(saved.revision, saved.at, saved.outcome);
        }
      });
    }
  });
}
