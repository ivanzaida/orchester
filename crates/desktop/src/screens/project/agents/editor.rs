//! Agent profile editor (`xT2ex`) with the page around it: the topbar says where the profile
//! stands in the lifecycle (`X3zkKN`) and offers Save draft and Publish vN (`j5YWqt`). The
//! editor has the tabs Profile, Permissions and Used in pipelines (`A7phQ`, see `tabs`); the
//! profile fields are in `fields`, the permissions in `permissions`.

use std::{
  ops::{Deref, DerefMut},
  sync::Arc,
};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, invocation::Platform, mcp::McpServerSelection},
  id::ConnectionId,
  permission::{AgentPermissions, capability::PermissionOption},
  resource::{AgentProfile, VersionNumber},
};

use super::{
  Editing, EditorContext,
  list::{ListSelection, profile_list},
  tabs::EditorTab,
};
use crate::{
  agents::{
    data::{ProfileChange, VersionState},
    form::{ModelSelection, ProfileErrors, ProfileForm, stored_effort},
  },
  messages::Message,
  screens::project::page::{self, TOPBAR_BUTTON_HEIGHT},
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    form::{Submit, form},
    icon::Icon,
    message_text,
    notice::Notice,
  },
};

/// The chosen connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionChoice(pub Option<ConnectionId>);

impl lurq::app::component::DevtoolsInspectable for ConnectionChoice {}

/// The chosen reasoning effort; `None` keeps the CLI's default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffortChoice(pub Option<String>);

impl lurq::app::component::DevtoolsInspectable for EffortChoice {}

/// The permissions as edited. Derefs to them; a newtype so the signal can be inspected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionsChoice(pub AgentPermissions);

impl lurq::app::component::DevtoolsInspectable for PermissionsChoice {}

impl Deref for PermissionsChoice {
  type Target = AgentPermissions;

  fn deref(&self) -> &AgentPermissions {
    &self.0
  }
}

impl DerefMut for PermissionsChoice {
  fn deref_mut(&mut self) -> &mut AgentPermissions {
    &mut self.0
  }
}

/// The MCP servers the profile gives its agents, as edited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpChoice(pub McpServerSelection);

impl lurq::app::component::DevtoolsInspectable for McpChoice {}

/// Options the last connection change replaced because the new connection cannot enforce them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resets(pub Vec<PermissionOption>);

impl lurq::app::component::DevtoolsInspectable for Resets {}

pub struct ProfileEditor {
  pub(super) context: EditorContext,
  pub(super) role: Signal<String>,
  pub(super) responsibilities: Signal<String>,
  pub(super) instructions: Signal<String>,
  pub(super) connection: Signal<ConnectionChoice>,
  pub(super) model: Signal<ModelSelection>,
  pub(super) custom_model: Signal<String>,
  pub(super) effort: Signal<EffortChoice>,
  pub(super) tab: Signal<EditorTab>,
  pub(super) permissions: Signal<PermissionsChoice>,
  pub(super) resets: Signal<Resets>,
  pub(super) mcp: Signal<McpChoice>,
  /// The command being added to the allowed commands, as typed.
  pub(super) new_command: Signal<String>,
  pub(super) command_error: Signal<Option<Message>>,
}

/// The form's current values checked against the stored profile.
pub(super) struct EditorState {
  pub profile: Result<AgentProfile, Box<ProfileErrors>>,
  /// The form differs from what is stored (always true for a new profile).
  pub dirty: bool,
  pub saving: bool,
  pub error: Option<Message>,
}

impl Component for ProfileEditor {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<EditorContext>()
      .expect("the agents screen provides the editor context");
    let editor = match &context.editing {
      Editing::Profile(entry) => Self::seeded(ctx, &context, &entry.profile),
      Editing::New => Self::blank(ctx, &context),
    };
    // Another connection offers other models; its first one replaces the choice. Permissions it
    // cannot enforce are reset to the most restrictive ones it can, and the reset is shown.
    let (connections, model) = (editor.context.agents.connections.clone(), editor.model.clone());
    let (permissions, resets) = (editor.permissions.clone(), editor.resets.clone());
    ctx.watch(&editor.connection, move |choice| {
      let connection = choice
        .0
        .and_then(|id| connections.iter().find(|connection| connection.id() == id));
      model.set(first_model(connection));
      if let Some(connection) = connection {
        let (fitted, reset) = permissions
          .get_untracked()
          .fitted_to(connection.provider(), Platform::current());
        permissions.set(PermissionsChoice(fitted));
        resets.set(Resets(reset));
      }
    });
    let effort = editor.effort.clone();
    ctx.watch(&editor.model, move |_| effort.set(EffortChoice(None)));
    editor
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let state = self.state();
    let entry = match &self.context.editing {
      Editing::Profile(entry) => Some(&**entry),
      Editing::New => None,
    };
    let content = match self.tab.get() {
      EditorTab::Profile => {
        let mut content = self.fields(ctx, &state);
        content.push(
          Notice::new(
            Icon::History,
            ctx.t("agents.runsKeepTitle"),
            ctx.t("agents.runsKeepDetail"),
          )
          .into(),
        );
        content
      }
      EditorTab::Permissions => self.permissions_tab(ctx),
      EditorTab::Usage => vec![self.pipeline_uses(ctx)],
    };
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Section)
      .spacing(ExtraSpacing::Form.role())
      .child(self.tab_strip(ctx))
      .with_children(entry.and_then(|entry| self.stale_notice(ctx, entry.state, &state)))
      .with_children(self.permission_notices(ctx))
      .with_children(content);
    let breadcrumb = ctx.t_args(
      "agents.breadcrumbProfile",
      [
        ("project", self.context.project.project.name().as_str()),
        ("role", &*self.title(ctx)),
      ],
    );
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, self.topbar_actions(ctx, &state)))
      .child(page::heading(&ctx.t("agents.title"), &ctx.t("agents.description")))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .child(profile_list(
            ctx,
            &self.context.agents,
            entry.map_or(ListSelection::New, |entry| ListSelection::Profile(entry.id)),
            &self.context.nav,
          ))
          .child(
            ScrollVertical::new(body)
              .id("agent-editor")
              .flex(1.0)
              .height(Dimension::Pct(100.0)),
          ),
      );
    // Enter in a field saves the draft, as Save draft does.
    form(self.save_draft(&state), page)
  }
}

impl ProfileEditor {
  fn seeded(ctx: &mut Ctx, context: &EditorContext, profile: &AgentProfile) -> Self {
    Self {
      role: ctx.signal(profile.role().as_str().to_owned()),
      responsibilities: ctx.signal(profile.responsibilities().to_owned()),
      instructions: ctx.signal(profile.instructions().as_str().to_owned()),
      connection: ctx.signal(ConnectionChoice(Some(profile.connection()))),
      model: ctx.signal(ModelSelection::Listed(profile.model().clone())),
      custom_model: ctx.signal(String::new()),
      effort: ctx.signal(EffortChoice(stored_effort(profile))),
      tab: ctx.signal(EditorTab::Profile),
      permissions: ctx.signal(PermissionsChoice(profile.permissions().clone())),
      resets: ctx.signal(Resets::default()),
      mcp: ctx.signal(McpChoice(profile.mcp_servers().clone())),
      new_command: ctx.signal(String::new()),
      command_error: ctx.signal(None),
      context: context.clone(),
    }
  }

  /// A new profile starts on the first usable connection and its first model, with the most
  /// restrictive permissions that connection enforces.
  fn blank(ctx: &mut Ctx, context: &EditorContext) -> Self {
    let connections = &context.agents.connections;
    let connection = connections
      .iter()
      .find(|connection| connection.status().is_usable())
      .or_else(|| connections.first());
    Self {
      role: ctx.signal(String::new()),
      responsibilities: ctx.signal(String::new()),
      instructions: ctx.signal(String::new()),
      connection: ctx.signal(ConnectionChoice(connection.map(|connection| connection.id()))),
      model: ctx.signal(first_model(connection)),
      custom_model: ctx.signal(String::new()),
      effort: ctx.signal(EffortChoice(None)),
      tab: ctx.signal(EditorTab::Profile),
      permissions: ctx.signal(PermissionsChoice(
        connection.map_or_else(AgentPermissions::before_permissions, |connection| {
          AgentPermissions::most_restrictive(connection.provider(), Platform::current())
        }),
      )),
      resets: ctx.signal(Resets::default()),
      mcp: ctx.signal(McpChoice::default()),
      new_command: ctx.signal(String::new()),
      command_error: ctx.signal(None),
      context: context.clone(),
    }
  }

  pub(super) fn selected_connection(&self) -> Option<&AgentConnection> {
    let id = self.connection.get().0?;
    self
      .context
      .agents
      .connections
      .iter()
      .find(|connection| connection.id() == id)
  }

  fn state(&self) -> EditorState {
    let (role, responsibilities, instructions, custom_model) = (
      self.role.get(),
      self.responsibilities.get(),
      self.instructions.get(),
      self.custom_model.get(),
    );
    let (model, effort, permissions) = (self.model.get(), self.effort.get(), self.permissions.get());
    let mcp = self.mcp.get();
    let profile = ProfileForm {
      role: &role,
      responsibilities: &responsibilities,
      instructions: &instructions,
      connection: self.selected_connection(),
      model: &model,
      custom_model: &custom_model,
      effort: effort.0.as_deref(),
      permissions: &permissions,
      mcp_servers: &mcp.0,
    }
    .profile();
    let dirty = match (&self.context.editing, &profile) {
      (Editing::Profile(entry), Ok(profile)) => *profile != entry.profile,
      _ => true,
    };
    let change = self.context.change.state().get();
    let saving = change.is_pending();
    EditorState {
      profile,
      dirty,
      saving,
      error: change.error.filter(|_| !saving),
    }
  }

  /// The role as typed, or "New agent" until there is one.
  fn title(&self, ctx: &Ctx) -> Arc<str> {
    let role = self.role.get();
    if role.trim().is_empty() {
      ctx.t("agents.newTitle")
    } else {
      role.trim().into()
    }
  }

  /// Save state, Save draft and Publish vN.
  fn topbar_actions(&self, ctx: &Ctx, state: &EditorState) -> Vec<Element> {
    let version = match &self.context.editing {
      Editing::Profile(entry) => Some(entry.state),
      Editing::New => None,
    };
    let status = match (&state.error, state.saving) {
      (Some(error), _) => message_text(ctx, error),
      (None, true) => ctx.t("agents.saving"),
      (None, false) => save_status(ctx, version, state.dirty),
    };
    let publishable = version.is_some_and(|version| matches!(version, VersionState::Draft { .. }));
    let next = version
      .and_then(VersionState::published)
      .map_or(1, |published| published.get() + 1);
    vec![
      Text::new(&status)
        .variant(TypographyStyle::Caption)
        .id("agent-save-status")
        .into(),
      self.save_button(ctx, state),
      self.publish_button(ctx, state, publishable && !state.dirty, next),
    ]
  }

  fn save_button(&self, ctx: &Ctx, state: &EditorState) -> Element {
    let button = ActionButton::new(ctx.t("agents.saveDraft"))
      .icon(Icon::Save)
      .id("agent-save-draft")
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    match self.save_draft(state) {
      Some(save) => button.on_click(move || save()),
      None => button.on_click(|| {}).enabled(false),
    }
    .render(ctx)
  }

  /// Saves the changed draft; `None` while the fields say what is missing, nothing changed,
  /// or a save is running.
  fn save_draft(&self, state: &EditorState) -> Submit {
    let change = match (&state.profile, &self.context.editing) {
      (Ok(profile), Editing::New) => ProfileChange::Create(profile.clone()),
      (Ok(profile), Editing::Profile(entry)) => ProfileChange::SaveDraft(entry.id, profile.clone()),
      (Err(_), _) => return None,
    };
    if !state.dirty || state.saving {
      return None;
    }
    let (action, repo_root) = (self.context.change.clone(), self.context.project.repo_root.clone());
    Some(Arc::new(move || action.run((repo_root.clone(), change.clone()))))
  }

  /// Publishes the saved draft; unsaved changes must be saved first.
  fn publish_button(&self, ctx: &Ctx, state: &EditorState, publishable: bool, next: u32) -> Element {
    let button = ActionButton::new(ctx.t_args("agents.publish", [("version", next.to_string().as_str())]))
      .primary()
      .icon(Icon::Check)
      .id("agent-publish")
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    let Editing::Profile(entry) = &self.context.editing else {
      return button.on_click(|| {}).enabled(false).render(ctx);
    };
    if !publishable || state.saving || self.permission_problem().is_some() {
      return button.on_click(|| {}).enabled(false).render(ctx);
    }
    let (action, repo_root, id) = (
      self.context.change.clone(),
      self.context.project.repo_root.clone(),
      entry.id,
    );
    button
      .on_click(move || action.run((repo_root.clone(), ProfileChange::Publish(id))))
      .render(ctx)
  }

  /// A draft started from an older version cannot be published as is (`PublishError::StaleDraft`);
  /// the user decides whether it replaces the newer version.
  fn stale_notice(&self, ctx: &Ctx, version: VersionState, state: &EditorState) -> Option<Element> {
    let VersionState::StaleDraft { based_on, published } = version else {
      return None;
    };
    let Editing::Profile(entry) = &self.context.editing else {
      return None;
    };
    let (action, repo_root, id) = (
      self.context.change.clone(),
      self.context.project.repo_root.clone(),
      entry.id,
    );
    let keep = ActionButton::new(ctx.t("agents.keepDraft"))
      .icon(Icon::FilePenLine)
      .id("agent-keep-draft")
      .in_form()
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    let keep = if state.saving || state.dirty {
      keep.on_click(|| {}).enabled(false)
    } else {
      keep.on_click(move || action.run((repo_root.clone(), ProfileChange::KeepDraft(id))))
    };
    let detail = ctx.t_args(
      "agents.staleDetail",
      [
        ("basedOn", &*version_text(ctx, based_on)),
        ("published", &*version_text(ctx, published)),
      ],
    );
    Some(
      Notice::new(Icon::AlertCircle, ctx.t("agents.staleTitle"), detail)
        .emphasized()
        .id("agent-stale-draft")
        .action(keep.render(ctx))
        .into(),
    )
  }
}

/// "Draft saved · Published v2", "Unsaved changes · Not published", "Published v2", …
fn save_status(ctx: &Ctx, version: Option<VersionState>, dirty: bool) -> Arc<str> {
  let published = version.and_then(VersionState::published);
  let published_text = published.map_or_else(
    || ctx.t("agents.notPublished"),
    |version| ctx.t_args("agents.published", [("version", version.get().to_string().as_str())]),
  );
  let (key, based_on) = match version {
    None => return ctx.t("agents.notSaved"),
    Some(_) if dirty => ("agents.unsavedChanges", None),
    Some(VersionState::Published(_)) => return published_text,
    Some(VersionState::Draft { .. }) => ("agents.draftSaved", None),
    Some(VersionState::StaleDraft { based_on, .. }) => ("agents.draftBasedOn", Some(based_on)),
  };
  let based_on = based_on.map_or_else(String::new, |based_on| version_text(ctx, based_on).to_string());
  ctx.t_args(key, [("published", &*published_text), ("basedOn", based_on.as_str())])
}

fn version_text(ctx: &Ctx, version: Option<VersionNumber>) -> Arc<str> {
  version.map_or_else(
    || ctx.t("agents.noVersion"),
    |version| ctx.t_args("agents.version", [("version", version.get().to_string().as_str())]),
  )
}

/// The first model the connection lists, or a typed one when it lists none.
fn first_model(connection: Option<&AgentConnection>) -> ModelSelection {
  match connection {
    None => ModelSelection::None,
    Some(connection) => connection.models().first().map_or(ModelSelection::Custom, |model| {
      ModelSelection::Listed(model.id().clone())
    }),
  }
}
