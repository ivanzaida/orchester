//! A team project's tasks as the screens list them (P6): each [`TaskRead`] as core's [`Task`], so the lists built for a
//! local project's tasks show the server's, with what a row says beside the task itself. A pipeline is named by its
//! slug on the server; [`PipelineSlugs`] says which of this computer's pipelines a slug is, and a task whose slug names
//! none is kept with no pipeline and its slug among the missing ones.

use std::{collections::HashMap, path::Path};

use orchester_api::v2::TaskRead;
use orchester_core::{
  id::{ResourceId, TaskId},
  project::TaskKey,
  resource::ResourceKind,
  store::ResourceStore,
  task::{Task, TaskDetails},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::sharing::{SharingError, project::convert};

/// This computer's pipelines of a project by the slug team tasks name them with, and back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PipelineSlugs {
  by_slug: HashMap<String, ResourceId>,
  by_id: HashMap<ResourceId, String>,
}

impl PipelineSlugs {
  /// The pipelines of the project at `repo_root`. Blocking.
  pub fn read(repo_root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let mut slugs = Self::default();
    for resource in db.resources(ResourceKind::Pipeline)? {
      let slug = resource.slug().as_str().to_owned();
      slugs.by_id.insert(resource.id(), slug.clone());
      slugs.by_slug.insert(slug, resource.id());
    }
    Ok(slugs)
  }

  pub fn id(&self, slug: &str) -> Option<ResourceId> {
    self.by_slug.get(slug).copied()
  }

  pub fn slug(&self, id: ResourceId) -> Option<&str> {
    self.by_id.get(&id).map(String::as_str)
  }
}

/// The tasks of one listing, in its order, with when each last changed, its key before it joined, the slug of a
/// pipeline this computer lacks and the tasks of the listing that block it and are not closed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListedTasks {
  pub tasks: Vec<Task>,
  pub updated: HashMap<TaskId, Timestamp>,
  pub former_keys: HashMap<TaskId, String>,
  pub missing_pipelines: HashMap<TaskId, String>,
  pub blocking: HashMap<TaskId, Vec<TaskId>>,
}

impl ListedTasks {
  /// `reads` as listed tasks; `pipeline` finds this computer's pipeline of a slug. A read that is no valid task is
  /// [`SharingError::InvalidAnswer`].
  pub fn of(reads: &[TaskRead], pipeline: impl Fn(&str) -> Option<ResourceId>) -> Result<Self, SharingError> {
    let mut listed = Self::default();
    for read in reads {
      let slug = read.task.content.pipeline.as_deref();
      let found = slug.and_then(&pipeline);
      if let (Some(slug), None) = (slug, found) {
        listed.missing_pipelines.insert(read.task.id, slug.to_owned());
      }
      if let Some(former) = &read.task.former_key {
        listed.former_keys.insert(read.task.id, former.clone());
      }
      listed
        .updated
        .insert(read.task.id, Timestamp::from_unix_millis(read.task.updated.at));
      listed.tasks.push(task_of(read, found)?);
    }
    for read in reads {
      let open = read
        .task
        .content
        .blocking
        .iter()
        .map(|blocking| blocking.task)
        .filter(|task| {
          listed
            .tasks
            .iter()
            .any(|listed| listed.id() == *task && !closed(listed))
        })
        .collect::<Vec<_>>();
      if !open.is_empty() {
        listed.blocking.insert(read.task.id, open);
      }
    }
    Ok(listed)
  }
}

fn closed(task: &Task) -> bool {
  task.organization().status().is_closed()
}

/// `read` as core's task, its pipeline `pipeline`. The server pins no pipeline version on a task.
pub fn task_of(read: &TaskRead, pipeline: Option<ResourceId>) -> Result<Task, SharingError> {
  let cached = convert::cached(&read.task)?;
  let key: TaskKey = read
    .task
    .key
    .parse()
    .map_err(|_| SharingError::InvalidAnswer("task key"))?;
  let details = TaskDetails {
    title: cached.title,
    brief: cached.brief,
    acceptance_criteria: cached.criteria,
    pipeline,
    limits: cached.limits,
  };
  Ok(
    Task::from_stored(cached.id, key, details, cached.status, None, cached.created_at)
      .with_organization(cached.organization)
      .with_module(cached.module)
      .with_revision(cached.revision),
  )
}
