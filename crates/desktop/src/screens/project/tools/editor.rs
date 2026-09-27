//! Tool editor (06.02 `YEWFo`, for a command tool): the topbar says where the tool stands in the
//! lifecycle and offers Save draft and Publish vN; the heading names the tool and its kind; the
//! Configuration tab holds the command's fields (see `fields`) beside what the tool may do and
//! where it is used (see `inspector`). Inputs & outputs, Access and Usage log do not exist yet. A
//! tool of another kind is shown but cannot be edited.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  name::Name,
  resource::{Tool, ToolAction, ToolKind, VersionNumber},
};

use super::{
  Editing, EditorContext, inspector,
  row::{kind_icon, kind_key},
};
use crate::{
  activity::operations,
  agents::data::VersionState,
  messages::Message,
  screens::project::page::{self, TOPBAR_BUTTON_HEIGHT},
  theme::{ExtraSpacing, ExtraTypography},
  tools::{
    data::ToolChange,
    form::{CommandErrors, CommandText, tool_name},
  },
  ui::{
    button::ActionButton,
    form::{Submit, form},
    icon::Icon,
    message_text,
    status_label::status_label,
    tab::{Tab, tab_row},
  },
};

/// Tab strip height (`fne6o`) and the right panel's width (`k97jh`).
const TABS_HEIGHT: f32 = 42.0;
const PANEL_WIDTH: f32 = 304.0;

pub struct ToolEditor {
  pub(super) context: EditorContext,
  /// A new tool's name; an existing tool keeps the name it was created with.
  pub(super) name: Signal<String>,
  pub(super) description: Signal<String>,
  pub(super) program: Signal<String>,
  pub(super) arguments: Signal<String>,
  pub(super) timeout: Signal<String>,
  pub(super) exit_codes: Signal<String>,
  pub(super) output_limit: Signal<String>,
  pub(super) environment: Signal<String>,
}

/// The fields checked against the stored tool.
pub(super) struct EditorState {
  pub tool: Result<Tool, Box<CommandErrors>>,
  pub name: Result<Name, Message>,
  /// The fields differ from what is stored (always true for a new tool).
  pub dirty: bool,
  pub saving: bool,
  pub error: Option<Message>,
}

impl EditorState {
  pub(super) fn errors(&self) -> CommandErrors {
    let mut errors = self
      .tool
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    errors.name = self.name.as_ref().err().cloned();
    errors
  }
}

impl Component for ToolEditor {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<EditorContext>()
      .expect("the tools screen provides the editor context");
    // A tool of another kind cannot be edited; only its description is shown.
    let text = match &context.editing {
      Editing::New => CommandText::new_tool(),
      Editing::Tool(entry) => CommandText::of(&entry.tool).unwrap_or_else(|| CommandText {
        description: entry.tool.description().to_owned(),
        ..CommandText::default()
      }),
    };
    Self {
      name: ctx.signal(String::new()),
      description: ctx.signal(text.description),
      program: ctx.signal(text.program),
      arguments: ctx.signal(text.arguments),
      timeout: ctx.signal(text.timeout),
      exit_codes: ctx.signal(text.exit_codes),
      output_limit: ctx.signal(text.output_limit),
      environment: ctx.signal(text.environment),
      context,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let state = self.state();
    let kind = self.kind();
    let content: Element = if kind == ToolKind::Command {
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(SpacingSize::Section)
          .spacing(ExtraSpacing::Form.role())
          .with_children(self.stale_notice(ctx, &state))
          .with_children(self.fields(ctx, &state)),
      )
      .id("tool-editor")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .into()
    } else {
      self.not_editable(ctx, kind)
    };
    let panels = Row::new().width(Dimension::Pct(100.0)).flex(1.0).child(content).child(
      Column::new()
        .width(Dimension::Px(PANEL_WIDTH))
        .height(Dimension::Pct(100.0))
        .child(inspector::inspector(ctx, &self.context, &state)),
    );
    let breadcrumb = ctx.t_args(
      "tools.breadcrumbTool",
      [
        ("project", self.context.project.project.name().as_str()),
        ("tool", &*self.title(ctx)),
      ],
    );
    let page = Column::new()
      .id("tool-page")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, self.topbar_actions(ctx, &state)))
      .child(self.heading(ctx, kind))
      .child(tabs(ctx))
      .child(panels);
    // Enter in a single-line field saves the draft, as Save draft does.
    form(self.save_draft(ctx, &state), page)
  }
}

impl ToolEditor {
  pub(super) fn kind(&self) -> ToolKind {
    match &self.context.editing {
      Editing::New => ToolKind::Command,
      Editing::Tool(entry) => entry.tool.kind(),
    }
  }

  fn state(&self) -> EditorState {
    let text = CommandText {
      description: self.description.get(),
      program: self.program.get(),
      arguments: self.arguments.get(),
      timeout: self.timeout.get(),
      exit_codes: self.exit_codes.get(),
      output_limit: self.output_limit.get(),
      environment: self.environment.get(),
    };
    let (tool, name) = match &self.context.editing {
      Editing::New => (text.tool(None), tool_name(&self.name.get())),
      Editing::Tool(entry) => (
        text.tool(entry.tool.access().cloned()),
        Name::new(&entry.name).map_err(|error| crate::messages::name_error(&error)),
      ),
    };
    let dirty = match (&self.context.editing, &tool) {
      // Nothing of a tool this version cannot edit can have changed.
      (Editing::Tool(entry), _) if entry.tool.kind() != ToolKind::Command => false,
      (Editing::Tool(entry), Ok(tool)) => *tool != entry.tool,
      _ => true,
    };
    let change = self.context.change.state().get();
    let saving = change.is_pending();
    EditorState {
      tool,
      name,
      dirty,
      saving,
      error: change.error.filter(|_| !saving),
    }
  }

  /// The tool's name, or "New command tool" until a new one has one.
  pub(super) fn title(&self, ctx: &Ctx) -> Arc<str> {
    match &self.context.editing {
      Editing::Tool(entry) => entry.name.clone(),
      Editing::New => {
        let name = self.name.get();
        if name.trim().is_empty() {
          ctx.t("tools.editor.newTitle")
        } else {
          name.trim().into()
        }
      }
    }
  }

  /// Name, kind and what the tool does (`GhoQg`).
  fn heading(&self, ctx: &Ctx, kind: ToolKind) -> Element {
    let description = self.description.get();
    // Without a description a stored command tool is its command line, as in the library.
    let description: Arc<str> = match (description.trim(), &self.context.editing) {
      ("", Editing::Tool(entry)) => match entry.tool.action() {
        ToolAction::Command(command) => command.command_line().into(),
        ToolAction::Http(_) | ToolAction::DatabaseQuery(_) => ctx.t("tools.noDescription"),
      },
      ("", Editing::New) => ctx.t("tools.editor.noDescription"),
      (description, _) => description.into(),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Rail.role())
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Lg)
      .spacing(ExtraSpacing::Stack.role())
      .child(
        Row::new()
          .spacing(SpacingSize::Md)
          .align_items(Alignment::Center)
          .child(
            Text::new(&self.title(ctx))
              .variant(ExtraTypography::PageTitle.role())
              .id("tool-title"),
          )
          .child(status_label(kind_icon(kind), &ctx.t(kind_key(kind)))),
      )
      .child(
        Text::new(&description)
          .variant(TypographyStyle::Body)
          .color(PaletteColor::TextSecondary)
          .id("tool-description"),
      )
      .into()
  }

  /// A tool this version cannot edit or run: what it is and why.
  fn not_editable(&self, ctx: &Ctx, kind: ToolKind) -> Element {
    let key = match kind {
      ToolKind::Http => "tools.editor.httpUnavailable",
      ToolKind::Database | ToolKind::Command => "tools.editor.databaseUnavailable",
    };
    Column::new()
      .id("tool-not-editable")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Section)
      .child(crate::ui::panel::RecoveryPanel::new(
        Icon::Lock,
        &ctx.t("tools.editor.unavailableTitle"),
        &ctx.t(key),
      ))
      .into()
  }

  /// Save state, Save draft and Publish vN (`simD0`).
  fn topbar_actions(&self, ctx: &Ctx, state: &EditorState) -> Vec<Element> {
    let version = self.version();
    let status = match (&state.error, state.saving) {
      (Some(error), _) => message_text(ctx, error),
      (None, true) => ctx.t("tools.saving"),
      (None, false) => save_status(ctx, version, state.dirty),
    };
    let editable = self.kind() == ToolKind::Command;
    let button = ActionButton::new(ctx.t("tools.saveDraft"))
      .icon(Icon::Save)
      .id("tool-save-draft")
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    let save = match (editable, self.save_draft(ctx, state)) {
      (true, Some(save)) => button.on_click(move || save()),
      (true, None) => button.on_click(|| {}).enabled(false),
      (false, _) => button,
    };
    vec![
      Text::new(&status)
        .variant(TypographyStyle::Caption)
        .id("tool-save-status")
        .into(),
      save.render(ctx),
      self.publish_button(ctx, state, version),
    ]
  }

  fn version(&self) -> Option<VersionState> {
    match &self.context.editing {
      Editing::Tool(entry) => Some(entry.state),
      Editing::New => None,
    }
  }

  /// Saves the changed draft, creating a new tool; `None` while a field says what is missing,
  /// nothing changed or a save is running.
  fn save_draft(&self, ctx: &Ctx, state: &EditorState) -> Submit {
    let (Ok(tool), false, true) = (&state.tool, state.saving, state.dirty) else {
      return None;
    };
    let change = match (&self.context.editing, &state.name) {
      (Editing::New, Ok(name)) => ToolChange::Create(name.clone(), tool.clone()),
      (Editing::Tool(entry), _) => ToolChange::SaveDraft(entry.id, tool.clone()),
      (Editing::New, Err(_)) => return None,
    };
    let name = self.title(ctx);
    let spec = operations::tool_change(&self.context.project, &name, &change, 0);
    let (action, repo_root) = (self.context.change.clone(), self.context.project.repo_root.clone());
    Some(Arc::new(move || {
      action.run((repo_root.clone(), change.clone(), spec.clone()));
    }))
  }

  /// Publishes the saved draft; unsaved changes must be saved first.
  fn publish_button(&self, ctx: &Ctx, state: &EditorState, version: Option<VersionState>) -> Element {
    let next = version
      .and_then(VersionState::published)
      .map_or(1, |published| published.get() + 1);
    let button = ActionButton::new(ctx.t_args("tools.publish", [("version", next.to_string().as_str())]))
      .primary()
      .icon(Icon::Check)
      .id("tool-publish")
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    let publishable = matches!(version, Some(VersionState::Draft { .. }));
    let Editing::Tool(entry) = &self.context.editing else {
      return button.on_click(|| {}).enabled(false).render(ctx);
    };
    if self.kind() != ToolKind::Command {
      return button.render(ctx);
    }
    if !publishable || state.saving || state.dirty {
      return button.on_click(|| {}).enabled(false).render(ctx);
    }
    let change = ToolChange::Publish(entry.id);
    let spec = operations::tool_change(&self.context.project, &entry.name, &change, next);
    let (action, repo_root) = (self.context.change.clone(), self.context.project.repo_root.clone());
    button
      .on_click(move || action.run((repo_root.clone(), change.clone(), spec.clone())))
      .render(ctx)
  }
}

/// Configuration, and the tabs that do not exist yet (`fne6o`).
fn tabs(ctx: &Ctx) -> Element {
  let height = Dimension::Pct(100.0);
  tab_row(
    Dimension::Px(TABS_HEIGHT),
    ExtraSpacing::Step.role(),
    Some(SpacingSize::Section),
    [
      Tab::new("tool-tab-configuration", ctx.t("tools.tabs.configuration"), height).selected(true),
      Tab::new("tool-tab-inputs", ctx.t("tools.tabs.inputs"), height),
      Tab::new("tool-tab-access", ctx.t("tools.tabs.access"), height),
      Tab::new("tool-tab-usage", ctx.t("tools.tabs.usage"), height),
    ],
  )
}

/// "Draft saved · Published v1", "Unsaved changes · Not published", "Published v1", …
fn save_status(ctx: &Ctx, version: Option<VersionState>, dirty: bool) -> Arc<str> {
  let published = version.and_then(VersionState::published);
  let published_text = published.map_or_else(
    || ctx.t("tools.notPublished"),
    |version| ctx.t_args("tools.published", [("version", version.get().to_string().as_str())]),
  );
  let key = match version {
    None => return ctx.t("tools.notSaved"),
    Some(_) if dirty => "tools.unsavedChanges",
    Some(VersionState::Published(_)) => return published_text,
    Some(VersionState::Draft { .. }) => "tools.draftSaved",
    Some(VersionState::StaleDraft { .. }) => "tools.staleDraftSaved",
  };
  ctx.t_args(key, [("published", &*published_text)])
}

/// "v2", or "none" before the first publish.
pub(super) fn version_text(ctx: &Ctx, version: Option<VersionNumber>) -> Arc<str> {
  version.map_or_else(
    || ctx.t("tools.noVersion"),
    |version| ctx.t_args("tools.version", [("version", version.get().to_string().as_str())]),
  )
}
