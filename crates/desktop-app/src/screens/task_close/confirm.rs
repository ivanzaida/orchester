//! What closing or deleting asks in place, under the task page's header or over the Tasks list's rows: that a task
//! whose run has not ended has that run stopped first (as Run again asks, in a decision card), the delete
//! confirmation (`super::delete`), or why the last request did not happen.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::tasks::{cancel_run::Stop, close::Closing};

use super::{Asked, Closer, Request, Subject, delete};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

/// What waits in place for the person, or why their last request did not happen; nothing otherwise. Its parts' ids
/// start with `id`.
pub fn panel(ctx: &mut Ctx, closer: &Closer, id: &str) -> Option<Element> {
  match closer.asked.get() {
    Asked::Nothing => failure(ctx, closer, id),
    Asked::StopRun { task, closing, run } => Some(stop_run(ctx, closer, id, (&task, closing, run))),
    Asked::Delete(task) => Some(delete::confirmation(ctx, closer, &task, id)),
  }
}

/// Why the last close or reopen did not happen, naming its task (the Tasks list shows it over every row), until the
/// next request.
fn failure(ctx: &Ctx, closer: &Closer, id: &str) -> Option<Element> {
  let state = closer.action.state().get();
  let pending = state.is_pending();
  let failed = state.error.filter(|_| !pending)?;
  let reason = message_text(ctx, &failed.message);
  let text = ctx.t_args("task_close.failed", [("task", &*failed.key), ("reason", &*reason)]);
  Some(field_error(&format!("{id}-error"), &text).into())
}

/// The run of `task` has not ended: stop it and close the task, or keep it running.
fn stop_run(
  ctx: &Ctx,
  closer: &Closer,
  id: &str,
  (task, closing, run): (&Subject, Closing, orchester_core::id::RunId),
) -> Element {
  let state = closer.action.state().get();
  let pending = state.is_pending();
  let key = &*task.key;
  let (explanation, confirm) = match closing {
    Closing::Done => ("task_close.stop.done", "task_close.stop.confirmDone"),
    Closing::Canceled => ("task_close.stop.canceled", "task_close.stop.confirmCanceled"),
  };
  let error = state
    .error
    .filter(|failed| !pending && failed.task == task.id)
    .map(|failed| field_error(&format!("{id}-stop-error"), &message_text(ctx, &failed.message)));
  let keep = ActionButton::new(
    format!("{id}-stop-keep"),
    ButtonKind::Secondary,
    ctx.t("task_close.stop.keep"),
  )
  .in_form();
  let keep = if pending {
    keep.disabled()
  } else {
    let asked = closer.asked.clone();
    keep.on_click(move || asked.set(Asked::Nothing))
  };
  let stop = ActionButton::new(format!("{id}-stop-confirm"), ButtonKind::Primary, ctx.t(confirm))
    .icon(Icon::CircleStop)
    .in_form();
  let stop = if pending {
    stop.loading()
  } else {
    let (closer, task) = (closer.clone(), task.clone());
    stop.on_click(move || closer.run(Request::Close(task.clone(), closing, Stop::Confirmed(Some(run)))))
  };
  card(&format!("{id}-stop"), CardSurface::Decision)
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&ctx.t_args("task_close.stop.title", [("task", key)]))
            .id(format!("{id}-stop-title"))
            .variant(TaskText::SectionTitle.role()),
        )
        .child(
          Text::new(&ctx.t_args(explanation, [("task", key)]))
            .id(format!("{id}-stop-text"))
            .variant(ExtraTypography::Prose.role()),
        ),
    )
    .with_children(error)
    .child(actions(format!("{id}-stop-actions"), [keep.into(), stop.into()]))
    .into()
}

/// A confirmation's buttons at its end.
pub(super) fn actions(id: String, buttons: [Element; 2]) -> Row {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Spacer::new().flex(1.0))
    .with_children(buttons)
}

/// `one` or `other` with `count`.
pub(super) fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> Arc<str> {
  let key = if count == 1 { one } else { other };
  ctx.t_args(key, [("count", count.to_string())])
}
