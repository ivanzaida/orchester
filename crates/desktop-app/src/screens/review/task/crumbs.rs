//! The breadcrumb of a task's review (`Review - v2` 02 `gSMiI`): *← Review / KON-18 · run 4*, whose *Review* leads back
//! to Review's list. While the run is not known (the page is read, or the task has none to review) it reads *← Review /
//! KON-18*.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_core::{project::TaskKey, run::RunNumber};

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
pub(super) fn crumbs(ctx: &mut Ctx, project: &ShownProject, (key, run): (&TaskKey, Option<RunNumber>)) -> Element {
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
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(breadcrumb(ctx, "review-crumbs", CrumbSeparator::Slash, trail))
    .into()
}
