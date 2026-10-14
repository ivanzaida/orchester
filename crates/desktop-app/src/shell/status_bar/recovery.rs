//! The runs the previous session left interrupted, pinned to the activity slot (`Activity · pinned` `uZTnL`
//! of `JAIYi`) while the person has not decided whether they continue: *2 runs interrupted when Orchester
//! closed* in `warning` with its pause, and *Review*, which opens their list on the project's Overview. It
//! stays wherever the Overview is scrolled and on the project's other pages.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  layout::layout_kind::ShrinkLimit,
  node::Element,
};

use super::{
  facts::{BarAction, Dispatch, ProjectFacts},
  item::{action, cut, give_way, glyph, item},
};
use crate::{
  routes::{OverviewTab, ProjectPage, Route},
  ui::icon::Icon,
};

/// The pinned item and *Review*; nothing when no run waits for the decision.
pub fn pinned(ctx: &mut Ctx, project: &ProjectFacts, dispatch: &Dispatch) -> Vec<Element> {
  let count = project.undecided_runs;
  let key = match count {
    0 => return Vec::new(),
    1 => "shell.status.recovery.one",
    _ => "shell.status.recovery.other",
  };
  let text = ctx.t_args(key, [("count", count.to_string())]);
  let row = item("status-recovery")
    .flex_shrink(1.0)
    .shrink_order(give_way::ACTIVITY)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::CirclePause, PaletteColor::Warning))
    .child(cut(ctx, "status-recovery-text", &text, PaletteColor::Warning));
  let (dispatch, id) = (dispatch.clone(), project.id);
  let review = action(
    "status-recovery-review",
    &ctx.t("shell.status.recovery.review"),
    move || {
      dispatch(BarAction::ReviewInterrupted(id));
      dispatch(BarAction::Go(Route::Project(
        id,
        ProjectPage::Overview(OverviewTab::default()),
      )));
    },
  );
  vec![row.into(), review]
}
