//! Effective prompt preview ShcL9 and full viewer fTuZZ, built from workspace composition.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::InjectionEvent,
  run::{Composition, InstructionVersion},
};
use orchester_workspace::instructions::{
  form::Picked,
  preview::{EditedInstruction, PreviewCatalog, PreviewProblem, Resolved},
};

use super::{
  panels::{Panel, Panels, Props},
  sources,
};
use crate::{
  theme::{ExtraSpacing, instructions},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{SelectField, field_error},
    icon::Icon,
  },
};

// Compact labels in the context selectors of ShcL9.
const CONTEXT_LABEL_WIDTH: f32 = 76.0;

pub fn mode_key(props: &Props) -> &'static str {
  if props.entry.as_ref().is_some_and(|entry| entry.state.has_draft()) {
    "instructions.previewDraftContext"
  } else {
    "instructions.previewContext"
  }
}
pub fn panel(ctx: &Ctx, state: &Panels, props: &Props, catalog: &PreviewCatalog) -> Element {
  let resolved = match catalog.resolve(&state.choice.get()) {
    Ok(resolved) => resolved,
    Err(problem) => {
      return Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(Text::new(&ctx.t(mode_key(props))).variant(instructions::overline()))
        .with_children((!catalog.tasks().is_empty()).then(|| task_field(ctx, state, catalog)))
        .child(problem_text(ctx, problem))
        .into();
    }
  };
  let controls = controls(ctx, state, catalog, &resolved);
  let Some(composition) = compose(props, state, &resolved) else {
    return Column::new()
      .child(controls)
      .child(Text::new(&ctx.t("instructions.fixBeforePreview")).variant(TypographyStyle::Description))
      .into();
  };
  let sources = sources::list(
    ctx,
    state,
    &composition,
    props.entry.as_ref().map(|entry| entry.id),
    false,
  );
  let open = props.panel.clone();
  let copy_button = copy_button(ctx, state, props, &composition);
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t(mode_key(props))).variant(instructions::overline()))
    .child(controls)
    .child(status(ctx, props))
    .child(unavailable(ctx, &composition))
    .child(
      ScrollVertical::new(sources)
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0)),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(lurq::layout::Alignment::Center)
        .child(
          ActionButton::new(
            "instruction-full-prompt",
            ButtonKind::Secondary,
            ctx.t("instructions.viewPrompt"),
          )
          .icon(Icon::Eye)
          .on_click(move || open.set(Panel::Full)),
        )
        .child(copy_button),
    )
    .into()
}
pub fn compose(props: &Props, state: &Panels, resolved: &Resolved<'_>) -> Option<Composition> {
  let entry = props.entry.as_ref().filter(|_| props.valid)?;
  let version = if entry.state.has_draft() {
    InstructionVersion::Draft {
      based_on: entry.based_on(),
    }
  } else {
    InstructionVersion::Published {
      number: entry
        .state
        .published()
        .expect("an instruction without a draft is published"),
    }
  };
  Some(resolved.compose(
    EditedInstruction {
      id: entry.id,
      name: &entry.name,
      slug: &entry.slug,
      body: &entry.body,
      version,
    },
    state.event.get().0,
  ))
}
fn task_field(ctx: &Ctx, state: &Panels, catalog: &PreviewCatalog) -> Element {
  let current = state.choice.get();
  let tasks = catalog
    .tasks()
    .iter()
    .map(|task| {
      let mut choice = current.clone();
      if choice.task != Some(task.task.id()) {
        choice.task = Some(task.task.id());
        choice.step = None;
        choice.run = None;
      }
      lurq::components::SelectOption::new(
        choice,
        format!("{} · {}", task.task.key(), task.task.details().title.as_str()),
      )
    })
    .collect::<Vec<_>>();
  let id = "instruction-preview-task";
  context_row(
    ctx,
    "instructions.task",
    SelectField::new(id, ctx.t("instructions.task"), state.choice.clone(), tasks)
      .placeholder(ctx.t("instructions.choose"))
      .render(ctx),
  )
  .into()
}
fn controls(ctx: &Ctx, state: &Panels, catalog: &PreviewCatalog, resolved: &Resolved<'_>) -> Element {
  let mut current = state.choice.get();
  current.task = Some(resolved.task.task.id());
  current.step = Some(resolved.step.key.clone());
  current.run = resolved.run.map(|run| run.id());
  if state.choice.get_untracked() != current {
    state.choice.set(current.clone());
  }
  let steps = resolved
    .steps
    .iter()
    .map(|step| {
      let mut choice = current.clone();
      choice.step = Some(step.key.clone());
      lurq::components::SelectOption::new(choice, format!("{} · {}", step.role, step.step))
    })
    .collect::<Vec<_>>();
  let mut runs = vec![lurq::components::SelectOption::new(
    {
      let mut choice = current.clone();
      choice.run = None;
      choice
    },
    ctx.t("instructions.newRun"),
  )];
  runs.extend(resolved.task.runs.iter().map(|run| {
    let mut choice = current.clone();
    choice.run = Some(run.id());
    lurq::components::SelectOption::new(
      choice,
      ctx.t_args("instructions.run", [("number", run.number().to_string())]),
    )
  }));
  let choose = |id, key, options| {
    context_row(
      ctx,
      key,
      SelectField::new(id, ctx.t(key), state.choice.clone(), options).render(ctx),
    )
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(task_field(ctx, state, catalog))
    .child(choose("instruction-preview-step", "instructions.agentStep", steps))
    .child(choose("instruction-preview-run", "instructions.runLabel", runs))
    .child(event_field(ctx, state))
    .into()
}
pub fn unavailable(ctx: &Ctx, composition: &Composition) -> Element {
  let mut notices = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role());
  for section in composition.sections() {
    for variable in &section.unavailable {
      notices = notices.child(field_error(
        "instruction-unavailable-variable",
        &ctx.t_args(
          "instructions.missingVariable",
          [("variable", variable.name()), ("source", section.name.as_str())],
        ),
      ));
    }
  }
  notices.into()
}
fn problem_text(ctx: &Ctx, problem: PreviewProblem) -> Element {
  Text::new(&ctx.t(match problem {
    PreviewProblem::NoTasks => "instructions.noTasks",
    PreviewProblem::NoPublishedPipeline => "instructions.noPipeline",
    PreviewProblem::NoAgentSteps => "instructions.noAgentSteps",
  }))
  .id("instruction-preview-unavailable")
  .variant(TypographyStyle::Description)
  .into()
}
pub(super) fn copy_button(ctx: &Ctx, state: &Panels, props: &Props, composition: &Composition) -> ActionButton {
  let receipt = super::panels::CopyReceipt {
    composition: composition.clone(),
    choice: state.choice.get(),
    event: state.event.get().0,
    entry: props.entry.clone(),
  };
  match state.copied.get() {
    super::panels::Copied::Done(previous) | super::panels::Copied::Failed(previous) if previous != receipt => {
      state.copied.set(super::panels::Copied::Not);
    }
    _ => {}
  }
  let copy = composition.text().to_owned();
  let clipboard = state.clipboard.clone();
  let copied = state.copied.clone();
  let label = ctx.t(match copied.get() {
    super::panels::Copied::Not => "instructions.copyPrompt",
    super::panels::Copied::Done(_) => "instructions.copied",
    super::panels::Copied::Failed(_) => "instructions.copyFailed",
  });
  let button = ActionButton::new(
    "instruction-copy-prompt",
    if props.panel.get() == Panel::Full {
      ButtonKind::Secondary
    } else {
      ButtonKind::Ghost
    },
    label,
  )
  .icon(Icon::Copy)
  .on_click(move || {
    copied.set(if clipboard.copy(&copy) {
      super::panels::Copied::Done(receipt.clone())
    } else {
      super::panels::Copied::Failed(receipt.clone())
    })
  });
  if composition
    .sections()
    .iter()
    .any(|section| !section.unavailable.is_empty())
  {
    button.disabled()
  } else {
    button
  }
}
fn event_field(ctx: &Ctx, state: &Panels) -> Element {
  context_row(
    ctx,
    "instructions.event",
    SelectField::new(
      "instruction-preview-event",
      ctx.t("instructions.event"),
      state.event.clone(),
      InjectionEvent::ALL
        .into_iter()
        .map(|event| {
          lurq::components::SelectOption::new(
            Picked(event),
            ctx.t(match event {
              InjectionEvent::Start => "instructions.start",
              InjectionEvent::Resume => "instructions.resume",
            }),
          )
        })
        .collect(),
    )
    .render(ctx),
  )
  .into()
}

fn context_row(ctx: &Ctx, label: &str, control: Element) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(lurq::layout::Alignment::Center)
    .child(
      Text::new(&ctx.t(label))
        .variant(TypographyStyle::Description)
        .width(Dimension::Px(CONTEXT_LABEL_WIDTH)),
    )
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(control))
}
fn status(ctx: &Ctx, props: &Props) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(crate::ui::icon::icon(
      if props.entry.as_ref().is_some_and(|e| e.state.has_draft()) {
        Icon::FilePen
      } else {
        Icon::CircleCheck
      },
      crate::theme::IconSize::Xs,
      lurq::app::theme::PaletteColor::TextMuted,
    ))
    .child(
      Text::new(&ctx.t(if props.entry.as_ref().is_some_and(|e| e.state.has_draft()) {
        "instructions.draftSubstituted"
      } else {
        "instructions.publishedOnly"
      }))
      .variant(TypographyStyle::Caption)
      .flex(1.0),
    )
}
