//! Connections (04.01 `a4Lqb`): the project's connections with the selected one's details,
//! adding and editing an agent provider (04.02 `h6CmJ`) and checking a connection again when
//! it needs attention (04.A `RZUKo`). Only agent provider CLIs exist, so the repository and
//! MCP server tabs and the search are unavailable.

mod dialog;
mod form;
mod inspector;
mod list;
mod status;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Modal, ModalTarget, Row, ScrollVertical, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{connection::AgentConnection, id::ConnectionId, name::Name};

use self::{
  dialog::ConnectionDialog,
  inspector::{InspectorActions, InspectorState, inspector},
  list::{RowState, connection_row},
};
use super::{
  ProjectNav,
  page::{self, TOPBAR_BUTTON_HEIGHT},
};
use crate::{
  activity::{Activity, OperationSpec, operations},
  background,
  connections::data::{ProjectConnections, recheck_in},
  messages::{self, Message},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{ValueTone, value_box_sized},
    icon::Icon,
    message_text,
    notice::Notice,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Tab row and search heights (`rxo81`, `ZH6bm`).
const TAB_ROW_HEIGHT: f32 = 42.0;
const SEARCH_HEIGHT: f32 = 38.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionTab {
  All,
  AgentProviders,
}

impl lurq::app::component::DevtoolsInspectable for ConnectionTab {}

/// The connection whose details are shown; `None` shows the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Selected(Option<ConnectionId>);

impl lurq::app::component::DevtoolsInspectable for Selected {}

/// Whether the add or edit dialog is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogMode {
  Closed,
  Add,
  Edit(AgentConnection),
}

impl lurq::app::component::DevtoolsInspectable for DialogMode {}

/// Provided to the dialog: the project, and what happens when it closes or saves.
#[derive(Clone)]
pub struct DialogContext {
  pub project: OpenProject,
  pub mode: DialogMode,
  /// Names of the project's other connections.
  pub taken: Vec<Name>,
  pub close: Arc<dyn Fn() + Send + Sync>,
  pub saved: Arc<dyn Fn(ConnectionId) + Send + Sync>,
}

/// The connection that was checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rechecked(ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Rechecked {}

type RecheckAction = FutureAction<(PathBuf, ConnectionId, OperationSpec), Rechecked, Message>;

enum Loaded {
  Loading,
  Ready(ProjectConnections),
  Failed(Message),
}

pub struct ConnectionsScreen {
  tab: Signal<ConnectionTab>,
  selected: Signal<Selected>,
  dialog: Signal<DialogMode>,
  /// The connection whose check runs, while the recheck action is pending.
  checking: Signal<Selected>,
  /// Bumped after a change, to read the connections again.
  revision: Signal<u64>,
  nav: ProjectNav,
}

impl Component for ConnectionsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let revision = ctx.signal(0_u64);
    let recheck = recheck_action(ctx);
    let reload = revision.clone();
    ctx.watch(&recheck.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        reload.update(|revision| *revision += 1);
      }
    });
    Self {
      tab: ctx.signal(ConnectionTab::All),
      selected: ctx.signal(Selected(None)),
      dialog: ctx.signal(DialogMode::Closed),
      checking: ctx.signal(Selected(None)),
      revision,
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let recheck = recheck_action(ctx);
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
      Loaded::Ready(data) if data.connections.is_empty() => self.empty(ctx),
      Loaded::Ready(data) => self.library(ctx, &open, data, &recheck),
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
      .child(self.dialog(ctx, &open, &loaded))
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

  fn add_button(&self, ctx: &Ctx, id: &'static str) -> Element {
    let dialog = self.dialog.clone();
    ActionButton::new(ctx.t("connections.add"))
      .primary()
      .icon(Icon::Plus)
      .id(id)
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
      .on_click(move || dialog.set(DialogMode::Add))
      .render(ctx)
  }

  /// Section tabs (`rxo81`): every connection is an agent provider so far.
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
        Tab::new(
          "connections-tab-repositories",
          ctx.t("connections.tabs.repositories"),
          height,
        ),
        Tab::new("connections-tab-mcp", ctx.t("connections.tabs.mcpServers"), height),
      ],
    )
  }

  /// No connections yet.
  fn empty(&self, ctx: &Ctx) -> Element {
    centered(
      RecoveryPanel::new(
        Icon::Plug,
        &ctx.t("connections.emptyTitle"),
        &ctx.t("connections.emptyDetail"),
      )
      .action(self.add_button(ctx, "empty-add-connection"))
      .into(),
    )
  }

  /// The list (`Bc9cT`) beside the selected connection's details (`ez6IT`).
  fn library(&self, ctx: &Ctx, open: &OpenProject, data: &ProjectConnections, recheck: &RecheckAction) -> Element {
    let selected = self
      .selected
      .get()
      .0
      .and_then(|id| data.connection(id))
      .or_else(|| data.connections.first())
      .expect("the library is shown only with connections");
    let checking = recheck.state().get();
    let checking_id = if checking.is_pending() {
      self.checking.get().0
    } else {
      None
    };
    let rows: Vec<Element> = data
      .connections
      .iter()
      .map(|connection| {
        let state = RowState {
          uses: data.used_by(connection.id()).count(),
          selected: connection.id() == selected.id(),
          checking: checking_id == Some(connection.id()),
        };
        let (signal, id) = (self.selected.clone(), connection.id());
        connection_row(ctx, connection, &state, move || signal.set(Selected(Some(id))))
      })
      .collect();
    let state = InspectorState {
      uses: data.used_by(selected.id()).collect(),
      checking: checking_id == Some(selected.id()),
      check_error: checking.error.clone().filter(|_| !checking.is_pending()),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(list_column(ctx, open, rows))
      .child(inspector(
        ctx,
        selected,
        &state,
        &self.actions(open, selected, recheck, state.checking),
      ))
      .into()
  }

  fn actions(
    &self,
    open: &OpenProject,
    connection: &AgentConnection,
    recheck: &RecheckAction,
    checking: bool,
  ) -> InspectorActions {
    let check = (!checking).then(|| {
      let (recheck, repo_root, id) = (recheck.clone(), open.repo_root.clone(), connection.id());
      let spec = operations::connection_check(open, connection.name().as_str());
      let checking = self.checking.clone();
      Arc::new(move || {
        checking.set(Selected(Some(id)));
        recheck.run((repo_root.clone(), id, spec.clone()));
      }) as Arc<dyn Fn() + Send + Sync>
    });
    let (dialog, editing) = (self.dialog.clone(), connection.clone());
    let nav = self.nav.clone();
    InspectorActions {
      check,
      edit: Arc::new(move || dialog.set(DialogMode::Edit(editing.clone()))),
      show_profile: Arc::new(move |profile| nav.show_profile(profile)),
    }
  }

  /// The add or edit dialog (`IAN4P`), over a backdrop.
  fn dialog(&self, ctx: &mut Ctx, open: &OpenProject, loaded: &Loaded) -> Element {
    let mode = self.dialog.get();
    let key = match &mode {
      DialogMode::Closed => return Modal::new(Column::new()).open_when(false).into(),
      DialogMode::Add => "connection-dialog-add".to_owned(),
      DialogMode::Edit(connection) => format!("connection-dialog-{}", connection.id()),
    };
    let editing = match &mode {
      DialogMode::Edit(connection) => Some(connection.id()),
      DialogMode::Add | DialogMode::Closed => None,
    };
    let taken = match loaded {
      Loaded::Ready(data) => data
        .connections
        .iter()
        .filter(|connection| Some(connection.id()) != editing)
        .map(|connection| connection.name().clone())
        .collect(),
      Loaded::Loading | Loaded::Failed(_) => Vec::new(),
    };
    let (close, saved) = (self.dialog.clone(), self.dialog.clone());
    let (selected, revision) = (self.selected.clone(), self.revision.clone());
    ctx.provide(DialogContext {
      project: open.clone(),
      mode,
      taken,
      close: Arc::new(move || close.set(DialogMode::Closed)),
      saved: Arc::new(move |id| {
        selected.set(Selected(Some(id)));
        revision.update(|revision| *revision += 1);
        saved.set(DialogMode::Closed);
      }),
    });
    Modal::new(ctx.mount_keyed::<ConnectionDialog>(&key, ()))
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
      .into()
  }
}

/// Search, rows, the provider notice and who owns the connections (`Bc9cT`).
fn list_column(ctx: &Ctx, open: &OpenProject, rows: Vec<Element>) -> Element {
  let search = value_box_sized(
    ctx,
    "connection-search",
    &ctx.t("connections.search"),
    ValueTone::Unavailable,
    None,
    Dimension::Px(SEARCH_HEIGHT),
  );
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .with_children(rows)
    .child(Notice::new(
      Icon::Workflow,
      ctx.t("connections.mixTitle"),
      ctx.t("connections.mixDetail"),
    ));
  Column::new()
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(SpacingSize::Md)
    .child(search)
    .child(
      ScrollVertical::new(content)
        .id("connection-list")
        .width(Dimension::Pct(100.0))
        .flex(1.0),
    )
    .child(
      Text::new(&ctx.t_args("connections.ownership", [("project", open.project.name().as_str())]))
        .variant(ExtraTypography::Detail.role())
        .color(PaletteColor::TextSecondary),
    )
    .into()
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

fn recheck_action(ctx: &mut Ctx) -> RecheckAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, id, spec): (PathBuf, ConnectionId, OperationSpec)| {
    let activity = activity.clone();
    async move {
      let check = async move {
        let checked = background::run(move || recheck_in(&repo_root, id)).await?;
        checked.map(|()| Rechecked(id)).map_err(|error| {
          tracing::error!(%error, "cannot store the connection check");
          messages::store_error(&error)
        })
      };
      activity.track(spec, check).await
    }
  })
}
