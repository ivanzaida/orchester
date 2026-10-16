//! Project settings (RExz4): four autosaving sections, with widening questions in place, and Sharing (`bWD9q`).
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
  /// The tab the route names.
  pub tab: Tab,
}
impl DevtoolsInspectable for Props {}

#[derive(Clone)]
pub(super) struct TabProps {
  pub project: ShownProject,
  pub status: Signal<Status>,
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
}
impl Component for SettingsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      status: ctx.signal(Status::Saved),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, tab } = ctx.props::<Props>().clone();
    let props = TabProps {
      project: project.clone(),
      status: self.status.clone(),
    };
    let header = page::header(ctx, (&project, tab), &self.status);
    let content = match tab {
      Tab::General => ctx.mount_keyed::<general::General>("project-settings-general", props),
      Tab::Limits => ctx.mount_keyed::<limits::Limits>("project-settings-limits", props),
      Tab::Environment => ctx.mount_keyed::<environment::Environment>("project-settings-environment", props),
      Tab::CodeIndex => ctx.mount_keyed::<index::CodeIndex>("project-settings-index", props),
      Tab::Sharing => ctx.mount_keyed::<sharing::Sharing>("project-settings-sharing", props),
    };
    page::screen(header, content)
  }
}
pub fn screen(ctx: &mut Ctx, project: &ShownProject, tab: Tab) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    &format!("project-settings-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      tab,
    },
  )
}
