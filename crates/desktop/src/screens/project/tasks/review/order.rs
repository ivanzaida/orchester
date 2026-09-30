//! Drag, keyboard moves and a single undo share the same reviewed order.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OrderChoice(pub(super) Option<TaskId>);
impl lurq::app::component::DevtoolsInspectable for OrderChoice {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TaskOrder {
  pub(super) tasks: Vec<TaskId>,
  previous: Option<Vec<TaskId>>,
}
impl lurq::app::component::DevtoolsInspectable for TaskOrder {}

impl TaskOrder {
  pub(super) fn new(tasks: Vec<TaskId>) -> Self {
    Self { tasks, previous: None }
  }

  fn move_task(&mut self, task: TaskId, offset: isize) {
    let Some(from) = self.tasks.iter().position(|id| *id == task) else {
      return;
    };
    let to = (from as isize)
      .saturating_add(offset)
      .clamp(0, self.tasks.len().saturating_sub(1) as isize) as usize;
    if from == to {
      return;
    }
    self.previous = Some(self.tasks.clone());
    let task = self.tasks.remove(from);
    self.tasks.insert(to, task);
  }

  fn undo(&mut self) {
    if let Some(previous) = self.previous.take() {
      self.tasks = previous;
    }
  }
}

impl SchedulingReview {
  fn order_controls(&self, ctx: &Ctx, rows: &[TaskRow], locked: bool) -> Element {
    let state = self.order.get();
    let chosen = self.chosen.get().0;
    let position = state.tasks.iter().position(|id| Some(*id) == chosen);
    let choices = rows.iter().map(|task| {
      (
        OrderChoice(Some(task.id)),
        ctx.t_args(
          "tasks.bulk.orderTaskValue",
          [("task", task.key.to_string()), ("title", task.title.to_string())],
        ),
      )
    });
    let selector = Element::from(
      Select::new(self.chosen.clone())
        .options(choices)
        .focusable(!locked)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(ORDER_HANDLE_SIZE))
        .style(select_style(ctx, ORDER_HANDLE_SIZE)),
    )
    .id("tasks-review-order-task");
    let undo = self.order.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("tasks.bulk.orderTask")).variant(TypographyStyle::Caption))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(Column::new().flex(1.0).child(selector))
          .child(self.move_button(ctx, -1, !locked && position.is_some_and(|p| p > 0)))
          .child(self.move_button(ctx, 1, !locked && position.is_some_and(|p| p + 1 < state.tasks.len())))
          .child(
            ActionButton::new(ctx.t("tasks.bulk.undoOrder"))
              .icon(Icon::Undo2)
              .id("tasks-review-order-undo")
              .in_form()
              .height(Dimension::Px(ORDER_HANDLE_SIZE))
              .on_click(move || undo.update(TaskOrder::undo))
              .enabled(!locked && state.previous.is_some())
              .render(ctx),
          ),
      )
      .into()
  }

  fn move_button(&self, ctx: &Ctx, offset: isize, enabled: bool) -> Element {
    let (id, label, icon) = if offset < 0 {
      ("tasks-review-move-up", "tasks.bulk.moveUp", Icon::ArrowUp)
    } else {
      ("tasks-review-move-down", "tasks.bulk.moveDown", Icon::ArrowDown)
    };
    let (chosen, order) = (self.chosen.clone(), self.order.clone());
    ActionButton::new(ctx.t(label))
      .icon(icon)
      .id(id)
      .in_form()
      .height(Dimension::Px(ORDER_HANDLE_SIZE))
      .on_click(move || {
        if let Some(task) = chosen.get().0 {
          order.update(|order| order.move_task(task, offset));
        }
      })
      .enabled(enabled)
      .render(ctx)
  }

  pub(super) fn task_order(&self, ctx: &mut Ctx, props: &ReviewProps, locked: bool) -> Element {
    let rows = self
      .order
      .get()
      .tasks
      .iter()
      .filter_map(|id| {
        props.tasks.ready()?.tasks.iter().find(|t| t.id() == *id).map(|task| {
          TaskRow::new(
            task,
            &props.tasks.ready().expect("ready tasks were found").pipelines,
            None,
          )
        })
      })
      .collect::<Vec<_>>();
    let list = Column::new()
      .width(Dimension::Pct(100.0))
      .with_children(rows.iter().enumerate().map(|(index, task)| {
        let handle = self.order_handle(ctx, task.id, locked);
        Row::new()
          .width(Dimension::Pct(100.0))
          .height(Dimension::Px(REVIEW_ROW_HEIGHT))
          .spacing(SpacingSize::Sm)
          .align_items(lurq::layout::Alignment::Center)
          .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
          .child(handle)
          .child(
            Column::new()
              .flex(1.0)
              .spacing(ExtraSpacing::Indicator.role())
              .child(
                Text::new(&task.title)
                  .variant(TypographyStyle::FieldLabel)
                  .color(lurq::app::theme::PaletteColor::TextPrimary),
              )
              .child(
                Text::new(&ctx.t_args(
                  "tasks.bulk.taskPipeline",
                  [
                    ("task", task.key.to_string()),
                    ("pipeline", message_text(ctx, &task.pipeline.message()).to_string()),
                  ],
                ))
                .variant(ExtraTypography::Hint.role()),
              ),
          )
          .child(Text::new(&(index + 1).to_string()).variant(TypographyStyle::Mono))
      }));
    Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Md)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .align_items(lurq::layout::Alignment::Center)
          .child(
            Text::new(&ctx.t(if self.mode.get() == Mode::Parallel {
              "tasks.bulk.priority"
            } else {
              "tasks.bulk.order"
            }))
            .variant(ExtraTypography::SectionHeading.role()),
          )
          .child(Spacer::new().flex(1.0))
          .child(Text::new(&ctx.t("tasks.bulk.dragToReorder")).variant(TypographyStyle::Caption)),
      )
      .child(
        ScrollVertical::new(list)
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .scrollbar(gutter_scrollbar()),
      )
      .child(self.order_controls(ctx, &rows, locked))
      .into()
  }

  fn order_handle(&self, ctx: &mut Ctx, task: TaskId, locked: bool) -> Element {
    let handle = surface()
      .id(format!("tasks-review-order-{task}"))
      .size(Dimension::Px(ORDER_HANDLE_SIZE), Dimension::Px(ORDER_HANDLE_SIZE))
      .align_items(lurq::layout::Alignment::Center)
      .justify(lurq::layout::layout_kind::Justify::Center)
      .cursor(lurq::node::CursorIcon::Grab)
      .hovered_style(Style::new().background(lurq::app::theme::PaletteColor::AccentMuted))
      .child(icon(
        Icon::GripVertical,
        IconSize::Small,
        lurq::app::theme::PaletteColor::TextPrimary,
      ));
    if locked {
      return handle.into();
    }
    let order = self.order.clone();
    let queue = self.queue.clone();
    Draggable::mount(
      ctx,
      DraggableProps::new()
        .movement(DragMovement::Manual)
        .override_policy(DragOverridePolicy::Clear)
        .on_drag_end(move |event| {
          // Drag callbacks outlive renders; a drag started before submission must not
          // change the displayed order after that order has been queued.
          let state = queue.state().get();
          if state.is_pending() || state.data.is_some() {
            return;
          }
          let offset = (event.total_delta_y / REVIEW_ROW_HEIGHT).round() as isize;
          order.update(|ids| ids.move_task(task, offset));
        }),
      handle,
    )
  }
}
