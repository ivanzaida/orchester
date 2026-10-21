//! New task (`New task · empty · dark` `s7jKkA`, `New task · filled · dark` `fJ4v9`) and Edit task (`Edit
//! task · draft · no changes · dark` `Qp1B0`, `Edit task · already ran` `v2c76`), one page in the layer
//! `New task` (`ZLTEI`): the brief, the pipeline with its execution plan, the limits, *Don't merge
//! automatically* (ORC-69, [`merging`]), and the action bar.
//! *Create* saves a draft and opens Tasks with the new row highlighted; *Create & run* saves, queues and
//! starts the run, and a run the runner did not start keeps the form with *Retry run* (`H8GYVZ`,
//! `t0fDw`). Edit task saves changes in place, discards them in place (`LVI8K`), and keeps them on this
//! computer when the page is left (`Fd1ai`). A newer version saved meanwhile is put under the edits, and
//! where both changed a field the person chooses (`Edit task · conflict · newer version` `PenUu`). The
//! rest of the Organization section is left out of the dogfood build (D4).

mod actions;
mod bar;
mod base;
mod brief;
mod conflict;
mod criteria;
mod fields;
mod form;
mod handlers;
mod head;
mod kept;
mod limits;
mod merging;
mod module;
mod plan;
mod state;
mod unbuilt;
mod values;

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, TaskId},
  project::TaskKey,
};
use orchester_workspace::tasks::data::ProjectTasks;

use self::{
  actions::RunBegan,
  form::{FormProps, TaskForm},
  state::Subject,
};
use super::placeholder::centered;
use crate::{
  navigation::{Navigation, link::Open},
  project::{ShownProject, TasksState},
  routes::{ProjectPage, Route, Screen, TaskList, TaskView},
  ui::{
    icon::Icon,
    message::message_text,
    state::{load_error, loading},
  },
};

/// The form's column (`Form` `V1U9lg`), which the action bar's content lines up with (`Inner` `uIqao`).
const FORM_WIDTH: f32 = 720.0;

pub fn new_task(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let tasks = match read_tasks(ctx, project, Screen::NewTask) {
    Ok(tasks) => tasks,
    Err(waiting) => return waiting,
  };
  let props = FormProps {
    project: project.clone(),
    subject: Subject::New,
    tasks: task_ids(&tasks),
    updated_at: None,
    separator: ctx.t("task_form.number.groupSeparator"),
    pipelines: Some(tasks.pipelines),
    missing_pipeline: None,
    modules: tasks.modules,
    shared: tasks.shared,
  };
  ctx.mount_keyed::<TaskForm>(&format!("task-form-new-{}", project.open.project.id()), props)
}

pub fn edit_task(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey) -> Element {
  let tasks = match read_tasks(ctx, project, Screen::EditTask) {
    Ok(tasks) => tasks,
    Err(waiting) => return waiting,
  };
  let Some(task) = tasks.tasks.iter().find(|task| task.key() == key) else {
    let title = ctx.t_args("task_form.load.notFound", [("key", key.to_string())]);
    let missing = centered(Icon::SearchX, &title, &ctx.t("task_form.load.notFoundBody")).id("task-form-not-found");
    return screen(Screen::EditTask, missing.into());
  };
  let task = Arc::new(task.clone());
  let task_id = task.id();
  let subject = if tasks.phases.contains_key(&task.id()) {
    Subject::Ran(task)
  } else {
    Subject::Draft(task)
  };
  let props = FormProps {
    project: project.clone(),
    updated_at: tasks.updated.get(&task_id).copied(),
    separator: ctx.t("task_form.number.groupSeparator"),
    subject,
    tasks: task_ids(&tasks),
    pipelines: Some(tasks.pipelines.clone()),
    missing_pipeline: tasks
      .missing_pipelines
      .get(&task_id)
      .map(|slug| Arc::from(slug.as_str())),
    modules: tasks.modules.clone(),
    shared: tasks.shared,
  };
  ctx.mount_keyed::<TaskForm>(&format!("task-form-{}-{key}", project.open.project.id()), props)
}

fn task_ids(tasks: &ProjectTasks) -> Arc<[TaskId]> {
  tasks.tasks.iter().map(|task| task.id()).collect()
}

/// The project's tasks and pipelines, which the form starts from; until they are read, `form`'s screen
/// says they are being read, or why they cannot be.
fn read_tasks(ctx: &mut Ctx, project: &ShownProject, form: Screen) -> Result<ProjectTasks, Element> {
  match project.feed.tasks.get() {
    TasksState::Ready(tasks) => Ok(tasks),
    TasksState::Loading => Err(screen(
      form,
      loading("task-form-loading", &ctx.t("task_form.load.reading")).into(),
    )),
    TasksState::Failed(error) => {
      let feed = project.feed.clone();
      let body = message_text(ctx, &error);
      let title = ctx.t("task_form.load.failed");
      let failed = load_error(ctx, "task-form-load-error", (&title, &body), move || feed.saved());
      Err(screen(form, failed))
    }
  }
}

/// `content` as the form's screen while there is no form to show.
fn screen(form: Screen, content: Element) -> Element {
  Column::new()
    .id(format!("screen-{}", form.name()))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(content)
    .into()
}

/// Where the form leads.
#[derive(Clone)]
pub struct Nav {
  navigation: Navigation,
  project: ProjectId,
}

impl Nav {
  fn new(ctx: &mut Ctx, project: ProjectId) -> Self {
    Self {
      navigation: Navigation::of(ctx),
      project,
    }
  }

  fn route(&self, page: ProjectPage) -> Route {
    Route::Project(self.project, page)
  }

  /// The Tasks list with `highlight`, the task Create just added, in the form's place so Back does not return to a
  /// form that was used.
  pub fn tasks(&self, highlight: TaskKey) {
    self
      .navigation
      .replace(self.route(ProjectPage::Tasks(TaskList::default().with_highlight(highlight))));
  }

  /// What opens the Tasks list where a press asks (`navigation::link`).
  pub fn to_tasks(&self) -> Open {
    self.navigation.to(self.route(ProjectPage::Tasks(TaskList::default())))
  }

  /// What opens the task `key`, on the tab a task opens on, where a press asks.
  pub fn to_task(&self, key: &TaskKey) -> Open {
    self
      .navigation
      .to(self.route(ProjectPage::Task(key.clone(), TaskView::default())))
  }

  /// The task whose run was asked to start, in the form's place: its Conversation, or its Summary, which says why,
  /// when the run failed before it started.
  pub fn started(&self, key: &TaskKey, began: RunBegan) {
    let view = match began {
      RunBegan::Yes => TaskView::Conversation,
      RunBegan::FailedBeforeStart => TaskView::Summary,
    };
    self
      .navigation
      .replace(self.route(ProjectPage::Task(key.clone(), view)));
  }

  /// What opens the Pipelines library where a press asks.
  pub fn to_pipelines(&self) -> Open {
    self.navigation.to(self.route(ProjectPage::Pipelines))
  }
}
