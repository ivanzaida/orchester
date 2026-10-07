//! The project's tasks could not be read again after a change, by the person or by automation: every
//! page keeps showing them as last read (no page is replaced and no form loses its edits), so the
//! shell says, above whichever page is shown, that they are the ones last read, why, and offers
//! *Retry*.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Outlet},
  node::{Element, dimension::Dimension},
};

use super::ProjectNav;
use crate::{
  messages::Message,
  ui::{button::ActionButton, icon::Icon, message_text, notice::Notice},
};

/// The shown page under the notice, when there is one; the page takes the height the notice leaves,
/// so its bottom stays above the status bar.
pub(super) fn page(ctx: &mut Ctx, nav: &ProjectNav, error: Option<&Message>) -> Element {
  Column::new()
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .with_children(notice(ctx, nav, error))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(Outlet::mount(ctx)),
    )
    .into()
}

/// The notice above the page while the tasks shown are the ones last read; `None` while they are
/// current.
fn notice(ctx: &Ctx, nav: &ProjectNav, error: Option<&Message>) -> Option<Element> {
  let error = error?;
  let nav = nav.clone();
  let retry = ActionButton::new(ctx.t("common.retry"))
    .id("tasks-read-again")
    .on_click(move || nav.tasks_changed())
    .render(ctx);
  let notice = Notice::new(
    Icon::AlertCircle,
    ctx.t("tasks.readAgainFailed"),
    message_text(ctx, error),
  )
  .id("tasks-read-again-failed")
  .action(retry);
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_top(SpacingSize::Md)
      .child(notice)
      .into(),
  )
}
