//! A local project's Sharing (`bWD9q`, the linking states of `N9NFio`): *Task list · Local only*; *Link to a server
//! project* with the connected server, the person's projects there or a project address (prefilled from
//! `orchester.toml` only when the person takes it), the repository compared with the project's hint ([`rows`]), and
//! *Link to Kontur…*, which asks in place: first about a mismatch (*Choose another project* or *Link anyway…*), then
//! what goes up ([`confirm`]). A project with tasks of its own is joined through its review, which this version does
//! not have yet, and says so. What linking sends closes the page ([`parts`]).

mod confirm;
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
    LinkFirst, ShareError,
    link::{LinkProblem, LinkTarget},
  },
};

use super::Facts;
use crate::{
  project::ShownProject,
  screens::settings::orchester::{PersonOnly, Said, sharing_said},
  services::Services,
  theme::ExtraSpacing,
};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub facts: Facts,
  pub revision: Signal<u64>,
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
  /// The project has tasks already: joining it is its review, not built in this version.
  Taken(String),
  Other(Said),
}

impl DevtoolsInspectable for Problem {}

/// Where the confirmation stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Asking {
  Nothing,
  /// The checkout's `origin` is not the project's hint: *Choose another project* or *Link anyway…*.
  Mismatch,
  /// What goes up, with *Link and upload*.
  Upload,
}

impl DevtoolsInspectable for Asking {}

/// A target the page resolved, ready to link.
#[derive(Clone, Debug)]
struct Target(LinkTarget);

impl PartialEq for Target {
  fn eq(&self, other: &Self) -> bool {
    self.0.entry == other.0.entry && self.0.project.id == other.0.project.id && self.0.remote == other.0.remote
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

/// The upload's idempotency key, made when the confirmation opens: a retried link links once.
#[derive(Clone, Debug, PartialEq, Eq)]
struct UploadKey(IdempotencyKey);

impl DevtoolsInspectable for UploadKey {}

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
  key: Signal<UploadKey>,
  resolve: FutureAction<Choice, Target, Problem>,
  link: FutureAction<(Target, UploadKey), (), Problem>,
  /// Linking (which contacts the server) and the upload are a person's (review 1, M1).
  person: PersonOnly,
}

impl Component for Link {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let (servers, origin) = (services.servers.clone(), props.facts.origin.clone());
    let resolve = ctx.future_action(move |choice: Choice| {
      let (servers, origin) = (servers.clone(), origin.clone());
      async move { resolve(&servers, choice, origin.as_ref()).await }
    });
    let (servers, root) = (services.servers, props.project.open.repo_root.clone());
    let link = ctx.future_action(move |(target, key): (Target, UploadKey)| {
      let (servers, root) = (servers.clone(), root.clone());
      async move { link(&servers, &root, target, key).await }
    });
    let first = props.facts.servers.first().map(|entry| entry.stored.id);
    let (asking, key) = (
      ctx.signal(Asking::Nothing),
      ctx.signal(UploadKey(IdempotencyKey::generate())),
    );
    let (next, fresh) = (asking.clone(), key.clone());
    ctx.watch(&resolve.state(), move |state| {
      if let Some(target) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        fresh.set(UploadKey(IdempotencyKey::generate()));
        next.set(if target.0.remote.warns() {
          Asking::Mismatch
        } else {
          Asking::Upload
        });
      }
    });
    Self {
      server: ctx.signal(Chosen(first)),
      picked: ctx.signal(Picked(None)),
      address: ctx.signal(String::new()),
      discovery_answered: ctx.signal(false),
      asking,
      key,
      resolve,
      link,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.follow_link(&props);
    let listed = self.listed(ctx, &props);
    Column::new()
      .id("project-settings-sharing")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(parts::task_list(ctx, &props))
      .child(self.link_section(ctx, &props, listed.as_ref()))
      .child(parts::admin_note(ctx, self.chosen_entry(&props)))
      .child(parts::sends(ctx))
  }
}

impl Link {
  /// Once the link went through, the tab reads the project again and shows it linked.
  fn follow_link(&self, props: &Props) {
    let state = self.link.state().get();
    if state.is_fulfilled() && self.asking.get() != Asking::Nothing {
      self.asking.set(Asking::Nothing);
      props.revision.update(|count| *count += 1);
    }
  }

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

/// The target `choice` names, with the checkout's `origin` compared.
async fn resolve(servers: &Servers, choice: Choice, origin: Option<&RepositoryIdentity>) -> Result<Target, Problem> {
  match choice {
    Choice::Listed(listed) => {
      let (entry, project) = *listed;
      Ok(Target(Servers::target(entry, project, origin)))
    }
    Choice::Address(text) => {
      let address =
        ProjectAddress::parse(&text).map_err(|_| Problem::Other(Said::new("sharing.link.addressInvalid")))?;
      match servers.link_target(&address, origin).await {
        Ok(target) => Ok(Target(target)),
        Err(LinkProblem::NotConnected(server)) => Err(Problem::NotConnected(server.authority())),
        Err(LinkProblem::Sharing(error)) => Err(sharing_problem(&error, &address.to_text())),
      }
    }
  }
}

/// Links the project at `root` to `target` as its first checkout: every task goes up with its number.
async fn link(servers: &Servers, root: &std::path::Path, target: Target, key: UploadKey) -> Result<(), Problem> {
  let request = LinkFirst {
    slug: target.0.project.slug.clone(),
    remote: match &target.0.remote {
      RemoteMatch::Same => target.0.project.remote_hint.clone(),
      RemoteMatch::Different { checkout, .. } => Some(checkout.clone()),
      RemoteMatch::NoHint | RemoteMatch::NoOrigin => None,
    },
    key: key.0,
    // The discovery file is N2's: linking writes nothing into the checkout.
    write_discovery: false,
  };
  match servers.link_first(&target.0.entry, root, request).await {
    Ok(_) => Ok(()),
    Err(ShareError::Taken { .. }) => Err(Problem::Taken(target.0.project.name.clone())),
    Err(ShareError::Sharing(error)) => Err(sharing_problem(&error, &target.0.project.address)),
  }
}

fn sharing_problem(error: &SharingError, address: &str) -> Problem {
  match error {
    SharingError::NoProject => Problem::NoProject(address.to_owned()),
    error => Problem::Other(sharing_said(error)),
  }
}
