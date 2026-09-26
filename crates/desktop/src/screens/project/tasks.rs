//! Tasks (02.03 `J9467`): the project's backlog with its tabs. All tasks and Drafts work;
//! queueing, review and completion do not exist yet, so their tabs are unavailable, and so are
//! selecting tasks, running them, creating several at once, searching, filtering and sorting.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, VirtualizedList},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  ProjectNav, Section, TasksState, page,
  task_rows::{QueueRow, table_heading},
};
use crate::{
  tasks::{
    data::ProjectTasks,
    list::{self, DraftSummary, TaskTab},
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    field::{ValueTone, value_box_sized},
    icon::{Icon, IconSize, icon},
    message_text,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
};

/// Topbar and its buttons, tab row, filter widths, selection bar and footer (`is9Lk`,
/// `gWN2C`, `YGDQj`, `XxplC`, `d5tAIZ`, `OjUxo`, `oYw3J`, `Bpth7`).
const TOPBAR_HEIGHT: f32 = 64.0;
const TOPBAR_BUTTON_HEIGHT: f32 = 36.0;
const CREATE_MULTIPLE_WIDTH: f32 = 142.0;
const NEW_TASK_WIDTH: f32 = 116.0;
const TAB_ROW_HEIGHT: f32 = 42.0;
const FILTER_HEIGHT: f32 = 36.0;
const PIPELINE_FILTER_WIDTH: f32 = 184.0;
const SORT_WIDTH: f32 = 164.0;
const SELECTION_HEIGHT: f32 = 44.0;
const SELECTION_BUTTON_HEIGHT: f32 = 32.0;
const RUN_SELECTED_WIDTH: f32 = 158.0;
const FOOTER_HEIGHT: f32 = 30.0;

#[derive(Clone, Debug, PartialEq)]
pub struct TasksProps {
  pub project: Arc<str>,
  pub tasks: TasksState,
}

impl lurq::app::component::DevtoolsInspectable for TasksProps {}

pub struct TasksScreen {
  tab: Signal<TaskTab>,
  nav: ProjectNav,
}

impl Component for TasksScreen {
  type Props = TasksProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      tab: ctx.signal(TaskTab::All),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TasksProps>().clone();
    let nav = self.nav.clone();
    let tab = self.tab.get();
    let body = match &props.tasks {
      TasksState::Loading => Column::new().into(),
      TasksState::Failed(error) => {
        centered(RecoveryPanel::new(Icon::AlertCircle, &ctx.t("tasks.loadFailed"), &message_text(ctx, error)).into())
      }
      TasksState::Ready(tasks) if tasks.tasks.is_empty() => empty(ctx, &nav),
      TasksState::Ready(tasks) => library(ctx, tasks, tab),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx, &props.project, &nav))
      .child(page::screen_heading(&ctx.t("tasks.title"), &ctx.t("tasks.description")))
      .child(self.tabs(ctx, props.tasks.ready()))
      .child(body)
  }
}

impl TasksScreen {
  fn tabs(&self, ctx: &Ctx, tasks: Option<&ProjectTasks>) -> Element {
    let selected = self.tab.get();
    let tabs = TaskTab::ALL.into_iter().map(|tab| {
      let item = Tab::new(tab_id(tab), ctx.t(tab.label_key()), Dimension::Px(TAB_ROW_HEIGHT))
        .min_width(Dimension::Px(tab_width(tab)));
      if !tab.is_available() {
        return item;
      }
      let signal = self.tab.clone();
      item
        .count(tasks.and_then(|tasks| tab.count(&tasks.tasks)))
        .selected(tab == selected)
        .on_click(move || signal.set(tab))
    });
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      SpacingSize::Sm,
      Some(SpacingSize::Section),
      tabs,
    )
  }
}

/// Tab widths in the design (`Iefue`, `WGkQY`, `vfoPn`, `gDolJ`, `fwdrp` in `XxplC`).
const fn tab_width(tab: TaskTab) -> f32 {
  match tab {
    TaskTab::All => 124.0,
    TaskTab::Drafts => 110.0,
    TaskTab::Queue => 106.0,
    TaskTab::NeedsReview => 150.0,
    TaskTab::Done => 102.0,
  }
}

const fn tab_id(tab: TaskTab) -> &'static str {
  match tab {
    TaskTab::All => "tasks-tab-all",
    TaskTab::Drafts => "tasks-tab-drafts",
    TaskTab::Queue => "tasks-tab-queue",
    TaskTab::NeedsReview => "tasks-tab-needs-review",
    TaskTab::Done => "tasks-tab-done",
  }
}

fn topbar(ctx: &Ctx, project: &str, nav: &ProjectNav) -> Element {
  let nav = nav.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t_args("tasks.breadcrumb", [("project", project)])).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("tasks.createMultiple"))
        .icon(Icon::ListPlus)
        .id("create-multiple")
        .size(
          Dimension::Px(CREATE_MULTIPLE_WIDTH),
          Dimension::Px(TOPBAR_BUTTON_HEIGHT),
        )
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("overview.newTask"))
        .primary()
        .icon(Icon::Plus)
        .id("tasks-new-task")
        .size(Dimension::Px(NEW_TASK_WIDTH), Dimension::Px(TOPBAR_BUTTON_HEIGHT))
        .on_click(move || nav.show(Section::NewTask))
        .render(ctx),
    )
    .into()
}

fn body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
}

fn centered(content: Element) -> Element {
  body()
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}

/// No tasks in the project yet.
fn empty(ctx: &Ctx, nav: &ProjectNav) -> Element {
  let nav = nav.clone();
  let action = ActionButton::new(ctx.t("overview.newTask"))
    .primary()
    .icon(Icon::Plus)
    .id("empty-new-task")
    .on_click(move || nav.show(Section::NewTask))
    .render(ctx);
  centered(
    RecoveryPanel::new(
      Icon::CircleCheck,
      &ctx.t("tasks.emptyTitle"),
      &ctx.t("tasks.emptyDetail"),
    )
    .action(action)
    .into(),
  )
}

/// Filters, the selection bar, the task table and its footer (`npcrO`).
fn library(ctx: &mut Ctx, tasks: &ProjectTasks, tab: TaskTab) -> Element {
  let rows = Arc::new(list::rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, tab));
  let table: Element = if rows.is_empty() {
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding(ExtraSpacing::Stack.role())
      .child(Text::new(&ctx.t("tasks.emptyTab")).variant(TypographyStyle::Caption))
      .into()
  } else {
    VirtualizedList::new_shared(ctx, rows)
      .list_key("task-rows")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .mount_keyed::<QueueRow, _, _, _>(|row| row.id, Clone::clone)
  };
  body()
    .child(filters(ctx))
    .child(selection_bar(ctx, DraftSummary::of(&tasks.tasks)))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .child(table_heading(ctx))
        .child(table),
    )
    .child(footer(ctx, tasks))
    .into()
}

/// Search, pipeline filter and sort (`d5tAIZ`): not available yet.
fn filters(ctx: &Ctx) -> Element {
  let filter = |id, key, width: Option<f32>| {
    let control = value_box_sized(
      ctx,
      id,
      &ctx.t(key),
      ValueTone::Unavailable,
      None,
      Dimension::Px(FILTER_HEIGHT),
    );
    let column = Column::new().child(control);
    match width {
      Some(width) => column.width(Dimension::Px(width)),
      None => column.flex(1.0),
    }
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(filter("tasks-search", "tasks.filters.search", None))
    .child(filter(
      "tasks-pipeline-filter",
      "tasks.filters.allPipelines",
      Some(PIPELINE_FILTER_WIDTH),
    ))
    .child(filter("tasks-sort", "tasks.filters.manualOrder", Some(SORT_WIDTH)))
    .into()
}

/// Selection and run actions (`OjUxo`): selecting and running are not available yet; the bar
/// says how many drafts could be queued.
fn selection_bar(ctx: &Ctx, summary: DraftSummary) -> Element {
  let readiness = ctx.t_args(
    "tasks.selection.readiness",
    [
      ("ready", summary.ready.to_string().as_str()),
      ("incomplete", summary.incomplete.to_string().as_str()),
    ],
  );
  Row::new()
    .id("task-selection")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SELECTION_HEIGHT))
    .padding_vertical(SpacingSize::Xs)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Group.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::Square, IconSize::Option, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("tasks.selection.unavailable"))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted),
    )
    .child(Text::new(&readiness).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("tasks.selection.setPipeline"))
        .icon(Icon::Workflow)
        .id("set-pipeline")
        .height(Dimension::Px(SELECTION_BUTTON_HEIGHT))
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("tasks.selection.runSelected"))
        .primary()
        .icon(Icon::Play)
        .id("run-selected")
        .size(
          Dimension::Px(RUN_SELECTED_WIDTH),
          Dimension::Px(SELECTION_BUTTON_HEIGHT),
        )
        .render(ctx),
    )
    .into()
}

/// Task and run counts (`Bpth7`).
fn footer(ctx: &Ctx, tasks: &ProjectTasks) -> Element {
  Row::new()
    .id("tasks-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .align_items(Alignment::Center)
    .child(
      Text::new(&message_text(ctx, &list::footer(tasks.tasks.len(), tasks.runs))).variant(TypographyStyle::Caption),
    )
    .into()
}
