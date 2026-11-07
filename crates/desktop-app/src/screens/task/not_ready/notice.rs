//! The refusal of a start for not being ready here, on a tab other than the Summary: a notice over the tabs, as the
//! frame's other notices are, with a link that opens the Summary, where the card lists each part and its fix.

use lurq::{app::ctx::Ctx, node::Element};

use super::{super::TaskContext, refused};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// The notice on `view`, if the person's last start was refused for not being ready here; none on the Summary, whose
/// card says it.
pub fn notice(ctx: &mut Ctx, task: &TaskContext, view: &TaskView) -> Option<Element> {
  if matches!(view, TaskView::Summary) {
    return None;
  }
  let refusal = refused(ctx, task)?;
  let team = task.team.is_some();
  let (title, body) = match (team, refusal.after_server_started()) {
    (_, true) => ("summary.notReady.titleAfterStart", "summary.notReady.notice.bodyTeam"),
    (true, false) => ("summary.notReady.titleTeam", "summary.notReady.notice.bodyTeam"),
    (false, false) => ("summary.notReady.title", "summary.notReady.notice.body"),
  };
  let body = ctx.t_args(body, [("pipeline", refusal.pipeline())]);
  let route = Route::Project(
    task.project.open.project.id(),
    task.page(TaskView::of(TaskTab::Summary)),
  );
  let open = ActionButton::new(
    "task-not-ready-notice-open",
    ButtonKind::Ghost,
    ctx.t("summary.notReady.notice.open"),
  )
  .icon(Icon::ArrowRight)
  .small()
  .tight()
  .opens(Navigation::of(ctx).to(route));
  Some(
    Notice::new("task-not-ready-notice", NoticeTone::Warning, ctx.t(title))
      .icon(Icon::CircleAlert)
      .body(body)
      .action(open)
      .into(),
  )
}
