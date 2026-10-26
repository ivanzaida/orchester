//! Keep unsaved text when changing instructions or leaving the page: *Leave with unsaved changes?*, the one wording
//! instructions and skills share (`06c`), asked in place above the page.
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::state::Selection;
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  screens::hub::question::{Question, Weight},
  theme::HubSpacing,
  ui::icon::Icon,
};

pub fn question(ctx: &mut Ctx, selection: &Selection, guard: &LeaveGuard) -> Option<Element> {
  if selection.pending.get().is_none() && !guard.requested() {
    return None;
  }
  let (keep, leave) = (selection.clone(), selection.clone());
  let (keep_guard, leave_guard) = (guard.clone(), guard.clone());
  let nav = Navigation::of(ctx);
  let question = Question::new(
    "instruction-leave-question",
    Weight::Neutral,
    (ctx.t("hub.leave.title"), ctx.t("hub.leave.sentence")),
    ((Icon::Pencil, ctx.t("hub.leave.keep")), ctx.t("hub.leave.leave")),
  )
  // Keep editing is the question's action; Leave editor is the way out.
  .on_confirm(false, move || {
    keep.pending.set(None);
    keep_guard.keep_editing();
  })
  .on_keep(move || {
    if leave.busy.get_untracked() {
      return;
    }
    if let Some(view) = leave.pending.get_untracked() {
      leave.dirty.set(false);
      leave.pending.set(None);
      leave.show(view);
    } else {
      leave_guard.leave(&nav);
    }
  });
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(HubSpacing::Side.role()).right(HubSpacing::Side.role()).top(HubSpacing::BodyTop.role()))
      .child(question)
      .into(),
  )
}
