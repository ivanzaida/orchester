//! Project settings (`Project settings · redesign` `XGtkg`): its title with the save status over six tabs, General,
//! Runs, Context, Connections, Secrets and Sharing, each a 720 column of groups of rows ([`kit`]). General, Runs and
//! Context save as a person leaves a field or flips a switch, widening changes asked in place; Connections and Secrets
//! save per action and say nothing of a save state. Add, open and edit flows are pages inside their tab with a crumb
//! back to it: a connection's, Add provider and Add MCP server under Connections, a secret's under Secrets, *Approved
//! for every run* under Runs, and the first link (`Share`) under Sharing.
mod context;
mod general;
mod index;
pub(crate) mod kit;
mod page;
mod rules;
mod runs;
mod sharing;
mod sources;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ConnectionId;

use crate::{
  project::ShownProject,
  routes::{ConnectionsView, NewConnection, ProjectPage, ProjectSettingsTab as Tab, SecretsView},
};

/// What the page shows under its tabs: one of its own tabs, a page under one, or Connections or Secrets, whose routes
/// are their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
  Tab(Tab),
  /// *Approved for every run*, under Runs.
  Rules,
  Connections(ConnectionsView),
  NewConnection(NewConnection),
  EditConnection(ConnectionId),
  Secrets(SecretsView),
}

impl Shown {
  const fn section(self) -> Section {
    match self {
      Self::Tab(Tab::General) => Section::General,
      Self::Tab(Tab::Runs) | Self::Rules => Section::Runs,
      Self::Tab(Tab::Context) => Section::Context,
      Self::Tab(Tab::Sharing) => Section::Sharing,
      Self::Connections(_) | Self::NewConnection(_) | Self::EditConnection(_) => Section::Connections,
      Self::Secrets(_) => Section::Secrets,
    }
  }

  /// The route's page of what shows.
  const fn page(self) -> ProjectPage {
    match self {
      Self::Tab(tab) => ProjectPage::Settings(tab),
      Self::Rules => ProjectPage::ProjectRules,
      Self::Connections(view) => ProjectPage::Connections(view),
      Self::NewConnection(kind) => ProjectPage::NewConnection(kind),
      Self::EditConnection(id) => ProjectPage::EditConnection(id),
      Self::Secrets(view) => ProjectPage::Secrets(view),
    }
  }
}

/// A tab of the page (`Tabs` `A5Sf0Q`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
  General,
  Runs,
  Context,
  Connections,
  Secrets,
  Sharing,
}

impl Section {
  const ALL: [Self; 6] = [
    Self::General,
    Self::Runs,
    Self::Context,
    Self::Connections,
    Self::Secrets,
    Self::Sharing,
  ];

  /// The tab's part of its element id (`settings-tab-runs`).
  const fn element(self) -> &'static str {
    match self {
      Self::General => "general",
      Self::Runs => "runs",
      Self::Context => "context",
      Self::Connections => "connections",
      Self::Secrets => "secrets",
      Self::Sharing => "sharing",
    }
  }

  const fn key(self) -> &'static str {
    match self {
      Self::General => "project_settings.general",
      Self::Runs => "project_settings.runs",
      Self::Context => "project_settings.context",
      Self::Connections => "project_settings.connections",
      Self::Secrets => "project_settings.secrets",
      Self::Sharing => "project_settings.sharing",
    }
  }

  /// What the tab shows when it opens.
  fn root(self) -> Shown {
    match self {
      Self::General => Shown::Tab(Tab::General),
      Self::Runs => Shown::Tab(Tab::Runs),
      Self::Context => Shown::Tab(Tab::Context),
      Self::Sharing => Shown::Tab(Tab::Sharing),
      Self::Connections => Shown::Connections(ConnectionsView::default()),
      Self::Secrets => Shown::Secrets(SecretsView::default()),
    }
  }

  /// Whether the tab saves as a person edits it, and says so at the title's right; Connections and Secrets save per
  /// action.
  const fn autosaves(self) -> bool {
    !matches!(self, Self::Connections | Self::Secrets)
  }
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  shown: Shown,
  /// Sharing opens with *Make local*'s question.
  make_local: bool,
}
impl DevtoolsInspectable for Props {}

#[derive(Clone)]
pub(super) struct TabProps {
  pub project: ShownProject,
  pub status: Signal<Status>,
  /// The first link's page, which Sharing opens under its tab.
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

impl Status {
  /// How much the status needs the person: a failure first, then a field to fix, a change not saved, a save running.
  const fn weight(&self) -> u8 {
    match self {
      Self::Saved => 0,
      Self::Saving => 1,
      Self::Unsaved => 2,
      Self::Invalid => 3,
      Self::Failed => 4,
    }
  }
}

/// The save statuses of a tab's sections, each saying its own, so one section's *Saved* never hides another's
/// unfinished edit; the page says the one that most needs the person.
#[derive(Clone)]
pub(super) struct SectionStatuses(Vec<Signal<Status>>);

impl SectionStatuses {
  /// `count` sections whose statuses say the page's `page`.
  pub(super) fn new(ctx: &mut Ctx, count: usize, page: &Signal<Status>) -> Self {
    let sections: Vec<Signal<Status>> = (0..count).map(|_| ctx.signal(Status::Saved)).collect();
    for section in &sections {
      let (all, page) = (sections.clone(), page.clone());
      ctx.watch(section, move |_| {
        let worst = all
          .iter()
          .map(|status| status.get_untracked())
          .max_by_key(Status::weight)
          .unwrap_or(Status::Saved);
        page.set(worst);
      });
    }
    Self(sections)
  }

  /// `props` for section `index`, which says its status in its own signal.
  pub(super) fn props(&self, props: &TabProps, index: usize) -> TabProps {
    TabProps {
      status: self.0[index].clone(),
      ..props.clone()
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
      shown,
      make_local,
    } = ctx.props::<Props>().clone();
    let props = TabProps {
      project: project.clone(),
      status: self.status.clone(),
      link_page: self.link_page.clone(),
      make_local,
    };
    let content = match (shown, self.link_page.get().0) {
      (Shown::Tab(Tab::Sharing), Some(upload)) => sharing::link_page(ctx, &project, upload, &self.link_page),
      (Shown::Tab(Tab::General), _) => ctx.mount_keyed::<general::General>("project-settings-general", props),
      (Shown::Tab(Tab::Runs), _) => ctx.mount_keyed::<runs::Runs>("project-settings-runs", props),
      (Shown::Tab(Tab::Context), _) => ctx.mount_keyed::<context::Context>("project-settings-context", props),
      (Shown::Tab(Tab::Sharing), _) => ctx.mount_keyed::<sharing::Sharing>("project-settings-sharing", props),
      (Shown::Rules, _) => ctx.mount_keyed::<rules::Rules>("project-settings-rules", props),
      (Shown::Connections(view), _) => super::connections::screen(ctx, &project, view),
      (Shown::NewConnection(kind), _) => super::connections::new_connection(ctx, &project, kind),
      (Shown::EditConnection(id), _) => super::connections::edit_connection(ctx, &project, id),
      (Shown::Secrets(view), _) => super::secrets::screen(ctx, &project, view),
    };
    page::settings(ctx, (&project, shown), &self.status, content)
  }
}

/// Project settings showing `shown`.
pub fn screen(ctx: &mut Ctx, project: &ShownProject, shown: Shown) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    &format!("project-settings-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      shown,
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
      shown: Shown::Tab(Tab::Sharing),
      make_local: true,
    },
  )
}
