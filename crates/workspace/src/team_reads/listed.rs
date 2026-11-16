//! A team project's tasks as the screens list them (P6): each [`TaskRead`] as core's [`Task`], so the lists built for a
//! local project's tasks show the server's, with what a row says beside the task itself. A team task names one of the
//! team's pipelines by its resource id (the person's decision of 2026-11-05, `docs/architecture/server.md` row 80);
//! [`TeamPipelines`] says which of the team's pipelines this session read, and a task whose pipeline is none of them is
//! kept with no pipeline and what it names among the missing ones.

use std::collections::{HashMap, HashSet};

use orchester_api::v2::TaskRead;
use orchester_core::{
  id::{ResourceId, TaskId},
  project::TaskKey,
  resource::ResourceKind,
  task::{Task, TaskDetails},
  time::Timestamp,
};

use crate::{
  sharing::{SharingError, project::convert},
  team_config::TeamConfig,
};

/// The team's pipelines as this session read them from the team server: each one not archived and with a published
/// version. A team task may name only these; a person's own local pipelines never run a team task.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamPipelines {
  /// `None` while the team's configuration has not been read this session ([`Self::unread`]).
  read: Option<HashSet<ResourceId>>,
}

impl TeamPipelines {
  /// The team's pipelines in `config`, the team's configuration read this session.
  pub fn of(config: &TeamConfig) -> Self {
    let read = config
      .resources(ResourceKind::Pipeline)
      .into_iter()
      .filter(|resource| resource.number().is_some())
      .map(|resource| resource.id)
      .collect();
    Self { read: Some(read) }
  }

  /// The team's pipelines while its configuration is being read, or could not be read: which pipelines are the team's
  /// cannot be told yet, so a task's list shows the pipeline each task names and says none is missing, rather than wait
  /// or call every one missing. Nothing may be *sent* naming a pipeline from this: the writers read [`Self::of`].
  pub fn unread() -> Self {
    Self { read: None }
  }

  /// Whether `pipeline` is one of the team's; `false` while the configuration is not read.
  pub fn contains(&self, pipeline: ResourceId) -> bool {
    self.read.as_ref().is_some_and(|read| read.contains(&pipeline))
  }

  /// The team pipeline a team task's `pipeline` names, as the server keeps it: its resource id. While the
  /// configuration is not read, the id the task names, if it is one.
  pub fn id(&self, pipeline: &str) -> Option<ResourceId> {
    let id = pipeline.parse().ok()?;
    match &self.read {
      Some(read) => read.contains(&id).then_some(id),
      None => Some(id),
    }
  }
}

/// The tasks of one listing, in its order, with when each last changed, its key before it joined, the pipeline it
/// names that is none of the team's pipelines here and the tasks of the listing that block it and are not closed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListedTasks {
  pub tasks: Vec<Task>,
  pub updated: HashMap<TaskId, Timestamp>,
  pub former_keys: HashMap<TaskId, String>,
  pub missing_pipelines: HashMap<TaskId, String>,
  pub blocking: HashMap<TaskId, Vec<TaskId>>,
}

impl ListedTasks {
  /// `reads` as listed tasks; `pipeline` finds the team pipeline a task names. A read that is no valid task is
  /// [`SharingError::InvalidAnswer`].
  pub fn of(reads: &[TaskRead], pipeline: impl Fn(&str) -> Option<ResourceId>) -> Result<Self, SharingError> {
    let mut listed = Self::default();
    for read in reads {
      let named = read.task.content.pipeline.as_deref();
      let found = named.and_then(&pipeline);
      if let (Some(named), None) = (named, found) {
        listed.missing_pipelines.insert(read.task.id, named.to_owned());
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
