//! New task (`New task · empty · dark` `s7jKkA`, `New task · filled · dark` `fJ4v9`) and Edit task (`Edit
//! task · draft · no changes · dark` `Qp1B0`, `Edit task · already ran` `v2c76`), one page in the layer
//! `New task` (`ZLTEI`): the brief, the pipeline with its execution plan, the limits, and the action bar.
//! *Create* saves a draft and opens Tasks with the new row highlighted; *Create & run* saves, queues and
//! starts the run, and a run the runner did not start keeps the form with *Retry run* (`H8GYVZ`,
//! `t0fDw`). Edit task saves changes in place, discards them in place (`LVI8K`), and keeps them on this
//! computer when the page is left (`Fd1ai`). The Organization section and the conflict with a newer
//! version of a shared task are not built yet (D4).

mod actions;
mod bar;
mod brief;
mod criteria;
mod form;
mod handlers;
mod head;
mod kept;
mod limits;
mod plan;
mod sections;
mod state;
mod unbuilt;
mod values;

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ProjectId, project::TaskKey};
use orchester_workspace::tasks::data::ProjectTasks;

use self::{
  form::{FormProps, TaskForm},
  state::Subject,
};
use super::placeholder::centered;
use crate::{
  navigation::Navigation,
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
    pipelines: Some(tasks.pipelines),
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
  let subject = if tasks.phases.contains_key(&task.id()) {
    Subject::Ran(task)
  } else {
    Subject::Draft(task)
  };
  let props = FormProps {
    project: project.clone(),
    subject,
    pipelines: Some(tasks.pipelines.clone()),
  };
  ctx.mount_keyed::<TaskForm>(&format!("task-form-{}-{key}", project.open.project.id()), props)
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

  /// The Tasks list; with `highlight`, the task Create just added, in the form's place so Back does not
  /// return to a form that was used.
  pub fn tasks(&self, highlight: Option<TaskKey>) {
    match highlight {
      Some(key) => self
        .navigation
        .replace(self.route(ProjectPage::Tasks(TaskList::default().with_highlight(key)))),
      None => self.navigation.go(self.route(ProjectPage::Tasks(TaskList::default()))),
    }
  }

  pub fn task(&self, key: &TaskKey) {
    self
      .navigation
      .go(self.route(ProjectPage::Task(key.clone(), TaskView::Summary)));
  }

  /// The Conversation of the task whose run started, in the form's place.
  pub fn started(&self, key: &TaskKey) {
    self
      .navigation
      .replace(self.route(ProjectPage::Task(key.clone(), TaskView::Conversation)));
  }

  pub fn pipelines(&self) {
    self.navigation.go(self.route(ProjectPage::Pipelines));
  }
}
