//! Project settings › Connections (`05 Connections` `dk46o`, nothing yet `07` `WYCDL`): the project's agent providers,
//! GitHub and MCP servers as groups of rows, each row opening its connection's page (`05b` `PGwka`,
//! [`super::inspector`]); Add provider and Add MCP server at their groups' heads; an MCP server's switch in its row,
//! its question asked in place under the group. Its other states: the connections being read, and reading failing.
//! Work on a connection reads them all again once done.

mod empty;
mod rows;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::{id::ConnectionId, time::Timestamp};
use orchester_workspace::{
  activity::operations, connections::data::ProjectConnections, messages::Message, tasks::outside::Outside,
};
use parking_lot::Mutex;

use super::{
  data::{self, Entry},
  inspector::{self, Actions, Asks, Confirm, Inspecting, Tested},
  work::{self, Start, Work, WorkAction},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route},
  screens::project_settings::kit,
  services::{
    Services,
    accounts::{self, Accounts},
  },
  ui::{
    message::message_text,
    state::{load_error, loading},
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

/// The last work started, on which connection and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LastOn(Option<(ConnectionId, Work, Timestamp)>);

impl DevtoolsInspectable for LastOn {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Asking(Option<(ConnectionId, Confirm)>);

impl DevtoolsInspectable for Asking {}

pub struct ConnectionsScreen {
  /// Counts the readings asked for: each piece of work done, and Try again.
  work: WorkAction,
  revision: Signal<u64>,
  running: Signal<Running>,
  /// The connection the last work ran on, whose failure its page says.
  last: Signal<LastOn>,
  asking: Signal<Asking>,
  /// A connection's name is edited in place of its page's title.
  renaming: Signal<bool>,
  /// A connection page's … menu.
  more: (Signal<bool>, ElementRef),
  /// A connection page's question asked in place, and the one its last render showed.
  question: (ElementRef, Mutex<Asking>),
  /// Counts the empty page's Check again: each looks for the CLIs again.
  recheck: Signal<u64>,
  /// What the tab showed last.
  shown: Mutex<Option<ConnectionsView>>,
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
      renaming: ctx.signal(false),
      more: (ctx.signal(false), ctx.element_ref()),
      question: (ctx.element_ref(), Mutex::new(Asking(None))),
      recheck: ctx.signal(0),
      shown: Mutex::new(None),
    };
    let work = screen.work.clone();
    screen.follow(ctx, &work);
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ListProps { project, view } = ctx.props::<ListProps>().clone();
    // Another page starts without the question, the rename or the menu of the one shown before.
    if self.shown.lock().replace(view).is_some_and(|before| before != view) {
      self.asking.set(Asking(None));
      self.renaming.set(false);
      self.more.0.set(false);
    }
    let work = self.work.clone();
    // Read again after a Try again press, and when automation changed the project's connections.
    let revision = self.revision.get() + project.feed.outside.get().of(Outside::Connections);
    let loaded = data::load(ctx, project.open.repo_root.clone(), revision);
    let accounts = accounts::listed(ctx);
    let Some(listed) = loaded.data else {
      return kit::scroll(match loaded.error {
        Some(error) if !loaded.is_pending() => self.failed(ctx, &error),
        _ => loading("connections-loading", &ctx.t("connections.reading")).into(),
      });
    };
    // Set only while the latest reading failed: `listed` is then an earlier one.
    let unread = loaded.error.as_ref();
    let chosen = view
      .selected()
      .and_then(|id| data::entries(&listed).into_iter().find(|entry| entry.id() == id));
    if let Some(entry) = chosen {
      return kit::scroll(self.page(ctx, &project, (&listed, &accounts), entry, (&work, unread)));
    }
    if data::entries(&listed).is_empty() {
      return kit::scroll(empty::page(ctx, &project, &self.recheck));
    }
    let asks = self.asks(&project, &listed);
    kit::scroll(rows::groups(
      ctx,
      &project,
      &listed,
      (&accounts, &asks),
      (self.running.get().0, self.asking.get().0),
    ))
  }
}

impl ConnectionsScreen {
  /// Reads the connections again once work on one is done, and leaves a removed server's page.
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
        && let Route::Project(project, ProjectPage::Connections(view)) = navigation.route_now()
        && view.selected() == Some(done.id)
      {
        navigation.replace(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default()),
        ));
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

  /// What starts work on a connection, listed in the status bar's activity.
  fn starter(
    &self,
    project: &ShownProject,
    listed: &ProjectConnections,
  ) -> Arc<dyn Fn(ConnectionId, Work) + Send + Sync> {
    let (work, running, last) = (self.work.clone(), self.running.clone(), self.last.clone());
    let open = project.open.clone();
    let names: Vec<(ConnectionId, String)> = data::entries(listed)
      .into_iter()
      .map(|entry| (entry.id(), entry.name().to_owned()))
      .collect();
    Arc::new(move |id: ConnectionId, piece: Work| {
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
      // The real clock, as the check's own time is: a check read back older than this is not this one's.
      last.set(LastOn(Some((id, piece, Timestamp::now()))));
      work.run(Start {
        project: open.project.id(),
        repo_root: open.repo_root.clone(),
        id,
        work: piece,
        spec,
      });
    })
  }

  /// What the list's switches and questions do, each on the server it is about.
  fn asks(&self, project: &ShownProject, listed: &ProjectConnections) -> rows::ListAsks {
    let start = self.starter(project, listed);
    let asking = self.asking.clone();
    Arc::new(move |id: ConnectionId| {
      let asking = asking.clone();
      Asks {
        start: start.clone(),
        confirm: Arc::new(move |confirm| asking.set(Asking(confirm.map(|confirm| (id, confirm))))),
      }
    })
  }

  /// The page of `entry`, with the work on it. `unread` is why the latest reading of the connections failed.
  fn page(
    &self,
    ctx: &mut Ctx,
    project: &ShownProject,
    (listed, accounts): (&ProjectConnections, &Accounts),
    entry: Entry<'_>,
    (work, unread): (&WorkAction, Option<&Message>),
  ) -> Element {
    let state = work.state().get();
    let id = entry.id();
    let running = self.running.get().0.filter(|(on, _)| *on == id).map(|(_, work)| work);
    let last = self.last.get().0.filter(|(on, ..)| *on == id);
    let error = state.error.clone().filter(|_| state.is_rejected() && running.is_none());
    let checked = last.filter(|(_, work, _)| *work == Work::Check);
    let tested = checked.map(|(_, _, started)| match (&error, unread) {
      _ if running.is_some() => Tested::Checking,
      (Some(error), _) => Tested::Failed(error),
      // Done, and the connections not read again yet: still reading them, or reading them failed.
      (None, unread) if entry.checked_at().is_none_or(|at| at < started) => {
        unread.map_or(Tested::Checking, Tested::Unread)
      }
      (None, _) => Tested::Checked,
    });
    let failed = error.as_ref().filter(|_| last.is_some() && checked.is_none());
    let asked = self.asking.get();
    let confirming = asked.0.filter(|(on, _)| *on == id).map(|(_, confirm)| confirm);
    let before = std::mem::replace(&mut *self.question.1.lock(), asked);
    let inspecting = Inspecting {
      listed,
      accounts,
      entry,
      open: &project.open,
      project: project.open.project.name().as_str(),
      running,
      failed,
      tested,
      confirming,
      reveal: confirming.is_some() && before != asked,
      now: Services::of(ctx).clock.now(),
    };
    let actions = self.actions(ctx, project, listed, id);
    inspector::page(ctx, &inspecting, &actions)
  }

  /// What the page of connection `inspected` does.
  fn actions(
    &self,
    ctx: &mut Ctx,
    project: &ShownProject,
    listed: &ProjectConnections,
    inspected: ConnectionId,
  ) -> Actions {
    let asking = self.asking.clone();
    let navigation = Navigation::of(ctx);
    let (project_id, edit) = (project.open.project.id(), navigation.clone());
    Actions {
      start: self.starter(project, listed),
      confirm: Arc::new(move |confirm| asking.set(Asking(confirm.map(|confirm| (inspected, confirm))))),
      edit: Arc::new(move |id| edit.to(Route::Project(project_id, ProjectPage::EditConnection(id)))),
      agents: navigation.to(Route::Project(project_id, ProjectPage::Agents)),
      back: navigation.to(Route::Project(
        project_id,
        ProjectPage::Connections(ConnectionsView::default()),
      )),
      renaming: self.renaming.clone(),
      more: self.more.clone(),
      question: self.question.0.clone(),
    }
  }
}
