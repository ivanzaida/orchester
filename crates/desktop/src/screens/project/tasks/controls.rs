//! Query controls and selection bar of J9467; pipeline assignment is implemented, not drawn yet.
use std::{collections::BTreeSet, path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Text, TextOverflow},
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
  messages::Message,
  tasks::{
    browse::{PipelineFilter, TaskSort},
    data::ProjectTasks,
    group::{self, hold_messages, problem_message},
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

/// The selection bar (`OjUxo`): the selection and its readiness, then its actions. The readiness
/// gives way with an ellipsis; the actions keep their width. In a window narrower than
/// `TASK_TABLE_PIPELINE_MIN_WINDOW` the actions go on a second line (implemented, not drawn yet).
pub fn selection_bar(
  ctx: &Ctx,
  screen: &TasksScreen,
  root: &Path,
  tasks: &ProjectTasks,
  visible: Arc<Vec<TaskRow>>,
) -> Element {
  let (ready, readiness) = readiness(ctx, screen, tasks);
  let summary = selection_summary(ctx, screen, &readiness, visible);
  let actions = selection_actions(ctx, screen, root, tasks, ready);
  // Reading the window's width re-renders the bar as the window resizes.
  let narrow = ctx.window().logical_width() < crate::theme::TASK_TABLE_PIPELINE_MIN_WINDOW;
  let bar = |content: Element| {
    Column::new()
      .id("task-selection")
      .width(Dimension::Pct(100.0))
      .padding_horizontal(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .rounded(RadiusSize::Sm)
      .child(content)
  };
  if !narrow {
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SELECTION_HEIGHT))
      .spacing(ExtraSpacing::Group.role())
      .align_items(lurq::layout::Alignment::Center)
      .child(summary)
      .child(actions);
    return bar(line.into()).into();
  }
  let lines = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(SELECTION_BUTTON_HEIGHT))
        .align_items(lurq::layout::Alignment::Center)
        .child(summary),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .justify(lurq::layout::layout_kind::Justify::End)
        .child(actions),
    );
  bar(lines.into()).into()
}

/// Select all, the count and how the selection stands; the last gives way with an ellipsis.
fn selection_summary(ctx: &Ctx, screen: &TasksScreen, readiness: &str, visible: Arc<Vec<TaskRow>>) -> Element {
  let selected = screen.selected.get();
  let all = !visible.is_empty() && visible.iter().all(|row| selected.0.contains(&row.id));
  let selection = screen.selected.clone();
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Group.role())
    .align_items(lurq::layout::Alignment::Center)
    .child(checkbox("tasks-select-all", all, move || {
      selection.update(|s| s.toggle_all(&visible))
    }))
    .child(
      Text::new(&ctx.t_args("tasks.bulk.selected", [("count", selected.0.len().to_string())]))
        .variant(TypographyStyle::FieldLabel)
        .nowrap(),
    )
    .child(
      Element::from(
        Text::new(readiness)
          .variant(TypographyStyle::Caption)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex(1.0),
      )
      .id("task-selection-readiness"),
    )
    .into()
}

/// Return to drafts when it applies, Set pipeline and Run selected, each at its full width.
fn selection_actions(ctx: &Ctx, screen: &TasksScreen, root: &Path, tasks: &ProjectTasks, ready: usize) -> Element {
  let selected = screen.selected.get();
  let count = selected.0.len();
  let releasable = group::releasable(tasks, &selected.0);
  let (assign, review) = (screen.assigning.clone(), screen.reviewing.clone());
  let pending = screen.assign.state().get().is_pending();
  Row::new()
    .spacing(ExtraSpacing::Group.role())
    .align_items(lurq::layout::Alignment::Center)
    .with_children((!releasable.is_empty()).then(|| release_button(ctx, screen, root, releasable)))
    .child(
      ActionButton::new(ctx.t("tasks.selection.setPipeline"))
        .icon(Icon::Workflow)
        .id("set-pipeline")
        .in_form()
        .height(Dimension::Px(SELECTION_BUTTON_HEIGHT))
        .on_click(move || assign.set(true))
        .enabled(count > 0 && !pending)
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
        .enabled(count > 0 && count == ready && !pending)
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

/// Why some selected tasks cannot run: the reason, the selection problem it comes from (none
/// for a group's hold) and the tasks it applies to.
struct Reason {
  message: Message,
  problem: Option<SelectionProblem>,
  tasks: Vec<String>,
}

impl Reason {
  /// One line: the task and its reason, or how many tasks, in words that fit a count.
  fn line(&self, ctx: &Ctx) -> Arc<str> {
    let reason = message_text(ctx, &self.message).to_string();
    let [task] = self.tasks.as_slice() else {
      let count = [("count", self.tasks.len().to_string())];
      return match self.problem {
        Some(SelectionProblem::HasRun) => ctx.t_args("tasks.bulk.hasRunMany", count),
        Some(SelectionProblem::NotDraft) => ctx.t_args("tasks.bulk.notDraftMany", count),
        _ => ctx.t_args(
          "tasks.bulk.notReadyMany",
          [("count", self.tasks.len().to_string()), ("reason", reason)],
        ),
      };
    };
    ctx.t_args("tasks.bulk.notReady", [("task", task.clone()), ("reason", reason)])
  }
}

/// Why selected tasks cannot run, one line per reason: a queued task its group holds says why the
/// group holds it, and tasks that share a reason share its line.
fn reasons(tasks: &ProjectTasks, selected: &BTreeSet<TaskId>) -> Vec<Reason> {
  let published: Vec<_> = tasks.pipelines.iter().map(|p| p.id).collect();
  let mut reasons: Vec<Reason> = Vec::new();
  for task in tasks.tasks.iter().filter(|t| selected.contains(&t.id())) {
    let has_run = tasks.phases.contains_key(&task.id());
    let held = (!has_run)
      .then(|| tasks.group_holds.get(&task.id()))
      .flatten()
      .and_then(|hold| hold_messages(hold).into_iter().next());
    let found: Vec<_> = match held {
      Some(reason) => vec![(reason, None)],
      None => task_selection::problems(task, has_run, &published)
        .into_iter()
        .map(|problem| (problem_message(&problem), Some(problem)))
        .collect(),
    };
    for (message, problem) in found {
      let key = task.key().to_string();
      match reasons.iter_mut().find(|reason| reason.message == message) {
        Some(reason) => reason.tasks.push(key),
        None => reasons.push(Reason {
          message,
          problem,
          tasks: vec![key],
        }),
      }
    }
  }
  reasons
}

pub fn problems(ctx: &Ctx, screen: &TasksScreen, tasks: &ProjectTasks) -> Option<Element> {
  let selected = screen.selected.get();
  let mut messages: Vec<Element> = reasons(tasks, &selected.0)
    .iter()
    .map(|reason| field_error(&reason.line(ctx)))
    .collect();
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
  (!messages.is_empty()).then(|| {
    Column::new()
      .id("task-problems")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(messages)
      .into()
  })
}
