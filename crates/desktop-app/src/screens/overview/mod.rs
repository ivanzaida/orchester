//! The project's Overview (`Overview · full page · dark` `q9mf0k`), its task list on `tab`. Not built yet,
//! except the notice of the runs the previous session left interrupted (`JSRBO`), at its top.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::placeholder::placeholder;
use crate::{
  lifecycle::recovery::RecoveryNotice,
  project::ShownProject,
  routes::{OverviewTab, Screen},
  theme::LifecycleSpacing,
};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, _tab: OverviewTab) -> Element {
  let notice = ctx.mount_keyed::<RecoveryNotice>("recovery-notice", project.clone());
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(SpacingSize::Section)
            .left(LifecycleSpacing::PanelSide.role())
            .right(LifecycleSpacing::PanelSide.role()),
        )
        .child(notice),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .child(placeholder(ctx, Screen::Overview)),
    )
    .into()
}
