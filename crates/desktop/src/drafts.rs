//! Saving a resource's draft while the user edits it (pipelines, instructions). One save runs at a
//! time; edits made while it runs wait in a single slot, newest wins, and the running save stores
//! them next. The follow-up save is chained inside the running future, never started from a watch
//! of the save's own state: lurq calls a watch while it holds that signal, so starting the action
//! there (which sets the same signal) never returns.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  id::ResourceId,
  resource::{Draft, DraftSaved, ResourceBody},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use parking_lot::Mutex;

use crate::{
  activity::{Activity, OperationSpec},
  background,
  messages::{self, Message},
};

/// A draft revision stored at a time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
  pub revision: u64,
  pub at: Timestamp,
  pub outcome: DraftSaved,
}

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// One revision of a resource to store as its draft.
pub struct SaveJob<B> {
  pub repo_root: PathBuf,
  pub id: ResourceId,
  pub revision: u64,
  pub draft: Draft<B>,
  pub spec: OperationSpec,
}

pub type SaveAction<B> = FutureAction<(SaveJob<B>, DraftQueue<B>), Saved, Message>;

struct Queue<B> {
  /// The newest revision waiting for the running save.
  next: Option<SaveJob<B>>,
  /// A save is running and takes `next` before it finishes.
  running: bool,
  /// The newest revision handed to a save; older or equal offers are already covered.
  offered: u64,
}

impl<B> Default for Queue<B> {
  fn default() -> Self {
    Self {
      next: None,
      running: false,
      offered: 0,
    }
  }
}

/// The revisions waiting to be saved, shared by an editor and its running save.
pub struct DraftQueue<B>(Arc<Mutex<Queue<B>>>);

impl<B> Clone for DraftQueue<B> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<B> Default for DraftQueue<B> {
  fn default() -> Self {
    Self(Arc::default())
  }
}

impl<B> DraftQueue<B> {
  /// Queues `job`; returns it when no save is running and the caller has to start one.
  pub fn offer(&self, job: SaveJob<B>) -> Option<SaveJob<B>> {
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
  fn next(&self) -> Option<SaveJob<B>> {
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

pub fn save_action<B>(ctx: &mut Ctx) -> SaveAction<B>
where
  B: ResourceBody + Send + Sync + 'static,
{
  let activity = crate::activity::of(ctx);
  ctx.future_action(move |(first, queue): (SaveJob<B>, DraftQueue<B>)| {
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

/// Stores `draft` as the resource's draft, or removes the draft when it repeats its version.
pub fn save_draft_in<B: ResourceBody>(
  repo_root: &Path,
  id: ResourceId,
  draft: &Draft<B>,
) -> Result<DraftSaved, StoreError> {
  ProjectDb::open(repo_root)?.save_draft(id, draft)
}

async fn save<B>(activity: &Activity, job: SaveJob<B>) -> Result<Saved, Message>
where
  B: ResourceBody + Send + Sync + 'static,
{
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
      tracing::error!(%error, "cannot save the draft");
      messages::store_error(&error)
    })
  };
  activity.track(spec, work).await
}
