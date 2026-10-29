//! Starting or continuing work on a shared task takes its run claim first (`docs/architecture/
//! server.md`, "Claims"): Create & run, Start, Run again, continuing a paused or interrupted run,
//! Send & continue and a follow-up all ask here before they ask the runner. A task someone else
//! holds starts nothing, and the person is told who holds it. A local project's tasks need no claim.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
};

use orchester_api::{
  ClaimTake, ErrorCode, SharingErrorDetails,
  client::{ClientError, ServerAddress},
};
use orchester_core::{id::TaskId, sharing::ClaimKind, time::Timestamp};

use super::{ProjectServer, store::RemoteTaskStore};
use crate::{
  background,
  messages::Message,
  sharing::{Servers, SharingError},
};

/// Why work on a shared task did not start.
#[derive(Debug, thiserror::Error)]
pub enum WorkRefusal {
  /// Someone else holds the task's claim.
  #[error("the task is held by {holder}")]
  Held { holder: String, stale: bool },
  /// The task was merged; its claim is complete and nobody takes it again.
  #[error("the task was merged")]
  Merged,
  /// The claim could not be asked for, of the server at `server` when it is known.
  #[error("the claim could not be taken: {source}")]
  Sharing {
    source: SharingError,
    server: Option<ServerAddress>,
  },
}

impl WorkRefusal {
  /// What the person reads.
  pub fn message(&self) -> Message {
    match self {
      Self::Held { holder, stale: false } => Message::new("claims.refused.held").with("holder", holder.clone()),
      Self::Held { holder, stale: true } => Message::new("claims.refused.heldStale").with("holder", holder.clone()),
      Self::Merged => Message::new("claims.refused.merged"),
      Self::Sharing {
        source,
        server: Some(address),
      } => source.message(address),
      Self::Sharing { server: None, .. } => Message::new("sharing.errors.unavailable"),
    }
  }

  fn sharing(source: SharingError, server: Option<ServerAddress>) -> Self {
    Self::Sharing { source, server }
  }
}

/// The run claim work on a task goes on under: one this attempt took, or one the person held
/// already (kept after a finished run, or working on their other computer), or none for a local
/// task. Work that does not start releases only a claim its attempt took.
#[must_use = "work that does not start releases the claim it took"]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkClaim {
  /// The project and task whose claim this attempt took.
  taken: Option<(PathBuf, TaskId)>,
  /// The shared task whose claim this attempt asked for, and when.
  asked: Option<(TaskId, Timestamp)>,
}

impl WorkClaim {
  /// Whether this attempt took the claim.
  pub const fn newly_taken(&self) -> bool {
    self.taken.is_some()
  }

  /// The claim of two attempts at the same work, e.g. queuing the task and then starting it.
  pub fn and(self, later: Self) -> Self {
    Self {
      taken: self.taken.or(later.taken),
      asked: later.asked.or(self.asked),
    }
  }

  /// The work did not start: a claim this attempt took is released, while no run of the task is
  /// under way here, so it does not stay held (and kept fresh by the heartbeat) for nothing. A claim
  /// the person held already stays, held against its task's latest run again.
  pub async fn release(self, servers: &Servers) {
    if let Some((task, at)) = self.asked {
      servers.forget_work_asked(task, at);
    }
    if let Some((root, task)) = self.taken {
      servers.release_taken_claim(&root, task).await;
    }
  }
}

impl Servers {
  /// Takes the run claim of `task` in the project at `repo_root` when it is shared, or confirms that
  /// the person holds it; a local project's task needs none. Until the work's run is recorded, the
  /// task's latest run here may be one that ended: the claim ends of runs that ended leave a claim
  /// asked for after its run ended alone.
  pub async fn claim_for_work(&self, repo_root: &Path, task: TaskId) -> Result<WorkClaim, WorkRefusal> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let server = background::run(move || ProjectServer::open(&servers, &root))
      .await
      .map_err(|message| WorkRefusal::sharing(SharingError::from(message), None))?
      .map_err(|error| WorkRefusal::sharing(error, None))?;
    let Some(server) = server else {
      return Ok(WorkClaim::default());
    };
    // Before the server is asked, so a claim end on its way meanwhile finds it.
    let asked = Timestamp::now();
    self.inner.work_asked.lock().insert(task, asked);
    let taken = match server.take_run_claim(task).await {
      Ok(taken) => taken,
      Err(refusal) => {
        self.forget_work_asked(task, asked);
        return Err(refusal);
      }
    };
    Ok(WorkClaim {
      taken: taken.then(|| (repo_root.to_owned(), task)),
      asked: Some((task, asked)),
    })
  }

  /// When work on each shared task last asked for its run claim this session, by this computer's clock.
  pub(super) fn work_asked(&self) -> HashMap<TaskId, Timestamp> {
    self.inner.work_asked.lock().clone()
  }

  /// Forgets that work on `task` asked for its claim at `at`, which did not start, unless other work asked since.
  fn forget_work_asked(&self, task: TaskId, at: Timestamp) {
    let mut asked = self.inner.work_asked.lock();
    if asked.get(&task) == Some(&at) {
      asked.remove(&task);
    }
  }

  /// Releases the run claim of `task` that work which did not start took, unless a run of it is
  /// under way here (an interrupted run that did not continue is not).
  async fn release_taken_claim(&self, repo_root: &Path, task: TaskId) {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let unused = background::run(move || -> Result<Option<ProjectServer>, SharingError> {
      let Some(server) = ProjectServer::open(&servers, &root)? else {
        return Ok(None);
      };
      let db = orchester_store::ProjectDb::open(&root)?;
      let me = server.me();
      let held = db
        .shared_task_info(task)?
        .and_then(|info| info.claim)
        .is_some_and(|claim| Some(claim.holder.id) == me && claim.kind == ClaimKind::Run);
      Ok((held && !has_live_run(&db, task)?).then_some(server))
    })
    .await;
    let server = match unused {
      Ok(Ok(Some(server))) => server,
      Ok(Ok(None)) | Err(_) => return,
      Ok(Err(error)) => {
        tracing::warn!(%error, %task, "cannot tell whether a claim taken for work is still held");
        return;
      }
    };
    if let Err(error) = server.release_claim(task).await {
      // It goes stale 15 minutes after the last heartbeat that names it.
      tracing::warn!(%error, %task, "a claim taken for work that did not start was not released");
    }
  }
}

impl ProjectServer {
  /// Takes or confirms the run claim of `task`; whether this call took it, as the server's answer
  /// says: a claim it took starts now, while one held already keeps its start and only beats.
  async fn take_run_claim(&self, task: TaskId) -> Result<bool, WorkRefusal> {
    let address = || Some(self.entry.address.clone());
    self
      .writable()
      .map_err(|error| WorkRefusal::sharing(error, address()))?;
    let take = ClaimTake { kind: ClaimKind::Run };
    let answer = self
      .call(|client, token, project| async move { client.claim(&token, project, task, &take).await })
      .await;
    let claimed = match answer {
      Ok(claimed) => claimed,
      Err(SharingError::Client(error)) => return Err(refusal(&error, address())),
      Err(error) => return Err(WorkRefusal::sharing(error, address())),
    };
    let taken = claimed
      .claim
      .as_ref()
      .is_some_and(|claim| claim.since == claim.heartbeat_at);
    let server = self.clone();
    background::run(move || -> Result<(), SharingError> {
      let db = orchester_store::ProjectDb::open(server.repo_root())?;
      let mut store = RemoteTaskStore::new(db, server)?;
      store.cache(std::slice::from_ref(&claimed), None)
    })
    .await
    .map_err(|message| WorkRefusal::sharing(message.into(), address()))?
    .map_err(|error| WorkRefusal::sharing(error, address()))?;
    self.servers.shared_changed();
    Ok(taken)
  }
}

fn refusal(error: &ClientError, server: Option<ServerAddress>) -> WorkRefusal {
  match (error.code(), error.details()) {
    (Some(ErrorCode::ClaimHeld), Some(SharingErrorDetails::Claim { claim: Some(claim) })) => WorkRefusal::Held {
      holder: claim.holder.display_name.clone(),
      stale: claim.stale,
    },
    (Some(ErrorCode::TaskMerged), _) => WorkRefusal::Merged,
    _ => WorkRefusal::sharing(SharingError::Client(error.clone()), server),
  }
}

impl ProjectServer {
  pub(super) async fn release_claim(&self, task: TaskId) -> Result<(), SharingError> {
    let released = self
      .call(|client, token, project| async move { client.release_claim(&token, project, task).await })
      .await?;
    self.cache_answer(released).await
  }

  /// Writes `task`, as the server answered a change of its claim, to the cache.
  pub(super) async fn cache_answer(&self, task: orchester_api::SharedTask) -> Result<(), SharingError> {
    let server = self.clone();
    background::run(move || -> Result<(), SharingError> {
      let db = orchester_store::ProjectDb::open(server.repo_root())?;
      RemoteTaskStore::new(db, server)?.cache(std::slice::from_ref(&task), None)
    })
    .await??;
    self.servers.shared_changed();
    Ok(())
  }
}

/// Whether a run of `task` works here, waits for a person or goes on once resumed.
fn has_live_run(db: &orchester_store::ProjectDb, task: TaskId) -> Result<bool, SharingError> {
  use orchester_core::store::RunStore;
  let Some(run) = db.active_task_run(task)? else {
    return Ok(false);
  };
  let run = db.run(run).map_err(crate::tasks::detail::run_store_error)?;
  Ok(run.is_some_and(|run| run.status().has_live_work()))
}
