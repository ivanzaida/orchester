//! A local project's Sharing (`bWD9q`, the linking states of `N9NFio`): *Task list · Local only*; *Link to a server
//! project* with the connected server, the person's projects there or a project address (prefilled from
//! `orchester.toml` only when the person takes it), the repository compared with the project's hint ([`rows`]), and
//! *Link to Kontur…*, which asks in place about a mismatch (*Choose another project* or *Link anyway…*) and then
//! opens what goes up on a page of its own (`KSnOW`, `super::upload`) ([`confirm`]). A checkout with tasks linked to a
//! project with tasks is joined through its review (`AiCwO`, `super::join`). An address with no project the person is
//! a member of says so, and to ask for an invite; nothing is sent. What linking
//! sends closes the page ([`parts`]). After *Make local* the page says what it did until the person dismisses it.

mod confirm;
mod discovery;
mod parts;
mod rows;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_api::SharedProject;
use orchester_core::{
  id::{IdempotencyKey, ProjectId, ServerId},
  repository::RepositoryIdentity,
};
use orchester_workspace::sharing::{
  ProjectAddress, RemoteMatch, ServerEntry, ServerProject, Servers, SharingError,
  project::{
    first_link::{LinkKind, LinkPlan},
    link::{LinkProblem, LinkTarget},
  },
};

pub(super) use self::parts::{join_sends, sends};
use super::{
  Facts,
  make_local::Made,
  upload::{LinkPage, Upload},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{JoinTarget, ProjectPage, Route},
  screens::settings::orchester::{PersonOnly, Said, sharing_said},
  services::Services,
  theme::ExtraSpacing,
};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub facts: Facts,
  /// The first link's page, opened once the project is resolved.
  pub page: Signal<LinkPage>,
  pub made_local: Signal<Made>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.facts == other.facts
  }
}

impl DevtoolsInspectable for Props {}

/// The server chosen in the select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Chosen(Option<ServerId>);

impl DevtoolsInspectable for Chosen {}

/// The project chosen among the person's projects on the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Picked(Option<ProjectId>);

impl DevtoolsInspectable for Picked {}

/// Why a link did not go ahead, as the page says it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Problem {
  /// The address is on a server this computer is not connected to; nothing was sent there.
  NotConnected(String),
  /// No project there the person is a member of; the server answers both alike.
  NoProject(String),
  Other(Said),
}

impl DevtoolsInspectable for Problem {}

/// Where the confirmation stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Asking {
  Nothing,
  /// The checkout's `origin` is not the project's hint: *Choose another project* or *Link anyway…*.
  Mismatch,
}

impl DevtoolsInspectable for Asking {}

/// A target the page resolved, with what linking it would do.
#[derive(Clone, Debug)]
struct Target(LinkTarget, LinkPlan);

impl PartialEq for Target {
  fn eq(&self, other: &Self) -> bool {
    self.0.entry == other.0.entry
      && self.0.project.id == other.0.project.id
      && self.0.remote == other.0.remote
      && self.1.kind == other.1.kind
      && self.1.discovery == other.1.discovery
  }
}

impl Target {
  /// Goes on with this target: a checkout with tasks linked to a project with tasks opens the join review; any other
  /// opens the first link's page.
  fn open(&self, page: &Signal<LinkPage>, (navigation, project): (&Navigation, ProjectId)) {
    if let LinkKind::Review { .. } = self.1.kind {
      if let Some(target) = JoinTarget::new(self.0.entry.stored.id, &self.0.project.slug) {
        navigation.go(Route::Project(project, ProjectPage::Join(target)));
      }
      return;
    }
    page.set(self.upload());
  }

  /// The first link's page for this target, with a new upload key.
  fn upload(&self) -> LinkPage {
    let remote = match &self.0.remote {
      RemoteMatch::Same => self.0.project.remote_hint.clone(),
      RemoteMatch::Different { checkout, .. } => Some(checkout.clone()),
      RemoteMatch::NoHint | RemoteMatch::NoOrigin => None,
    };
    LinkPage(Some(std::sync::Arc::new(Upload {
      entry: self.0.entry.clone(),
      plan: self.1.clone(),
      remote,
      key: IdempotencyKey::generate(),
    })))
  }
}

impl DevtoolsInspectable for Target {}

/// The person's projects on the chosen server.
#[derive(Clone, Debug)]
struct Listed(Vec<ServerProject>);

impl PartialEq for Listed {
  fn eq(&self, other: &Self) -> bool {
    self.0.len() == other.0.len()
      && self
        .0
        .iter()
        .zip(&other.0)
        .all(|(one, two)| one.project.id == two.project.id)
  }
}

impl DevtoolsInspectable for Listed {}

/// What the page sends to resolve a target: a project of the list, or an address.
#[derive(Clone)]
enum Choice {
  Listed(Box<(ServerEntry, SharedProject)>),
  Address(String),
}

pub(super) struct Link {
  server: Signal<Chosen>,
  picked: Signal<Picked>,
  address: Signal<String>,
  /// The person took the address `orchester.toml` names, or set it aside (*Not now*).
  discovery_answered: Signal<bool>,
  asking: Signal<Asking>,
  resolve: FutureAction<Choice, Target, Problem>,
  /// Linking contacts the server: it is a person's (review 1, M1).
  person: PersonOnly,
}

impl Component for Link {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let (servers, origin, root) = (
      services.servers,
      props.facts.origin.clone(),
      props.project.open.repo_root.clone(),
    );
    let resolve = ctx.future_action(move |choice: Choice| {
      let (servers, origin, root) = (servers.clone(), origin.clone(), root.clone());
      async move { resolve(&servers, &root, choice, origin.as_ref()).await }
    });
    let first = props.facts.servers.first().map(|entry| entry.stored.id);
    let asking = ctx.signal(Asking::Nothing);
    let (next, page, navigation) = (asking.clone(), props.page, Navigation::of(ctx));
    let project = props.project.open.project.id();
    ctx.watch(&resolve.state(), move |state| {
      if let Some(target) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        if target.0.remote.warns() {
          next.set(Asking::Mismatch);
        } else {
          target.open(&page, (&navigation, project));
        }
      }
    });
    Self {
      server: ctx.signal(Chosen(first)),
      picked: ctx.signal(Picked(None)),
      address: ctx.signal(String::new()),
      discovery_answered: ctx.signal(false),
      asking,
      resolve,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let listed = self.listed(ctx, &props);
    Column::new()
      .id("project-settings-sharing")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .with_children(super::make_local::made(
        ctx,
        &props.made_local,
        props.project.open.project.name().as_str(),
      ))
      .child(parts::task_list(ctx, &props))
      .child(self.link_section(ctx, &props, listed.as_ref()))
      .child(parts::admin_note(ctx, self.chosen_entry(&props)))
      .child(parts::sends(ctx))
  }
}

impl Link {
  fn chosen_entry<'a>(&self, props: &'a Props) -> Option<&'a ServerEntry> {
    let chosen = self.server.get().0?;
    props.facts.servers.iter().find(|entry| entry.stored.id == chosen)
  }

  /// The person's projects on the chosen server.
  fn listed(&self, ctx: &mut Ctx, props: &Props) -> Option<Result<Listed, Said>> {
    let entry = self.chosen_entry(props).cloned();
    let servers = Services::of(ctx).servers;
    let state = ctx
      .future((entry, servers), |(entry, servers)| async move {
        let Some(entry) = entry else {
          return Ok(Listed(Vec::new()));
        };
        servers
          .server_projects(&entry)
          .await
          .map(Listed)
          .map_err(|error| sharing_said(&error))
      })
      .state()
      .get();
    match (state.data, state.error) {
      (_, Some(error)) => Some(Err(error)),
      (Some(listed), _) => Some(Ok(listed)),
      _ => None,
    }
  }
}

/// The target `choice` names, with the checkout's `origin` compared and what linking it would do. A checkout with
/// tasks of its own linked to a project with tasks is a join, reviewed first.
async fn resolve(
  servers: &Servers,
  root: &std::path::Path,
  choice: Choice,
  origin: Option<&RepositoryIdentity>,
) -> Result<Target, Problem> {
  let target = match choice {
    Choice::Listed(listed) => {
      let (entry, project) = *listed;
      Servers::target(entry, project, origin)
    }
    Choice::Address(text) => {
      let address =
        ProjectAddress::parse(&text).map_err(|_| Problem::Other(Said::new("sharing.link.addressInvalid")))?;
      match servers.link_target(&address, origin).await {
        Ok(target) => target,
        Err(LinkProblem::NotConnected(server)) => return Err(Problem::NotConnected(server.authority())),
        Err(LinkProblem::Sharing(error)) => return Err(sharing_problem(&error, &address.to_text())),
      }
    }
  };
  let plan = servers
    .link_plan(&target.entry, root, target.project.clone())
    .await
    .map_err(|error| sharing_problem(&error, &target.project.address))?;
  Ok(Target(target, plan))
}

fn sharing_problem(error: &SharingError, address: &str) -> Problem {
  match error {
    SharingError::NoProject => Problem::NoProject(address.to_owned()),
    error => Problem::Other(sharing_said(error)),
  }
}
