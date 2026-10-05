//! Starting or continuing work on a shared task takes its run claim first (`docs/architecture/
//! server.md`, "Claims"): Create & run, Start, Run again, continuing a paused or interrupted run,
//! Send & continue and a follow-up all ask here before they ask the runner. A task someone else
//! holds starts nothing, and the person is told who holds it. A local project's tasks need no claim.

use std::path::Path;

use orchester_api::{
  ClaimTake, ErrorCode, SharingErrorDetails,
  client::{ClientError, ServerAddress},
};
use orchester_core::{id::TaskId, sharing::ClaimKind};

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

impl Servers {
  /// Takes the run claim of `task` in the project at `repo_root` when it is shared, or confirms that
  /// the person holds it; a local project's task needs none.
  pub async fn claim_for_work(&self, repo_root: &Path, task: TaskId) -> Result<(), WorkRefusal> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let server = background::run(move || ProjectServer::open(&servers, &root))
      .await
      .map_err(|message| WorkRefusal::sharing(SharingError::from(message), None))?
      .map_err(|error| WorkRefusal::sharing(error, None))?;
    let Some(server) = server else {
      return Ok(());
    };
    server.take_run_claim(task).await
  }
}

impl ProjectServer {
  async fn take_run_claim(&self, task: TaskId) -> Result<(), WorkRefusal> {
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
    Ok(())
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

impl Servers {
  /// After work on `task` failed to start or its save did not go through: the run claim the person
  /// holds is released when no run of the task is under way here, so it does not stay held (and
  /// kept fresh by the heartbeat) for work that never started. A claim taken by hand stays.
  pub async fn release_unused_claim(&self, repo_root: &Path, task: TaskId) {
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
      Ok((held && db.active_task_run(task)?.is_none()).then_some(server))
    })
    .await;
    let server = match unused {
      Ok(Ok(Some(server))) => server,
      Ok(Ok(None)) | Err(_) => return,
      Ok(Err(error)) => {
        tracing::warn!(%error, %task, "cannot tell whether an unused claim is held");
        return;
      }
    };
    if let Err(error) = server.release_claim(task).await {
      // It goes stale 15 minutes after the last heartbeat that names it.
      tracing::warn!(%error, %task, "an unused claim was not released");
    }
  }
}

impl ProjectServer {
  async fn release_claim(&self, task: TaskId) -> Result<(), SharingError> {
    let released = self
      .call(|client, token, project| async move { client.release_claim(&token, project, task).await })
      .await?;
    let server = self.clone();
    background::run(move || -> Result<(), SharingError> {
      let db = orchester_store::ProjectDb::open(server.repo_root())?;
      RemoteTaskStore::new(db, server)?.cache(std::slice::from_ref(&released), None)
    })
    .await??;
    self.servers.shared_changed();
    Ok(())
  }
}
