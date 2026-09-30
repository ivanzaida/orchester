use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Select},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::task::organization::{Priority, TaskFilter, TaskSort, WorkflowStatus};

use crate::{
  tasks::{
    data::ProjectTasks,
    organization::{CONTROL_HEIGHT, priority_copy, workflow_copy},
  },
  ui::{button::ActionButton, field::select_style},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Filters(pub TaskFilter);
impl lurq::app::component::DevtoolsInspectable for Filters {}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Sort(pub TaskSort);
impl lurq::app::component::DevtoolsInspectable for Sort {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Facet {
  Status,
  Priority,
  Tags,
}
impl lurq::app::component::DevtoolsInspectable for Facet {}
#[derive(Clone)]
pub(super) struct FilterProps {
  pub tasks: ProjectTasks,
  pub filter: Signal<Filters>,
  pub sort: Signal<Sort>,
}
impl PartialEq for FilterProps {
  fn eq(&self, other: &Self) -> bool {
    self.tasks == other.tasks
  }
}
impl lurq::app::component::DevtoolsInspectable for FilterProps {}
pub(super) struct TaskFilters {
  open: Signal<Option<Facet>>,
}
impl Component for TaskFilters {
  type Props = FilterProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self { open: ctx.signal(None) }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FilterProps>().clone();
    let mut row = Row::new().spacing(SpacingSize::Sm);
    for (facet, key) in [
      (Facet::Status, "tasks.workflow.label"),
      (Facet::Priority, "tasks.priority.label"),
      (Facet::Tags, "tasks.tags.label"),
    ] {
      let open = self.open.clone();
      row = row.child(
        ActionButton::new(ctx.t(key))
          .id(match facet {
            Facet::Status => "filter-status",
            Facet::Priority => "filter-priority",
            Facet::Tags => "filter-tags",
          })
          .on_click(move || open.update(|value| *value = if *value == Some(facet) { None } else { Some(facet) }))
          .render(ctx),
      );
    }
    let options = [
      (Sort(TaskSort::Updated), ctx.t("tasks.sort.updated")),
      (Sort(TaskSort::Status), ctx.t("tasks.sort.status")),
      (Sort(TaskSort::Priority), ctx.t("tasks.sort.priority")),
    ];
    let clear = props.filter.clone();
    row = row
      .child(
        Select::new(props.sort.clone())
          .options(options)
          .height(Dimension::Px(CONTROL_HEIGHT))
          .style(select_style(ctx, CONTROL_HEIGHT)),
      )
      .child(
        ActionButton::new(ctx.t("tasks.filters.clear"))
          .id("filters-clear")
          .on_click(move || clear.set(Filters::default()))
          .render(ctx),
      );
    let mut column = Column::new().spacing(SpacingSize::Sm).child(row);
    if let Some(facet) = self.open.get() {
      column = column.child(menu(ctx, &props, facet));
    }
    column
  }
}

fn menu(ctx: &Ctx, props: &FilterProps, facet: Facet) -> Element {
  let current = props.filter.get().0;
  let mut column = Column::new().spacing(SpacingSize::Xs);
  match facet {
    Facet::Status => {
      for status in WorkflowStatus::ALL {
        let filter = props.filter.clone();
        column = column.child(
          ActionButton::new(ctx.t(workflow_copy(status).1))
            .icon(check(current.statuses.contains(&status)))
            .on_click(move || filter.update(|filter| toggle(&mut filter.0.statuses, status)))
            .render(ctx)
            .id(format!("filter-status-{status:?}")),
        );
      }
    }
    Facet::Priority => {
      for priority in Priority::ALL {
        let filter = props.filter.clone();
        column = column.child(
          ActionButton::new(ctx.t(priority_copy(priority).1))
            .icon(check(current.priorities.contains(&priority)))
            .on_click(move || filter.update(|filter| toggle(&mut filter.0.priorities, priority)))
            .render(ctx)
            .id(format!("filter-priority-{priority:?}")),
        );
      }
    }
    Facet::Tags => {
      for tag in &*props.tasks.tags {
        let (filter, id) = (props.filter.clone(), tag.id());
        column = column.child(
          ActionButton::new(Arc::from(tag.name().as_str()))
            .icon(check(current.tags.contains(&id)))
            .on_click(move || filter.update(|filter| toggle(&mut filter.0.tags, id)))
            .render(ctx),
        );
      }
      let filter = props.filter.clone();
      column = column.child(
        ActionButton::new(ctx.t(if current.all_tags {
          "tasks.filters.matchAll"
        } else {
          "tasks.filters.matchAny"
        }))
        .on_click(move || filter.update(|filter| filter.0.all_tags = !filter.0.all_tags))
        .render(ctx),
      );
    }
  }
  column.into()
}
fn toggle<T: PartialEq>(values: &mut Vec<T>, value: T) {
  if values.contains(&value) {
    values.retain(|item| *item != value);
  } else {
    values.push(value);
  }
}
fn check(selected: bool) -> crate::ui::icon::Icon {
  if selected {
    crate::ui::icon::Icon::Check
  } else {
    crate::ui::icon::Icon::Square
  }
}
