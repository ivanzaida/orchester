//! Connections as a list beside an inspector (`Connections · All connections · dark` `z4QquH`, `GitHub`
//! `cxjvN`, `MCP servers` `O6Vo2a`, `Agent providers` `cC6TI`): the header and tabs, the tab's connections
//! with a notice about them and a note on whose they are, and the inspector of the chosen one (the tab's
//! first until one is chosen). Its other states: no connection yet (`oK3aE`), a tab without connections,
//! the connections being read, and reading failing. Work on a connection reads them all again once done.

mod header;
mod table;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Spacer},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ConnectionId;
use orchester_workspace::{
  activity::operations, connections::data::ProjectConnections, messages::Message, tasks::outside::Outside,
};

use self::header::AddMenu;
use super::{
  data::{self, Entry},
  inspector::{self, Actions, Confirm, Inspecting},
  work::{self, Start, Work, WorkAction},
};
use crate::{
  navigation::{Navigation, press::OpenIn},
  project::ShownProject,
  routes::{ConnectionsTab, ConnectionsView, NewConnection, ProjectPage, Route, Screen},
  services::{
    Services,
    accounts::{self, Accounts},
  },
  theme::{ConnectionsSpacing, ExtraSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::note,
    message::message_text,
    notice::handling_notice,
    state::{EmptyState, load_error, loading},
    unavailable::reason_line,
  },
};

#[derive(Clone, PartialEq)]
pub struct ListProps {
  pub project: ShownProject,
  pub view: ConnectionsView,
}

impl DevtoolsInspectable for ListProps {}

/// The work running now, and on which connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Running(Option<(ConnectionId, Work)>);

impl DevtoolsInspectable for Running {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LastOn(Option<ConnectionId>);

impl DevtoolsInspectable for LastOn {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Asking(Option<(ConnectionId, Confirm)>);

impl DevtoolsInspectable for Asking {}

pub struct ConnectionsScreen {
  /// Counts the readings asked for: each piece of work done, and Try again.
  work: WorkAction,
  revision: Signal<u64>,
  running: Signal<Running>,
  /// The connection the last work ran on, whose failure its inspector says.
  last: Signal<LastOn>,
  asking: Signal<Asking>,
  menu: AddMenu,
}

impl Component for ConnectionsScreen {
  type Props = ListProps;

  fn create(ctx: &mut Ctx) -> Self {
    let screen = Self {
      work: work::action(ctx),
      revision: ctx.signal(0),
      running: ctx.signal(Running(None)),
      last: ctx.signal(LastOn(None)),
      asking: ctx.signal(Asking(None)),
      menu: AddMenu {
        open: ctx.signal(false),
        anchor: ctx.element_ref(),
      },
    };
    let work = screen.work.clone();
    screen.follow(ctx, &work);
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ListProps { project, view } = ctx.props::<ListProps>().clone();
    let work = self.work.clone();
    // Read again after a Try again press, and when automation changed the project's connections.
    let revision = self.revision.get() + project.feed.outside.get().of(Outside::Connections);
    let loaded = data::load(ctx, project.open.repo_root.clone(), revision);
    let accounts = accounts::listed(ctx);
    let id = project.open.project.id();
    let sides = if super::narrow(ctx) {
      ConnectionsSpacing::NarrowPage
    } else {
      ConnectionsSpacing::Page
    };
    let page = Column::new()
      .id(format!("screen-{}", Screen::Connections.name()))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Roomy.role())
          .bottom(SpacingSize::Xl)
          .left(sides.role())
          .right(sides.role()),
      )
      .spacing(SpacingSize::Lg)
      .child(header::header(ctx, id, view.tab(), &self.menu));
    let Some(listed) = loaded.data else {
      let page = page.child(header::tabs(ctx, id, view.tab(), None));
      return match loaded.error {
        Some(error) if !loaded.is_pending() => page.child(self.failed(ctx, &error)),
        _ => page.child(loading("connections-loading", &ctx.t("connections.reading"))),
      };
    };
    let counts = data::counts(&listed);
    let page = page.child(header::tabs(ctx, id, view.tab(), Some(counts)));
    if counts[0] == 0 && view.tab() == ConnectionsTab::All {
      return page.child(no_connections(ctx, &project));
    }
    page.child(self.body(ctx, (&project, view), (&listed, &accounts), &work))
  }
}

impl ConnectionsScreen {
  /// Reads the connections again once work on one is done, and leaves a removed server's inspector.
  fn follow(&self, ctx: &mut Ctx, work: &WorkAction) {
    let (revision, running) = (self.revision.clone(), self.running.clone());
    let navigation = Navigation::of(ctx);
    ctx.watch(&work.state(), move |state| {
      if state.is_pending() {
        return;
      }
      running.set(Running(None));
      // Work that was done is read again as the change it recorded (`work::action`); work that failed
      // changed nothing it says, and is read again in case it did.
      if !state.is_fulfilled() {
        revision.update(|revision| *revision += 1);
      }
      if let Some(done) = state.data.filter(|_| state.is_fulfilled())
        && done.work == Work::Remove
      {
        // The tab the screen shows now, which may not be the one it opened on.
        if let Route::Project(project, ProjectPage::Connections(view)) = navigation.route_now() {
          let view = ConnectionsView::new(view.tab());
          navigation.replace(Route::Project(project, ProjectPage::Connections(view)));
        }
      }
    });
  }

  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.revision.clone();
    let body = message_text(ctx, error);
    load_error(
      ctx,
      "connections-load-error",
      (&ctx.t("connections.loadFailed"), &body),
      move || revision.update(|revision| *revision += 1),
    )
  }

  /// The tab's connections and the inspector of the chosen one, or why the tab lists none.
  fn body(
    &self,
    ctx: &mut Ctx,
    (project, view): (&ShownProject, ConnectionsView),
    (listed, accounts): (&ProjectConnections, &Accounts),
    work: &WorkAction,
  ) -> Element {
    let entries = data::entries(listed, view.tab());
    let Some(first) = entries.first().copied() else {
      return empty_tab(ctx, project, view.tab());
    };
    let chosen = view
      .selected()
      .and_then(|id| entries.iter().copied().find(|entry| entry.id() == id))
      .unwrap_or(first);
    let open = self.open(ctx, project, view);
    let narrow = super::narrow(ctx);
    let list = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .spacing(ConnectionsSpacing::List.role())
      .child(table::table(
        ctx,
        (listed, accounts),
        &entries,
        (Some(chosen.id()), &open),
        narrow,
      ))
      .with_children(tab_notice(ctx, view.tab(), project, narrow))
      .child(Spacer::new().flex(1.0))
      .child(ownership(ctx, project));
    let inspector = self.inspector(ctx, project, (listed, accounts), chosen, work);
    Row::new()
      .id("connections-body")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .spacing(if narrow {
        ConnectionsSpacing::NarrowBody.role()
      } else {
        SpacingSize::Xl
      })
      .child(list)
      .child(inspector)
      .into()
  }

  /// What opens a connection's inspector, a place of its own, by a browser's rules (`navigation::link`); in the
  /// active tab it drops the question asked about the one shown.
  fn open(&self, ctx: &mut Ctx, project: &ShownProject, view: ConnectionsView) -> table::OpenConnection {
    let navigation = Navigation::of(ctx);
    let (id, asking) = (project.open.project.id(), self.asking.clone());
    Arc::new(move |connection| {
      let route = Route::Project(
        id,
        ProjectPage::Connections(ConnectionsView::new(view.tab()).with_selected(connection)),
      );
      let (navigation, asking) = (navigation.clone(), asking.clone());
      Arc::new(move |at: OpenIn| {
        if at == OpenIn::ActiveTab {
          asking.set(Asking(None));
        }
        navigation.open(route.clone(), at);
      })
    })
  }

  fn inspector(
    &self,
    ctx: &mut Ctx,
    project: &ShownProject,
    (listed, accounts): (&ProjectConnections, &Accounts),
    entry: Entry<'_>,
    work: &WorkAction,
  ) -> Element {
    let state = work.state().get();
    let id = entry.id();
    let running = self.running.get().0.filter(|(on, _)| *on == id).map(|(_, work)| work);
    let failed = state
      .error
      .clone()
      .filter(|_| state.is_rejected() && running.is_none() && self.last.get().0 == Some(id));
    let confirming = self
      .asking
      .get()
      .0
      .filter(|(on, _)| *on == id)
      .map(|(_, confirm)| confirm);
    let inspecting = Inspecting {
      listed,
      accounts,
      entry,
      project: project.open.project.name().as_str(),
      running,
      failed: failed.as_ref(),
      confirming,
      now: Services::of(ctx).clock.now(),
    };
    let actions = self.actions(ctx, project, listed, (id, work));
    inspector::inspector(ctx, &inspecting, &actions)
  }

  /// What the inspector of connection `inspected` does.
  fn actions(
    &self,
    ctx: &mut Ctx,
    project: &ShownProject,
    listed: &ProjectConnections,
    (inspected, work): (ConnectionId, &WorkAction),
  ) -> Actions {
    let (work, running, last) = (work.clone(), self.running.clone(), self.last.clone());
    let open = project.open.clone();
    let names: Vec<(ConnectionId, String)> = data::entries(listed, ConnectionsTab::All)
      .into_iter()
      .map(|entry| (entry.id(), entry.name().to_owned()))
      .collect();
    let start = Arc::new(move |id: ConnectionId, piece: Work| {
      let name = names
        .iter()
        .find(|(named, _)| *named == id)
        .map_or("", |(_, name)| name.as_str());
      let spec = match piece {
        Work::Check | Work::UseActiveAccount => operations::connection_check(&open, name),
        Work::SetEnabled(enabled) => operations::mcp_server_switch(&open, name, enabled),
        Work::Remove => operations::mcp_server_remove(&open, name),
        Work::SetAccount(_) => operations::connection_save(&open, name),
      };
      running.set(Running(Some((id, piece))));
      last.set(LastOn(Some(id)));
      work.run(Start {
        project: open.project.id(),
        repo_root: open.repo_root.clone(),
        id,
        work: piece,
        spec,
      });
    });
    let asking = self.asking.clone();
    let navigation = Navigation::of(ctx);
    let (project_id, edit) = (project.open.project.id(), navigation.clone());
    Actions {
      start,
      confirm: Arc::new(move |confirm| asking.set(Asking(confirm.map(|confirm| (inspected, confirm))))),
      edit: Arc::new(move |id| edit.to(Route::Project(project_id, ProjectPage::EditConnection(id)))),
      agents: navigation.to(Route::Project(project_id, ProjectPage::Agents)),
    }
  }
}

/// What the tab's kind is about (`Notice · Each agent can use a different provider` `oy0c2`), which a narrow
/// window has no room for (`List column` `d7H93k` in `b3aoUZ`). On GitHub, why its Add GitHub connection is
/// locked follows, in a narrow window too.
fn tab_notice(ctx: &Ctx, tab: ConnectionsTab, project: &ShownProject, narrow: bool) -> Option<Column> {
  let (glyph, key) = match tab {
    ConnectionsTab::All | ConnectionsTab::AgentProviders => (Icon::Workflow, "connections.notice.providers"),
    ConnectionsTab::GitHub => (Icon::GitPullRequest, "connections.notice.github"),
    ConnectionsTab::McpServers => (Icon::Server, "connections.notice.mcp"),
  };
  let name = project.open.project.name().as_str();
  let reason = (tab == ConnectionsTab::GitHub)
    .then(|| reason_line("connections-add-reason", &ctx.t("connections.github.addUnavailable")));
  if narrow && reason.is_none() {
    return None;
  }
  let notice = (!narrow).then(|| handling_notice("connections-notice", glyph, &ctx.t_args(key, [("project", name)])));
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(notice)
      .with_children(reason),
  )
}

/// Whose the connections are (`Note · Connections belong to Kontur.` `OVuKt`).
fn ownership(ctx: &Ctx, project: &ShownProject) -> Row {
  let name = project.open.project.name().as_str();
  note(
    "connections-ownership",
    Icon::Info,
    &ctx.t_args("connections.ownership", [("project", name)]),
  )
}

/// No connection at all (`No connections yet · All connections` in `oK3aE`): what a connection is, and
/// adding each kind.
fn no_connections(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let add = |kind: NewConnection| navigation.to(Route::Project(id, ProjectPage::NewConnection(kind)));
  let name = project.open.project.name().as_str();
  let empty = EmptyState::new(
    "connections-empty",
    Icon::Plug,
    ctx.t("connections.emptyTitle"),
    ctx.t_args("connections.emptyDetail", [("project", name)]),
  )
  .action(
    ActionButton::new(
      "connections-empty-agent",
      ButtonKind::Primary,
      ctx.t("connections.addAgent"),
    )
    .icon(Icon::Bot)
    .opens(add(NewConnection::AgentProvider)),
  )
  .action(
    ActionButton::new(
      "connections-empty-github",
      ButtonKind::Secondary,
      ctx.t("connections.github.add"),
    )
    .icon(Icon::GitPullRequest)
    .unavailable(ctx.t("connections.github.addUnavailable")),
  )
  .action(
    ActionButton::new(
      "connections-empty-mcp",
      ButtonKind::Secondary,
      ctx.t("connections.mcp.add"),
    )
    .icon(Icon::Server)
    .opens(add(NewConnection::McpServer)),
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(ConnectionsSpacing::List.role())
    .child(empty)
    .child(Spacer::new().flex(1.0))
    .child(ownership(ctx, project))
    .into()
}

/// A tab whose kind has no connection yet.
fn empty_tab(ctx: &Ctx, project: &ShownProject, tab: ConnectionsTab) -> Element {
  let (glyph, title, detail) = match tab {
    ConnectionsTab::GitHub => (
      Icon::GitPullRequest,
      "connections.github.emptyTitle",
      "connections.github.emptyDetail",
    ),
    ConnectionsTab::McpServers => (
      Icon::Server,
      "connections.mcp.emptyTitle",
      "connections.mcp.emptyDetail",
    ),
    ConnectionsTab::All | ConnectionsTab::AgentProviders => {
      (Icon::Bot, "connections.emptyProvidersTitle", "connections.emptyDetail")
    }
  };
  let name = project.open.project.name().as_str();
  let mut empty = EmptyState::new(
    "connections-empty-tab",
    glyph,
    ctx.t(title),
    ctx.t_args(detail, [("project", name)]),
  );
  if tab == ConnectionsTab::GitHub {
    empty = empty.child(reason_line(
      "connections-add-reason",
      &ctx.t("connections.github.addUnavailable"),
    ));
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(ConnectionsSpacing::List.role())
    .child(empty)
    .child(Spacer::new().flex(1.0))
    .child(ownership(ctx, project))
    .into()
}
