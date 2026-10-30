//! Project settings › Sharing (`bWD9q` not linked, `L5o9Xy` linked, `N9NFio` their states; layer `RExz4`): where the
//! project's task list lives. A local project links to a project on a connected team server, chosen from the
//! person's projects there or by its address, typed or prefilled from `orchester.toml` and contacted only after the
//! person confirms; its checkout's `origin` is compared with the project's repository hint, and a mismatch warns and
//! lets the person go on ([`link`]). What the link uploads is confirmed on a page of its own (`KSnOW`, [`upload`]). A
//! linked project shows where it lives, its discovery file, its members read-only with *Manage members and invites*
//! (the server admin, in the browser), *Make local* and *Leave* ([`linked`]).

mod bindings;
mod confirm;
mod join;
mod link;
mod linked;
mod make_local;
mod members;
mod upload;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension},
};
use orchester_core::{repository::RepositoryIdentity, store::TaskStore};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::{
  background,
  sharing::{
    ServerEntry, Servers,
    project::{
      discovery::{self, LinkedDiscovery, ProjectDiscovery},
      link::checkout_origin,
    },
  },
};

pub(super) use self::{
  join::screen as join_screen,
  upload::{LinkPage, link_page},
};
use super::{TabProps, page};
use crate::{
  screens::settings::orchester::{Said, follow, follow_shared},
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
  /// A linked project's `orchester.toml` against its address.
  pub linked_discovery: Option<LinkedDiscovery>,
  /// A linked project's server as the list holds it, signed in to or not; `None` once it left the list.
  pub linked: Option<ServerEntry>,
  /// The keys the local tasks had when the project joined, for *Keys before joining*; empty for one that didn't.
  pub joined: Vec<orchester_store::JoinedKey>,
  /// What the join left to say, until the person dismisses it.
  pub joined_note: Option<linked::joined::Note>,
  /// The team's versions name references this computer binds: the page shows *On this computer*.
  pub references: bool,
}

impl DevtoolsInspectable for Facts {}

impl Facts {
  /// The linked project's server, when it is still in the list.
  pub fn linked_server(&self) -> Option<&ServerEntry> {
    self.linked.as_ref()
  }
}

pub(super) struct Sharing {
  /// Counts *Try again*, a link and a leave, each of which reads the tab again.
  revision: Signal<u64>,
  /// What *Make local* did, said over the local page until the person dismisses it.
  made_local: Signal<make_local::Made>,
  /// Opened for *Make local*'s question, the page scrolls to it once it shows.
  scroll: ScrollState,
  scrolled: Signal<bool>,
}

impl Component for Sharing {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let (made_local, scroll) = (ctx.signal(make_local::Made::default()), ScrollState::new());
    // What Make local did is said at the top of the page, which its question had scrolled to the bottom of.
    let top = scroll.clone();
    ctx.watch(&made_local, move |made| {
      if made.0.is_some() {
        top.scroll_to_top_pending();
      }
    });
    Self {
      revision: ctx.signal(0),
      made_local,
      scroll,
      scrolled: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let services = Services::of(ctx);
    let revision =
      follow(ctx, (&services.servers, &services.cloud)) + follow_shared(ctx, &services.servers) + self.revision.get();
    let root = props.project.open.repo_root.clone();
    let workspace = services.workspace.clone();
    let state = ctx
      .future(
        (revision, root, services.servers.clone(), services.programs.clone()),
        move |(_, root, servers, programs)| {
          let workspace = workspace.clone();
          async move {
            background::run(move || read((&servers, &workspace), &programs, &root))
              .await
              .map_err(|_| Said::new("cloud.errors.local"))?
          }
        },
      )
      .state()
      .get();
    let content = match (state.data, state.error) {
      (Some(facts), _) if facts.link.is_some() => {
        if props.make_local && !self.scrolled.get() {
          // Make local is the page's last part: its question is at the bottom.
          self.scrolled.set(true);
          self.scroll.scroll_to_bottom_pending();
        }
        ctx.mount_keyed::<linked::Linked>(
          "project-settings-sharing-linked",
          linked::Props {
            project: props.project,
            facts,
            revision: self.revision.clone(),
            made_local: self.made_local.clone(),
            ask_make_local: props.make_local,
          },
        )
      }
      (Some(facts), _) => ctx.mount_keyed::<link::Link>(
        "project-settings-sharing-link",
        link::Props {
          project: props.project,
          facts,
          page: props.link_page,
          made_local: self.made_local.clone(),
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
    page::scroll_with(Column::new().width(Dimension::Pct(100.0)).child(content), &self.scroll)
  }
}

fn read(
  (servers, workspace): (&Servers, &orchester_workspace::workspace::Workspace),
  programs: &orchester_exec::programs::Programs,
  root: &std::path::Path,
) -> Result<Facts, Said> {
  let local = |_| Said::new("cloud.errors.local");
  let db = ProjectDb::open(root).map_err(local)?;
  let link = db.sharing().map_err(local)?;
  let joined = db.joined_keys().map_err(local)?;
  let joined_note = (!joined.is_empty())
    .then(|| linked::joined::read(workspace, db.project().id()))
    .flatten();
  let tasks = db.tasks().map_err(local)?.len();
  let references = !db.reference_bindings().map_err(local)?.is_empty();
  let linked_discovery = link
    .as_ref()
    .and_then(|link| link.address.as_deref())
    .map(|address| discovery::inspect_linked(programs, root, address));
  let entries = servers.entries().map_err(|_| Said::new("cloud.errors.local"))?;
  let linked = link
    .as_ref()
    .and_then(|link| entries.iter().find(|entry| entry.stored.id == link.server).cloned());
  let servers = entries.into_iter().filter(ServerEntry::in_use).collect();
  Ok(Facts {
    link,
    servers,
    origin: checkout_origin(programs, root),
    discovery: discovery::project(root),
    tasks,
    linked_discovery,
    linked,
    joined,
    joined_note,
    references,
  })
}
