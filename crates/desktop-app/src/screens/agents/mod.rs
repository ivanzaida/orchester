//! Project agent profiles, layer Agents Gg0hY. The list, a new profile and a profile's editor tab are the
//! route's ([`Shown`]), so Back and Forward step through them.
mod actions;
mod active;
mod editor;
mod enforcement;
mod entries;
mod fields;
mod list;
mod panels;
mod permissions;
mod policy_panel;
mod uses;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::{agents::data::ProjectAgents, background, messages, tasks::outside::Outside};

use crate::{
  navigation::Navigation,
  project::{ShownProject, own::OwnChanges},
  routes::{AgentTab, ProjectPage, Route},
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

/// What the route shows of the profiles: their list, a new profile, or one profile, each editor on its tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
  List,
  New(AgentTab),
  Edit(ResourceId, AgentTab),
}
impl Shown {
  fn page(self) -> ProjectPage {
    match self {
      Self::List => ProjectPage::Agents,
      Self::New(tab) => ProjectPage::NewAgent(tab),
      Self::Edit(id, tab) => ProjectPage::Agent(id, tab),
    }
  }
}

/// Moves between the list, a new profile and a profile's tabs, each a place of the tab's history.
#[derive(Clone)]
pub(super) struct AgentsNav {
  project: ProjectId,
  navigation: Navigation,
}
impl AgentsNav {
  /// A place the person chose.
  pub fn show(&self, shown: Shown) {
    self.navigation.go(Route::Project(self.project, shown.page()));
  }
  /// The profile a save named, on the tab the editor shows now, in place of the editor it was saved from:
  /// a new profile gets its id.
  pub fn saved(&self, id: ResourceId) {
    let tab = match self.navigation.route_now() {
      Route::Project(_, ProjectPage::NewAgent(tab) | ProjectPage::Agent(_, tab)) => tab,
      _ => AgentTab::default(),
    };
    let shown = Shown::Edit(id, tab);
    self.navigation.replace(Route::Project(self.project, shown.page()));
  }
}

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
  pub shown: Shown,
}
impl DevtoolsInspectable for Props {}

pub struct AgentsScreen {
  nav: AgentsNav,
  revision: Signal<u64>,
  retry: Signal<u64>,
  /// The profile changes, the screen's own too.
  profiles: OwnChanges,
}
pub fn screen(ctx: &mut Ctx, project: &ShownProject, shown: Shown) -> Element {
  ctx.mount_keyed::<AgentsScreen>(
    &format!("agents-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      shown,
    },
  )
}

impl Component for AgentsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: AgentsNav {
        project: ctx.props::<Props>().project.open.project.id(),
        navigation: Navigation::of(ctx),
      },
      revision: ctx.signal(0),
      retry: ctx.signal(0),
      profiles: OwnChanges::new(ctx, &[Outside::Profiles]),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, shown } = ctx.props::<Props>().clone();
    let revision = self.revision.get();
    let outside = project.feed.outside.get();
    let dep = (
      revision,
      self.retry.get(),
      outside.of(Outside::Connections),
      outside.of(Outside::Pipelines),
      self.profiles.count(&project.feed),
      // The profiles' active uses: the runs that run them start and end.
      project.feed.states.get(),
    );
    let hold = crate::services::Services::of(ctx).hold;
    let loaded = ctx
      .future((project.open.repo_root.clone(), dep), move |(root, (revision, ..))| {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "agents");
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || ProjectAgents::for_screen(&root))
            .await?
            .map(|listed| (revision, listed))
            .map_err(|error| messages::store_error(&error))
        }
      })
      .state()
      .get();
    let frame = Column::new()
      .id("screen-agents")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    if loaded.is_pending() && loaded.data.as_ref().is_none_or(|(saved, _)| *saved != revision) {
      return frame.child(loading("agents-loading", &ctx.t("agents.reading")));
    }
    let error = loaded.error.clone().filter(|_| loaded.is_rejected());
    if let Some(error) = &error
      && (loaded.data.is_none() || shown == Shown::List)
    {
      let retry = self.retry.clone();
      return frame.child(load_error(
        ctx,
        "agents-load-error",
        (&ctx.t("agents.loadFailed"), &message_text(ctx, error)),
        move || retry.update(|n| *n += 1),
      ));
    }
    let reading = loaded.is_pending();
    let Some((_, listed)) = loaded.data else {
      return frame.child(loading("agents-loading", &ctx.t("agents.reading")));
    };
    frame.child(self.content(ctx, (project, shown), listed, error, reading))
  }
}
impl AgentsScreen {
  fn content(
    &self,
    ctx: &mut Ctx,
    (project, shown): (ShownProject, Shown),
    listed: ProjectAgents,
    load_error: Option<messages::Message>,
    reading: bool,
  ) -> Element {
    let revision = self.revision.get();
    let (entry, tab) = match shown {
      Shown::List => return list::list(ctx, &project, &listed, &self.nav),
      Shown::New(tab) => (None, tab),
      Shown::Edit(id, tab) => match listed.profile(id) {
        Some(entry) => (Some(entry.clone()), tab),
        None => return lurq::components::Text::new(&ctx.t("agents.gone")).into(),
      },
    };
    let key = format!("agent-editor-{:?}-{revision}", entry.as_ref().map(|e| e.id));
    ctx.mount_keyed::<editor::Editor>(
      &key,
      editor::Props {
        project,
        listed,
        entry,
        tab,
        nav: self.nav.clone(),
        revision: self.revision.clone(),
        retry: self.retry.clone(),
        profiles: self.profiles.clone(),
        load_error,
        reading,
      },
    )
  }
}
