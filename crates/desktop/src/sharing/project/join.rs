//! *Join* (`docs/architecture/server.md`, "Two people share one repository"; design 10.21, 10.22,
//! 10.26 D): the repository is shared on a server already and the person is a member. Their local
//! tasks are reviewed first, all ticked, with the server's similar titles as a hint; a task with a
//! run that has not ended stays ticked, as working on a shared task needs a claim. Confirming
//! uploads the ticked tasks in one import with new numbers, their ids and their former keys (those
//! with an unfinished run claimed for the person), then the cache takes the server project's tasks
//! and tags, the unticked tasks stay here *Not shared*, and the project takes the server's key. The
//! import is one step on the server: when it fails nothing was uploaded; a retry under the same key
//! after a lost answer joins once.

use std::{collections::HashMap, num::NonZeroU32, path::Path};

use orchester_api::{DroppedLink, SharedProject, TasksImport};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  project::ProjectKey,
  repository::RepositoryIdentity,
  run::{BranchName, RunNumber, TaskPhase},
  sharing::similar_titles,
  store::{MergeStore, RunStore, TaskStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink, StoreError};

use super::{convert, organization::tag, share};
use crate::{
  background,
  sharing::{ServerEntry, Servers, SharingError},
  tasks::detail::run_store_error,
  workspace::OpenProject,
};

/// The server project a repository is shared as, and what joining it would do (10.21).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinReview {
  pub project: orchester_core::id::ProjectId,
  pub key: ProjectKey,
  /// The number the first uploaded task gets.
  pub next: u32,
  /// The local tasks, by number.
  pub tasks: Vec<ReviewTask>,
}

impl lurq::app::component::DevtoolsInspectable for JoinReview {}

/// A local task in the review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewTask {
  pub task: Task,
  /// Its latest run and where the task stands with it; `None` without a run.
  pub run: Option<(RunNumber, TaskPhase)>,
  /// How many runs it has, and the branch of the latest, which keeps its name.
  pub runs: usize,
  pub branch: Option<BranchName>,
  /// The server's task with the most similar title, as a hint only.
  pub similar: Option<SimilarTask>,
}

impl ReviewTask {
  /// A task with a run that has not ended is uploaded claimed: it cannot be unticked.
  pub fn locked(&self) -> bool {
    self.run.is_some_and(|(_, phase)| phase.run_not_ended())
  }
}

/// A server task whose title looks like a local one's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimilarTask {
  pub key: String,
  pub title: String,
  pub created_by: String,
}

/// The project joined: as it is opened now, with its new key, and the blocking links the server
/// dropped because they named a task that stays here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joined {
  pub open: OpenProject,
  /// Each dropped link as the keys before joining of the task and of the task it named.
  pub dropped: Vec<(String, String)>,
}

impl lurq::app::component::DevtoolsInspectable for Joined {}

/// Why the project did not join, and whether its tasks reached the server first: an answer lost
/// after the import leaves them there, and joining again under the same key finishes without
/// uploading them twice.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct JoinFailed {
  pub uploaded: bool,
  #[source]
  pub error: SharingError,
}

impl Servers {
  /// What joining the project `identity` is shared as on `entry` would do for the project at
  /// `repo_root`. `403 not_member` when the person is not a member.
  pub async fn join_review(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    identity: RepositoryIdentity,
  ) -> Result<JoinReview, SharingError> {
    let project = self.shared_project(entry, identity).await?;
    let id = project.id;
    let server_tasks = self
      .with_token(entry, |client, token| async move { client.tasks(&token, id).await })
      .await?
      .tasks;
    let hints: Vec<SimilarTask> = server_tasks
      .iter()
      .map(|task| SimilarTask {
        key: task.key.clone(),
        title: task.content.title.clone(),
        created_by: task.created.by.display_name.clone(),
      })
      .collect();
    let root = repo_root.to_owned();
    let tasks = background::run(move || review_tasks(&root, &hints)).await??;
    Ok(JoinReview {
      project: project.id,
      key: ProjectKey::new(&project.key).map_err(|_| SharingError::InvalidAnswer("project key"))?,
      next: u32::try_from(project.next_number).map_err(|_| SharingError::InvalidAnswer("next number"))?,
      tasks,
    })
  }

  /// Joins `review`'s server project on `entry` with the project at `repo_root`, uploading the
  /// `ticked` tasks and every task with a run that has not ended under `key`.
  pub async fn join(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    review: &JoinReview,
    (ticked, key): (Vec<TaskId>, IdempotencyKey),
  ) -> Result<Joined, JoinFailed> {
    let _in_progress = share::InProgress::start(self, repo_root);
    let failed = |uploaded: bool| move |error: SharingError| JoinFailed { uploaded, error };
    let root = repo_root.to_owned();
    let (import, claimed) = background::run(move || upload(&root, &ticked, key))
      .await
      .map_err(SharingError::from)
      .and_then(|read| read.map_err(SharingError::from))
      .map_err(failed(false))?;
    let uploaded = !import.tasks.is_empty();
    let dropped = if uploaded {
      let names: HashMap<TaskId, String> = import
        .tasks
        .iter()
        .filter_map(|task| Some((task.id, task.former_key.clone()?)))
        .collect();
      let project = review.project;
      let created = self
        .with_token(entry, |client, token| async move {
          client.import_tasks(&token, project, &import).await
        })
        .await
        .map_err(failed(false))?;
      dropped_keys(&created.dropped_links, &names, repo_root)
        .await
        .map_err(failed(true))?
    } else {
      Vec::new()
    };
    let open = self
      .link_joined(entry, repo_root, review, &claimed)
      .await
      .map_err(failed(uploaded))?;
    Ok(Joined { open, dropped })
  }

  /// Caches `review`'s server project as it is now and links the project at `repo_root` to it.
  async fn link_joined(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    review: &JoinReview,
    claimed: &HashMap<TaskId, NonZeroU32>,
  ) -> Result<OpenProject, SharingError> {
    let project = review.project;
    let shared = self
      .with_token(
        entry,
        |client, token| async move { client.project(&token, project).await },
      )
      .await?;
    let tags = self
      .with_token(entry, |client, token| async move { client.tags(&token, project).await })
      .await?;
    let tasks = self
      .with_token(
        entry,
        |client, token| async move { client.tasks(&token, project).await },
      )
      .await?;
    let link = SharingLink {
      server: entry.stored.id,
      project,
      key: review.key.clone(),
      role: shared.role,
      joined_at: Timestamp::now(),
      cursor: tasks.cursor,
      last_synced_at: Timestamp::now(),
      removed_at: None,
    };
    let (root, workspace, claimed) = (repo_root.to_owned(), self.inner.workspace.clone(), claimed.clone());
    let open = background::run(move || -> Result<OpenProject, SharingError> {
      let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
      let cached = tasks.tasks.iter().map(convert::cached).collect::<Result<Vec<_>, _>>()?;
      ProjectDb::open(&root)?.join_sharing(&link, (&tags, &cached), &claimed)?;
      // The application's list of projects shows the project's new key.
      Ok(workspace.open(&root)?)
    })
    .await??;
    self.shared_changed();
    Ok(open)
  }

  /// The project `identity` is shared as on `entry`.
  async fn shared_project(
    &self,
    entry: &ServerEntry,
    identity: RepositoryIdentity,
  ) -> Result<SharedProject, SharingError> {
    self
      .with_token(entry, |client, token| async move {
        client.projects(&token, Some(&identity)).await
      })
      .await?
      .into_iter()
      .next()
      .ok_or(SharingError::InvalidAnswer("no project for the repository"))
  }
}

fn review_tasks(root: &Path, hints: &[SimilarTask]) -> Result<Vec<ReviewTask>, StoreError> {
  let db = ProjectDb::open(root)?;
  db.tasks()?
    .into_iter()
    .map(|task| {
      let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
      let count = runs.len();
      let latest = runs.into_iter().filter_map(Result::ok).next_back();
      let branch = latest.as_ref().map(|run| run.branch().clone());
      let run = match latest {
        Some(run) => {
          let merge = db.run_merge(run.id())?;
          Some((run.number(), TaskPhase::of(&task, Some(&run), merge.as_ref())))
        }
        None => None,
      };
      let title = task.details().title.as_str();
      let similar = similar_titles(title, hints.iter().map(|hint| hint.title.as_str()))
        .first()
        .map(|index| hints[*index].clone());
      Ok(ReviewTask {
        task,
        run,
        runs: count,
        branch,
        similar,
      })
    })
    .collect()
}

/// The import of the ticked tasks and of every task with a run that has not ended, by number,
/// with the number of each one's unfinished run, for which it is claimed.
fn upload(
  root: &Path,
  ticked: &[TaskId],
  key: IdempotencyKey,
) -> Result<(TasksImport, HashMap<TaskId, NonZeroU32>), StoreError> {
  let db = ProjectDb::open(root)?;
  let names: HashMap<_, _> = db
    .project_tags()?
    .into_iter()
    .map(|tag| (tag.id(), tag.name().as_str().to_owned()))
    .collect();
  let mut tasks = Vec::new();
  let mut claimed = HashMap::new();
  for task in db.tasks()? {
    let active = db.active_task_run(task.id())?;
    if let Some(run) = active {
      let run = db
        .run(run)
        .map_err(run_store_error)?
        .ok_or(StoreError::TaskNotFound(task.id()))?;
      claimed.insert(
        task.id(),
        NonZeroU32::new(run.number().get()).expect("run numbers start at 1"),
      );
    } else if !ticked.contains(&task.id()) {
      continue;
    }
    tasks.push(share::upload_task(&db, &task, &names, share::UploadAs::Import)?);
  }
  Ok((
    TasksImport {
      idempotency_key: key,
      tasks,
    },
    claimed,
  ))
}

/// The links the server dropped, by the keys before joining of their tasks; a task that is not in
/// the upload is named by its local key.
async fn dropped_keys(
  dropped: &[DroppedLink],
  uploaded: &HashMap<TaskId, String>,
  root: &Path,
) -> Result<Vec<(String, String)>, SharingError> {
  if dropped.is_empty() {
    return Ok(Vec::new());
  }
  let (dropped, uploaded, root) = (dropped.to_vec(), uploaded.clone(), root.to_owned());
  let keys = background::run(move || -> Result<_, StoreError> {
    let db = ProjectDb::open(&root)?;
    let key = |id: TaskId| -> Result<String, StoreError> {
      if let Some(key) = uploaded.get(&id) {
        return Ok(key.clone());
      }
      Ok(
        db.task(id)?
          .map_or_else(|| id.to_string(), |task| task.key().to_string()),
      )
    };
    dropped
      .iter()
      .map(|link| Ok((key(link.task)?, key(link.blocking)?)))
      .collect::<Result<Vec<_>, StoreError>>()
  })
  .await??;
  Ok(keys)
}
