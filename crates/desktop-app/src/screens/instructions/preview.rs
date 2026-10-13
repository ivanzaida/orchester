//! Effective prompt preview ShcL9 and full viewer fTuZZ, built from workspace composition.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
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
    field::{SelectField, field, field_error},
  },
};

// Full prompt header CKzcD and source index IAt1E.
const VIEWER_HEADER_HEIGHT: f32 = 52.0;
const SOURCE_INDEX_WIDTH: f32 = 300.0;

pub fn panel(ctx: &Ctx, state: &Panels, props: &Props, catalog: &PreviewCatalog) -> Element {
  let resolved = match catalog.resolve(&state.choice.get()) {
    Ok(resolved) => resolved,
    Err(problem) => return problem_text(ctx, problem),
  };
  let controls = controls(ctx, state, catalog, &resolved);
  let Some(composition) = compose(props, state, &resolved) else {
    return Column::new()
      .child(controls)
      .child(Text::new(&ctx.t("instructions.fixBeforePreview")).variant(TypographyStyle::Description))
      .into();
  };
  let sources = sources::list(ctx, &composition);
  let open = props.panel.clone();
  let copy_button = copy_button(ctx, state, props, &composition);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(Text::new(&ctx.t("instructions.previewContext")).variant(instructions::overline()))
    .child(controls)
    .child(
      Text::new(&ctx.t(if props.entry.as_ref().is_some_and(|e| e.state.has_draft()) {
        "instructions.draftSubstituted"
      } else {
        "instructions.publishedOnly"
      }))
      .variant(TypographyStyle::Caption),
    )
    .child(unavailable(ctx, &composition))
    .child(sources)
    .child(
      ActionButton::new(
        "instruction-full-prompt",
        ButtonKind::Secondary,
        ctx.t("instructions.viewPrompt"),
      )
      .on_click(move || open.set(Panel::Full)),
    )
    .child(copy_button)
    .child(Text::new(&ctx.t("instructions.secretNote")).variant(TypographyStyle::Caption))
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
fn controls(ctx: &Ctx, state: &Panels, catalog: &PreviewCatalog, resolved: &Resolved<'_>) -> Element {
  let mut current = state.choice.get();
  current.task = Some(resolved.task.task.id());
  current.step = Some(resolved.step.key.clone());
  current.run = resolved.run.map(|run| run.id());
  if state.choice.get_untracked() != current {
    state.choice.set(current.clone());
  }
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
    field(
      id,
      Some(&ctx.t(key)),
      SelectField::new(id, ctx.t(key), state.choice.clone(), options).render(ctx),
      None,
    )
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(choose("instruction-preview-task", "instructions.task", tasks))
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
pub fn full(ctx: &mut Ctx, state: &Panels, props: &Props, catalog: &PreviewCatalog) -> Element {
  let resolved = match catalog.resolve(&state.choice.get()) {
    Ok(resolved) => resolved,
    Err(problem) => return problem_text(ctx, problem),
  };
  let Some(composition) = compose(props, state, &resolved) else {
    return Text::new(&ctx.t("instructions.fixBeforePreview")).into();
  };
  let mut contents = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(Text::new(&ctx.t("instructions.effective")).variant(TypographyStyle::Title))
    .child(Text::new(&ctx.t("instructions.viewerNote")).variant(TypographyStyle::Caption))
    .child(unavailable(ctx, &composition));
  for (index, section) in composition.sections().iter().enumerate() {
    contents = contents.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(&ctx.t_args(
            "instructions.sourceTitle",
            [("number", (index + 1).to_string()), ("name", section.name.clone())],
          ))
          .variant(TypographyStyle::Label),
        )
        .child(
          Text::new(&section.text)
            .variant(instructions::prompt_text())
            .selectable(true),
        ),
    );
  }
  full_frame(ctx, state, props, &composition, contents)
}
fn copy_button(ctx: &Ctx, state: &Panels, props: &Props, composition: &Composition) -> ActionButton {
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
  let button = ActionButton::new("instruction-copy-prompt", ButtonKind::Secondary, label).on_click(move || {
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
  field(
    "instruction-preview-event-field",
    Some(&ctx.t("instructions.event")),
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
    None,
  )
  .into()
}
fn full_frame(ctx: &mut Ctx, state: &Panels, props: &Props, composition: &Composition, contents: Column) -> Element {
  let back = props.panel.clone();
  let copy = copy_button(ctx, state, props, composition);
  let entry_name = props
    .entry
    .as_ref()
    .map_or_else(|| ctx.t("instructions.new"), |entry| entry.name.clone());
  let panel = props.panel.clone();
  let crumbs = crate::ui::breadcrumb::breadcrumb(
    ctx,
    "instruction-viewer-crumbs",
    crate::ui::breadcrumb::CrumbSeparator::Chevron,
    vec![
      crate::ui::breadcrumb::Crumb::link(ctx.t("instructions.title"), move || panel.set(Panel::Preview)),
      crate::ui::breadcrumb::Crumb::here(entry_name),
    ],
  );
  let header = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(VIEWER_HEADER_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Tabs.role()))
    .spacing(SpacingSize::Md)
    .align_items(lurq::layout::Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(crumbs)
    .child(
      Text::new(&ctx.t("instructions.effective"))
        .variant(TypographyStyle::Title)
        .flex(1.0),
    )
    .child(Text::new(&ctx.t("instructions.readOnly")).variant(TypographyStyle::Caption))
    .child(copy);
  let sources = Column::new()
    .width(Dimension::Px(SOURCE_INDEX_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Tabs.role())
    .spacing(SpacingSize::Md)
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t("instructions.previewContext")).variant(instructions::overline()))
    .child(sources::list(ctx, composition))
    .child(Text::new(&ctx.t("instructions.viewerNote")).variant(TypographyStyle::Caption));
  let footer = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .child(
      Text::new(&ctx.t("instructions.secretNote"))
        .variant(TypographyStyle::Caption)
        .flex(1.0),
    )
    .child(
      ActionButton::new("instruction-viewer-back", ButtonKind::Ghost, ctx.t("instructions.back"))
        .on_click(move || back.set(Panel::Preview)),
    );
  Column::new()
    .id("instruction-full-viewer")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(sources)
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .height(Dimension::Pct(100.0))
            .child(
              lurq::components::ScrollVertical::new(contents.padding(ExtraSpacing::Tabs.role()))
                .flex(1.0)
                .min_height(Dimension::Px(0.0)),
            )
            .child(footer),
        ),
    )
    .into()
}
