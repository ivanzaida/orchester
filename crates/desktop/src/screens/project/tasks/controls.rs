//! Query controls and selection bar of J9467; pipeline assignment is implemented, not drawn yet.
use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Spacer, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  task_selection::{self, SelectionProblem},
};

use super::{
  FILTER_HEIGHT, PIPELINE_FILTER_WIDTH, RUN_SELECTED_WIDTH, SELECTION_BUTTON_HEIGHT, SELECTION_HEIGHT, SORT_WIDTH,
  TasksScreen, selection::checkbox,
};
use crate::{
  tasks::{
    browse::{PipelineFilter, TaskSort},
    data::ProjectTasks,
    group::{self, problem_message},
    list::TaskRow,
  },
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    field::{field_error, select_style, styled_input},
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

fn search(ctx: &Ctx, screen: &TasksScreen) -> Row {
  let input = ctx.theme().form().input.clone();
  let border = if screen.search_focused.get() {
    lurq::node::border::Border::inside(lurq::app::theme::BorderSize::Md, input.border_focus)
  } else {
    lurq::node::border::Border::inside(lurq::app::theme::BorderSize::Sm, input.border)
  };
  let (focus, blur) = (screen.search_focused.clone(), screen.search_focused.clone());
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FILTER_HEIGHT))
    .padding_custom(input.padding)
    .spacing(SpacingSize::Sm)
    .align_items(lurq::layout::Alignment::Center)
    .background(input.background)
    .border(border)
    .rounded(input.radius)
    .child(
      styled_input(ctx, screen.search.clone(), TypographyStyle::FieldLabel)
        .id("tasks-search")
        .on_focus(move || focus.set(true))
        .on_blur(move || blur.set(false))
        .single_line()
        .flex(1.0)
        .placeholder(&ctx.t("tasks.filters.search")),
    )
    .child(icon(Icon::Search, IconSize::Dismiss, PaletteColor::TextSecondary))
}

pub fn filters(ctx: &Ctx, screen: &TasksScreen, tasks: &ProjectTasks) -> Element {
  let pipelines = std::iter::once((PipelineFilter(None), ctx.t("tasks.filters.allPipelines"))).chain(
    tasks
      .pipelines
      .iter()
      .map(|p| (PipelineFilter(Some(p.id)), p.name.clone())),
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Column::new().flex(1.0).child(search(ctx, screen)))
    .child(
      Element::from(
        Select::new(screen.pipeline.clone())
          .options(pipelines)
          .width(Dimension::Px(PIPELINE_FILTER_WIDTH))
          .height(Dimension::Px(FILTER_HEIGHT))
          .style(select_style(ctx, FILTER_HEIGHT)),
      )
      .id("tasks-pipeline-filter"),
    )
    .child(
      Element::from(
        Select::new(screen.sort.clone())
          .options([
            (TaskSort::Manual, ctx.t("tasks.filters.manualOrder")),
            (TaskSort::Newest, ctx.t("tasks.filters.newest")),
            (TaskSort::Title, ctx.t("tasks.filters.title")),
          ])
          .width(Dimension::Px(SORT_WIDTH))
          .height(Dimension::Px(FILTER_HEIGHT))
          .style(select_style(ctx, FILTER_HEIGHT)),
      )
      .id("tasks-sort"),
    )
    .into()
}

/// How the selected tasks stand for a first run: ready drafts, drafts that miss details, and
/// tasks already queued or started, which are neither.
fn readiness(ctx: &Ctx, screen: &TasksScreen, tasks: &ProjectTasks) -> (usize, Arc<str>) {
  let selected = screen.selected.get();
  let published: Vec<_> = tasks.pipelines.iter().map(|p| p.id).collect();
  let (mut ready, mut incomplete, mut started) = (0, 0, 0);
  for task in tasks.tasks.iter().filter(|t| selected.0.contains(&t.id())) {
    let problems = task_selection::problems(task, tasks.phases.contains_key(&task.id()), &published);
    if problems.is_empty() {
      ready += 1;
    } else if problems
      .iter()
      .any(|p| matches!(p, SelectionProblem::NotDraft | SelectionProblem::HasRun))
    {
      started += 1;
    } else {
      incomplete += 1;
    }
  }
  let counts = [
    ("ready", ready.to_string()),
    ("incomplete", incomplete.to_string()),
    ("started", started.to_string()),
  ];
  let text = if started == 0 {
    ctx.t_args("tasks.selection.readiness", counts)
  } else {
    ctx.t_args("tasks.selection.readinessStarted", counts)
  };
  (ready, text)
}

pub fn selection_bar(
  ctx: &Ctx,
  screen: &TasksScreen,
  root: &Path,
  tasks: &ProjectTasks,
  visible: Arc<Vec<TaskRow>>,
) -> Element {
  let selected = screen.selected.get();
  let (ready, readiness) = readiness(ctx, screen, tasks);
  let count = selected.0.len();
  let releasable = group::releasable(tasks, &selected.0);
  let all = !visible.is_empty() && visible.iter().all(|row| selected.0.contains(&row.id));
  let selection = screen.selected.clone();
  let assign = screen.assigning.clone();
  let review = screen.reviewing.clone();
  Row::new()
    .id("task-selection")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SELECTION_HEIGHT))
    .padding_vertical(SpacingSize::Xs)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Group.role())
    .align_items(lurq::layout::Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Sm)
    .child(checkbox("tasks-select-all", all, move || {
      selection.update(|s| s.toggle_all(&visible))
    }))
    .child(
      Text::new(&ctx.t_args("tasks.bulk.selected", [("count", count.to_string())]))
        .variant(TypographyStyle::FieldLabel),
    )
    .child(Text::new(&readiness).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .with_children((!releasable.is_empty()).then(|| release_button(ctx, screen, root, releasable)))
    .child(
      ActionButton::new(ctx.t("tasks.selection.setPipeline"))
        .icon(Icon::Workflow)
        .id("set-pipeline")
        .in_form()
        .height(Dimension::Px(SELECTION_BUTTON_HEIGHT))
        .on_click(move || assign.set(true))
        .enabled(count > 0 && !screen.assign.state().get().is_pending())
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t_args("tasks.bulk.runSelected", [("count", count.to_string())]))
        .primary()
        .icon(Icon::Play)
        .id("run-selected")
        .in_form()
        .size(
          Dimension::Px(RUN_SELECTED_WIDTH),
          Dimension::Px(SELECTION_BUTTON_HEIGHT),
        )
        .on_click(move || review.set(true))
        .enabled(count > 0 && count == ready && !screen.assign.state().get().is_pending())
        .render(ctx),
    )
    .into()
}

/// Takes the selected tasks a held group keeps from starting back to the drafts (implemented,
/// not drawn yet; the secondary selection-bar button beside Set pipeline).
fn release_button(ctx: &Ctx, screen: &TasksScreen, root: &Path, tasks: Vec<TaskId>) -> Element {
  let (action, root) = (screen.release.clone(), root.to_path_buf());
  ActionButton::new(ctx.t("tasks.bulk.releaseHeld"))
    .icon(Icon::Undo2)
    .id("tasks-release-held")
    .in_form()
    .height(Dimension::Px(SELECTION_BUTTON_HEIGHT))
    .on_click(move || action.run((root.clone(), tasks.clone())))
    .enabled(!screen.release.state().get().is_pending())
    .render(ctx)
}

pub fn assignment(ctx: &Ctx, screen: &TasksScreen, root: &Path, tasks: &ProjectTasks) -> Element {
  let options = std::iter::once((PipelineFilter(None), ctx.t("tasks.pipeline.notChosen"))).chain(
    tasks
      .pipelines
      .iter()
      .map(|p| (PipelineFilter(Some(p.id)), p.name.clone())),
  );
  let selected_pipeline = screen.assign_pipeline.get().0;
  let action = screen.assign.clone();
  let selected: Vec<_> = screen.selected.get().0.iter().copied().collect();
  let root = root.to_path_buf();
  let dismiss = screen.assigning.clone();
  let pending = screen.assign.state().get().is_pending();
  Row::new()
    .id("tasks-pipeline-assignment")
    .spacing(SpacingSize::Sm)
    .child(
      Element::from(
        Select::new(screen.assign_pipeline.clone())
          .options(options)
          .width(Dimension::Px(PIPELINE_FILTER_WIDTH))
          .style(select_style(ctx, FILTER_HEIGHT)),
      )
      .id("tasks-assignment-pipeline"),
    )
    .child(
      ActionButton::new(ctx.t("tasks.bulk.applyPipeline"))
        .id("tasks-apply-pipeline")
        .in_form()
        .on_click(move || {
          if let Some(pipeline) = selected_pipeline {
            action.run((root.clone(), selected.clone(), pipeline));
          }
        })
        .enabled(selected_pipeline.is_some() && !pending)
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("common.cancel"))
        .id("tasks-cancel-pipeline")
        .in_form()
        .on_click(move || dismiss.set(false))
        .enabled(!pending)
        .render(ctx),
    )
    .into()
}

pub fn problems(ctx: &Ctx, screen: &TasksScreen, tasks: &ProjectTasks) -> Vec<Element> {
  let selected = screen.selected.get();
  let published: Vec<_> = tasks.pipelines.iter().map(|p| p.id).collect();
  let mut messages = Vec::new();
  for task in tasks.tasks.iter().filter(|t| selected.0.contains(&t.id())) {
    for problem in task_selection::problems(task, tasks.phases.contains_key(&task.id()), &published) {
      messages.push(field_error(&ctx.t_args(
        "tasks.bulk.notReady",
        [
          ("task", task.key().to_string()),
          ("reason", message_text(ctx, &problem_message(&problem)).to_string()),
        ],
      )));
    }
  }
  if let Some(error) = screen.release.state().get().error.as_ref() {
    messages.push(field_error(&message_text(ctx, error)));
  }
  let state = screen.assign.state().get();
  if let Some(error) = state.error.as_ref() {
    messages.push(field_error(&message_text(ctx, error)));
  }
  if !state.is_pending()
    && let Some(skipped) = state.data.as_ref()
  {
    messages.extend(skipped.iter().map(|m| field_error(&message_text(ctx, m))));
  }
  messages
}
