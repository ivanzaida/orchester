//! A page under Services whose task has not landed yet: a service's, a stack's or a suite's editor, their New pages,
//! Import draft and Flaky tests. It keeps the crumb back to Services and its title, and says it is not built yet.

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::super::{
  kit::{back, page_head},
  page,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, ProjectSettingsTab, Route, ServicesPage},
  theme::ExtraSpacing,
};

/// The page `shown` under Services of `project`, not built yet.
pub(in crate::screens::project_settings) fn not_built(
  ctx: &mut Ctx,
  project: &ShownProject,
  shown: ServicesPage,
) -> Element {
  let title = ctx.t(match shown {
    ServicesPage::NewService => "project_settings.services.newService",
    ServicesPage::NewStack => "project_settings.services.newStack",
    ServicesPage::NewSuite => "project_settings.services.newSuite",
    ServicesPage::Import => "project_settings.services.importDraft",
    ServicesPage::Flaky => "project_settings.services.flaky",
    ServicesPage::Service(_) => "project_settings.services.page.service",
    ServicesPage::Stack(_) => "project_settings.services.page.stack",
    ServicesPage::Suite(_) => "project_settings.services.page.suite",
  });
  let crumb = back(
    "settings-services-page-crumb",
    (&ctx.t("project_settings.services.tab"), Some(&title)),
    Navigation::of(ctx).to(Route::Project(
      project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Services),
    )),
  );
  let head = page_head(
    "settings-services-page",
    &title,
    Some(&ctx.t("project_settings.services.page.notBuilt")),
  );
  page::scroll(
    Column::new()
      .id(format!("settings-services-page-{}", shown.name()))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb)
      .child(head),
  )
}
