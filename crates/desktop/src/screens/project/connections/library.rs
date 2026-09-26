//! The connections of the selected tab (`Bc9cT`, `gQDwZ`) beside the selected one's details
//! (`ez6IT`, `l8BJVr`), or what to add when the tab has none.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, GitHubConnection},
  id::ConnectionId,
};

use super::{
  ConnectionsScreen, DialogMode, PickAction, PickTarget, View,
  actions::{self, Work, WorkAction},
  centered,
  github_inspector::{GitHubActions, GitHubInspector},
  inspector::{InspectorActions, InspectorState, inspector},
  list::{RowState, connection_row, github_row},
};
use crate::{
  activity::operations,
  connections::data::ProjectConnections,
  messages::Message,
  navigation::route::{AgentSelection, ConnectionTab, Page},
  theme::ExtraTypography,
  ui::{
    field::{ValueTone, value_box_sized},
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
}

impl Listed<'_> {
  fn id(self) -> ConnectionId {
    match self {
      Self::Agent(connection) => connection.id(),
      Self::GitHub(connection) => connection.id(),
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
        };
        let actions = self.agent_actions(open, connection, work, busy.on.is_some());
        inspector(ctx, connection, &state, &actions)
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
        }
        .render(ctx, &actions)
      }
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(list_column(ctx, open, rows))
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
    let nav = self.nav.clone();
    let select = move || nav.show(view.page(Some(id)));
    match listed {
      Listed::Agent(connection) => {
        let state = RowState {
          uses: data.used_by(id).count(),
          selected: id == selected,
          checking: working == Some(id),
        };
        connection_row(ctx, connection, &state, select)
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
    }
  }

  /// No connections in the tab: what the tab is for, and adding one.
  fn empty(&self, ctx: &Ctx, tab: ConnectionTab) -> Element {
    let (title, detail) = match tab {
      ConnectionTab::All => ("connections.emptyTitle", "connections.emptyDetail"),
      ConnectionTab::AgentProviders => ("connections.emptyProvidersTitle", "connections.emptyDetail"),
      ConnectionTab::GitHub => ("github.emptyTitle", "github.emptyDetail"),
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
    let nav = self.nav.clone();
    InspectorActions {
      check,
      edit: Arc::new(move || dialog.set(DialogMode::EditAgent(editing.clone()))),
      show_profile: Arc::new(move |profile| nav.show(Page::Agents(AgentSelection::Profile(profile)))),
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

/// The connections the tab lists: GitHub first, as the design orders them, then agent providers.
fn listed(data: &ProjectConnections, tab: ConnectionTab) -> Vec<Listed<'_>> {
  let github = data.github.iter().map(Listed::GitHub);
  let agents = data.connections.iter().map(Listed::Agent);
  match tab {
    ConnectionTab::All => github.chain(agents).collect(),
    ConnectionTab::AgentProviders => agents.collect(),
    ConnectionTab::GitHub => github.collect(),
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
