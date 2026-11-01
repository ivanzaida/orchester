//! Linking a local checkout to a server project (`docs/architecture/server.md`, "Migration", rows 12 and 49; design
//! `KSnOW`, which `N9NFio` reuses as the first link's upload page): what linking would do, read before the person
//! confirms it, and the link itself. The first checkout linked to an empty project uploads its tasks with their
//! numbers; a checkout without tasks is linked first and takes a project's tasks as they are in its first download
//! ([`super::download`]); a checkout with tasks linked to a project that has tasks joins it with new numbers after the
//! person reviewed them (N3). `orchester.toml` is written only when the person asks for it, and never over a file that
//! names anything else.

use std::path::Path;

use orchester_api::SharedProject;
use orchester_core::{id::IdempotencyKey, project::TaskKey, repository::RepositoryIdentity};
use orchester_store::{ProjectDb, StoreError};

use super::{
  ShareError, Shared,
  discovery::{self, DiscoveryState, discovery_state},
  join::JoinTasks,
  share::{InProgress, upload_keys},
};
use crate::{
  background,
  sharing::{ProjectAddress, ServerEntry, Servers, SharingError},
};

/// What linking a checkout to a server project would do.
#[derive(Clone, Debug)]
pub struct LinkPlan {
  pub project: SharedProject,
  pub kind: LinkKind,
  pub discovery: DiscoveryState,
  /// The project is this computer's and the server the account's: the link moves it into the account, and what
  /// confirms the link says so.
  pub moves_to_account: bool,
}

/// How the checkout's tasks and the server project's meet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkKind {
  /// The server project has no task: every local task goes up with its number (row 12). `tasks` by number, `next`
  /// the key the server gives the next new task, `claimed` the tasks with a run that has not ended.
  Upload {
    tasks: Vec<TaskKey>,
    next: TaskKey,
    claimed: Vec<TaskKey>,
  },
  /// The checkout has no task: the server project's `tasks` come down as they are.
  Download { tasks: usize },
  /// Both have tasks: the local ones join with new numbers once the person reviewed them, which is N3's.
  Review { local: usize, server: usize },
}

/// What the person confirmed.
pub struct LinkRequest {
  /// The checkout's `origin`, for the project's remote hint when it has none.
  pub remote: Option<RepositoryIdentity>,
  /// The upload's idempotency key, made when the plan was read: a retry links once.
  pub key: IdempotencyKey,
  /// Whether to write `orchester.toml` naming the project.
  pub write_discovery: bool,
}

impl Servers {
  /// What linking the project at `repo_root` to `project` on `entry` would do: it asks how many tasks the server
  /// project has.
  pub async fn link_plan(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    project: SharedProject,
  ) -> Result<LinkPlan, SharingError> {
    super::join::still_local(repo_root).await?;
    ProjectAddress::reported(&project.address, &entry.address)?;
    let id = project.id;
    let server = self
      .with_token(entry, |client, token| async move { client.tasks(&token, id).await })
      .await?
      .tasks
      .len();
    let (root, address) = (repo_root.to_owned(), project.address.clone());
    let (workspace, target) = (self.inner.workspace.clone(), entry.stored.id);
    let (local, discovery, moves_to_account) = background::run(move || -> Result<_, StoreError> {
      let db = ProjectDb::open(&root)?;
      let moves = workspace.servers_moving_project(db.project().id())?.contains(&target);
      Ok((upload_keys(&db)?, discovery_state(&root, &address), moves))
    })
    .await??;
    let (tasks, next, claimed) = local;
    let kind = match (tasks.len(), server) {
      (_, 0) => LinkKind::Upload { tasks, next, claimed },
      (0, server) => LinkKind::Download { tasks: server },
      (local, server) => LinkKind::Review { local, server },
    };
    Ok(LinkPlan {
      project,
      kind,
      discovery,
      moves_to_account,
    })
  }

  /// Links the project at `repo_root` as `plan` says, once the person confirmed `request`. When the checkout's tasks
  /// are no longer those `plan` read (one was added meanwhile, by another screen or MCP), nothing is sent:
  /// [`SharingError::ProjectChanged`], and the person reviews the link again. From that check on the project takes no
  /// change from this computer's screens until the link is made.
  pub async fn link_checkout(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    plan: &LinkPlan,
    request: LinkRequest,
  ) -> Result<Shared, ShareError> {
    let _in_progress = InProgress::start(self, repo_root);
    super::join::still_local(repo_root).await?;
    let reviewed = match &plan.kind {
      LinkKind::Upload { tasks, .. } => tasks.clone(),
      LinkKind::Download { .. } => Vec::new(),
      LinkKind::Review { .. } => {
        return Err(ShareError::Taken {
          project: plan.project.id,
          member: true,
          owners: Vec::new(),
        });
      }
    };
    let root = repo_root.to_owned();
    let (current, ..) = background::run(move || upload_keys(&ProjectDb::open(&root)?))
      .await
      .map_err(SharingError::from)??;
    if current != reviewed {
      return Err(SharingError::ProjectChanged.into());
    }
    let write = request.write_discovery && plan.discovery == DiscoveryState::Absent;
    if let LinkKind::Upload { .. } = plan.kind {
      return self
        .link_upload(
          entry,
          repo_root,
          plan.project.clone(),
          (request.remote, request.key),
          write,
        )
        .await;
    }
    self.link_download(entry, repo_root, plan, (request.key, write)).await
  }

  /// Links a checkout without tasks to `plan`'s project as it is on the server: its tasks come down in the first
  /// download, which the link only starts, so the project opens before they are in ([`super::download`]).
  async fn link_download(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    plan: &LinkPlan,
    (key, write): (IdempotencyKey, bool),
  ) -> Result<Shared, ShareError> {
    let review = self.review(entry, repo_root, plan.project.clone()).await?;
    self
      .join_taking((entry, JoinTasks::FirstDownload), repo_root, &review, (Vec::new(), key))
      .await
      .map_err(|failed| failed.error)?;
    let (root, address) = (repo_root.to_owned(), review.address);
    Ok(
      background::run(move || -> Result<Shared, SharingError> {
        let link = ProjectDb::open(&root)?.sharing()?.ok_or(StoreError::NotShared)?;
        let discovery = write.then(|| discovery::write(&root, &address));
        Ok(Shared { link, discovery })
      })
      .await
      .map_err(SharingError::from)??,
    )
  }
}
