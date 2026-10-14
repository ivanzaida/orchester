//! Edit connection (the inspector's *Edit connection*, `M42rM`): for an MCP server its form (`mcp_form`); for an agent
//! CLI connection the account it runs on, chosen as in Add connection (`WaKJN`), with its name and program as they
//! are, which this app does not edit yet. Saving runs the connection on the chosen account and checks it there; a
//! paused run's next step then starts there as a new session, which the page says. Choosing the account is a
//! person's decision ([`super::account::AccountPicker`]).

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{connection::AgentConnection, id::ConnectionId};
use orchester_workspace::activity::operations;

use super::{
  account, data,
  mcp_form::{McpForm, McpFormProps},
  page::{Head, action_bar, page},
  work::{self, Start, Work, WorkAction},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route, Screen},
  services::accounts::listed,
  ui::{
    breadcrumb::Crumb,
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::{self, PropertyValue, property},
  },
};

#[derive(Clone, PartialEq)]
pub struct EditProps {
  pub project: ShownProject,
  pub connection: ConnectionId,
}

impl DevtoolsInspectable for EditProps {}

pub struct EditConnection {
  /// The account chosen, by id; empty until the connection is read.
  chosen: Signal<String>,
  work: WorkAction,
}

impl Component for EditConnection {
  type Props = EditProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditProps>().clone();
    let work = work::action(ctx);
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    ctx.watch(&work.state(), move |state| {
      if let Some(done) = state.data.filter(|_| state.is_fulfilled()) {
        navigation.go(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default().with_selected(done.id)),
        ));
      }
    });
    Self {
      chosen: ctx.signal(String::new()),
      work,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditProps>().clone();
    let listed = data::load(ctx, props.project.open.repo_root.clone(), 0).data;
    let Some(listed) = listed else {
      return Column::new()
        .id(format!("screen-{}", Screen::EditConnection.name()))
        .width(Dimension::Pct(100.0))
        .into();
    };
    let Some(connection) = listed
      .connections
      .iter()
      .find(|connection| connection.id() == props.connection)
      .cloned()
    else {
      return ctx.mount_keyed::<McpForm>(
        &format!(
          "connection-edit-mcp-{}-{}",
          props.project.open.project.id(),
          props.connection
        ),
        McpFormProps {
          project: props.project.clone(),
          server: Some(props.connection),
        },
      );
    };
    self.agent_page(ctx, &props, &connection)
  }
}

impl EditConnection {
  fn agent_page(&self, ctx: &mut Ctx, props: &EditProps, connection: &AgentConnection) -> Element {
    let accounts = listed(ctx);
    let provider = connection.provider();
    let current = accounts
      .of_connection(connection.account(), provider)
      .map(|account| account.id().to_string())
      .unwrap_or_default();
    if self.chosen.get().is_empty() && !current.is_empty() {
      self.chosen.set(current.clone());
    }
    let changed = !self.chosen.get().is_empty() && self.chosen.get() != current;
    let rows = Column::new()
      .id("connection-edit-properties")
      .width(Dimension::Pct(100.0))
      .child(property(
        "connection-edit-name",
        &ctx.t("connections.dialog.name"),
        connection.name().as_str(),
        PropertyValue::Plain,
      ))
      .child(property(
        "connection-edit-program",
        &ctx.t("connections.property.programFile"),
        &connection
          .executable()
          .map(|path| path.display().to_string())
          .unwrap_or_default(),
        PropertyValue::Mono,
      ));
    let sections = vec![
      rows.into(),
      account::add_row(ctx, &accounts, provider, &self.chosen),
      inspector::note(
        "connection-account-note",
        Icon::Info,
        &ctx.t("connections.account.changeNote"),
      )
      .into(),
    ];
    let head = self.head(ctx, props, connection, changed);
    let bar = self.bar(ctx, props, connection, changed);
    page(ctx, Screen::EditConnection, head, sections, bar)
  }

  fn head(&self, ctx: &mut Ctx, props: &EditProps, connection: &AgentConnection, changed: bool) -> Head {
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    let title = ctx.t_args(
      "connections.editAgent.title",
      [("connection", connection.name().as_str())],
    );
    let status = if changed {
      (
        Icon::CircleDashed,
        PaletteColor::TextMuted,
        ctx.t("connections.dialog.notSaved"),
      )
    } else {
      (
        Icon::CircleCheck,
        PaletteColor::Success,
        ctx.t("connections.editAgent.saved"),
      )
    };
    Head {
      crumbs: vec![
        Crumb::link(ctx.t("connections.title"), move || {
          navigation.go(Route::Project(
            project,
            ProjectPage::Connections(ConnectionsView::default()),
          ));
        }),
        Crumb::here(title.clone()),
      ],
      title,
      description: ctx.t("connections.editAgent.description"),
      status,
    }
  }

  /// *Cancel* back to the connection, and *Save*, which runs it on the chosen account.
  fn bar(&self, ctx: &mut Ctx, props: &EditProps, connection: &AgentConnection, changed: bool) -> Element {
    let navigation = Navigation::of(ctx);
    let (project, id) = (props.project.open.project.id(), connection.id());
    let cancel = ActionButton::new(
      "connection-edit-cancel",
      ButtonKind::Secondary,
      ctx.t("connections.cancel"),
    )
    .on_click(move || {
      navigation.go(Route::Project(
        project,
        ProjectPage::Connections(ConnectionsView::default().with_selected(id)),
      ));
    });
    let save = ActionButton::new(
      "connection-edit-save",
      ButtonKind::Primary,
      ctx.t("connections.editAgent.save"),
    );
    let account = self.chosen.get().parse().ok().filter(|_| changed);
    let save = match account {
      _ if self.work.state().get().is_pending() => save.loading(),
      Some(account) => {
        let (work, open, name) = (
          self.work.clone(),
          props.project.open.clone(),
          connection.name().as_str().to_owned(),
        );
        save.on_click(move || {
          work.run(Start {
            repo_root: open.repo_root.clone(),
            id,
            work: Work::SetAccount(account),
            spec: operations::connection_save(&open, &name),
          });
        })
      }
      None => save.disabled(),
    };
    let reason = self
      .work
      .state()
      .get()
      .error
      .map(|error| crate::ui::message::message_text(ctx, &error));
    action_bar(vec![cancel.into(), save.into()], reason.as_deref())
  }
}
