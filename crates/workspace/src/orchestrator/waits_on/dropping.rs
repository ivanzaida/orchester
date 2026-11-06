//! *Drop the wait*, the person's (ORC-784; `docs/design/orchestrator-watch.md` decision 10): on the waiting task's
//! Summary and Conversation, and on the wait's row in the leading conversation. The run goes on at once without the
//! waited task's work and with no confirmation: the wait is kept as dropped, and a run whose agent waits for a message
//! gets one, sent for the person as the composer would answer it, that tells it to go on without that work. A run that
//! waits on a decision on its plan goes on once the person decides it on its card. No agent turn starts: the leading
//! conversation's row shows the drop in place.

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  run::Answerer,
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::view;
use crate::{
  background,
  messages::{self, Message},
  runner::Runners,
  sharing::Servers,
  tasks::{
    conversation::{
      composer::{ComposerMode, Then},
      follow::{RunnerOf, Sent, Stored, follow},
      outgoing::{Way, prepare_in, store_answer_in},
    },
    detail::run_store_error,
  },
  workspace::OpenProject,
};

/// What the waiting agent reads once the person dropped its wait on task `waited`.
fn told(waited: &str) -> String {
  format!(
    "The person dropped this run's wait on {waited}: its work will not come. Go on without it, and do what you need \
     of it here."
  )
}

/// The message stored for a dropped wait, which follows as the composer's would.
struct Told {
  task: TaskId,
  key: IdempotencyKey,
  then: Then,
  pipeline: String,
}

/// Drops `run`'s wait in project `open` at once, and lets the message that tells its agent so follow through `open`'s
/// runner; what became of that message, `None` when the run waits on a plan and gets none. Refused once the run no
/// longer waits on a task.
pub async fn drop_wait(
  (runners, servers): (&Runners, &Servers),
  open: &OpenProject,
  run: RunId,
) -> Result<Option<Sent>, Message> {
  let root = open.repo_root.clone();
  let stored = background::run(move || store(&root, run)).await??;
  let Some(stored) = stored else {
    return Ok(None);
  };
  let message = Stored {
    root: &open.repo_root,
    task: stored.task,
    run,
    key: stored.key,
    pipeline: &stored.pipeline,
  };
  Ok(Some(follow((runners, servers, RunnerOf::Project(open)), &message, stored.then).await))
}

/// Keeps `run`'s wait as dropped in the project at `root`, and stores the message for its agent when it waits for one.
fn store(root: &Path, run: RunId) -> Result<Option<Told>, Message> {
  let stored = |error: StoreError| messages::store_error(&error);
  let mut db = ProjectDb::open(root).map_err(stored)?;
  let found = db
    .run(run)
    .map_err(|error| stored(run_store_error(error)))?
    .ok_or_else(gone)?;
  let wait = view::read(&db, &found).map_err(stored)?;
  let Some(wait) = wait.filter(|wait| wait.state.holds()) else {
    return Err(gone());
  };
  let prepared = prepare_in(root, (found.task(), Some(run)), (false, None)).map_err(|error| error.message())?;
  if !db.drop_run_wait(run, Timestamp::now()).map_err(stored)? {
    return Err(gone());
  }
  if !matches!(prepared.mode, ComposerMode::Answer { .. }) {
    return Ok(None);
  }
  let key = IdempotencyKey::generate();
  let text = told(&wait.waited);
  let (then, _) = store_answer_in(root, &prepared, (key, Way::Primary), (&text, &[], None), Answerer::Person)
    .map_err(|error| error.message())?;
  let pipeline = prepared
    .detail
    .run
    .as_ref()
    .map(|view| view.pipeline.to_string())
    .unwrap_or_default();
  Ok(Some(Told {
    task: found.task(),
    key,
    then,
    pipeline,
  }))
}

/// The run no longer waits on a task: it went on, or someone dropped its wait first.
fn gone() -> Message {
  Message::new("orchestrator.waitsOn.dropGone")
}
