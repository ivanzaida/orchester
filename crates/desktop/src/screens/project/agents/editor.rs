//! Agent profile editor (`xT2ex`) with the page around it: the topbar says where the profile
//! stands in the lifecycle (`X3zkKN`) and offers Save draft and Publish vN (`j5YWqt`). The
//! fields are in `fields`.

use std::sync::Arc;

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
  connection::Connection,
  id::ConnectionId,
  resource::{AgentProfile, VersionNumber},
};

use super::{
  Editing, EditorContext,
  list::{ListSelection, profile_list},
};
use crate::{
  agents::{
    data::{ProfileChange, VersionState},
    form::{ModelSelection, ProfileErrors, ProfileForm, stored_effort},
  },
  messages::Message,
  screens::project::page::{self, TOPBAR_BUTTON_HEIGHT},
  theme::ExtraSpacing,
  ui::{button::ActionButton, icon::Icon, message_text, notice::Notice},
};

/// The chosen connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionChoice(pub Option<ConnectionId>);

impl lurq::app::component::DevtoolsInspectable for ConnectionChoice {}

/// The chosen reasoning effort; `None` keeps the CLI's default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffortChoice(pub Option<String>);

impl lurq::app::component::DevtoolsInspectable for EffortChoice {}

pub struct ProfileEditor {
  pub(super) context: EditorContext,
  pub(super) role: Signal<String>,
  pub(super) responsibilities: Signal<String>,
  pub(super) instructions: Signal<String>,
  pub(super) connection: Signal<ConnectionChoice>,
  pub(super) model: Signal<ModelSelection>,
  pub(super) custom_model: Signal<String>,
  pub(super) effort: Signal<EffortChoice>,
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
    // Another connection offers other models; its first one replaces the choice.
    let (connections, model) = (editor.context.agents.connections.clone(), editor.model.clone());
    ctx.watch(&editor.connection, move |choice| {
      let connection = choice
        .0
        .and_then(|id| connections.iter().find(|connection| connection.id() == id));
      model.set(first_model(connection));
    });
    let effort = editor.effort.clone();
    ctx.watch(&editor.model, move |_| effort.set(EffortChoice(None)));
    editor
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let state = self.state();
    let entry = match &self.context.editing {
      Editing::Profile(entry) => Some(entry),
      Editing::New => None,
    };
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Section)
      .spacing(ExtraSpacing::Form.role())
      .with_children(entry.and_then(|entry| self.stale_notice(ctx, entry.state, &state)))
      .with_children(self.fields(ctx, &state))
      .child(Notice::new(
        Icon::History,
        ctx.t("agents.runsKeepTitle"),
        ctx.t("agents.runsKeepDetail"),
      ));
    let breadcrumb = ctx.t_args(
      "agents.breadcrumbProfile",
      [
        ("project", self.context.project.project.name().as_str()),
        ("role", &*self.title(ctx)),
      ],
    );
    Column::new()
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
      )
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
      context: context.clone(),
    }
  }

  /// A new profile starts on the first usable connection and its first model.
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
      connection: ctx.signal(ConnectionChoice(connection.map(Connection::id))),
      model: ctx.signal(first_model(connection)),
      custom_model: ctx.signal(String::new()),
      effort: ctx.signal(EffortChoice(None)),
      context: context.clone(),
    }
  }

  pub(super) fn selected_connection(&self) -> Option<&Connection> {
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
    let (model, effort) = (self.model.get(), self.effort.get());
    let profile = ProfileForm {
      role: &role,
      responsibilities: &responsibilities,
      instructions: &instructions,
      connection: self.selected_connection(),
      model: &model,
      custom_model: &custom_model,
      effort: effort.0.as_deref(),
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
    let change = match (&state.profile, &self.context.editing) {
      (Ok(profile), Editing::New) => ProfileChange::Create(profile.clone()),
      (Ok(profile), Editing::Profile(entry)) => ProfileChange::SaveDraft(entry.id, profile.clone()),
      // The fields say what is missing; nothing to save until then.
      (Err(_), _) => return button.on_click(|| {}).enabled(false).render(ctx),
    };
    if !state.dirty || state.saving {
      return button.on_click(|| {}).enabled(false).render(ctx);
    }
    let (action, repo_root) = (self.context.change.clone(), self.context.project.repo_root.clone());
    button
      .on_click(move || action.run((repo_root.clone(), change.clone())))
      .render(ctx)
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
    if !publishable || state.saving {
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
fn first_model(connection: Option<&Connection>) -> ModelSelection {
  match connection {
    None => ModelSelection::None,
    Some(connection) => connection.models().first().map_or(ModelSelection::Custom, |model| {
      ModelSelection::Listed(model.id().clone())
    }),
  }
}
