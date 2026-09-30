//! Scheduling review W1Dq6/IyJ5a; cancel returns to the unchanged selection.
mod order;
mod readback;
use order::{OrderChoice, TaskOrder};
mod settings;
use std::{num::NonZeroU32, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, SpacingSize, TypographyStyle},
  },
  components::{
    Column, DragMovement, DragOverridePolicy, Draggable, DraggableProps, Row, ScrollVertical, Select, Spacer, Text,
  },
  core::Signal,
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  task::TaskLimits,
  task_group::{Scheduling, TaskGroup},
  tokens::grouped,
};

use super::{
  super::{ProjectNav, TasksState, page},
  TOPBAR_BUTTON_HEIGHT, TOPBAR_HEIGHT,
};
use crate::{
  background,
  budget::{self, read_budget, read_minutes},
  messages::Message,
  tasks::{group, list::TaskRow},
  theme::{
    ExtraColor, ExtraSpacing, ExtraTypography, TASK_REVIEW_FIELD_HEIGHT as REVIEW_FIELD_HEIGHT,
    TASK_REVIEW_HANDLE_SIZE as ORDER_HANDLE_SIZE, TASK_REVIEW_RAIL_WIDTH as REVIEW_RAIL_WIDTH,
    TASK_REVIEW_ROW_HEIGHT as REVIEW_ROW_HEIGHT, gutter_scrollbar,
  },
  ui::{
    button::ActionButton,
    field::{FieldNote, field, field_error, select_style},
    form::form,
    icon::{Icon, IconSize, icon},
    message_text,
    option_card::OptionCard,
    pressable::surface,
  },
};

#[derive(Clone)]
pub struct ReviewProps {
  pub project: Arc<str>,
  pub root: Arc<PathBuf>,
  pub tasks: TasksState,
  pub selected: Vec<TaskId>,
  pub close: Signal<bool>,
  pub runner_online: bool,
  pub agent_slots: Option<NonZeroU32>,
}
impl PartialEq for ReviewProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.root == other.root
      && self.tasks == other.tasks
      && self.selected == other.selected
      && self.runner_online == other.runner_online
      && self.agent_slots == other.agent_slots
  }
}
impl lurq::app::component::DevtoolsInspectable for ReviewProps {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
  Parallel,
  Sequential,
}
impl lurq::app::component::DevtoolsInspectable for Mode {}

pub struct SchedulingReview {
  seeded: Signal<bool>,
  order: Signal<TaskOrder>,
  chosen: Signal<OrderChoice>,
  mode: Signal<Mode>,
  concurrent: Signal<u32>,
  cap: Signal<String>,
  budget: Signal<String>,
  minutes: Signal<String>,
  editing_time: Signal<bool>,
  focused: crate::ui::unit_input::FocusedInput,
  fixed_policy: Signal<u8>,
  key: IdempotencyKey,
  queue: FutureAction<(PathBuf, TaskGroup, TaskLimits), (), Vec<Message>>,
}

impl Component for SchedulingReview {
  type Props = ReviewProps;
  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("project shell provides navigation");
    let queue = queue_action(ctx);
    ctx.watch(&queue.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        nav.tasks_changed();
      }
    });
    Self {
      seeded: ctx.signal(false),
      order: ctx.signal(TaskOrder::new(Vec::new())),
      chosen: ctx.signal(OrderChoice(None)),
      mode: ctx.signal(Mode::Parallel),
      concurrent: ctx.signal(2),
      cap: ctx.signal(String::new()),
      budget: ctx.signal(String::new()),
      minutes: ctx.signal(String::new()),
      editing_time: ctx.signal(false),
      focused: ctx.signal(None),
      fixed_policy: ctx.signal(0),
      key: IdempotencyKey::generate(),
      queue,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Keep the create-time action's polling slot alive on every render.
    let _queue = queue_action(ctx);
    let props = ctx.props::<ReviewProps>().clone();
    self.seed(ctx, &props);
    let state = self.queue.state().get();
    let pending = state.is_pending();
    let completed = state.data.is_some() && !pending;
    let parsed = self.configuration(ctx);
    let submit = match &parsed {
      Ok((group, limits)) if props.runner_online && !pending && !completed => {
        let (root, group, limits, action) = (props.root.as_ref().clone(), group.clone(), *limits, self.queue.clone());
        Some(Arc::new(move || action.run((root.clone(), group.clone(), limits))) as Arc<dyn Fn() + Send + Sync>)
      }
      _ => None,
    };
    let count = self.order.get().tasks.len();
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(self.topbar(ctx, &props, pending))
      .child(page::screen_heading(
        &ctx.t_args("tasks.bulk.reviewTitle", [("count", count.to_string())]),
        &ctx.t("tasks.bulk.reviewDescription"),
      ))
      .child(
        Row::new()
          .flex(1.0)
          .width(Dimension::Pct(100.0))
          .padding_left(SpacingSize::Section)
          .padding_right(ExtraSpacing::BesideGutter.role())
          .spacing(ExtraSpacing::Rail.role())
          .child(self.task_order(ctx, &props, pending || completed))
          .child(self.settings(ctx, &props, pending || completed)),
      )
      .with_children(parsed.err().map(|e| field_error(&message_text(ctx, &e))))
      .with_children((!props.runner_online).then(|| field_error(&ctx.t("runs.start.runnerUnavailable"))))
      .with_children(
        state
          .error
          .as_ref()
          .into_iter()
          .flatten()
          .map(|e| field_error(&message_text(ctx, e))),
      )
      .with_children(completed.then(|| {
        Element::from(Text::new(&ctx.t("tasks.bulk.queued")).variant(TypographyStyle::Description))
          .id("tasks-review-result")
      }))
      .child(self.actions(ctx, &props, submit.clone(), pending, completed));
    form(submit, content)
  }
}

fn queue_action(ctx: &mut Ctx) -> FutureAction<(PathBuf, TaskGroup, TaskLimits), (), Vec<Message>> {
  ctx.future_action(|(root, group, limits): (PathBuf, TaskGroup, TaskLimits)| async move {
    background::run(move || group::queue(&root, &group, limits).map(|_| ()))
      .await
      .map_err(|e| vec![e])?
  })
}

impl SchedulingReview {
  fn seed(&self, ctx: &Ctx, props: &ReviewProps) {
    if self.seeded.get() {
      return;
    }
    let Some(tasks) = props.tasks.ready() else {
      return;
    };
    let order: Vec<_> = tasks
      .tasks
      .iter()
      .filter(|t| props.selected.contains(&t.id()))
      .map(|t| t.id())
      .collect();
    let selected: Vec<_> = tasks.tasks.iter().filter(|t| order.contains(&t.id())).collect();
    let separator = ctx.t("number.groupSeparator");
    if let Some(limits) = selected.first().and_then(|t| t.details().limits) {
      self.budget.set(grouped(limits.budget().get(), &separator));
      self.minutes.set(budget::minutes_text(limits.max_duration()));
    }
    let cap = selected
      .iter()
      .filter_map(|t| t.details().limits)
      .fold(0_u64, |sum, l| sum.saturating_add(l.budget().get()));
    self.cap.set(grouped(cap, &separator));
    self.concurrent.set(if order.len() > 1 { 2 } else { 1 });
    self.chosen.set(OrderChoice(order.first().copied()));
    self.order.set(TaskOrder::new(order));
    self.seeded.set(true);
  }

  fn configuration(&self, ctx: &Ctx) -> Result<(TaskGroup, TaskLimits), Message> {
    let separator = ctx.t("number.groupSeparator");
    let cap = read_budget(&self.cap.get(), &separator, budget::unbounded())?;
    let budget = read_budget(&self.budget.get(), &separator, budget::unbounded())?;
    let duration = read_minutes(&self.minutes.get())?;
    let limits = TaskLimits::new(budget, duration).expect("read_minutes rejects zero");
    let mode = match self.mode.get() {
      Mode::Parallel => {
        Scheduling::Parallel(NonZeroU32::new(self.concurrent.get()).expect("selector offers positive task limits"))
      }
      Mode::Sequential => Scheduling::Sequential,
    };
    let group = TaskGroup::new(self.key, self.order.get().tasks, mode, cap)
      .map_err(|_| Message::new("tasks.bulk.emptySelection"))?;
    Ok((group, limits))
  }

  fn topbar(&self, ctx: &Ctx, props: &ReviewProps, pending: bool) -> Element {
    let close = props.close.clone();
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TOPBAR_HEIGHT))
      .padding_horizontal(SpacingSize::Section)
      .align_items(lurq::layout::Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        Text::new(&ctx.t_args("tasks.bulk.breadcrumb", [("project", props.project.to_string())]))
          .variant(TypographyStyle::Description),
      )
      .child(Spacer::new().flex(1.0))
      .child(
        ActionButton::new(ctx.t("tasks.bulk.back"))
          .icon(Icon::ArrowLeft)
          .id("tasks-review-back")
          .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
          .on_click(move || close.set(false))
          .enabled(!pending)
          .render(ctx),
      )
      .into()
  }

  fn actions(
    &self,
    ctx: &Ctx,
    props: &ReviewProps,
    submit: crate::ui::form::Submit,
    pending: bool,
    complete: bool,
  ) -> Element {
    let close = props.close.clone();
    let mut queue = ActionButton::new(ctx.t_args(
      "tasks.bulk.queue",
      [("count", self.order.get().tasks.len().to_string())],
    ))
    .primary()
    .icon(Icon::Play)
    .size(
      Dimension::Px(crate::theme::TASK_REVIEW_QUEUE_WIDTH),
      Dimension::Px(crate::theme::TASK_REVIEW_ACTION_HEIGHT),
    )
    .id("tasks-review-queue")
    .in_form();
    queue = match submit {
      Some(submit) => queue.on_click(move || submit()),
      None => queue.disabled(),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(crate::theme::TASK_REVIEW_FOOTER_HEIGHT))
      .padding_horizontal(SpacingSize::Section)
      .align_items(lurq::layout::Alignment::Center)
      .spacing(SpacingSize::Sm)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Element::from(Text::new(&self.summary(ctx)).variant(TypographyStyle::Caption)).id("tasks-review-summary"))
      .child(Spacer::new().flex(1.0))
      .child(
        ActionButton::new(ctx.t(if complete { "tasks.bulk.back" } else { "common.cancel" }))
          .icon(if complete { Icon::ArrowLeft } else { Icon::X })
          .height(Dimension::Px(crate::theme::TASK_REVIEW_ACTION_HEIGHT))
          .id("tasks-review-cancel")
          .in_form()
          .on_click(move || close.set(false))
          .enabled(!pending)
          .render(ctx),
      )
      .child(queue.render(ctx))
      .into()
  }
}
