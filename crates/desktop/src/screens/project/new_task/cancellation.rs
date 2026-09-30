use lurq::components::{Modal, ModalTarget};
use orchester_core::store::RunStore;
use orchester_store::ProjectDb;

use super::*;
use crate::{
  background, messages,
  ui::{backdrop::backdrop, button::ActionButton, field::field_error, message_text},
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
  let mut content = Column::new()
    .id("edit-task-cancel-confirmation")
    .width(Dimension::Px(CONFIRM_WIDTH))
    .padding(SpacingSize::Section)
    .spacing(SpacingSize::Lg)
    .background(ExtraColor::Surface.role())
    .rounded(RadiusSize::Lg)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t("tasks.cancel.title")).variant(ExtraTypography::DialogTitle.role()));
  let loaded = load;
  if let Some(ActiveRun(Some(run))) = loaded.data {
    content = content.child(
      Text::new(&ctx.t_args(
        "tasks.cancel.explanation",
        [
          ("task", editing.task.key().to_string().as_str()),
          ("run", run.number().to_string().as_str()),
          ("branch", run.branch().as_str()),
        ],
      ))
      .variant(TypographyStyle::Description),
    );
  }
  if let Some(error) = loaded.error {
    content = content.child(field_error(&message_text(ctx, &error)));
  }
  if let Some(error) = save
    .state()
    .get()
    .error
    .filter(|error| error.key != "tasks.cancel.confirmRequired")
  {
    content = content.child(field_error(&message_text(ctx, &error)));
  }
  let close = open.clone();
  content = content.child(
    Row::new()
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(ctx.t("tasks.cancel.keep"))
          .id("edit-cancel-keep")
          .in_form()
          .on_click(move || close.set(false))
          .enabled(!pending)
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t(if pending {
          "tasks.cancel.stopping"
        } else {
          "tasks.cancel.confirm"
        }))
        .id("edit-cancel-stop")
        .in_form()
        .on_click(move || {
          if let Some(submit) = &submit {
            submit();
          }
        })
        .enabled(!pending)
        .render(ctx),
      ),
  );
  Modal::new(backdrop(ctx, form(None, content)))
    .target(ModalTarget::Root)
    .open(open)
    .dismiss_on_escape(!pending)
    .into()
}

// N1vwaM: cancel confirmation in Edit task.
const CONFIRM_WIDTH: f32 = 620.0;

fn active_run(ctx: &mut Ctx, root: &std::path::Path, task: TaskId) -> lurq::app::ctx::FutureState<ActiveRun, Message> {
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
