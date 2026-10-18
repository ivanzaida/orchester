//! Edit connection (the inspector's *Edit connection*, `M42rM`): for an MCP server its form (`mcp_form`); for an agent
//! CLI connection the account it runs on, chosen as in Add connection (`WaKJN`), with its name and program as they
//! are, which this app does not edit yet. Saving runs the connection on the chosen account and checks it there; a
//! paused run's next step then starts there as a new session, which the page says. Choosing the account is a
//! person's decision ([`super::account::AccountPicker`]), and so is running it on an API provider instead
//! ([`super::provider_form`]).

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, ConnectionAccount},
  id::ConnectionId,
  secret::{Secret, SecretValue},
};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    api_provider::{ProviderDraft, RunsOn, set_runs_on_in},
    native::KeySecret,
  },
  messages::Message,
  tasks::outside::Outside,
};

use super::{
  account, data,
  mcp_form::{McpForm, McpFormProps},
  page::{Head, action_bar, page},
  provider_form::{KeyValue, ProviderForm},
  work::{self, Start, Work, WorkAction},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route, Screen},
  services::{Services, accounts::listed},
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

/// What Edit connection saves when the connection's API provider changes: its project, the connection, what it runs
/// on and the key typed for a new secret.
type RunsOnSave = FutureAction<(PathBuf, ConnectionId, RunsOn, Option<SecretValue>), data::Saved, Message>;

pub struct EditConnection {
  /// The account chosen, by id; empty until the connection is read.
  chosen: Signal<String>,
  work: WorkAction,
  provider: ProviderForm,
  runs_on: RunsOnSave,
}

impl Component for EditConnection {
  type Props = EditProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditProps>().clone();
    let work = work::action(ctx);
    let runs_on = runs_on_action(ctx);
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    let back = move |id: ConnectionId| {
      navigation.go(Route::Project(
        project,
        ProjectPage::Connections(ConnectionsView::default().with_selected(id)),
      ));
    };
    let after_work = back.clone();
    ctx.watch(&work.state(), move |state| {
      if let Some(done) = state.data.filter(|_| state.is_fulfilled()) {
        after_work(done.id);
      }
    });
    let changes = Services::of(ctx).changes;
    ctx.watch(&runs_on.state(), move |state| {
      if let Some(data::Saved(id)) = state.data.filter(|_| state.is_fulfilled()) {
        changes.record(project, Outside::Connections);
        back(id);
      }
    });
    Self {
      chosen: ctx.signal(String::new()),
      work,
      provider: ProviderForm::new(ctx),
      runs_on,
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
    self.agent_page(ctx, &props, (&connection, &listed.secrets))
  }
}

impl EditConnection {
  fn agent_page(
    &self,
    ctx: &mut Ctx,
    props: &EditProps,
    (connection, listed_secrets): (&AgentConnection, &[Secret]),
  ) -> Element {
    let accounts = listed(ctx);
    let provider = connection.provider();
    let current = accounts
      .of_connection(connection.account(), provider)
      .map(|account| account.id().to_string())
      .unwrap_or_default();
    if self.chosen.get().is_empty() && !current.is_empty() {
      self.chosen.set(current.clone());
    }
    self.provider.seed(connection.api_provider());
    let account_changed = !self.chosen.get().is_empty() && self.chosen.get() != current;
    let account = super::account::chosen_account(&accounts, provider, &self.chosen.get())
      .map_or(ConnectionAccount::Default, ConnectionAccount::of);
    let wanted = self.provider.runs_on(listed_secrets, account);
    let provider_changed = provider_changed(connection, wanted.as_ref(), self.provider.on());
    let changed = account_changed || provider_changed;
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
    let mut sections = vec![rows.into()];
    if !self.provider.on() {
      sections.push(account::add_row(ctx, &accounts, provider, &self.chosen));
    }
    sections.push(
      inspector::note(
        "connection-account-note",
        Icon::Info,
        &ctx.t("connections.account.changeNote"),
      )
      .into(),
    );
    sections.push(self.provider.section(ctx, provider, listed_secrets));
    let head = self.head(ctx, props, connection, changed);
    let bar = if provider_changed {
      self.provider_bar(ctx, props, connection, wanted, listed_secrets)
    } else {
      self.bar(ctx, props, connection, changed)
    };
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
        Crumb::to(
          ctx.t("connections.title"),
          navigation.to(Route::Project(
            project,
            ProjectPage::Connections(ConnectionsView::default()),
          )),
        ),
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
            project: open.project.id(),
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

impl EditConnection {
  /// *Cancel* and *Save* while the connection's API provider changes: Save runs it on what the page holds, a person's
  /// decision when that is a provider.
  fn provider_bar(
    &self,
    ctx: &mut Ctx,
    props: &EditProps,
    connection: &AgentConnection,
    wanted: Option<RunsOn>,
    secrets: &[Secret],
  ) -> Element {
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
    let state = self.runs_on.state().get();
    let save = match wanted {
      _ if state.is_pending() => save.loading(),
      Some(wanted) => {
        let (action, value, repo) = (
          self.runs_on.clone(),
          self.provider.key_value(),
          props.project.open.repo_root.clone(),
        );
        let gate = ctx.use_context::<InputGate>();
        save.on_click(move || saved(&action, (&repo, id, &wanted), &value, gate.as_ref()))
      }
      None => save.disabled(),
    };
    let rejected = state.is_rejected();
    let reason = state
      .error
      .filter(|_| rejected)
      .map(|error| crate::ui::message::message_text(ctx, &error))
      .or_else(|| self.provider.reason(ctx, secrets));
    action_bar(vec![cancel.into(), save.into()], reason.as_deref())
  }
}

/// Whether the page would change what `connection` runs on as to its API provider: one it does not run on, another
/// URL, key or models, or none any more.
fn provider_changed(connection: &AgentConnection, wanted: Option<&RunsOn>, on: bool) -> bool {
  let current = connection.api_provider();
  match (current, wanted) {
    (None, _) => on,
    (Some(_), _) if !on => true,
    (Some(current), Some(RunsOn::Provider(ProviderDraft { base_url, key, models }))) => {
      current.base_url() != base_url
        || !matches!(key, KeySecret::Existing(id) if *id == current.key())
        || !current.model_ids().eq(models.iter())
    }
    (Some(_), _) => true,
  }
}

/// Saves `wanted` for connection `id` of the project at `repo`, with the key typed for a new secret; automation's
/// input does so for a provider only where a person lets it.
fn saved(
  action: &RunsOnSave,
  (repo, id, wanted): (&PathBuf, ConnectionId, &RunsOn),
  value: &KeyValue,
  gate: Option<&InputGate>,
) {
  if matches!(wanted, RunsOn::Provider(_)) && !input_may_decide(gate, Guarded::PermissionWidening) {
    tracing::warn!("synthetic input saved a connection on an API provider; not taken");
    return;
  }
  action.run((repo.clone(), id, wanted.clone(), value.for_saving(wanted)));
}

fn runs_on_action(ctx: &mut Ctx) -> RunsOnSave {
  let props = ctx.props::<EditProps>().clone();
  let services = Services::of(ctx);
  let (activity, vault) = (services.activity, services.vault);
  ctx.future_action(
    move |(repo, id, runs_on, value): (PathBuf, ConnectionId, RunsOn, Option<SecretValue>)| {
      let (activity, vault, open) = (activity.clone(), vault.clone(), props.project.open.clone());
      async move {
        let spec = operations::connection_save(&open, &id.to_string());
        let save = async move {
          background::run(move || set_runs_on_in(&repo, vault.get(), id, runs_on, value).map(|()| data::Saved(id)))
            .await?
        };
        activity.track(spec, save).await
      }
    },
  )
}
