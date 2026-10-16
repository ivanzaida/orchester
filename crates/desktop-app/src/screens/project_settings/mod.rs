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

use crate::project::ShownProject;

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tab {
  General,
  Limits,
  Environment,
  CodeIndex,
  Sharing,
}
impl DevtoolsInspectable for Tab {}
impl Tab {
  /// The tabs before *Memory*, which is not built yet; *Sharing* comes after it ([`Self::SHARING`]).
  pub const ALL: [Self; 4] = [Self::General, Self::Limits, Self::Environment, Self::CodeIndex];
  pub const fn name(self) -> &'static str {
    match self {
      Self::General => "general",
      Self::Limits => "limits",
      Self::Environment => "environment",
      Self::CodeIndex => "codeIndex",
      Self::Sharing => "sharing",
    }
  }
  pub const fn key(self) -> &'static str {
    match self {
      Self::General => "project_settings.general",
      Self::Limits => "project_settings.limits",
      Self::Environment => "project_settings.environment",
      Self::CodeIndex => "project_settings.codeIndex",
      Self::Sharing => "project_settings.sharing",
    }
  }
  pub const fn description(self) -> &'static str {
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
  tab: Signal<Tab>,
  status: Signal<Status>,
}
impl Component for SettingsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      tab: ctx.signal(Tab::General),
      status: ctx.signal(Status::Saved),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let props = TabProps {
      project: project.clone(),
      status: self.status.clone(),
    };
    let tab = self.tab.get();
    let header = page::header(ctx, (&project, tab), &self.tab, &self.status);
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
pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    &format!("project-settings-{}", project.open.project.id()),
    Props {
      project: project.clone(),
    },
  )
}
