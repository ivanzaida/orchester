//! Add MCP server and Edit MCP server (`Connections · Edit MCP server · kontur-docs · full page · dark`
//! `M20DGB`, remote fields and values it cannot save in `euVYY`): one page in the Connections tab with the
//! server's name, how it is reached (a program on this computer with its arguments, folder and variables,
//! or a remote server with its endpoint and bearer secret) and the tools that run without asking. Add is
//! the same page with empty fields. A field whose value cannot be used says why under it; the action bar
//! says the first reason in field order. The fields are checked and saved by `connections::mcp`, the rules
//! the MCP tools use. A save an MCP client's window input brings is taken only when it does not widen what
//! agents may do, or a person lets automation change agent permissions (`set_mcp_server_enabled`'s rule).

mod fields;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
    theme::PaletteColor,
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  connection::mcp::{McpServer, McpServerName},
  id::ConnectionId,
  store::McpServerStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    data::ProjectConnections,
    mcp::{McpServerDraft, McpServerErrors, McpServerText, store_in, widenings},
  },
  messages::{self, Message},
  workspace::Workspace,
};

use self::fields::Fields;
use super::{
  data::{self, Saved},
  page::{Head, action_bar, page},
};
use crate::{
  mcp::input_gate::InputGate,
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsTab, ConnectionsView, ProjectPage, Route, Screen},
  services::Services,
  ui::{
    breadcrumb::Crumb,
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    state::{load_error, loading},
  },
};

#[derive(Clone, PartialEq)]
pub struct McpFormProps {
  pub project: ShownProject,
  /// The server being edited; `None` adds one.
  pub server: Option<ConnectionId>,
}

impl DevtoolsInspectable for McpFormProps {}

/// What a save stores.
#[derive(Clone)]
struct Save {
  repo_root: PathBuf,
  id: Option<ConnectionId>,
  draft: McpServerDraft,
  by_automation: bool,
}

type SaveAction = FutureAction<Save, Saved, Message>;

pub struct McpForm {
  fields: Fields,
  /// The fields hold the stored server's values (or a new one's) once they were filled.
  seeded: Signal<bool>,
  /// A save was asked for: every field's error shows, also of fields left empty.
  tried: Signal<bool>,
  /// Counts Try again presses after reading the connections failed.
  revision: Signal<u64>,
  /// Tells an MCP client's window input from a person's; `None` in a window without the app's MCP server,
  /// where no client's input arrives.
  gate: Option<InputGate>,
}

impl Component for McpForm {
  type Props = McpFormProps;

  fn create(ctx: &mut Ctx) -> Self {
    let form = Self {
      fields: Fields::new(ctx),
      seeded: ctx.signal(false),
      tried: ctx.signal(false),
      revision: ctx.signal(0),
      gate: ctx.use_context::<InputGate>(),
    };
    let save = save_action(ctx);
    let project = ctx.props::<McpFormProps>().project.open.project.id();
    let navigation = Navigation::of(ctx);
    ctx.watch(&save.state(), move |state| {
      if let Some(Saved(id)) = state.data.filter(|_| state.is_fulfilled()) {
        let view = ConnectionsView::new(ConnectionsTab::McpServers).with_selected(id);
        navigation.replace(Route::Project(project, ProjectPage::Connections(view)));
      }
    });
    form
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<McpFormProps>().clone();
    let save = save_action(ctx);
    let loaded = data::load(ctx, props.project.open.repo_root.clone(), self.revision.get());
    let screen = if props.server.is_some() {
      Screen::EditConnection
    } else {
      Screen::NewConnection
    };
    let Some(listed) = loaded.data else {
      let waiting = match loaded.error {
        Some(error) if !loaded.is_pending() => {
          let (body, revision) = (message_text(ctx, &error), self.revision.clone());
          load_error(
            ctx,
            "connection-form-load-error",
            (&ctx.t("connections.loadFailed"), &body),
            move || revision.update(|revision| *revision += 1),
          )
        }
        _ => loading("connection-form-loading", &ctx.t("connections.reading")).into(),
      };
      return lurq::components::Column::new()
        .id(format!("screen-{}", screen.name()))
        .child(waiting)
        .into();
    };
    let stored = props.server.and_then(|id| listed.mcp_server(id).cloned());
    if props.server.is_some() && stored.is_none() {
      return gone(ctx, screen);
    }
    self.seed(stored.as_ref(), &listed);
    let text = self.fields.text();
    let taken = taken(&listed, props.server);
    let checked = text.draft(&taken, &listed.secrets);
    let errors = checked
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let unchanged = stored
      .as_ref()
      .is_some_and(|server| McpServerText::of(server, &listed.secrets) == text);
    let sections = self.fields.sections(
      ctx,
      (&listed, &errors),
      self.tried.get(),
      props.project.open.project.name().as_str(),
    );
    let reason = reason(ctx, (&checked, &errors), unchanged, &save.state().get());
    let bar = self.bar(ctx, &props, (&save, checked.ok()), reason.as_deref());
    let head = head(ctx, &props, stored.as_ref(), (unchanged, &save.state().get()));
    page(ctx, screen, head, sections, bar)
  }
}

impl McpForm {
  /// Fills the fields once: with the stored server's values when editing.
  fn seed(&self, stored: Option<&McpServer>, listed: &ProjectConnections) {
    if self.seeded.get_untracked() {
      return;
    }
    let text = stored.map_or_else(McpServerText::new_server, |server| {
      McpServerText::of(server, &listed.secrets)
    });
    self.fields.fill(&text);
    self.seeded.set(true);
  }

  fn bar(
    &self,
    ctx: &mut Ctx,
    props: &McpFormProps,
    (save, draft): (&SaveAction, Option<McpServerDraft>),
    reason: Option<&str>,
  ) -> Element {
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    let back = ConnectionsView::new(ConnectionsTab::McpServers);
    let back = match props.server {
      Some(id) => back.with_selected(id),
      None => back,
    };
    let cancel = ActionButton::new("connection-cancel", ButtonKind::Secondary, ctx.t("connections.cancel"))
      .on_click(move || navigation.go(Route::Project(project, ProjectPage::Connections(back))));
    let button = ActionButton::new("connection-save", ButtonKind::Primary, ctx.t("connections.dialog.save"));
    let tried = self.tried.clone();
    let button = match (save.state().get().is_pending(), draft) {
      (true, _) => button.loading(),
      (false, Some(draft)) if reason.is_none() => {
        let (save, gate, repo_root, id) = (
          save.clone(),
          self.gate.clone(),
          props.project.open.repo_root.clone(),
          props.server,
        );
        button.on_click(move || {
          save.run(Save {
            repo_root: repo_root.clone(),
            id,
            draft: draft.clone(),
            by_automation: gate.as_ref().is_some_and(InputGate::input_in_flight),
          });
        })
      }
      (false, _) if !self.tried.get() => button.on_click(move || tried.set(true)),
      (false, _) => button.disabled(),
    };
    action_bar(vec![cancel.into(), button.into()], reason)
  }
}

/// The names of the project's other servers.
fn taken(listed: &ProjectConnections, editing: Option<ConnectionId>) -> Vec<McpServerName> {
  listed
    .mcp
    .iter()
    .filter(|server| Some(server.id()) != editing)
    .map(|server| server.name().clone())
    .collect()
}

/// The first reason Save cannot be used, in field order; `None` once it can.
fn reason(
  ctx: &Ctx,
  (checked, errors): (&Result<McpServerDraft, Box<McpServerErrors>>, &McpServerErrors),
  unchanged: bool,
  saving: &FutureState<Saved, Message>,
) -> Option<Arc<str>> {
  if let Some(error) = saving.error.as_ref().filter(|_| saving.is_rejected()) {
    return Some(ctx.t_args(
      "connections.dialog.reasons.failed",
      [("reason", &*message_text(ctx, error))],
    ));
  }
  if checked.is_ok() {
    return unchanged.then(|| ctx.t("connections.mcp.dialog.unchanged"));
  }
  let first = [
    &errors.name,
    &errors.program,
    &errors.arguments,
    &errors.working_directory,
    &errors.variables,
    &errors.url,
    &errors.exempt_tools,
  ]
  .into_iter()
  .flatten()
  .next()?;
  Some(ctx.t_args(
    "connections.dialog.reasons.name",
    [("reason", &*message_text(ctx, first))],
  ))
}

fn head(
  ctx: &mut Ctx,
  props: &McpFormProps,
  stored: Option<&McpServer>,
  (unchanged, saving): (bool, &FutureState<Saved, Message>),
) -> Head {
  let navigation = Navigation::of(ctx);
  let project = props.project.open.project.id();
  let to = move |view: ConnectionsView| {
    let navigation = navigation.clone();
    move || navigation.go(Route::Project(project, ProjectPage::Connections(view)))
  };
  let mut crumbs = vec![
    Crumb::link(ctx.t("connections.title"), to(ConnectionsView::default())),
    Crumb::link(
      ctx.t("connections.tabs.mcpServers"),
      to(ConnectionsView::new(ConnectionsTab::McpServers)),
    ),
  ];
  let (title, status) = match stored {
    Some(server) => {
      let view = ConnectionsView::new(ConnectionsTab::McpServers).with_selected(server.id());
      crumbs.push(Crumb::link(server.name().as_str(), to(view)).mono());
      crumbs.push(Crumb::here(ctx.t("connections.mcp.dialog.editCrumb")));
      let status = if unchanged {
        (
          Icon::Check,
          PaletteColor::TextMuted,
          ctx.t("connections.mcp.dialog.noChanges"),
        )
      } else {
        (
          Icon::CircleDot,
          PaletteColor::TextSecondary,
          ctx.t("connections.mcp.dialog.unsaved"),
        )
      };
      (ctx.t("connections.mcp.dialog.editTitle"), status)
    }
    None => {
      crumbs.push(Crumb::here(ctx.t("connections.mcp.dialog.addTitle")));
      let status = (
        Icon::CircleDashed,
        PaletteColor::TextMuted,
        ctx.t("connections.dialog.notSaved"),
      );
      (ctx.t("connections.mcp.dialog.addTitle"), status)
    }
  };
  let status = if saving.is_pending() {
    (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      ctx.t("connections.dialog.saving"),
    )
  } else {
    status
  };
  Head {
    crumbs,
    title,
    description: ctx.t_args(
      "connections.mcp.dialog.description",
      [("project", props.project.open.project.name().as_str())],
    ),
    status,
  }
}

/// The server to edit is no longer the project's.
fn gone(ctx: &Ctx, screen: Screen) -> Element {
  let missing = crate::ui::state::EmptyState::new(
    "connection-form-gone",
    Icon::SearchX,
    ctx.t("connections.mcp.dialog.goneTitle"),
    ctx.t("connections.mcp.dialog.goneDetail"),
  );
  lurq::components::Column::new()
    .id(format!("screen-{}", screen.name()))
    .padding(crate::theme::ExtraSpacing::Roomy.role())
    .child(missing)
    .into()
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let services = Services::of(ctx);
  let (activity, workspace) = (services.activity, services.workspace);
  let open = ctx.props::<McpFormProps>().project.open.clone();
  ctx.future_action(move |save: Save| {
    let (activity, workspace, open) = (activity.clone(), workspace.clone(), open.clone());
    async move {
      let spec = operations::connection_save(&open, save.draft.name.as_str());
      let store = async move { background::run(move || store(&workspace, save)).await? };
      activity.track(spec, store).await
    }
  })
}

/// Stores `save`, once a person decided it: an MCP client's window input may not widen what agents may do
/// unless a person lets automation.
fn store(workspace: &Workspace, save: Save) -> Result<Saved, Message> {
  if save.by_automation {
    let db = ProjectDb::open(&save.repo_root).map_err(|error| messages::store_error(&error))?;
    let stored = match save.id {
      Some(id) => db.mcp_server(id).map_err(|error| messages::store_error(&error))?,
      None => None,
    };
    let widens = !widenings(stored.as_ref(), &save.draft).is_empty();
    let allowed = workspace
      .automation_changes_permissions()
      .map_err(|error| messages::store_error(&error))?;
    if widens && !allowed {
      tracing::warn!("an MCP client's window input would widen an MCP server; not taken");
      return Err(Message::new("appMcp.errors.personOnly"));
    }
  }
  store_in(&save.repo_root, save.id, save.draft)
    .map(Saved)
    .map_err(|error| error.message())
}
