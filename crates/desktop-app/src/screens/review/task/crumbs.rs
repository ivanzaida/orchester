//! The breadcrumb of a task's review (`Review - v2` 02 `gSMiI`): *← Review / KON-18 · run 4*, whose *Review* leads back
//! to Review's list. While the run is not known (the page is read, or the task has none to review) it reads *← Review /
//! KON-18*. At its right end the pager of *Waiting for you* ([`super::pager`]), when the task belongs to it.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{project::TaskKey, run::RunNumber};
use orchester_workspace::tasks::review_list::Pager;

use super::pager;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  theme::{ExtraSpacing, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
  },
};

/// The breadcrumb of the review of the task `key`, with the run reviewed once it is known.
pub(super) fn crumbs(
  ctx: &mut Ctx,
  project: &ShownProject,
  (key, run): (&TaskKey, Option<RunNumber>),
  waiting: Option<&Pager>,
) -> Element {
  let here = match run {
    Some(run) => ctx.t_args(
      "review_page.crumb",
      [("key", key.to_string()), ("run", run.get().to_string())],
    ),
    None => key.to_string().into(),
  };
  let trail = vec![
    Crumb::to(
      ctx.t("shell.screens.review"),
      Navigation::of(ctx).to(Route::Project(project.open.project.id(), ProjectPage::Review)),
    ),
    Crumb::here(here).mono(),
  ];
  Row::new()
    .id("review-crumbs-row")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(breadcrumb(ctx, "review-crumbs", CrumbSeparator::Slash, trail))
    .child(Row::new().flex(1.0))
    .with_children(waiting.map(|waiting| pager::pager(ctx, project, waiting)))
    .into()
}
