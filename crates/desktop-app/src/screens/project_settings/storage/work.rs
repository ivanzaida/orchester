//! Prepare exactly what the modal lists; removing uses the task/MCP job locks and rechecks the run.
use std::{
  path::Path,
  sync::{Arc, mpsc},
};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  id::RunId,
  integration::Leftovers,
  run::Run,
  store::{RunStore, TaskStore, WorktreeStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  messages::{self, Message},
  runner::ExecutorFolders,
  storage,
  tasks::{
    detail::run_store_error,
    integration::leftovers_in,
    integration_jobs::{RemoveArgs, Remover},
    outside::Outside,
  },
  workspace::OpenProject,
};

use crate::services::Services;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
  pub run: Run,
  pub title: String,
  pub left: Leftovers,
  pub sizes: Vec<Option<u64>>,
  pub person_only: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan(pub Vec<Candidate>);
impl DevtoolsInspectable for Plan {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed(pub Vec<(RunId, Message)>);
impl DevtoolsInspectable for Removed {}
pub type Prepare = FutureAction<Vec<RunId>, Plan, Message>;
pub type Remove = FutureAction<Plan, Removed, Message>;

pub fn prepare(ctx: &mut Ctx, open: OpenProject) -> Prepare {
  let services = Services::of(ctx);
  let (programs, runners) = (services.programs, services.runners);
  ctx.future_action(move |runs: Vec<RunId>| {
    let (root, programs) = (open.repo_root.clone(), programs.clone());
    let folders = ExecutorFolders::new(runners.clone(), open.clone());
    async move {
      background::run(move || {
        let db = ProjectDb::open_read_only(&root).map_err(|e| messages::store_error(&e))?;
        let mut plan = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in runs.into_iter().filter(|id| seen.insert(*id)) {
          let run = db
            .run(id)
            .map_err(|e| messages::store_error(&run_store_error(e)))?
            .filter(|run| run.status().is_terminal())
            .ok_or_else(|| Message::new("appMcp.errors.runNotEnded"))?;
          let task = db
            .task(run.task())
            .map_err(|e| messages::store_error(&e))?
            .ok_or_else(|| Message::new("appMcp.errors.runNotEnded"))?;
          let title = format!("{} · {}", task.key(), task.details().title);
          let left = leftovers_in(&root, &programs, &[Arc::new(run.clone())])
            .map_err(|e| e.message(", "))?
            .0
            .into_iter()
            .next()
            .map(|(_, left)| left)
            .ok_or_else(messages::interrupted)?;
          // The runner sizes each worktree; a sizing error is logged and stays visibly unknown, as it must not prevent
          // reviewing the real paths.
          let sizes = left
            .worktrees
            .iter()
            .map(|tree| storage::measure_worktree(&folders, Path::new(&tree.path)))
            .collect();
          let finishing = db
            .worktree_removal(id)
            .map_err(|e| messages::store_error(&e))?
            .is_some();
          let only_forgotten = left.worktrees.iter().all(|tree| tree.forgotten);
          let person_only = left.loses_files() && !(finishing && only_forgotten);
          plan.push(Candidate {
            person_only,
            run,
            title,
            left,
            sizes,
          });
        }
        Ok(Plan(plan))
      })
      .await?
    }
  })
}

pub fn remove(ctx: &mut Ctx, open: OpenProject) -> Remove {
  let services = Services::of(ctx);
  let separator = ctx.t("common.listSeparator");
  ctx.future_action(move |plan: Plan| {
    let (services, open, separator) = (services.clone(), open.clone(), separator.clone());
    async move {
      background::run(move || {
        let running = services.activity.start(
          orchester_workspace::activity::OperationSpec::new(
            storage::cleanup_running(),
            storage::cleanup_running(),
            storage::cleanup_running(),
          )
          .project(open.project.name().as_str())
          .quiet()
          .target(orchester_workspace::activity::Target::Project {
            repo_root: open.repo_root.clone(),
            place: orchester_workspace::activity::Place::Settings,
          }),
        );
        let mut freed = Some(0u64);
        let mut removed = 0usize;
        let mut failed = Vec::new();
        for candidate in plan.0 {
          let size = candidate
            .sizes
            .iter()
            .try_fold(0u64, |sum, size| size.map(|n| sum.saturating_add(n)));
          let id = candidate.run.id();
          if let Err(error) = remove_one(&services, &open, candidate, separator.clone()) {
            failed.push((id, error));
          } else {
            removed += 1;
            freed = freed.zip(size).map(|(sum, n)| sum.saturating_add(n));
          }
        }
        running.finish_as(storage::cleanup_outcome(failed.len(), removed, freed));
        Ok(Removed(failed))
      })
      .await?
    }
  })
}

fn remove_one(
  services: &Services,
  open: &OpenProject,
  candidate: Candidate,
  separator: Arc<str>,
) -> Result<(), Message> {
  let jobs = services.jobs.of(open.project.id());
  let db = ProjectDb::open_read_only(&open.repo_root).map_err(|e| messages::store_error(&e))?;
  let task_runs = db
    .runs_of_task(candidate.run.task())
    .map_err(|e| messages::store_error(&run_store_error(e)))?;
  if task_runs
    .iter()
    .any(|run| jobs.of(run.as_ref().map_or_else(|bad| bad.id, |run| run.id())).busy())
  {
    return Err(Message::new("appMcp.errors.integrationRunning"));
  }
  let (answer, outcome) = mpsc::channel();
  let (changes, project, removed) = (services.changes.clone(), open.project.id(), jobs.clone());
  jobs.of(candidate.run.id()).start_storage_remove(RemoveArgs {
    open: open.clone(),
    task: candidate.run.task(),
    key: candidate.title,
    number: candidate.run.number().get(),
    run: candidate.run,
    confirmed: candidate.left,
    worktrees: services.worktree_remover(open),
    activity: services.activity.clone(),
    separator,
    changed: Arc::new(move || {
      changes.record(project, Outside::Tasks);
      removed.count_outside_removal();
    }),
    by: Remover::Automation(answer),
  });
  outcome
    .recv()
    .map_err(|_| Message::new("appMcp.errors.integrationRunning"))?
    .map_err(|refused| refused.message)
}

pub fn total(plan: &Plan) -> Option<u64> {
  plan
    .0
    .iter()
    .flat_map(|candidate| &candidate.sizes)
    .try_fold(0u64, |sum, bytes| bytes.map(|n| sum.saturating_add(n)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedKeep(pub orchester_core::workspaces::KeepFree);
impl DevtoolsInspectable for SavedKeep {}
pub type Save = FutureAction<orchester_core::workspaces::KeepFree, SavedKeep, Message>;
pub fn save(ctx: &mut Ctx, open: OpenProject) -> Save {
  let changes = Services::of(ctx).changes;
  ctx.future_action(move |keep| {
    let (open, changes) = (open.clone(), changes.clone());
    async move {
      background::run(move || {
        storage::save_keep_free(&open.repo_root, keep).map_err(|e| messages::store_error(&e))?;
        changes.record(open.project.id(), Outside::ProjectSettings);
        Ok(SavedKeep(keep))
      })
      .await?
    }
  })
}
