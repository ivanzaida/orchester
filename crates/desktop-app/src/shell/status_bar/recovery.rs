//! The runs the previous session left interrupted, pinned to the activity slot (`09 · Runs interrupted when
//! Orchester closed` `VIqZD` in `Status bar · redesign`) while the person has not decided whether they continue:
//! on a raised fill, *2 runs interrupted when Orchester closed* in `warning` with its hand, and *Review*, which
//! opens their list on the project's Overview. It stays wherever the Overview is scrolled and on the project's
//! other pages.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  layout::layout_kind::ShrinkLimit,
  node::{Element, padding::Padding},
};

use super::{
  facts::{BarAction, Dispatch, ProjectFacts},
  item::{button, cut, give_way, glyph, item},
};
use crate::{
  navigation::press::OpenIn,
  routes::{OverviewTab, ProjectPage, Route},
  ui::{icon::Icon, pressable::Press},
};

/// The pinned item with *Review* inside it; nothing when no run waits for the decision.
pub fn pinned(ctx: &mut Ctx, project: &ProjectFacts, dispatch: &Dispatch) -> Vec<Element> {
  let count = project.undecided_runs;
  let key = match count {
    0 => return Vec::new(),
    1 => "shell.status.recovery.one",
    _ => "shell.status.recovery.other",
  };
  let text = ctx.t_args(key, [("count", count.to_string())]);
  let (dispatch, id) = (dispatch.clone(), project.id);
  let review = button(
    "status-recovery-review",
    &ctx.t("shell.status.recovery.review"),
    // Review marks the project's interrupted runs as under review as well as opening them: an action, not a link.
    Press::run(move || {
      dispatch(BarAction::ReviewInterrupted(id));
      dispatch(BarAction::Go(
        Route::Project(id, ProjectPage::Overview(OverviewTab::default())),
        OpenIn::ActiveTab,
      ));
    }),
  );
  let row = item("status-recovery")
    .padding(Padding::horizontal(SpacingSize::Sm).right(SpacingSize::Xs))
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .flex_shrink(1.0)
    .shrink_order(give_way::ACTIVITY)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::Hand, PaletteColor::Warning))
    .child(cut(ctx, "status-recovery-text", &text, PaletteColor::Warning))
    .child(review);
  vec![row.into()]
}
