use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, OutsidePress, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Popup, Row, Select, Text},
  core::{ElementRef, Signal},
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
  show: Signal<bool>,
  anchors: [ElementRef; 3],
  query: Signal<String>,
  match_all: Signal<bool>,
}
impl Component for TaskFilters {
  type Props = FilterProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      query: ctx.signal(String::new()),
      match_all: ctx.signal(false),
      open: ctx.signal(None),
      show: ctx.signal(false),
      anchors: std::array::from_fn(|_| ElementRef::default()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FilterProps>().clone();
    let mode = props.filter.get().0.all_tags;
    if self.match_all.get_untracked() != mode {
      self.match_all.set(mode);
    }
    let filter = props.filter.clone();
    ctx.watch(&self.match_all, move |mode| {
      filter.update(|value| value.0.all_tags = *mode)
    });
    let mut row = self.facet_buttons(ctx, &props);
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
          .style(select_style(ctx, CONTROL_HEIGHT).min_menu_width(crate::tasks::organization::ORGANIZATION_MENU_WIDTH)),
      )
      .with_children(
        (!props.filter.get().0.statuses.is_empty()
          || !props.filter.get().0.priorities.is_empty()
          || !props.filter.get().0.tags.is_empty())
        .then(|| {
          ActionButton::new(ctx.t("tasks.filters.clear"))
            .id("filters-clear")
            .on_click(move || clear.set(Filters::default()))
            .render(ctx)
        }),
      );
    let mut column = Column::new()
      .spacing(SpacingSize::Sm)
      .child(row)
      .child(filter_summary(ctx, &props));
    if let Some(facet) = self.open.get() {
      let index = match facet {
        Facet::Status => 0,
        Facet::Priority => 1,
        Facet::Tags => 2,
      };
      column = column.child(
        Popup::new(
          self.anchors[index].clone(),
          menu(ctx, &props, facet, &self.query, self.match_all.clone()),
        )
        .open(self.show.clone())
        .placement(Placement::BottomStart)
        .outside_press(OutsidePress::PassThrough),
      );
    }
    column
  }
}

impl TaskFilters {
  fn facet_buttons(&self, ctx: &Ctx, props: &FilterProps) -> Row {
    let mut row = Row::new().spacing(SpacingSize::Sm);
    for (index, (facet, key)) in [
      (Facet::Status, "tasks.workflow.label"),
      (Facet::Priority, "tasks.priority.label"),
      (Facet::Tags, "tasks.tags.label"),
    ]
    .into_iter()
    .enumerate()
    {
      let open = self.open.clone();
      let show = self.show.clone();
      row = row.child(
        ActionButton::new(facet_label(
          ctx,
          key,
          match facet {
            Facet::Status => props.filter.get().0.statuses.len(),
            Facet::Priority => props.filter.get().0.priorities.len(),
            Facet::Tags => props.filter.get().0.tags.len(),
          },
        ))
        .icon(if facet == Facet::Tags {
          crate::ui::icon::Icon::Tag
        } else {
          crate::ui::icon::Icon::ChevronDown
        })
        .id(match facet {
          Facet::Status => "filter-status",
          Facet::Priority => "filter-priority",
          Facet::Tags => "filter-tags",
        })
        .ref_element(self.anchors[index].clone())
        .on_click(move || {
          open.set(Some(facet));
          show.set(true);
        })
        .render(ctx),
      );
    }
    row
  }
}

fn menu(ctx: &Ctx, props: &FilterProps, facet: Facet, query: &Signal<String>, match_all: Signal<bool>) -> Element {
  let current = props.filter.get().0;
  let mut column = Column::new()
    .width(Dimension::Px(MENU_WIDTH))
    .padding(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .spacing(SpacingSize::Xs);
  match facet {
    Facet::Status => {
      for status in WorkflowStatus::ALL {
        let filter = props.filter.clone();
        column = column.child(
          crate::ui::menu_option::menu_option(
            &ctx.t(workflow_copy(status).1),
            current.statuses.contains(&status),
            true,
            move || filter.update(|filter| toggle(&mut filter.0.statuses, status)),
          )
          .id(format!("filter-status-{status:?}")),
        );
      }
    }
    Facet::Priority => {
      for priority in Priority::ALL {
        let filter = props.filter.clone();
        column = column.child(
          crate::ui::menu_option::menu_option(
            &ctx.t(priority_copy(priority).1),
            current.priorities.contains(&priority),
            true,
            move || filter.update(|filter| toggle(&mut filter.0.priorities, priority)),
          )
          .id(format!("filter-priority-{priority:?}")),
        );
      }
    }
    Facet::Tags => {
      let input = query.clone();
      column = column
        .child(Text::new(&ctx.t("tasks.filters.searchTags")).variant(lurq::app::theme::TypographyStyle::Caption))
        .child(
          crate::ui::field::text_input(
            ctx,
            "filter-tag-search",
            input.clone(),
            false,
            lurq::app::theme::TypographyStyle::Body,
          )
          .on_input(move |event: lurq::app::events::TextInputEvent| input.set(event.new_value)),
        );
      for tag in props
        .tasks
        .tags
        .iter()
        .filter(|tag| tag.name().as_str().to_lowercase().contains(&query.get().to_lowercase()))
      {
        let (filter, id) = (props.filter.clone(), tag.id());
        column = column.child(
          crate::ui::menu_option::menu_option(tag.name().as_str(), current.tags.contains(&id), true, move || {
            filter.update(|filter| toggle(&mut filter.0.tags, id))
          })
          .id(format!("filter-tag-{id}")),
        );
      }
      column = column.child(
        Element::from(
          Select::new(match_all)
            .options([
              (true, ctx.t("tasks.filters.matchAll")),
              (false, ctx.t("tasks.filters.matchAny")),
            ])
            .style(select_style(ctx, CONTROL_HEIGHT)),
        )
        .id("filter-tag-match"),
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
fn facet_label(ctx: &Ctx, key: &str, count: usize) -> Arc<str> {
  if count == 0 {
    ctx.t(key)
  } else {
    ctx.t_args(
      "tasks.filters.facetCount",
      [("facet", ctx.t(key).as_ref()), ("count", count.to_string().as_str())],
    )
  }
}
fn filter_summary(ctx: &Ctx, props: &FilterProps) -> Element {
  let filter = props.filter.get().0;
  let status = filter
    .statuses
    .iter()
    .map(|value| ctx.t(workflow_copy(*value).1).to_string())
    .collect::<Vec<_>>();
  let priority = filter
    .priorities
    .iter()
    .map(|value| ctx.t(priority_copy(*value).1).to_string())
    .collect::<Vec<_>>();
  let tags = props
    .tasks
    .tags
    .iter()
    .filter(|tag| filter.tags.contains(&tag.id()))
    .map(|tag| tag.name().to_string())
    .collect::<Vec<_>>();
  Row::new()
    .spacing(SpacingSize::Sm)
    .with_children(
      [status, priority, tags]
        .iter()
        .filter(|values| !values.is_empty())
        .map(|values| crate::tasks::organization::tag_chip(&values.join(&ctx.t("common.listSeparator")))),
    )
    .into()
}

// l7kBSq: multi-selection filter menus.
const MENU_WIDTH: f32 = 320.0;
