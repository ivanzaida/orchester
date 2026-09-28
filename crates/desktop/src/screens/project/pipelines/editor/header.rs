//! Above the canvas: the topbar with the version and save state and Publish or Version history
//! (`oWMR7`), the toolbar with Add step, Pipeline settings and the draft's state (`EbAzx`), and
//! the strip of problems that block publishing (`U7y0m`). While a published version is shown,
//! the toolbar leads back to the draft and says the version is read-only.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  name::Name,
  resource::{EditProblem, Step, VersionNumber},
};

use super::{EditorHandle, Selection};
use crate::{
  messages::Message,
  navigation::route::PipelineView,
  pipelines::{
    describe::step_count,
    editor::{EditorState, SaveState},
    fields::unused_step_name,
    problems::{ProblemKind, ProblemView, problem_view},
  },
  screens::project::pipelines::PipelinesNav,
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::surface,
    status_label::topbar_status_label,
  },
};

/// Topbar, toolbar and problem strip heights, and their small buttons (`oWMR7`, `EbAzx`,
/// `U7y0m`, `eo0io`).
const TOPBAR_HEIGHT: f32 = 64.0;
const TOOLBAR_HEIGHT: f32 = 52.0;
const STRIP_HEIGHT: f32 = 48.0;
const SMALL_BUTTON_HEIGHT: f32 = 30.0;

/// Whether a draft save is running, and why the last one failed.
pub struct Saving {
  pub pending: bool,
  pub error: Option<Message>,
}

/// Breadcrumb, version label, save state, Run and Publish (`oWMR7`).
pub fn topbar(
  ctx: &Ctx,
  project: &str,
  state: &EditorState,
  nav: &PipelinesNav,
  saving: &Saving,
  publish: impl Fn() + Send + Sync + 'static,
) -> Element {
  let breadcrumb = nav
    .library_link()
    .on(surface().id("pipeline-breadcrumb-library"))
    .child(
      Text::new(&ctx.t_args("pipelines.library.breadcrumb", [("project", project)]))
        .variant(TypographyStyle::Description),
    );
  let draft = state.draft_number().to_string();
  let label = match state.latest() {
    Some(latest) if !state.has_changes() => topbar_status_label(
      Some(Icon::CircleCheck),
      &ctx.t_args(
        "pipelines.version.publishedLabel",
        [("version", latest.number.to_string().as_str())],
      ),
      "pipeline-version",
    ),
    _ => topbar_status_label(
      None,
      &ctx.t_args("pipelines.version.draftLabel", [("draft", draft.as_str())]),
      "pipeline-version",
    ),
  };
  let status = save_status(ctx, state, saving);
  let run_label = state.latest().map_or_else(
    || ctx.t("pipelines.editor.run"),
    |latest| {
      ctx.t_args(
        "pipelines.editor.runVersion",
        [("version", latest.number.to_string().as_str())],
      )
    },
  );
  let publish_button = if state.has_changes() || state.latest().is_none() {
    ActionButton::new(ctx.t_args("pipelines.editor.publish", [("draft", draft.as_str())]))
      .primary()
      .icon(Icon::Check)
      .id("pipeline-publish")
      .on_click(publish)
      .enabled(state.can_publish())
  } else {
    history_button(ctx, state, nav).id("pipeline-version-history")
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(breadcrumb)
    .child(
      Text::new(&ctx.t("pipelines.editor.breadcrumbSeparator"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .child(
      Text::new(state.name())
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary),
    )
    .child(label)
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&status)
        .variant(TypographyStyle::Caption)
        .id("pipeline-save-status"),
    )
    .child(
      ActionButton::new(run_label)
        .icon(Icon::Play)
        .id("pipeline-run")
        .render(ctx),
    )
    .child(publish_button.render(ctx))
    .into()
}

fn save_status(ctx: &Ctx, state: &EditorState, saving: &Saving) -> Arc<str> {
  if let Some(error) = &saving.error {
    return ctx.t_args("pipelines.editor.saveFailed", [("reason", &*message_text(ctx, error))]);
  }
  let status = match state.save_state() {
    SaveState::Invalid => Message::new("pipelines.editor.notSaved"),
    SaveState::Pending => Message::new("pipelines.editor.saving"),
    SaveState::Saved if saving.pending => Message::new("pipelines.editor.saving"),
    SaveState::Saved => match (state.has_draft(), state.latest()) {
      (true, Some(latest)) => Message::new("pipelines.editor.draftSaved").with("version", latest.number.to_string()),
      (true, None) => Message::new("pipelines.editor.draftSavedNeverPublished"),
      (false, _) => Message::new("pipelines.editor.noDraftChanges").with("draft", state.draft_number().to_string()),
    },
  };
  message_text(ctx, &status)
}

/// Opens the version history on the latest version.
fn history_button(ctx: &Ctx, state: &EditorState, nav: &PipelinesNav) -> ActionButton {
  let button = ActionButton::new(ctx.t("pipelines.editor.versionHistory")).icon(Icon::History);
  let Some(latest) = state.latest() else {
    return button;
  };
  button.link(nav.link(state.id(), PipelineView::Version(latest.number)))
}

fn toolbar_row() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
}

fn small_button(ctx: &Ctx, button: ActionButton) -> Element {
  button.height(Dimension::Px(SMALL_BUTTON_HEIGHT)).render(ctx)
}

/// Add step, Pipeline settings and what the draft's state means for publishing (`EbAzx`).
/// While the draft has changes, the topbar shows Publish, so Version history is here.
pub fn toolbar(ctx: &Ctx, state: &EditorState, handle: &EditorHandle, nav: &PipelinesNav) -> Element {
  let (add, settings) = (handle.clone(), handle.clone());
  let new_step = new_step_name(ctx, state.edit().steps());
  let small = |button: ActionButton| small_button(ctx, button);
  let (glyph, color, summary) = toolbar_summary(ctx, state);
  let history = (state.latest().is_some() && state.has_changes())
    .then(|| small(history_button(ctx, state, nav).id("pipeline-toolbar-history")));
  toolbar_row()
    .child(small(
      ActionButton::new(ctx.t("pipelines.editor.addStep"))
        .icon(Icon::Plus)
        .id("pipeline-add-step")
        .on_click(move || add_step(&add, new_step.clone())),
    ))
    .child(small(
      ActionButton::new(ctx.t("pipelines.editor.settings"))
        .icon(Icon::SlidersHorizontal)
        .id("pipeline-settings")
        .on_click(move || settings.select(Selection::Pipeline)),
    ))
    .with_children(history)
    .child(Spacer::new().flex(1.0))
    .child(icon(glyph, IconSize::Dismiss, color.clone()))
    .child(
      Text::new(&summary)
        .variant(TypographyStyle::Caption)
        .color(color)
        .id("pipeline-state"),
    )
    .into()
}

/// While a published version is shown: back to the draft, and that the version is read-only.
pub fn version_toolbar(ctx: &Ctx, state: &EditorState, nav: &PipelinesNav, version: VersionNumber) -> Element {
  let id = state.id();
  toolbar_row()
    .child(small_button(
      ctx,
      ActionButton::new(ctx.t("pipelines.history.backToEditor"))
        .icon(Icon::ArrowLeft)
        .id("pipeline-history-back")
        .link(nav.link(id, PipelineView::Draft)),
    ))
    .child(Spacer::new().flex(1.0))
    .child(icon(Icon::Lock, IconSize::Dismiss, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args(
        "pipelines.history.readOnly",
        [("version", version.to_string().as_str())],
      ))
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextSecondary)
      .id("pipeline-state"),
    )
    .into()
}

/// The name of a step added now, unique among `steps`.
pub fn new_step_name(ctx: &Ctx, steps: &[Step]) -> Name {
  unused_step_name(steps, &ctx.t("pipelines.editor.newStep"), |number| {
    ctx
      .t_args(
        "pipelines.editor.newStepNumbered",
        [("number", number.to_string().as_str())],
      )
      .to_string()
  })
}

/// Adds a step after the selected one and selects it.
pub fn add_step(handle: &EditorHandle, name: Name) {
  let after = match handle.selection.get() {
    Selection::Step(key) => Some(key),
    Selection::Pipeline => None,
  };
  if let Some(key) = handle.edit(|edit| edit.add_step(name, after.as_ref())) {
    handle.select(Selection::Step(key));
  }
}

fn toolbar_summary(ctx: &Ctx, state: &EditorState) -> (Icon, PaletteColor, String) {
  let steps = message_text(ctx, &step_count(state.edit().steps().len())).to_string();
  let draft = state.draft_number().to_string();
  let problems = state.problems();
  let text = |key: &str, args: &[(&str, &str)]| {
    let mut all = vec![("steps", steps.as_str())];
    all.extend_from_slice(args);
    ctx.t_args(key, all).to_string()
  };
  if problems == [EditProblem::NoSteps] {
    let summary = text("pipelines.toolbar.noSteps", &[("draft", &draft)]);
    return (Icon::CircleDashed, PaletteColor::TextSecondary, summary);
  }
  if !problems.is_empty() {
    let count = problems.len().to_string();
    let key = if problems.len() == 1 {
      "pipelines.toolbar.problems.one"
    } else {
      "pipelines.toolbar.problems.other"
    };
    let summary = text(key, &[("count", &count), ("draft", &draft)]);
    return (Icon::AlertCircle, PaletteColor::TextPrimary, summary);
  }
  match state.latest() {
    Some(latest) if !state.has_changes() => {
      let version = latest.number.to_string();
      let summary = text("pipelines.toolbar.published", &[("version", &version)]);
      (Icon::CircleCheck, PaletteColor::TextSecondary, summary)
    }
    _ => (
      Icon::CircleCheck,
      PaletteColor::TextSecondary,
      text("pipelines.toolbar.valid", &[]),
    ),
  }
}

/// What blocks publishing, each a button that selects the step it is about (`U7y0m`). Not
/// shown for a pipeline without steps: the canvas says how to add one.
pub fn problems_strip(ctx: &Ctx, state: &EditorState, handle: &EditorHandle) -> Option<Element> {
  let problems = state.problems();
  if problems.is_empty() || problems == [EditProblem::NoSteps] {
    return None;
  }
  let then = ctx.t("pipelines.flow.then");
  let views: Vec<ProblemView> = problems
    .iter()
    .map(|problem| problem_view(state.edit(), problem, &then))
    .collect();
  let count = views.len().to_string();
  let summary = match (state.latest(), views.len()) {
    (Some(latest), 1) => ctx.t_args(
      "pipelines.problems.blocked.oneRunning",
      [("version", latest.number.to_string().as_str())],
    ),
    (Some(latest), _) => ctx.t_args(
      "pipelines.problems.blocked.otherRunning",
      [
        ("count", count.as_str()),
        ("version", latest.number.to_string().as_str()),
      ],
    ),
    (None, 1) => ctx.t("pipelines.problems.blocked.one"),
    (None, _) => ctx.t_args("pipelines.problems.blocked.other", [("count", count.as_str())]),
  };
  // The problem buttons wrap onto more lines, so every problem stays visible and selectable.
  let buttons = Row::new()
    .flex(1.0)
    .wrap()
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .justify(Justify::End)
    .with_children(views.iter().map(|view| problem_button(ctx, view, handle)));
  let strip = Row::new()
    .id("pipeline-problems")
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(STRIP_HEIGHT))
    .padding_horizontal(SpacingSize::Lg)
    .padding_vertical(SpacingSize::Sm)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(Icon::AlertCircle, IconSize::Dismiss, PaletteColor::TextPrimary))
    .child(
      Text::new(&summary)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(buttons);
  Some(strip.into())
}

fn problem_button(ctx: &Ctx, view: &ProblemView, handle: &EditorHandle) -> Element {
  let glyph = match view.kind {
    ProblemKind::Unassigned => Icon::UserX,
    ProblemKind::UnassignedTool => Icon::Wrench,
    ProblemKind::Cycle => Icon::Repeat,
    ProblemKind::NoSteps | ProblemKind::Rule => Icon::AlertCircle,
  };
  let button = ActionButton::new(message_text(ctx, &view.message))
    .icon(glyph)
    .id("pipeline-problem")
    .height(Dimension::Px(SMALL_BUTTON_HEIGHT));
  let button = match view.step.clone() {
    Some(step) => {
      let handle = handle.clone();
      button.on_click(move || handle.select(Selection::Step(step.clone())))
    }
    None => button,
  };
  button.render(ctx)
}
