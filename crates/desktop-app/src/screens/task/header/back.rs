//! The way back (`Back row` `xGNCX` in `j8DakG`): a 13 `arrow-left` and `Tasks / KON-12` in 12 `text_muted`, the key
//! in Geist Mono `text_secondary`, 6 apart. The arrow and *Tasks* open the Tasks list; a task kept *Not shared* is
//! found behind the list's Not shared filter (`R4elt`), which its trail names.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::Element,
};

use super::HeaderProps;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList},
  theme::{ExtraSpacing, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    pressable::opening,
  },
};

pub fn row(ctx: &mut Ctx, props: &HeaderProps) -> Element {
  let navigation = Navigation::of(ctx);
  let project = props.task.project.open.project.id();
  let tasks = Route::Project(project, ProjectPage::Tasks(TaskList::default()));
  let mut crumbs = vec![Crumb::to(ctx.t("task.crumbs.tasks"), navigation.to(tasks.clone()))];
  if props.task.detail.not_shared() {
    let kept = TaskList::default().with_not_shared(true);
    crumbs.push(Crumb::to(
      ctx.t("task.crumbs.notShared"),
      navigation.to(Route::Project(project, ProjectPage::Tasks(kept))),
    ));
  }
  crumbs.push(Crumb::here(props.task.detail.task.key().to_string()).mono());
  let arrow = opening(navigation.to(tasks))
    .id("task-back")
    .describe("a11y_name", ctx.t("task.crumbs.back"))
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted));
  Row::new()
    .id("task-back-row")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(arrow)
    .child(breadcrumb(ctx, "task-crumbs", CrumbSeparator::Slash, crumbs))
    .into()
}
