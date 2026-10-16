//! Project settings › Sharing (`bWD9q` not linked, `L5o9Xy` linked, `N9NFio` their states; layer `RExz4`): where the
//! project's task list lives. A local project links to a project on a connected team server, chosen from the
//! person's projects there or by its address, typed or prefilled from `orchester.toml` and contacted only after the
//! person confirms; its checkout's `origin` is compared with the project's repository hint, and a mismatch warns and
//! lets the person go on ([`link`]). A linked project shows where it lives and offers *Leave* ([`linked`]); its
//! members, *Make local* and the discovery file are N2's.

mod link;
mod linked;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{repository::RepositoryIdentity, store::TaskStore};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::{
  background,
  sharing::{
    ServerEntry, Servers,
    project::{
      discovery::{self, ProjectDiscovery},
      link::checkout_origin,
    },
  },
};

use super::{TabProps, page};
use crate::{
  screens::settings::orchester::{Said, follow},
  services::Services,
  ui::state::{load_error, loading},
};

/// What the tab shows: the link, the servers this computer is signed in to, the checkout's `origin`, what
/// `orchester.toml` names, and the local tasks a first link uploads.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Facts {
  pub link: Option<SharingLink>,
  /// The servers in the list an account is signed in to from this computer.
  pub servers: Vec<ServerEntry>,
  pub origin: Option<RepositoryIdentity>,
  pub discovery: ProjectDiscovery,
  /// The local tasks, which a link to an empty project uploads with their numbers.
  pub tasks: usize,
}

impl DevtoolsInspectable for Facts {}

impl Facts {
  /// The linked project's server, when it is still in the list.
  pub fn linked_server(&self) -> Option<&ServerEntry> {
    let link = self.link.as_ref()?;
    self.servers.iter().find(|entry| entry.stored.id == link.server)
  }
}

pub(super) struct Sharing {
  /// Counts *Try again*, a link and a leave, each of which reads the tab again.
  revision: Signal<u64>,
}

impl Component for Sharing {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let services = Services::of(ctx);
    let revision = follow(ctx, (&services.servers, &services.cloud)) + self.revision.get();
    let root = props.project.open.repo_root.clone();
    let state = ctx
      .future(
        (revision, root, services.servers.clone(), services.programs.clone()),
        |(_, root, servers, programs)| async move {
          background::run(move || read(&servers, &programs, &root))
            .await
            .map_err(|_| Said::new("cloud.errors.local"))?
        },
      )
      .state()
      .get();
    let content = match (state.data, state.error) {
      (Some(facts), _) if facts.link.is_some() => ctx.mount_keyed::<linked::Linked>(
        "project-settings-sharing-linked",
        linked::Props {
          project: props.project,
          facts,
          revision: self.revision.clone(),
        },
      ),
      (Some(facts), _) => ctx.mount_keyed::<link::Link>(
        "project-settings-sharing-link",
        link::Props {
          project: props.project,
          facts,
          revision: self.revision.clone(),
        },
      ),
      (None, Some(reason)) => {
        let (text, revision) = (reason.text(ctx), self.revision.clone());
        load_error(
          ctx,
          "project-settings-sharing-error",
          (&ctx.t("sharing.loadFailed"), &text),
          move || revision.update(|count| *count += 1),
        )
      }
      (None, None) => loading("project-settings-sharing-loading", &ctx.t("sharing.reading")).into(),
    };
    page::scroll(Column::new().width(Dimension::Pct(100.0)).child(content))
  }
}

fn read(
  servers: &Servers,
  programs: &orchester_exec::programs::Programs,
  root: &std::path::Path,
) -> Result<Facts, Said> {
  let local = |_| Said::new("cloud.errors.local");
  let db = ProjectDb::open(root).map_err(local)?;
  let link = db.sharing().map_err(local)?;
  let tasks = db.tasks().map_err(local)?.len();
  let servers = servers
    .entries()
    .map_err(|_| Said::new("cloud.errors.local"))?
    .into_iter()
    .filter(ServerEntry::in_use)
    .collect();
  Ok(Facts {
    link,
    servers,
    origin: checkout_origin(programs, root),
    discovery: discovery::project(root),
    tasks,
  })
}
