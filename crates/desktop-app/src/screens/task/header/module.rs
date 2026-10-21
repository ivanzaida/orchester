//! The task's module in its header (ORC-48, no frame yet): *Module* and a select of the project's modules and *No
//! module*, which moves the task as soon as it is chosen, through the project's task writer off the UI thread. A
//! project without modules shows nothing for a task in none; a move that is refused says why beside the select.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ModuleId, TaskId},
  module::Module,
  task::organization::ChangeActor,
};
use orchester_workspace::{
  background, messages::Message, modules::ModuleChoice, sharing::Servers, tasks::writer::TaskWriter,
};
use parking_lot::Mutex;

use super::super::TaskContext;
use crate::{
  project::{ProjectFeed, TasksState},
  services::Services,
  ui::{
    field::{SelectField, field_error},
    message::message_text,
  },
};

/// The select's width, as the Tasks list's module filter.
const SELECT_WIDTH: f32 = 180.0;

/// The module field of `task`'s header, when the project has modules or the task is in one.
pub fn field(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let modules = match task.project.feed.tasks.get() {
    TasksState::Ready(tasks) => tasks.modules,
    TasksState::Loading | TasksState::Failed(_) => return None,
  };
  let current = task.detail.task.module();
  if (modules.is_empty() && current.is_none()) || task.detail.not_shared() {
    return None;
  }
  Some(ctx.mount_keyed::<ModuleField>(
    &format!("task-module-{}", task.detail.task.id()),
    FieldProps {
      root: task.project.open.repo_root.clone(),
      feed: task.project.feed.clone(),
      task: task.detail.task.id(),
      current,
      modules,
    },
  ))
}

#[derive(Clone, PartialEq)]
pub struct FieldProps {
  root: PathBuf,
  feed: ProjectFeed,
  task: TaskId,
  current: Option<ModuleId>,
  modules: Arc<Vec<Module>>,
}

impl DevtoolsInspectable for FieldProps {}

#[derive(Clone)]
pub struct Move {
  root: PathBuf,
  task: TaskId,
  module: Option<ModuleId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved;

impl DevtoolsInspectable for Moved {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused(Message);

impl DevtoolsInspectable for Refused {}

pub struct ModuleField {
  chosen: Signal<ModuleChoice>,
  /// The module the task was last read in, which a choice is compared with.
  stored: Arc<Mutex<Option<ModuleId>>>,
  action: FutureAction<Move, Moved, Refused>,
}

impl Component for ModuleField {
  type Props = FieldProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<FieldProps>().clone();
    let servers = Services::of(ctx).servers;
    let action = ctx.future_action(move |args: Move| {
      let servers = servers.clone();
      async move {
        background::run(move || move_task(&args, &servers))
          .await
          .map_err(Refused)?
      }
    });
    let chosen = ctx.signal(ModuleChoice(props.current));
    let stored = Arc::new(Mutex::new(props.current));
    let (watched, run) = (Arc::clone(&stored), action.clone());
    let (root, task) = (props.root.clone(), props.task);
    ctx.watch(&chosen, move |choice: &ModuleChoice| {
      if *watched.lock() != choice.0 {
        run.run(Move {
          root: root.clone(),
          task,
          module: choice.0,
        });
      }
    });
    let feed = props.feed;
    ctx.watch(&action.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        feed.saved();
      }
    });
    Self { chosen, stored, action }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FieldProps>().clone();
    // The task as read again wins: what was stored meanwhile is shown.
    let mut stored = self.stored.lock();
    if *stored != props.current {
      *stored = props.current;
      self.chosen.set(ModuleChoice(props.current));
    }
    drop(stored);
    let mut options = vec![SelectOption::new(ModuleChoice(None), ctx.t("task.module.none"))];
    options.extend(
      props
        .modules
        .iter()
        .map(|module| SelectOption::new(ModuleChoice(Some(module.id())), module.name().as_str())),
    );
    let select = SelectField::new("task-module", ctx.t("task.module.label"), self.chosen.clone(), options)
      .width(Dimension::Px(SELECT_WIDTH))
      .render(ctx);
    let state = self.action.state().get();
    let refused = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|Refused(message)| field_error("task-module-error", &message_text(ctx, message)));
    Row::new()
      .id("task-module-field")
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Text::new(&ctx.t("task.module.label")).variant(TypographyStyle::FieldLabel))
      .child(select)
      .with_children(refused)
  }
}

fn move_task(args: &Move, servers: &Servers) -> Result<Moved, Refused> {
  let refused = |error: &orchester_workspace::tasks::writer::WriteError| {
    tracing::warn!(%error, "the task was not moved to the module");
    Refused(error.message())
  };
  let mut writer = TaskWriter::open(&args.root, servers).map_err(|error| refused(&error))?;
  writer
    .set_task_module(args.task, args.module, ChangeActor::Person)
    .map_err(|error| refused(&error))?;
  Ok(Moved)
}
