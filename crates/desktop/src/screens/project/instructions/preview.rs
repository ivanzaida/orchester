//! Injection preview (`RGk5C`): for a chosen task, agent step, run and injection event (`TnjFo`),
//! the prompt's sources in assembly order (`SV0BN`), the rules it leaves out and why, and View full
//! prompt. The same context is chosen in the resolved prompt and the variable catalog.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Select, Spacer, Text, TextOverflow},
  core::{Signal, SignalValue},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{resource::InjectionEvent, run::Composition};

use super::{copy, editor::InstructionEditor, fields::trigger_row};
use crate::{
  background,
  instructions::{
    data::InstructionEntry,
    form::Picked,
    preview::{PreviewCatalog, PreviewChoice, PreviewProblem, Resolved},
  },
  messages::{self, Message},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::select_style,
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// Panel width, context select height and source row height (`RGk5C`, `iMVsv`, `PfFwy`).
const PANEL_WIDTH: f32 = 320.0;
const CONTEXT_SELECT_HEIGHT: f32 = 32.0;
const SOURCE_ROW_HEIGHT: f32 = 48.0;

/// The catalog as the load future carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Catalog(pub Arc<PreviewCatalog>);

impl lurq::app::component::DevtoolsInspectable for Catalog {}

/// What previews are made of, once read; the reason when it could not be read.
pub(super) type LoadedCatalog = Result<Option<Arc<PreviewCatalog>>, Message>;

impl InstructionEditor {
  /// Reads the preview catalog again whenever the instructions are read again.
  pub(super) fn catalog(&self, ctx: &mut Ctx) -> LoadedCatalog {
    let deps = (self.context.project.repo_root.clone(), self.context.revision.get());
    let load = ctx.future(deps, |(repo_root, _)| async move {
      let catalog = background::run(move || PreviewCatalog::load_in(&repo_root)).await?;
      catalog.map(|catalog| Catalog(Arc::new(catalog))).map_err(|error| {
        tracing::error!(%error, "cannot read what the instruction preview needs");
        messages::store_error(&error)
      })
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Err(error),
      (data, None) => Ok(data.map(|Catalog(catalog)| catalog)),
    }
  }

  pub(super) fn preview_panel(&self, ctx: &Ctx, live: &InstructionEntry, catalog: &LoadedCatalog) -> Element {
    let mut panel = Column::new()
      .id("instruction-preview")
      .width(Dimension::Px(PANEL_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Step.role())
      .spacing(ExtraSpacing::Inline.role())
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("instructions.preview.title")).variant(ExtraTypography::CardTitle.role()));
    let catalog = match catalog {
      Err(error) => return panel.child(note(&message_text(ctx, error))).into(),
      Ok(None) => return panel.into(),
      Ok(Some(catalog)) => catalog,
    };
    let resolved = match catalog.resolve(&self.preview.get()) {
      Ok(resolved) => resolved,
      Err(problem) => return panel.child(note(&ctx.t(problem_key(problem)))).into(),
    };
    let event = self.preview_event.get().0;
    let composition = resolved.compose(self.edited(live), event);
    panel = panel
      .child(self.context_selectors(ctx, catalog, &resolved, live))
      .child(
        Text::new(&ctx.t_args(
          "instructions.preview.sourceCount",
          [("count", composition.sections().len().to_string().as_str())],
        ))
        .variant(ExtraTypography::Overline.role()),
      )
      .child(self.source_rows(ctx, &composition, resolved.pipeline_name()));
    for excluded in composition.excluded() {
      let text = copy::excluded(ctx, excluded, &self.context.data);
      panel = panel.child(Text::new(&text).variant(ExtraTypography::Hint.role()));
    }
    let (open, source) = (self.viewer_open.clone(), self.viewer_source.clone());
    let view = ActionButton::new(ctx.t("instructions.preview.viewFull"))
      .icon(Icon::FileText)
      .id("instruction-view-prompt")
      .fill_width()
      .on_click(move || {
        source.set(0);
        open.set(true);
      })
      .render(ctx);
    panel.child(Spacer::new().flex(1.0)).child(view).into()
  }

  /// The execution context (`TnjFo`): mode, task, agent and step, run and event, and whether
  /// draft rules are substituted.
  pub(super) fn context_selectors(
    &self,
    ctx: &Ctx,
    catalog: &PreviewCatalog,
    resolved: &Resolved<'_>,
    live: &InstructionEntry,
  ) -> Element {
    let run = if resolved.run.is_some() {
      "instructions.preview.recordedRun"
    } else {
      "instructions.preview.newRun"
    };
    let mode = ctx.t_args(
      if live.state.has_draft() {
        "instructions.preview.modeDraft"
      } else {
        "instructions.preview.modePublished"
      },
      [("context", &*ctx.t(run))],
    );
    let phase = ctx.t(if live.state.has_draft() {
      "instructions.preview.draftSubstituted"
    } else {
      "instructions.preview.publishedOnly"
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(Text::new(&mode).variant(ExtraTypography::ContextNote.role()))
      .child(self.task_select(ctx, catalog, resolved))
      .child(self.step_select(ctx, resolved))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(self.run_select(ctx, resolved))
          .child(self.event_select(ctx)),
      )
      .child(Text::new(&phase).variant(ExtraTypography::ContextNote.role()))
      .into()
  }

  pub(super) fn task_select(&self, ctx: &Ctx, catalog: &PreviewCatalog, resolved: &Resolved<'_>) -> Element {
    let current = self.preview.get();
    let shown = resolved.task.task.id();
    let options = catalog
      .tasks()
      .iter()
      .map(|task| {
        let id = task.task.id();
        let value = if id == shown {
          current.clone()
        } else {
          PreviewChoice {
            task: Some(id),
            ..PreviewChoice::default()
          }
        };
        (value, task_label(ctx, &task.task))
      })
      .collect();
    context_select(
      ctx,
      "instruction-preview-task",
      &self.preview,
      options,
      task_label(ctx, &resolved.task.task),
    )
  }

  pub(super) fn step_select(&self, ctx: &Ctx, resolved: &Resolved<'_>) -> Element {
    let current = self.preview.get();
    let options = resolved
      .steps
      .iter()
      .map(|step| {
        let value = PreviewChoice {
          task: Some(resolved.task.task.id()),
          step: Some(step.key.clone()),
          run: current.run,
        };
        let label = ctx.t_args(
          "instructions.preview.agentStep",
          [("role", &*step.role), ("step", &*step.step)],
        );
        (value, label)
      })
      .collect();
    let label = ctx.t_args(
      "instructions.preview.agentStep",
      [("role", resolved.role()), ("step", resolved.step.name.as_str())],
    );
    context_select(ctx, "instruction-preview-step", &self.preview, options, label)
  }

  pub(super) fn run_select(&self, ctx: &Ctx, resolved: &Resolved<'_>) -> Element {
    let choice = |run| PreviewChoice {
      task: Some(resolved.task.task.id()),
      step: Some(resolved.step.key.clone()),
      run,
    };
    let mut options = vec![(choice(None), ctx.t("instructions.preview.newRunOption"))];
    options.extend(
      resolved
        .task
        .runs
        .iter()
        .rev()
        .map(|run| (choice(Some(run.id())), run_label(ctx, run))),
    );
    let label = resolved
      .run
      .map_or_else(|| ctx.t("instructions.preview.newRunOption"), |run| run_label(ctx, run));
    context_select(ctx, "instruction-preview-run", &self.preview, options, label)
  }

  pub(super) fn event_select(&self, ctx: &Ctx) -> Element {
    let options = InjectionEvent::ALL
      .into_iter()
      .map(|event| (Picked(event), copy::event_label(ctx, event)))
      .collect();
    let label = copy::event_label(ctx, self.preview_event.get().0);
    context_select(ctx, "instruction-preview-event", &self.preview_event, options, label)
  }

  /// The prompt's sources in assembly order; a row opens the resolved prompt at that source.
  fn source_rows(&self, ctx: &Ctx, composition: &Composition, pipeline: &str) -> Element {
    let rows = composition.sections().iter().enumerate().map(|(index, section)| {
      let (title, metadata) = copy::source_text(ctx, section, pipeline);
      let (open, source) = (self.viewer_open.clone(), self.viewer_source.clone());
      Button::empty()
        .id(Arc::<str>::from(format!("instruction-source-{index}")))
        .focusable(false)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(SOURCE_ROW_HEIGHT))
        .padding_vertical(ExtraSpacing::Indicator.role())
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Start)
        .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
        .cursor(CursorIcon::Pointer)
        .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
        .on_click(move |_| {
          source.set(index);
          open.set(true);
        })
        .child(number(index))
        .child(source_description(&title, &metadata))
        .child(icon(Icon::ChevronDown, IconSize::Marker, PaletteColor::TextSecondary))
    });
    Column::new().width(Dimension::Pct(100.0)).with_children(rows).into()
  }
}

/// "01", in the source list's mono.
pub(super) fn number(index: usize) -> Element {
  Text::new(&format!("{:02}", index + 1))
    .variant(TypographyStyle::Mono)
    .color(PaletteColor::TextMuted)
    .into()
}

pub(super) fn source_description(title: &str, metadata: &str) -> Element {
  Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(title)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(metadata)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

fn note(text: &str) -> Element {
  Text::new(text).variant(ExtraTypography::Hint.role()).into()
}

pub(super) const fn problem_key(problem: PreviewProblem) -> &'static str {
  match problem {
    PreviewProblem::NoTasks => "instructions.preview.noTasks",
    PreviewProblem::NoPublishedPipeline => "instructions.preview.noPipeline",
    PreviewProblem::NoAgentSteps => "instructions.preview.noAgentSteps",
  }
}

pub(super) fn task_label(ctx: &Ctx, task: &orchester_core::task::Task) -> Arc<str> {
  ctx.t_args(
    "instructions.preview.task",
    [
      ("key", task.key().to_string().as_str()),
      ("title", task.details().title.as_str()),
    ],
  )
}

pub(super) fn run_label(ctx: &Ctx, run: &orchester_core::run::Run) -> Arc<str> {
  let attempt = run.attempts().last().map_or(1, |attempt| attempt.number().get());
  ctx.t_args(
    "instructions.preview.run",
    [
      ("number", format!("{:03}", run.number().get()).as_str()),
      ("attempt", attempt.to_string().as_str()),
    ],
  )
}

/// A context select (`iMVsv`) whose trigger shows `label`: the choice as resolved, which may be a
/// default the signal does not hold yet.
pub(super) fn context_select<T>(
  ctx: &Ctx,
  id: &'static str,
  value: &Signal<T>,
  options: Vec<(T, Arc<str>)>,
  label: Arc<str>,
) -> Element
where
  T: Clone + PartialEq + Send + Sync + 'static + SignalValue,
  Vec<T>: SignalValue,
{
  Column::new()
    .id(id)
    .flex(1.0)
    .width(Dimension::Pct(100.0))
    .child(
      Select::new(value.clone())
        .options(options)
        .trigger(move |_| trigger_row(&label, ExtraTypography::Footnote.role()))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(CONTEXT_SELECT_HEIGHT))
        .style(select_style(ctx, CONTEXT_SELECT_HEIGHT)),
    )
    .into()
}
