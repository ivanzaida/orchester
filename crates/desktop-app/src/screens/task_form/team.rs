//! Edit task on a team project's task (team authority P6d, ORC-734): the task as its server reads it, with the revision
//! a save is made on, never this computer's copy; a save sends what changed through the dispatcher
//! (`crate::team::edit`), and its outcome becomes the form's ([`SaveOutcome`]). The server keeps one version: a change
//! made first by someone else stops the save with the task as it is now, for the person to reapply theirs or keep it
//! (`super::team_conflict`, frame 05 `LmxUY` of `Team authority · redesign`). A member never reaches the form
//! (ORC-716); a refusal they meet anyway says so. A local project's form saves through its writer, as it always did.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_api::SharedTask;
use orchester_core::{
  id::{ResourceId, TaskId},
  name::Name,
  project::TaskKey,
  sharing::TaskRevision,
  task::{Task, TaskDetails},
  time::Timestamp,
};

use super::{
  actions::{SaveArgs, SaveOutcome},
  reading, screen,
  values::person_points,
};
use crate::{
  project::ShownProject,
  routes::Screen,
  services::Services,
  team::{
    TeamFailure, TeamSource,
    act::{Acted, Acting, Holds},
    edit::{Edited, TaskChange},
    invalidation::Stale,
    pages::{TeamTask, team_task},
    view,
  },
  ui::state::load_error,
};

/// A team task's save: the project's reads, which its server is reached through, and the task as the edits were made
/// on it.
#[derive(Clone)]
pub struct TeamSave {
  pub source: TeamSource,
  pub task: Arc<Task>,
}

/// Why the server did not take a team task's save, or its rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
  /// The server answered as `Acted` says.
  Acted(Acted),
  /// The person changed the task's points or base branch, which the team server keeps nowhere: nothing was sent.
  Unkept,
}

/// A team task's save that did not go through as a whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamStop {
  pub stop: Stop,
  /// The task as the server holds it after the changes it took first; `None` when it took none.
  pub saved: Option<Arc<Task>>,
  /// After someone else's change first: the task as the server holds it now, when it could be read.
  pub current: Option<Changed>,
}

/// The task as someone else's change left it, who made it and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
  pub task: Arc<Task>,
  pub by: String,
  pub at: Timestamp,
}

impl TeamStop {
  /// Someone changed the task first.
  pub const fn changed(&self) -> bool {
    matches!(self.stop, Stop::Acted(Acted::Changed(_)))
  }
}

/// A team save's stop, while the form says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamStopped(pub Arc<TeamStop>);

impl lurq::app::component::DevtoolsInspectable for TeamStopped {}

/// The team task `key` of `project` as its server reads it, with the project's reads; `Ok(None)` for no such task;
/// else the form's screen while it is read or why it cannot be.
pub(super) fn read(
  ctx: &mut Ctx,
  project: &ShownProject,
  key: &TaskKey,
) -> Result<Option<(TeamSource, TeamTask)>, Element> {
  view::provide_client(ctx);
  let source = match view::source(ctx, project) {
    Ok(Some(source)) => source,
    Ok(None) => return Err(reading(ctx, Screen::EditTask)),
    Err(failure) => return Err(failed(ctx, key, &failure)),
  };
  let read = ctx.query(team_task(source.clone(), key.clone()));
  if let Some(failure) = read.error() {
    return Err(failed(ctx, key, &failure));
  }
  match read.data() {
    None => Err(reading(ctx, Screen::EditTask)),
    Some(found) => Ok((*found).clone().map(|task| (source, task))),
  }
}

/// Why the team task cannot be read, with Try again.
fn failed(ctx: &mut Ctx, key: &TaskKey, failure: &TeamFailure) -> Element {
  let body = match failure {
    TeamFailure::Unavailable { .. } => "task_form.load.teamUnavailable",
    TeamFailure::SignedOut => "task_form.load.teamSignedOut",
    TeamFailure::NotMember => "task_form.load.teamNotMember",
    TeamFailure::Unsupported { .. } | TeamFailure::Refused => "task_form.load.teamRefused",
  };
  let title = ctx.t_args("task_form.load.teamFailed", [("key", key.to_string())]);
  let client = Services::of(ctx).team.client();
  let failed = load_error(ctx, "task-form-load-error", (&title, &ctx.t(body)), move || {
    Stale::EVERY.apply(&client);
  });
  screen(Screen::EditTask, failed)
}

/// Saves `args`, an edit of the team task `task` read at `read`, through `acting`: what the person changed, in order.
pub async fn store(
  acting: &Acting,
  team: &TeamSave,
  (task, read): (TaskId, TaskRevision),
  args: &SaveArgs,
) -> SaveOutcome {
  let made_on = &team.task;
  if args.points != person_points(made_on) || !args.base_branch.keeps(made_on.base()) {
    return stopped(Stop::Unkept, None, None);
  }
  let base = args.base.clone().unwrap_or_else(|| made_on.details().clone());
  let mut changes = Vec::new();
  if base != args.details {
    changes.push(TaskChange::Details(base, args.details.clone()));
  }
  let (read_organization, edited_organization) = &args.organization;
  if read_organization != edited_organization {
    changes.push(TaskChange::Organization(
      read_organization.clone(),
      edited_organization.clone(),
    ));
  }
  if args.module != made_on.module() {
    changes.push(TaskChange::Module(args.module));
  }
  let Edited { saved, stopped: stop } = acting.edit_task(&team.source, (task, read), changes).await;
  // The details go first, so a task the server took any change of holds the pipeline the person chose.
  let saved = saved
    .and_then(|shared| as_task(made_on, args.details.pipeline, &shared))
    .map(Arc::new);
  match stop {
    None => SaveOutcome::Stored(saved.unwrap_or_else(|| made_on.clone())),
    Some(acted) => {
      let current = match &acted {
        Acted::Changed(Some(Holds::Task(shared))) => changed(made_on, shared),
        _ => None,
      };
      stopped(Stop::Acted(acted), saved, current)
    }
  }
}

fn stopped(stop: Stop, saved: Option<Arc<Task>>, current: Option<Changed>) -> SaveOutcome {
  SaveOutcome::Team(Arc::new(TeamStop { stop, saved, current }))
}

fn changed(made_on: &Task, shared: &SharedTask) -> Option<Changed> {
  Some(Changed {
    task: Arc::new(as_task(made_on, made_on.details().pipeline, shared)?),
    by: shared.updated.by.display_name.clone(),
    at: Timestamp::from_unix_millis(shared.updated.at),
  })
}

/// `made_on` as the server holds it now (`shared`): its content, organization, module and revision, with the team
/// pipeline `pipeline`, which this computer reads with the task from the id the server names it by. Its limits stay:
/// no save of the form changes them (D6).
fn as_task(made_on: &Task, pipeline: Option<ResourceId>, shared: &SharedTask) -> Option<Task> {
  let content = &shared.content;
  let details = TaskDetails {
    title: Name::new(&content.title).ok()?,
    brief: content.brief.clone(),
    acceptance_criteria: content.criteria.clone(),
    pipeline,
    limits: made_on.details().limits,
  };
  let organization = content.organization().core().ok()?;
  let task = Task::from_stored(
    made_on.id(),
    made_on.key().clone(),
    details,
    made_on.status(),
    made_on.pipeline_version(),
    made_on.created_at(),
  );
  Some(
    task
      .with_organization(organization)
      .with_module(content.module)
      .with_revision(shared.revision)
      .with_base(made_on.base().clone())
      .with_estimate(made_on.estimate().cloned()),
  )
}
