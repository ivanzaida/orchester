//! The confirmation that saving an edited task as Canceled stops its active run (`N1vwaM`).

use lurq::{
  app::ctx::{Ctx, FutureState},
  components::{Column, Modal, ModalTarget},
  core::Signal,
  node::Element,
};
use orchester_core::{id::TaskId, store::RunStore};
use orchester_store::ProjectDb;

use super::{NewTaskProps, SaveAction};
use crate::{
  background,
  messages::{self, Message},
  ui::{
    backdrop::backdrop,
    cancel_dialog::cancel_dialog,
    form::{Submit, form},
    message_text,
  },
};

#[derive(Clone, PartialEq)]
struct ActiveRun(Option<orchester_core::run::Run>);
impl lurq::app::component::DevtoolsInspectable for ActiveRun {}

pub(super) fn confirmation(
  ctx: &mut Ctx,
  open: Signal<bool>,
  props: &NewTaskProps,
  save: &SaveAction,
  submit: Submit,
) -> Element {
  if !open.get() {
    return Modal::new(Column::new()).open_when(false).into();
  }
  let Some(editing) = &props.editing else {
    return Modal::new(Column::new()).open_when(false).into();
  };
  let task = editing.task.id();
  let load = active_run(ctx, &props.project.repo_root, task);
  let pending = save.state().get().is_pending();
  // The run is shown once read; a failed read says why in place of the save's error.
  let run = load.data.and_then(|ActiveRun(run)| run);
  let error = save
    .state()
    .get()
    .error
    .filter(|error| !super::persistence::needs_cancel_confirmation(error))
    .or(load.error)
    .map(|error| message_text(ctx, &error).to_string());
  let close = open.clone();
  let dialog = crate::screens::project::task::single_cancel(
    ctx,
    (&editing.task, run.as_ref()),
    ("edit-task-cancel-confirmation", "edit-cancel-keep", "edit-cancel-stop"),
    pending,
    error.as_deref(),
  );
  let content = cancel_dialog(
    ctx,
    dialog,
    move || close.set(false),
    move || {
      if let Some(submit) = &submit {
        submit();
      }
    },
  );
  Modal::new(backdrop(ctx, form(None, content)))
    .target(ModalTarget::Root)
    .open(open)
    .dismiss_on_escape(!pending)
    .into()
}

fn active_run(ctx: &mut Ctx, root: &std::path::Path, task: TaskId) -> FutureState<ActiveRun, Message> {
  ctx
    .future((root.to_path_buf(), task), |(root, task)| async move {
      background::run(move || {
        let db = ProjectDb::open(&root)?;
        let run = db
          .active_task_run(task)?
          .map(|run| db.run(run).map_err(crate::tasks::detail::run_store_error))
          .transpose()?
          .flatten();
        Ok::<_, orchester_store::StoreError>(ActiveRun(run))
      })
      .await?
      .map_err(|error| messages::store_error(&error))
    })
    .state()
    .get()
}
