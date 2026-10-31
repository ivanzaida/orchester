//! Profiles, the Agents hub's first tab (`01 Profiles` `B94gsw`, `01b Profile` `acOlR`, `01c` `WAWHr`): the list, a
//! new profile and a profile's page are the route's ([`Shown`]), so Back and Forward step through them. A profile's
//! page is one page: its role, connection and model, and permissions beside what enforces them, what else reaches
//! its prompts and where it is used. Edits save as a draft as they are made.
mod active;
mod autosave;
mod editor;
mod enforcement;
mod entries;
mod fields;
mod list;
mod model;
mod page_head;
mod permissions;
mod policy_panel;
mod prompts;
mod questions;
mod reading;
mod shared;
mod side;
mod uses;
mod words;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Text,
  core::Signal,
  node::Element,
};
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::{agents::data::ProjectAgents, background, messages, tasks::outside::Outside};

use crate::{
  navigation::{Navigation, link::Open},
  project::{ShownProject, own::OwnChanges},
  routes::{AgentTab, ProjectPage, Route},
  screens::hub::{self, Head, HubTab},
  ui::failure::LastFailure,
};

/// What the route shows of the profiles: their list, a new profile, or one profile. The tab a route names is kept
/// for its links; the page shows all of it.
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

/// Moves between the list, a new profile and a profile, each a place of the tab's history.
#[derive(Clone)]
pub(super) struct AgentsNav {
  project: ProjectId,
  navigation: Navigation,
}
impl AgentsNav {
  /// What opens a place the person chose, by a browser's rules (`navigation::link`).
  pub fn to(&self, shown: Shown) -> Open {
    self.navigation.to(Route::Project(self.project, shown.page()))
  }
  /// What opens Import resources on its Agents tab (`G69AHh`).
  pub fn import(&self) -> Open {
    self.navigation.to(Route::Project(
      self.project,
      ProjectPage::Import(crate::routes::ImportTab::Agents),
    ))
  }
  /// The profile a new profile's first save created, in place of the new profile.
  pub fn saved(&self, id: ResourceId) {
    let tab = match self.navigation.route_now() {
      Route::Project(_, ProjectPage::NewAgent(tab) | ProjectPage::Agent(_, tab)) => tab,
      _ => AgentTab::default(),
    };
    self
      .navigation
      .replace(Route::Project(self.project, Shown::Edit(id, tab).page()));
  }
  /// The list, in place of a profile that was removed.
  pub fn removed(&self) {
    self
      .navigation
      .replace(Route::Project(self.project, Shown::List.page()));
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
  /// Counts the reloads of a profile that read it anew into its page.
  revision: Signal<u64>,
  retry: Signal<u64>,
  /// The profile changes, the screen's own too.
  profiles: OwnChanges,
  /// Why the profiles couldn't be read, kept while they are read again.
  failure: LastFailure,
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
      failure: LastFailure::default(),
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
      outside.of(Outside::Instructions) + outside.of(Outside::Skills),
      // The project's Allow Autopilot, which a profile set to Autopilot reads (02e).
      outside.of(Outside::AutoMode),
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
    let failed = self.failure.follow(&loaded);
    let retry = self.retry.clone();
    let what = ctx.t("agents.loadFailed");
    let failure = hub::read_failure(
      ctx,
      &project,
      (HubTab::Profiles, "agents-load-error", &what),
      failed.clone(),
      move || retry.update(|n| *n += 1),
    );
    if let Some(failure) = failure
      && (loaded.data.is_none() || shown == Shown::List)
    {
      let body = hub::list_body("agents-failed", failure.page(ctx));
      return frame(ctx, &project, Head::default(), body);
    }
    if loaded.is_pending() && loaded.data.as_ref().is_none_or(|(saved, _)| *saved != revision) {
      return self.reading(ctx, &project, shown);
    }
    let error = failed.filter(|(_, retrying)| !retrying).map(|(error, _)| error);
    let reading = loaded.is_pending();
    let Some((_, listed)) = loaded.data else {
      return self.reading(ctx, &project, shown);
    };
    self.content(ctx, (project, shown), listed, (error, reading))
  }
}
impl AgentsScreen {
  /// The list or the profile's page while the profiles are read.
  fn reading(&self, ctx: &mut Ctx, project: &ShownProject, shown: Shown) -> Element {
    match shown {
      Shown::List => {
        let head = list::reading_head(ctx, &self.nav);
        let body = reading::list(ctx);
        frame(ctx, project, head, body)
      }
      Shown::New(_) | Shown::Edit(..) => {
        let head = list::reading_head(ctx, &self.nav);
        let body = reading::profile(ctx, &self.nav);
        frame(ctx, project, head, body)
      }
    }
  }
}
impl AgentsScreen {
  fn content(
    &self,
    ctx: &mut Ctx,
    (project, shown): (ShownProject, Shown),
    listed: ProjectAgents,
    (load_error, reading): (Option<messages::Message>, bool),
  ) -> Element {
    let revision = self.revision.get();
    let entry = match shown {
      Shown::List => {
        let team = hub::team::of(&project);
        let head = list::head(ctx, (&listed, team.as_ref()), &self.nav);
        let body = list::list(ctx, &project, &listed, &self.nav);
        return frame(ctx, &project, head, body);
      }
      Shown::New(_) => None,
      Shown::Edit(id, _) => match listed.profile(id) {
        Some(entry) => Some(entry.clone()),
        None => {
          let body = hub::list_body("agents-gone", Text::new(&ctx.t("agents.gone")));
          return frame(ctx, &project, Head::default(), body);
        }
      },
    };
    let key = format!("agent-editor-{:?}-{revision}", entry.as_ref().map(|e| e.id));
    ctx.mount_keyed::<editor::Editor>(
      &key,
      editor::Props {
        project,
        listed,
        entry,
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

/// The hub on its Profiles tab around `body`, as the screen's element.
fn frame(ctx: &mut Ctx, project: &ShownProject, head: Head, body: impl Into<Element>) -> Element {
  lurq::components::Column::new()
    .id("screen-agents")
    .width(lurq::node::dimension::Dimension::Pct(100.0))
    .height(lurq::node::dimension::Dimension::Pct(100.0))
    .child(hub::page(ctx, project, (HubTab::Profiles, head), body))
    .into()
}
