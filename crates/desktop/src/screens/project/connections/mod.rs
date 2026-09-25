//! Connections (04.01 `a4Lqb`): the project's connections with the selected one's details,
//! adding and editing an agent provider (04.02 `h6CmJ`) or a GitHub connection (04.06
//! `D4mDB`), GitHub's details and states (04.05 `CHa9s`, 04.C1–C5) with the account panel
//! (01.D `hNPA1`), and checking a connection again when it needs attention (04.A `RZUKo`).
//! MCP servers and the search are not available yet.

mod actions;
mod dialog;
mod dialog_frame;
mod form;
mod github_dialog;
mod github_form;
mod github_inspector;
mod inspector;
mod library;
mod list;
mod status;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Modal, ModalTarget},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, GitHubConnection},
  id::ConnectionId,
  name::Name,
  repository::RepositoryLink,
};

use self::{
  actions::{Work, WorkAction, work_action},
  dialog::{ConnectionDialog, PickedFile, pick_action},
  github_dialog::GitHubDialog,
};
use super::{
  super::github::account_panel::{AccountPanel, AccountPanelProps},
  ProjectNav,
  page::{self, TOPBAR_BUTTON_HEIGHT},
};
use crate::{
  activity::{OperationSpec, operations},
  background,
  connections::data::ProjectConnections,
  messages::{self, Message},
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    command_row::Copied,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Tab row height (`rxo81`).
const TAB_ROW_HEIGHT: f32 = 42.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionTab {
  All,
  AgentProviders,
  GitHub,
}

impl lurq::app::component::DevtoolsInspectable for ConnectionTab {}

/// The connection whose details are shown; `None` shows the first listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selected(pub Option<ConnectionId>);

impl lurq::app::component::DevtoolsInspectable for Selected {}

/// The type chosen in a new connection's Connection type select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindChoice {
  Agent,
  GitHub,
}

impl lurq::app::component::DevtoolsInspectable for KindChoice {}

/// Which dialog is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogMode {
  Closed,
  AddAgent,
  AddGitHub,
  EditAgent(AgentConnection),
  EditGitHub(GitHubConnection),
  /// Change GitHub account or access, for this connection.
  Account(ConnectionId),
}

impl lurq::app::component::DevtoolsInspectable for DialogMode {}

/// Provided to the add and edit dialogs: the project, and what happens when they close or save.
#[derive(Clone)]
pub struct DialogContext {
  pub project: OpenProject,
  pub mode: DialogMode,
  /// Names of the project's other connections.
  pub taken: Vec<Name>,
  /// The project's repository link, if it has one.
  pub link: Option<Arc<RepositoryLink>>,
  pub close: Arc<dyn Fn() + Send + Sync>,
  pub saved: Arc<dyn Fn(ConnectionId) + Send + Sync>,
  /// Switches a new connection to the other type's dialog.
  pub retype: Arc<dyn Fn(KindChoice) + Send + Sync>,
}

type PickAction = FutureAction<Arc<str>, PickedFile, Message>;

/// The connection whose program file is being chosen, and how storing it is reported.
#[derive(Clone, Debug, PartialEq)]
struct PickTarget {
  repo_root: PathBuf,
  id: ConnectionId,
  spec: OperationSpec,
}

impl lurq::app::component::DevtoolsInspectable for PickTarget {}

enum Loaded {
  Loading,
  Ready(ProjectConnections),
  Failed(Message),
}

pub struct ConnectionsScreen {
  tab: Signal<ConnectionTab>,
  selected: Signal<Selected>,
  dialog: Signal<DialogMode>,
  /// The connection work runs on, while the work action is pending.
  working: Signal<Selected>,
  /// The connection a program file is being chosen for.
  picking: Signal<Option<PickTarget>>,
  /// The last command copied from the inspector.
  copied: Signal<Copied>,
  /// Bumped after a change, to read the connections again.
  revision: Signal<u64>,
  nav: ProjectNav,
}

impl Component for ConnectionsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let revision = ctx.signal(0_u64);
    let work = work_action(ctx);
    let pick = pick_action(ctx);
    let reload = revision.clone();
    ctx.watch(&work.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        reload.update(|revision| *revision += 1);
      }
    });
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let dialog = if nav.take_add_github() {
      DialogMode::AddGitHub
    } else {
      DialogMode::Closed
    };
    let screen = Self {
      tab: ctx.signal(ConnectionTab::All),
      selected: ctx.signal(Selected(None)),
      dialog: ctx.signal(dialog),
      working: ctx.signal(Selected(None)),
      picking: ctx.signal(None),
      copied: ctx.signal(Copied::default()),
      revision,
      nav,
    };
    screen.run_picked_program(ctx, &work, &pick);
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    // Futures are matched by call order: the actions come first, as in `create`.
    let work = work_action(ctx);
    let pick = pick_action(ctx);
    let loaded = self.load(ctx, &open);
    let body = match &loaded {
      Loaded::Loading => Column::new().flex(1.0).into(),
      Loaded::Failed(error) => centered(
        RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("connections.loadFailed"),
          &message_text(ctx, error),
        )
        .into(),
      ),
      Loaded::Ready(data) => self.library(ctx, &open, data, &work, &pick),
    };
    let breadcrumb = ctx.t_args("connections.breadcrumb", [("project", open.project.name().as_str())]);
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, [self.add_button(ctx, "add-connection")]))
      .child(page::heading(
        &ctx.t("connections.title"),
        &ctx.t("connections.description"),
      ))
      .child(self.tabs(ctx))
      .child(body)
      .child(self.dialog(ctx, &open, &loaded, &work))
  }
}

impl ConnectionsScreen {
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) -> Loaded {
    let deps = (open.repo_root.clone(), self.revision.get());
    let load = ctx.future(deps, |(repo_root, _)| async move {
      background::run(move || ProjectConnections::load_in(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's connections");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Loaded::Failed(error),
      (Some(data), None) => Loaded::Ready(data),
      (None, None) => Loaded::Loading,
    }
  }

  /// A program file chosen for a connection (gh not found) is stored and checked.
  fn run_picked_program(&self, ctx: &mut Ctx, work: &WorkAction, pick: &PickAction) {
    let (work, picking, working) = (work.clone(), self.picking.clone(), self.working.clone());
    ctx.watch(&pick.state(), move |state| {
      let (Some(PickedFile(Some(path))), Some(target)) = (&state.data, picking.get_untracked()) else {
        return;
      };
      picking.set(None);
      working.set(Selected(Some(target.id)));
      work.run((target.repo_root, target.id, Work::Program(path.clone()), target.spec));
    });
  }

  fn add_button(&self, ctx: &Ctx, id: &'static str) -> Element {
    let (dialog, tab) = (self.dialog.clone(), self.tab.clone());
    ActionButton::new(ctx.t("connections.add"))
      .primary()
      .icon(Icon::Plus)
      .id(id)
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
      .on_click(move || {
        dialog.set(match tab.get_untracked() {
          ConnectionTab::GitHub => DialogMode::AddGitHub,
          ConnectionTab::All | ConnectionTab::AgentProviders => DialogMode::AddAgent,
        });
      })
      .render(ctx)
  }

  /// Section tabs (`rxo81`, `BCZsv`): MCP servers are not available yet.
  fn tabs(&self, ctx: &Ctx) -> Element {
    let selected = self.tab.get();
    let height = Dimension::Pct(100.0);
    let available = |id, key, tab: ConnectionTab| {
      let signal = self.tab.clone();
      Tab::new(id, ctx.t(key), height)
        .selected(selected == tab)
        .on_click(move || signal.set(tab))
    };
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      ExtraSpacing::Step.role(),
      Some(SpacingSize::Section),
      [
        available("connections-tab-all", "connections.tabs.all", ConnectionTab::All),
        available(
          "connections-tab-providers",
          "connections.tabs.agentProviders",
          ConnectionTab::AgentProviders,
        ),
        available(
          "connections-tab-github",
          "connections.tabs.github",
          ConnectionTab::GitHub,
        ),
        Tab::new("connections-tab-mcp", ctx.t("connections.tabs.mcpServers"), height),
      ],
    )
  }

  /// The open dialog (`IAN4P`, `d8yPD5`, `hNPA1`), over a backdrop.
  fn dialog(&self, ctx: &mut Ctx, open: &OpenProject, loaded: &Loaded, work: &WorkAction) -> Element {
    let mode = self.dialog.get();
    let Loaded::Ready(data) = loaded else {
      return Modal::new(Column::new()).open_when(false).into();
    };
    let content = match &mode {
      DialogMode::Closed => return Modal::new(Column::new()).open_when(false).into(),
      DialogMode::Account(id) => match self.account_panel(ctx, open, data, *id, work) {
        Some(panel) => panel,
        None => return Modal::new(Column::new()).open_when(false).into(),
      },
      DialogMode::AddAgent | DialogMode::EditAgent(_) => {
        let key = dialog_key(&mode);
        self.provide_dialog_context(ctx, open, data, mode.clone());
        ctx.mount_keyed::<ConnectionDialog>(&key, ())
      }
      DialogMode::AddGitHub | DialogMode::EditGitHub(_) => {
        let key = dialog_key(&mode);
        self.provide_dialog_context(ctx, open, data, mode.clone());
        ctx.mount_keyed::<GitHubDialog>(&key, ())
      }
    };
    Modal::new(content)
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
      .into()
  }

  fn provide_dialog_context(&self, ctx: &mut Ctx, open: &OpenProject, data: &ProjectConnections, mode: DialogMode) {
    let editing = match &mode {
      DialogMode::EditAgent(connection) => Some(connection.id()),
      DialogMode::EditGitHub(connection) => Some(connection.id()),
      _ => None,
    };
    let taken = data
      .connections
      .iter()
      .map(|connection| (connection.id(), connection.name()))
      .chain(
        data
          .github
          .iter()
          .map(|connection| (connection.id(), connection.name())),
      )
      .filter(|(id, _)| Some(*id) != editing)
      .map(|(_, name)| name.clone())
      .collect();
    let (close, saved, retype) = (self.dialog.clone(), self.dialog.clone(), self.dialog.clone());
    let (selected, revision) = (self.selected.clone(), self.revision.clone());
    ctx.provide(DialogContext {
      project: open.clone(),
      mode,
      taken,
      link: data.link.clone(),
      close: Arc::new(move || close.set(DialogMode::Closed)),
      saved: Arc::new(move |id| {
        selected.set(Selected(Some(id)));
        revision.update(|revision| *revision += 1);
        saved.set(DialogMode::Closed);
      }),
      retype: Arc::new(move |kind| {
        retype.set(match kind {
          KindChoice::Agent => DialogMode::AddAgent,
          KindChoice::GitHub => DialogMode::AddGitHub,
        });
      }),
    });
  }

  /// Change GitHub account or access (01.D) for connection `id`, while it exists.
  fn account_panel(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    data: &ProjectConnections,
    id: ConnectionId,
    work: &WorkAction,
  ) -> Option<Element> {
    let connection = data.github_connection(id)?.clone();
    let busy = work.state().get().is_pending();
    let check = (!busy).then(|| {
      let spec = operations::connection_check(open, connection.name().as_str());
      actions::start(work, &self.working, &open.repo_root, id, Work::Recheck, spec)
    });
    let close = self.dialog.clone();
    let props = AccountPanelProps {
      expected: data.link_of(id).map(|link| link.expected_login().clone()),
      project: open.project.name().as_str().into(),
      check,
      close: Arc::new(move || close.set(DialogMode::Closed)),
      connection,
    };
    Some(ctx.mount_keyed::<AccountPanel>(&format!("github-account-{id}"), props))
  }
}

fn dialog_key(mode: &DialogMode) -> String {
  match mode {
    DialogMode::EditAgent(connection) => format!("connection-dialog-{}", connection.id()),
    DialogMode::EditGitHub(connection) => format!("github-dialog-{}", connection.id()),
    DialogMode::AddGitHub => "github-dialog-add".to_owned(),
    DialogMode::AddAgent | DialogMode::Closed | DialogMode::Account(_) => "connection-dialog-add".to_owned(),
  }
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}
