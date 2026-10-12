//! Keep unsaved text when changing instructions or leaving the page.
use super::state::Selection;
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  theme::{ExtraSpacing, ExtraTypography},
  ui::button::{ActionButton, ButtonKind},
};
use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
  node::Element,
};

pub fn question(ctx: &mut Ctx, selection: &Selection, guard: &LeaveGuard) -> Option<Element> {
  if selection.pending.get().is_none() && !guard.requested() {
    return None;
  }
  let (keep, leave) = (selection.clone(), selection.clone());
  let (keep_guard, leave_guard) = (guard.clone(), guard.clone());
  let nav = Navigation::of(ctx);
  let confirm =
    ActionButton::new("instruction-leave", ButtonKind::Secondary, ctx.t("instructions.leave")).on_click(move || {
      if leave.busy.get_untracked() {
        return;
      }
      if let Some(view) = leave.pending.get_untracked() {
        leave.dirty.set(false);
        leave.pending.set(None);
        leave.view.set(view);
      } else {
        leave_guard.leave(&nav);
      }
    });
  Some(
    Column::new()
      .id("instruction-leave-question")
      .spacing(ExtraSpacing::Snug.role())
      .padding(ExtraSpacing::Tabs.role())
      .child(Text::new(&ctx.t("instructions.leaveQuestion")).variant(ExtraTypography::Strong.role()))
      .child(Text::new(&ctx.t("instructions.leaveDetail")).variant(ExtraTypography::Note.role()))
      .child(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .child(
            ActionButton::new("instruction-keep", ButtonKind::Primary, ctx.t("instructions.keep")).on_click(
              move || {
                keep.pending.set(None);
                keep_guard.keep_editing();
              },
            ),
          )
          .child(if selection.busy.get() {
            confirm.disabled()
          } else {
            confirm
          }),
      )
      .into(),
  )
}
