//! The connections of the selected tab (`Bc9cT`, `gQDwZ`) beside the selected one's details
//! (`JuVaA`, `C0jjP`, an MCP server's as `OXa0o` in 04.D `QdLRd`), or what to add when the tab has none.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, ApiEndpoint, GitHubConnection, mcp::McpServer},
  id::ConnectionId,
};

use super::{
  ConnectionsScreen, DialogMode, PickAction, PickTarget, View,
  actions::{self, Work, WorkAction},
  centered,
  github_inspector::{GitHubActions, GitHubInspector},
  inspector::{InspectorActions, InspectorState, inspector},
  list::{RowState, connection_row, github_row, mcp_row},
  mcp_inspector::{McpActions, McpInspector},
};
use crate::{
  activity::operations,
  connections::data::ProjectConnections,
  messages::Message,
  navigation::route::{AgentSelection, ConnectionTab, Page},
  theme::{ExtraTypography, gutter_scrollbar},
  ui::{
    field::{ValueTone, value_box_sized},
    gutter::{beside_gutter, clear_of_gutter},
    icon::Icon,
    notice::Notice,
    panel::RecoveryPanel,
  },
  workspace::OpenProject,
};

/// Search height (`ZH6bm`).
const SEARCH_HEIGHT: f32 = 38.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// A connection in the list, of either kind.
#[derive(Clone, Copy)]
enum Listed<'a> {
  Agent(&'a AgentConnection),
  GitHub(&'a GitHubConnection),
  Mcp(&'a McpServer),
}

impl Listed<'_> {
  fn id(self) -> ConnectionId {
    match self {
      Self::Agent(connection) => connection.id(),
      Self::GitHub(connection) => connection.id(),
      Self::Mcp(server) => server.id(),
    }
  }
}

/// What the work action is doing to which connection.
struct Busy {
  /// The connection work runs on, while it runs.
  on: Option<ConnectionId>,
  /// Why the last work failed.
  error: Option<Message>,
}

impl ConnectionsScreen {
  pub(super) fn library(
    &self,
    ctx: &Ctx,
    open: &OpenProject,
    data: &ProjectConnections,
    (work, pick): (&WorkAction, &PickAction),
    view: View,
  ) -> Element {
    let listed = listed(data, view.tab);
    let Some(selected) = listed
      .iter()
      .copied()
      .find(|listed| Some(listed.id()) == view.selected)
      .or_else(|| listed.first().copied())
    else {
      return self.empty(ctx, view.tab);
    };
    let state = work.state().get();
    let busy = Busy {
      on: if state.is_pending() { self.working.get().0 } else { None },
      error: state.error.clone().filter(|_| !state.is_pending()),
    };
    let rows: Vec<Element> = listed
      .iter()
      .map(|listed| self.row(ctx, data, *listed, (view, selected.id()), busy.on))
      .collect();
    let details = match selected {
      Listed::Agent(connection) => {
        let state = InspectorState {
          uses: data.used_by(connection.id()).collect(),
          checking: busy.on == Some(connection.id()),
          check_error: busy.error,
          launch: data.launch_of(connection.id()),
          secrets: &data.secrets,
        };
        let actions = self.agent_actions(open, connection, work, busy.on.is_some());
        inspector(ctx, connection, (&state, &self.scrolls.inspector), &actions)
      }
      Listed::GitHub(connection) => {
        let actions = self.github_actions(ctx, open, connection, work, pick);
        GitHubInspector {
          connection,
          link: data.link_of(connection.id()),
          project: open.project.name().as_str(),
          checking: busy.on == Some(connection.id()),
          error: busy.error,
          copied: &self.copied,
          launch: data.launch_of(connection.id()),
          scroll: &self.scrolls.inspector,
        }
        .render(ctx, &actions)
      }
      Listed::Mcp(server) => McpInspector {
        server,
        uses: data.used_by(server.id()).collect(),
        secrets: &data.secrets,
        working: busy.on == Some(server.id()),
        error: busy.error,
        scroll: &self.scrolls.inspector,
      }
      .render(ctx, &self.mcp_actions(open, server, work, busy.on.is_some())),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(list_column(
        ctx,
        open,
        (rows, &self.scrolls.list),
        view.tab != ConnectionTab::McpServers,
      ))
      .child(details)
      .into()
  }

  fn row(
    &self,
    ctx: &Ctx,
    data: &ProjectConnections,
    listed: Listed,
    (view, selected): (View, ConnectionId),
    working: Option<ConnectionId>,
  ) -> Element {
    let id = listed.id();
    let select = self.nav.link(view.page(Some(id)));
    match listed {
      Listed::Agent(connection) => {
        let state = RowState {
          uses: data.used_by(id).count(),
          selected: id == selected,
          checking: working == Some(id),
        };
        connection_row(ctx, connection, (&state, &data.secrets), select)
      }
      Listed::GitHub(connection) => {
        let link = data.link_of(id);
        let state = RowState {
          uses: usize::from(link.is_some()),
          selected: id == selected,
          checking: working == Some(id),
        };
        let github = connection.state(link.map(|link| link.expected_login()));
        github_row(ctx, connection, &github, &state, select)
      }
      Listed::Mcp(server) => {
        let state = RowState {
          uses: data.used_by(id).count(),
          selected: id == selected,
          checking: false,
        };
        mcp_row(ctx, server, &state, select)
      }
    }
  }

  /// No connections in the tab: what the tab is for, and adding one.
  fn empty(&self, ctx: &Ctx, tab: ConnectionTab) -> Element {
    let (title, detail) = match tab {
      ConnectionTab::All => ("connections.emptyTitle", "connections.emptyDetail"),
      ConnectionTab::AgentProviders => ("connections.emptyProvidersTitle", "connections.emptyDetail"),
      ConnectionTab::GitHub => ("github.emptyTitle", "github.emptyDetail"),
      ConnectionTab::McpServers => ("connections.mcp.emptyTitle", "connections.mcp.emptyDetail"),
    };
    let glyph = if tab == ConnectionTab::GitHub {
      Icon::GitPullRequest
    } else {
      Icon::Plug
    };
    centered(
      RecoveryPanel::new(glyph, &ctx.t(title), &ctx.t(detail))
        .action(self.add_button(ctx, "empty-add-connection", tab))
        .into(),
    )
  }

  fn agent_actions(
    &self,
    open: &OpenProject,
    connection: &AgentConnection,
    work: &WorkAction,
    busy: bool,
  ) -> InspectorActions {
    let (nav, secrets_nav) = (self.nav.clone(), self.nav.clone());
    let key = connection.api().map(ApiEndpoint::key);
    let profile = Arc::new(move |profile| nav.link(Page::Agents(AgentSelection::Profile(profile))));
    let secrets = Arc::new(move || secrets_nav.link(Page::Secrets(key)));
    // This version cannot test a native connection; the inspector says why.
    if connection.provider().is_native() {
      let (edit, models, editing) = (self.dialog.clone(), self.dialog.clone(), connection.clone());
      let models_of = connection.clone();
      return InspectorActions {
        check: None,
        edit: Some(Arc::new(move || edit.set(DialogMode::EditNative(editing.clone())))),
        edit_models: Some(Arc::new(move || {
          models.set(DialogMode::NativeModels(models_of.clone()))
        })),
        profile,
        secrets,
      };
    }
    let check = (!busy).then(|| {
      let spec = operations::connection_check(open, connection.name().as_str());
      actions::start(
        work,
        &self.working,
        &open.repo_root,
        connection.id(),
        Work::Recheck,
        spec,
      )
    });
    let (dialog, editing) = (self.dialog.clone(), connection.clone());
    InspectorActions {
      check,
      edit: Some(Arc::new(move || dialog.set(DialogMode::EditAgent(editing.clone())))),
      edit_models: None,
      profile,
      secrets,
    }
  }

  fn mcp_actions(&self, open: &OpenProject, server: &McpServer, work: &WorkAction, busy: bool) -> McpActions {
    let toggle = (!busy).then(|| {
      let spec = operations::mcp_server_switch(open, server.name().as_str(), !server.enabled());
      let work_kind = Work::SetEnabled(!server.enabled());
      actions::start(work, &self.working, &open.repo_root, server.id(), work_kind, spec)
    });
    let (dialog, editing, nav) = (self.dialog.clone(), server.clone(), self.nav.clone());
    McpActions {
      toggle,
      edit: Arc::new(move || dialog.set(DialogMode::EditMcp(editing.clone()))),
      profile: Arc::new(move |profile| nav.link(Page::Agents(AgentSelection::Profile(profile)))),
    }
  }

  fn github_actions(
    &self,
    ctx: &Ctx,
    open: &OpenProject,
    connection: &GitHubConnection,
    work: &WorkAction,
    pick: &PickAction,
  ) -> GitHubActions {
    let busy = work.state().get().is_pending();
    let (id, name) = (connection.id(), connection.name().as_str());
    let start = |work_kind: Work, spec| actions::start(work, &self.working, &open.repo_root, id, work_kind, spec);
    let check = (!busy).then(|| start(Work::Recheck, operations::connection_check(open, name)));
    let use_account = (!busy).then(|| start(Work::UseActiveAccount, operations::expected_account(open, name)));
    let choose_program = (!busy).then(|| {
      let (pick, picking) = (pick.clone(), self.picking.clone());
      let title = ctx.t("github.dialog.chooseProgram");
      let target = PickTarget {
        repo_root: open.repo_root.clone(),
        id,
        spec: operations::connection_save(open, name),
      };
      Arc::new(move || {
        picking.set(Some(target.clone()));
        pick.run(title.clone());
      }) as Handler
    });
    let (account, edit, editing) = (self.dialog.clone(), self.dialog.clone(), connection.clone());
    GitHubActions {
      check,
      change_account: Arc::new(move || account.set(DialogMode::Account(id))),
      choose_program,
      use_account,
      edit: Arc::new(move || edit.set(DialogMode::EditGitHub(editing.clone()))),
    }
  }
}

/// The connections the tab lists: GitHub first, as the design orders them, then agent providers, then MCP
/// servers.
fn listed(data: &ProjectConnections, tab: ConnectionTab) -> Vec<Listed<'_>> {
  let github = data.github.iter().map(Listed::GitHub);
  let agents = data.connections.iter().map(Listed::Agent);
  let mcp = data.mcp.iter().map(Listed::Mcp);
  match tab {
    ConnectionTab::All => github.chain(agents).chain(mcp).collect(),
    ConnectionTab::AgentProviders => agents.collect(),
    ConnectionTab::GitHub => github.collect(),
    ConnectionTab::McpServers => mcp.collect(),
  }
}

/// Search, rows, the provider notice (not on the MCP servers tab, where it does not apply) and who owns
/// the connections (`Bc9cT`).
fn list_column(
  ctx: &Ctx,
  open: &OpenProject,
  (rows, scroll): (Vec<Element>, &ScrollState),
  provider_notice: bool,
) -> Element {
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
    .with_children(provider_notice.then(|| {
      Notice::new(
        Icon::Workflow,
        ctx.t("connections.mixTitle"),
        ctx.t("connections.mixDetail"),
      )
    }));
  // The rows scroll beside the gutter; the search and the ownership line keep out of it.
  Column::new()
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .padding_right(beside_gutter(ctx, SpacingSize::Section))
    .spacing(SpacingSize::Md)
    .child(clear_of_gutter(search))
    .child(
      ScrollVertical::new(content)
        .id("connection-list")
        .with_scroll_state(scroll.clone())
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(clear_of_gutter(
      Text::new(&ctx.t_args("connections.ownership", [("project", open.project.name().as_str())]))
        .variant(ExtraTypography::Detail.role())
        .color(PaletteColor::TextSecondary),
    ))
    .into()
}
