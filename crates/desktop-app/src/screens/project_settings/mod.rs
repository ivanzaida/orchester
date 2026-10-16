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

use crate::project::ShownProject;

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
  /// The tab it opens on, and whether Sharing opens *Make local*'s question.
  pub start: Tab,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
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
  link_page: Signal<sharing::LinkPage>,
}
impl Component for SettingsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let start = ctx.props::<Props>().start;
    Self {
      tab: ctx.signal(start),
      status: ctx.signal(Status::Saved),
      link_page: ctx.signal(sharing::LinkPage::default()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let tab = self.tab.get();
    if let (Tab::Sharing, Some(upload)) = (tab, self.link_page.get().0) {
      return sharing::link_page(ctx, &project, upload, &self.link_page);
    }
    let props = TabProps {
      project: project.clone(),
      status: self.status.clone(),
      link_page: self.link_page.clone(),
      make_local: ctx.props::<Props>().make_local,
    };
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
      start: Tab::General,
      make_local: false,
    },
  )
}

/// Project settings open on Sharing, with *Make local*'s question open when `make_local` asks for it.
pub fn sharing_screen(ctx: &mut Ctx, project: &ShownProject, make_local: bool) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    &format!("project-settings-{}-sharing-{make_local}", project.open.project.id()),
    Props {
      project: project.clone(),
      start: Tab::Sharing,
      make_local,
    },
  )
}
