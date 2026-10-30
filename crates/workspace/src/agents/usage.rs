//! Published and draft pipeline uses, and the versions active runs pinned.
use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  project::TaskKey,
  resource::{AgentProfile, ModelId, Pipeline, Resource, ResourceKind, StepKey, StepKind, VersionNumber},
  run::{Run, RunEventKind, StepExecutor, StepStatus},
  store::{ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineUse {
  pub profile: ResourceId,
  pub pipeline: ResourceId,
  pub name: Arc<str>,
  pub version: u32,
  pub draft: bool,
  pub position: usize,
  pub step: Arc<str>,
  pub model: Option<ModelId>,
  pub model_overridden: bool,
  pub narrowed: bool,
  pub running: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveUse {
  pub profile: ResourceId,
  pub version: VersionNumber,
  pub profile_name: Option<Arc<str>>,
  pub position: usize,
  /// The run's recorded Started event, absent when its log does not contain one.
  pub started_at: Option<Timestamp>,
  pub task: TaskKey,
  pub title: Arc<str>,
  pub pipeline: Arc<str>,
  pub pipeline_version: VersionNumber,
  pub step: Arc<str>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
  pub steps: Vec<PipelineUse>,
  pub active: Vec<ActiveUse>,
}
impl Usage {
  pub fn count(&self, profile: ResourceId) -> usize {
    self
      .steps
      .iter()
      .filter(|step| step.profile == profile)
      .map(|step| step.pipeline)
      .collect::<std::collections::BTreeSet<_>>()
      .len()
  }
  /// A running run whose record cannot be read is left out of the counts: its task page says why, and one
  /// damaged record does not keep every profile from being read.
  pub fn load(db: &ProjectDb) -> Result<Self, StoreError> {
    let running: Vec<Run> = db
      .running_runs()
      .map_err(crate::tasks::detail::run_store_error)?
      .into_iter()
      .filter_map(Result::ok)
      .collect();
    let mut usage = Self::default();
    for resource in db.resources(ResourceKind::Pipeline)? {
      let latest = db.latest_version::<Pipeline>(resource.id())?;
      if let Some(version) = &latest {
        usage.steps.extend(pipeline_steps(
          db,
          &resource,
          version.number().get(),
          false,
          version.body(),
          &running,
        )?);
      }
      if let Some(draft) = db.draft::<Pipeline>(resource.id())? {
        let next = latest
          .as_ref()
          .map_or(1, |version| version.number().get().saturating_add(1));
        usage
          .steps
          .extend(pipeline_steps(db, &resource, next, true, draft.body(), &running)?);
      }
    }
    for run in running {
      usage.active.extend(active_uses(db, &run)?);
    }
    Ok(usage)
  }
}

fn pipeline_steps(
  db: &ProjectDb,
  resource: &Resource,
  version: u32,
  draft: bool,
  body: &Pipeline,
  running: &[Run],
) -> Result<Vec<PipelineUse>, StoreError> {
  let (id, name) = (resource.id(), resource.name().as_str());
  let mut uses = Vec::new();
  for (index, step) in body.steps().iter().enumerate() {
    let StepKind::Agent {
      profile: Some(profile),
      model,
      repository,
      permissions,
    } = &step.kind
    else {
      continue;
    };
    let published = db.latest_version::<AgentProfile>(*profile)?;
    let effective = published.as_ref().map(|v| v.body());
    let count = if draft {
      0
    } else {
      running
        .iter()
        .filter(|run| {
          let pinned = run.snapshot().pipeline();
          pinned.resource == id
            && pinned.number.get() == version
            && executing(run, &step.key)
            && run
              .snapshot()
              .step(&step.key)
              .is_some_and(|s| matches!(s.executor(),StepExecutor::Agent { profile: p, .. } if p.resource == *profile))
        })
        .count()
    };
    uses.push(PipelineUse {
      profile: *profile,
      pipeline: id,
      name: name.into(),
      version,
      draft,
      position: index + 1,
      step: step.name.as_str().into(),
      model: model.clone().or_else(|| effective.map(|p| p.model().clone())),
      model_overridden: model.is_some(),
      narrowed: effective.is_some_and(|p| p.permissions().narrowed(*repository, permissions) != *p.permissions()),
      running: count,
    });
  }
  Ok(uses)
}

fn executing(run: &Run, key: &StepKey) -> bool {
  run.current_attempt().is_some_and(|attempt| {
    attempt
      .executions()
      .iter()
      .any(|e| e.step() == key && e.status() == StepStatus::Running)
  })
}
fn active_uses(db: &ProjectDb, run: &Run) -> Result<Vec<ActiveUse>, StoreError> {
  let task = db.task(run.task())?.ok_or(StoreError::TaskNotFound(run.task()))?;
  let pinned = run.snapshot().pipeline();
  let resource = db
    .resource(pinned.resource)?
    .ok_or(StoreError::ResourceNotFound(pinned.resource))?;
  let started_at = db
    .events(run.id())
    .map_err(crate::tasks::detail::run_store_error)?
    .iter()
    .find(|event| matches!(event.kind(), RunEventKind::Started))
    .map(|event| event.at());
  let mut uses = Vec::new();
  for (index, step) in run
    .snapshot()
    .steps()
    .iter()
    .enumerate()
    .filter(|(_, step)| executing(run, step.key()))
  {
    let StepExecutor::Agent { profile, .. } = step.executor() else {
      continue;
    };
    let name = run
      .snapshot()
      .body()
      .step(step.key())
      .map_or(step.key().as_str(), |step| step.name.as_str());
    uses.push(ActiveUse {
      profile: profile.resource,
      version: profile.number,
      profile_name: db
        .version::<AgentProfile>(profile.resource, profile.number)?
        .map(|version| version.body().role().as_str().into()),
      position: index + 1,
      started_at,
      task: task.key().clone(),
      title: task.details().title.as_str().into(),
      pipeline: resource.name().as_str().into(),
      pipeline_version: pinned.number,
      step: name.into(),
    });
  }
  Ok(uses)
}
