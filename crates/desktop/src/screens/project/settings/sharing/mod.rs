//! Project settings › Sharing (`docs/design/orchester-workspace.md`, "Project settings › Sharing";
//! 10.11–10.14, 10.17): where the project's task list lives. A local project shows its repository's
//! identity with what the signed-in servers have for it, *Share on…* with its confirmation, and the
//! offer of the repository's `orchester.toml`; a shared one shows its server project, the discovery
//! file, the members and their roles, and *Make local*. Everything that changes the server asks it
//! and then reads the project again; nothing is changed here that the server refused.

mod dialogs;
mod join_offer;
mod labels;
mod local;
mod mapping;
mod members;
mod shared;

use std::path::{Path, PathBuf};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Modal, ModalTarget, ScrollVertical, Text},
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension},
};
use orchester_api::{Member, MemberRole};
use orchester_core::{
  id::{IdempotencyKey, ServerId},
  repository::RepositoryIdentity,
  store::TaskStore,
};
use orchester_store::ProjectDb;

use super::super::ProjectNav;
use crate::{
  background,
  messages::{self, Message},
  programs::Programs,
  sharing::{
    ServerEntry, Servers, SharingError,
    feed::ServerFeed,
    project::{
      IdentityProblem, MemberChange, ProjectServer, ShareError, SharePlan,
      discovery::{self, DiscoveryFile},
      plan, repository_identity,
      state::ProjectSharing,
    },
  },
  theme::ExtraTypography,
  ui::form::form,
  workspace::OpenProject,
};

/// Width of the Sharing content (`egewZ`, 792 with its padding).
const CONTENT_WIDTH: f32 = 792.0;

#[derive(Clone, PartialEq)]
pub(super) struct SharingProps {
  pub open: OpenProject,
  pub sharing: Option<ProjectSharing>,
}

impl lurq::app::component::DevtoolsInspectable for SharingProps {}

/// What the tab reads of the project and its repository.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Facts {
  tasks: usize,
  identity: Result<RepositoryIdentity, IdentityProblem>,
  discovery: DiscoveryFile,
  /// What sharing would do; only for a local project whose repository can be shared.
  plan: Option<SharePlan>,
  /// The keys the local tasks had when the project joined its server project (10.22).
  joined: Vec<orchester_store::JoinedKey>,
}

impl lurq::app::component::DevtoolsInspectable for Facts {}

/// Which dialog is open.
#[derive(Clone, Debug)]
enum SharingDialog {
  Closed,
  /// The Share confirmation, with the key its request is sent under (a retry shares once).
  Share {
    server: ServerId,
    key: IdempotencyKey,
  },
  MakeOwner(Member),
  MakeSelfMember,
  Remove(Member),
  Leave,
  MakeLocal,
}

impl PartialEq for SharingDialog {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Share { key: a, .. }, Self::Share { key: b, .. }) => a == b,
      (Self::MakeOwner(a), Self::MakeOwner(b)) | (Self::Remove(a), Self::Remove(b)) => a.user.id == b.user.id,
      _ => std::mem::discriminant(self) == std::mem::discriminant(other),
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for SharingDialog {}

/// The tasks kept *Not shared* that *Make local* returned to the list, with their keys (10.26 C).
#[derive(Clone, Debug, PartialEq, Eq)]
struct MadeLocal(std::sync::Arc<[orchester_store::ReturnedTask]>);

impl lurq::app::component::DevtoolsInspectable for MadeLocal {}

/// What Share did: shared the project, or met the repository's project on the server
/// (`409 remote_taken`, 10.16 C) and changed nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ShareDone {
  Shared,
  Taken { server: ServerId, owners: Vec<String> },
}

impl lurq::app::component::DevtoolsInspectable for ShareDone {}

/// The server whose project Share met, with its owners, until the person sets the notice aside.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Taken(Option<(ServerId, Vec<String>)>);

impl lurq::app::component::DevtoolsInspectable for Taken {}

/// The server *Share on* names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ServerChoice(Option<ServerId>);

impl lurq::app::component::DevtoolsInspectable for ServerChoice {}

/// The role *Add member* gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RoleChoice(MemberRole);

impl lurq::app::component::DevtoolsInspectable for RoleChoice {}

/// A change of the members, as the tab asks the server.
#[derive(Clone, Debug, PartialEq, Eq)]
enum MemberWork {
  Add { username: String, role: MemberRole },
  Change(MemberChange),
}

impl lurq::app::component::DevtoolsInspectable for MemberWork {}

/// The members as the server lists them, in the order it lists them.
#[derive(Clone, Debug)]
struct Members(std::sync::Arc<[Member]>);

impl PartialEq for Members {
  fn eq(&self, other: &Self) -> bool {
    self.0.len() == other.0.len()
      && self
        .0
        .iter()
        .zip(other.0.iter())
        .all(|(a, b)| a.user == b.user && a.role == b.role && a.added_at == b.added_at)
  }
}

impl lurq::app::component::DevtoolsInspectable for Members {}

type ShareAction = FutureAction<(ServerEntry, RepositoryIdentity, IdempotencyKey, bool), ShareDone, Message>;
type MemberAction = FutureAction<MemberWork, (), Message>;
type LocalAction = FutureAction<(), MadeLocal, Message>;
type DiscoveryAction = FutureAction<(), (), Message>;

/// The tab's futures, requested in this order in `create` and `render`.
#[derive(Clone)]
struct Actions {
  share: ShareAction,
  member: MemberAction,
  make_local: LocalAction,
  discovery: DiscoveryAction,
}

pub(super) struct SharingTab {
  dialog: Signal<SharingDialog>,
  /// Bumped after a change, to read the project and the members again.
  revision: Signal<u64>,
  server: Signal<ServerChoice>,
  /// *Not now* on the discovery offer, for this visit of the tab.
  dismissed: Signal<bool>,
  username: Signal<String>,
  role: Signal<RoleChoice>,
  /// The Share confirmation's *Write orchester.toml*; on unless the person turns it off.
  write_discovery: Signal<bool>,
  nav: ProjectNav,
  scroll: ScrollState,
  /// The open dialog's panel, focused when it opens so Tab starts in it.
  frame: ElementRef,
  /// The dialog shown last, so the focus moves only when another one opens.
  focused: Signal<SharingDialog>,
  taken: Signal<Taken>,
  /// Copy put the person's username on the clipboard (10.16 A).
  copied: Signal<bool>,
  /// *Dismiss* on the notice after a join or a Make local, for this visit of the tab.
  notice_dismissed: Signal<bool>,
}

impl Component for SharingTab {
  type Props = SharingProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      dialog: ctx.signal(SharingDialog::Closed),
      revision: ctx.signal(0_u64),
      server: ctx.signal(ServerChoice::default()),
      dismissed: ctx.signal(false),
      username: ctx.signal(String::new()),
      role: ctx.signal(RoleChoice(MemberRole::Member)),
      write_discovery: ctx.signal(true),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
      scroll: ScrollState::new(),
      frame: ElementRef::new(),
      focused: ctx.signal(SharingDialog::Closed),
      taken: ctx.signal(Taken::default()),
      copied: ctx.signal(false),
      notice_dismissed: ctx.signal(false),
    };
    let actions = tab.actions(ctx);
    tab.watch(ctx, &actions);
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SharingProps>().clone();
    let actions = self.actions(ctx);
    let facts = match self.facts(ctx, &props.open.repo_root, props.sharing.is_some()) {
      Ok(facts) => facts,
      Err(error) => {
        return Column::new()
          .id("settings-sharing")
          .padding(SpacingSize::Section)
          .child(crate::ui::panel::RecoveryPanel::new(
            crate::ui::icon::Icon::AlertCircle,
            &ctx.t("sharing.loadFailed"),
            &crate::ui::message_text(ctx, &error),
          ));
      }
    };
    let mut content = Column::new()
      .id("settings-sharing")
      .width(Dimension::Px(CONTENT_WIDTH))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("sharing.heading")).variant(ExtraTypography::SectionHeading.role()));
    let dialog = match (&facts, &props.sharing) {
      (Some(facts), None) => {
        content = content.with_children(self.local(ctx, &props.open, facts, &actions));
        self.local_dialog(ctx, &props.open, facts, &actions)
      }
      (Some(facts), Some(sharing)) => {
        content = content.with_children(self.shared(ctx, &props.open, sharing, facts, &actions));
        self.shared_dialog(ctx, &props.open, sharing, facts, &actions)
      }
      (None, _) => None,
    };
    let shown = self.dialog.get();
    if dialog.is_some() && self.focused.get_untracked() != shown {
      ctx.focus(&self.frame);
    }
    if self.focused.get_untracked() != shown {
      self.focused.set(shown);
    }
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(form(
        None,
        ScrollVertical::new(content)
          .with_scroll_state(self.scroll.clone())
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      ))
      // Over the whole window, as every dialog of the app: the sidebar and the tabs take no input.
      .with_children(dialog.map(|dialog| {
        Modal::new(dialog)
          .target(ModalTarget::Root)
          .dismiss_on_escape(false)
      }))
  }
}

impl SharingTab {
  fn actions(&self, ctx: &mut Ctx) -> Actions {
    let servers = ServerFeed::of(ctx).servers;
    let root = ctx.props::<SharingProps>().open.repo_root.clone();
    Actions {
      share: share_action(ctx, &servers, &root),
      member: member_action(ctx, &servers, &root),
      make_local: local_action(ctx, &servers, &root),
      discovery: discovery_action(ctx, &servers, &root),
    }
  }

  /// A finished change closes its dialog and reads the project and the tasks again.
  fn watch(&self, ctx: &mut Ctx, actions: &Actions) {
    let done = |dialog: Signal<SharingDialog>, revision: Signal<u64>, nav: ProjectNav| {
      move |pending: bool, failed: bool| {
        if pending {
          return;
        }
        revision.update(|revision| *revision += 1);
        nav.tasks_changed();
        if !failed {
          dialog.set(SharingDialog::Closed);
        }
      }
    };
    let finished = done(self.dialog.clone(), self.revision.clone(), self.nav.clone());
    let taken = self.taken.clone();
    ctx.watch(&actions.share.state(), move |state| {
      if let Some(ShareDone::Taken { server, owners }) = state.data.clone().filter(|_| state.is_fulfilled()) {
        taken.set(Taken(Some((server, owners))));
      }
      finished(state.is_pending(), state.is_rejected());
    });
    let (finished, username) = (
      done(self.dialog.clone(), self.revision.clone(), self.nav.clone()),
      self.username.clone(),
    );
    ctx.watch(&actions.member.state(), move |state| {
      // Idle again when a dialog opens after an error: nothing finished then.
      if !state.is_fulfilled() && !state.is_rejected() {
        return;
      }
      if state.is_fulfilled() {
        username.set(String::new());
      }
      finished(false, state.is_rejected());
    });
    let finished = done(self.dialog.clone(), self.revision.clone(), self.nav.clone());
    ctx.watch(&actions.make_local.state(), move |state| {
      finished(state.is_pending(), state.is_rejected());
    });
    let revision = self.revision.clone();
    ctx.watch(&actions.discovery.state(), move |state| {
      if !state.is_pending() {
        revision.update(|revision| *revision += 1);
      }
    });
    // An error belongs to the change it reports: a dialog opened afterwards starts without it.
    let member = actions.member.state();
    ctx.watch(&self.dialog, move |dialog| {
      if *dialog != SharingDialog::Closed && member.get_untracked().is_rejected() {
        member.set(lurq::app::ctx::FutureState::idle());
      }
    });
  }

  /// The project and its repository, read again after each change; `None` until read.
  fn facts(&self, ctx: &mut Ctx, root: &Path, shared: bool) -> Result<Option<Facts>, Message> {
    let programs = ctx
      .use_context::<Programs>()
      .expect("the root provides Programs before mounting screens");
    let deps = (root.to_path_buf(), self.revision.get(), shared);
    let load = ctx.future(deps, move |(root, _, shared): (PathBuf, u64, bool)| {
      let programs = programs.clone();
      async move {
        background::run(move || read_facts(&programs, &root, shared))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot read the project's sharing");
            messages::store_error(&error)
          })
      }
    });
    let state = load.state().get();
    let pending = state.is_pending();
    match (state.data, state.error) {
      (_, Some(error)) if !pending => Err(error),
      (data, _) => Ok(data),
    }
  }
}

fn read_facts(programs: &Programs, root: &Path, shared: bool) -> Result<Facts, orchester_store::StoreError> {
  let db = ProjectDb::open(root)?;
  let tasks = db.tasks()?.len();
  let joined = db.joined_keys()?;
  drop(db);
  let identity = repository_identity(programs, root);
  let plan = match (&identity, shared) {
    (Ok(identity), false) => Some(plan(root, identity.clone())?),
    _ => None,
  };
  let discovery = discovery::inspect(programs, root);
  Ok(Facts {
    tasks,
    identity,
    discovery,
    plan,
    joined,
  })
}

fn share_action(ctx: &mut Ctx, servers: &Servers, root: &Path) -> ShareAction {
  let (servers, root) = (servers.clone(), root.to_path_buf());
  ctx.future_action(
    move |(entry, identity, key, write): (ServerEntry, RepositoryIdentity, IdempotencyKey, bool)| {
      let (servers, root) = (servers.clone(), root.clone());
      async move {
        match servers.share(&entry, &root, identity, key, write).await {
          Ok(_) => Ok(ShareDone::Shared),
          Err(ShareError::Taken { owners, .. }) => Ok(ShareDone::Taken {
            server: entry.stored.id,
            owners: owners.into_iter().map(|owner| owner.display_name).collect(),
          }),
          Err(ShareError::Sharing(error)) => Err(error.message(&entry.address)),
        }
      }
    },
  )
}

/// The project's server, for a change of it.
async fn project_server(servers: &Servers, root: &Path) -> Result<ProjectServer, Message> {
  let (servers, root) = (servers.clone(), root.to_path_buf());
  background::run(move || ProjectServer::open(&servers, &root))
    .await?
    .map_err(|error| error_message(&error, None))?
    .ok_or_else(|| Message::new("sharing.errors.notShared"))
}

fn error_message(error: &SharingError, server: Option<&ProjectServer>) -> Message {
  match server {
    Some(server) => error.message(&server.entry().address),
    None => Message::new("sharing.errors.unavailable"),
  }
}

fn member_action(ctx: &mut Ctx, servers: &Servers, root: &Path) -> MemberAction {
  let (servers, root) = (servers.clone(), root.to_path_buf());
  ctx.future_action(move |work: MemberWork| {
    let (servers, root) = (servers.clone(), root.clone());
    async move {
      let server = project_server(&servers, &root).await?;
      let done = match work {
        MemberWork::Add { username, role } => server.add_member(username, role).await.map(drop),
        MemberWork::Change(change) => server.change_member(change).await,
      };
      done.map_err(|error| error_message(&error, Some(&server)))
    }
  })
}

fn local_action(ctx: &mut Ctx, servers: &Servers, root: &Path) -> LocalAction {
  let (servers, root) = (servers.clone(), root.to_path_buf());
  ctx.future_action(move |()| {
    let (servers, root) = (servers.clone(), root.clone());
    async move {
      let server = project_server(&servers, &root).await?;
      server
        .make_local()
        .await
        .map(|returned| MadeLocal(returned.into()))
        .map_err(|error| error_message(&error, Some(&server)))
    }
  })
}

fn discovery_action(ctx: &mut Ctx, servers: &Servers, root: &Path) -> DiscoveryAction {
  let (servers, root) = (servers.clone(), root.to_path_buf());
  ctx.future_action(move |()| {
    let (servers, root) = (servers.clone(), root.clone());
    async move {
      let server = project_server(&servers, &root).await?;
      let url = server.entry().address.as_str().to_owned();
      background::run(move || discovery::write(&root, &url))
        .await?
        .map_err(|error| labels::discovery_error(&error))
    }
  })
}
