//! Project settings (RExz4): four autosaving sections, with widening questions in place, and Sharing (`bWD9q`), whose
//! first link opens its own page over the tabs (`KSnOW`).
mod environment;
mod general;
mod index;
mod limits;
mod page;
mod sharing;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};

use crate::{project::ShownProject, routes::ProjectSettingsTab as Tab};

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
  /// The tab the route names, and whether Sharing opens *Make local*'s question.
  pub tab: Tab,
  pub make_local: bool,
}
impl DevtoolsInspectable for Props {}

#[derive(Clone)]
pub(super) struct TabProps {
  pub project: ShownProject,
  pub status: Signal<Status>,
  /// The first link's page, which Sharing opens over the tabs.
  pub link_page: Signal<sharing::LinkPage>,
  /// Sharing opens with *Make local*'s question.
  pub make_local: bool,
}
impl PartialEq for TabProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
  }
}
impl DevtoolsInspectable for TabProps {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Status {
  Saved,
  Unsaved,
  Invalid,
  Saving,
  Failed,
}
impl DevtoolsInspectable for Status {}
/// What the screen says about a tab; the tabs themselves are the route's ([`Tab`]).
impl Tab {
  /// The tabs before *Memory*, which is not built yet; *Sharing* comes after it.
  pub(super) const BEFORE_MEMORY: [Self; 4] = [Self::General, Self::Limits, Self::Environment, Self::CodeIndex];
  /// The tab's part of its element id (`settings-tab-codeIndex`).
  pub(super) const fn element(self) -> &'static str {
    match self {
      Self::General => "general",
      Self::Limits => "limits",
      Self::Environment => "environment",
      Self::CodeIndex => "codeIndex",
      Self::Sharing => "sharing",
    }
  }
  pub(super) const fn key(self) -> &'static str {
    match self {
      Self::General => "project_settings.general",
      Self::Limits => "project_settings.limits",
      Self::Environment => "project_settings.environment",
      Self::CodeIndex => "project_settings.codeIndex",
      Self::Sharing => "project_settings.sharing",
    }
  }
  pub(super) const fn description(self) -> &'static str {
    match self {
      Self::General => "project_settings.generalDescription",
      Self::Limits => "project_settings.limitsDescription",
      Self::Environment => "project_settings.environmentDescription",
      Self::CodeIndex => "project_settings.codeIndexDescription",
      Self::Sharing => "project_settings.sharingDescription",
    }
  }
}
struct SettingsScreen {
  status: Signal<Status>,
  link_page: Signal<sharing::LinkPage>,
}
impl Component for SettingsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      status: ctx.signal(Status::Saved),
      link_page: ctx.signal(sharing::LinkPage::default()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      project,
      tab,
      make_local,
    } = ctx.props::<Props>().clone();
    if let (Tab::Sharing, Some(upload)) = (tab, self.link_page.get().0) {
      return sharing::link_page(ctx, &project, upload, &self.link_page);
    }
    let props = TabProps {
      project: project.clone(),
      status: self.status.clone(),
      link_page: self.link_page.clone(),
      make_local,
    };
    let content = match tab {
      Tab::General => ctx.mount_keyed::<general::General>("project-settings-general", props),
      Tab::Limits => ctx.mount_keyed::<limits::Limits>("project-settings-limits", props),
      Tab::Environment => ctx.mount_keyed::<environment::Environment>("project-settings-environment", props),
      Tab::CodeIndex => ctx.mount_keyed::<index::CodeIndex>("project-settings-index", props),
      Tab::Sharing => ctx.mount_keyed::<sharing::Sharing>("project-settings-sharing", props),
    };
    page::settings(ctx, (&project, tab), &self.status, content)
  }
}
pub fn screen(ctx: &mut Ctx, project: &ShownProject, tab: Tab) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    &format!("project-settings-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      tab,
      make_local: false,
    },
  )
}

/// Joining the project `target` names (`AiCwO`), over Project settings › Sharing.
pub fn join_screen(ctx: &mut Ctx, project: &ShownProject, target: &crate::routes::JoinTarget) -> Element {
  sharing::join_screen(ctx, project, target)
}

/// Project settings on Sharing with *Make local*'s question open: a mount of its own, so the question opens
/// each time the page is reached.
pub fn make_local_screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    &format!("project-settings-{}-make-local", project.open.project.id()),
    Props {
      project: project.clone(),
      tab: Tab::Sharing,
      make_local: true,
    },
  )
}
