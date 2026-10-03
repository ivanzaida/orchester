//! The confirmation that saving an edited task as Canceled stops its active run (`N1vwaM`).

use lurq::{
  app::ctx::{Ctx, FutureState},
  components::{Column, Modal, ModalTarget},
  core::Signal,
  node::Element,
};
use orchester_core::{id::TaskId, store::RunStore};
use orchester_store::ProjectDb;

use super::{NewTaskProps, SaveAction, SaveRequest};
use crate::{
  background,
  messages::{self, Message},
  ui::{backdrop::backdrop, cancel_dialog::cancel_dialog, form::form, message_text},
};

#[derive(Clone, PartialEq)]
struct ActiveRun(Option<orchester_core::run::Run>);
impl lurq::app::component::DevtoolsInspectable for ActiveRun {}

pub(super) fn confirmation(
  ctx: &mut Ctx,
  open: Signal<bool>,
  props: &NewTaskProps,
  save: &SaveAction,
  submit: Option<SaveRequest>,
) -> Element {
  if !open.get() {
    return Modal::new(Column::new()).open_when(false).into();
  }
  let Some(editing) = &props.editing else {
    return Modal::new(Column::new()).open_when(false).into();
  };
  let task = editing.task.id();
  // Read again after each refusal: a run that started since is the one to name.
  let refusal = save.state().get().error;
  let load = active_run(ctx, &props.project.repo_root, task, refusal);
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
  let run_id = run.as_ref().map(orchester_core::run::Run::id);
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
    // Stop exactly the run the dialog shows; without one there is none to stop.
    move || {
      if let Some(submit) = &submit {
        submit(run_id);
      }
    },
  );
  Modal::new(backdrop(ctx, form(None, content)))
    .target(ModalTarget::Root)
    .open(open)
    .dismiss_on_escape(!pending)
    .into()
}

fn active_run(
  ctx: &mut Ctx,
  root: &std::path::Path,
  task: TaskId,
  refusal: Option<Message>,
) -> FutureState<ActiveRun, Message> {
  ctx
    .future((root.to_path_buf(), task, refusal), |(root, task, _)| async move {
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
