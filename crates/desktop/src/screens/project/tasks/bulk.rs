//! Bulk Set status, Set priority, Add tag and Remove tag for the selected tasks (ORC-39 `HI91q`).

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{TagId, TaskId},
  task::organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;

use super::super::{
  ProjectNav,
  tag_picker::{TagPicker, TagPickerProps},
};
use crate::{
  background,
  messages::{self, Message},
  runner::Runners,
  tasks::{
    browse::Selection,
    data::ProjectTasks,
    organization::{OrganizationValue, priority_copy, workflow_copy},
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{backdrop::backdrop, button::ActionButton, field::field_error, form::form, message_text},
};

/// The selected tasks to organize, as the task list reads them.
#[derive(Clone)]
pub(super) struct BulkProps {
  pub root: PathBuf,
  pub tasks: ProjectTasks,
  pub selected: Signal<Selection>,
}
impl PartialEq for BulkProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.tasks == other.tasks
  }
}
impl lurq::app::component::DevtoolsInspectable for BulkProps {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
  Status,
  Priority,
  AddTags,
  RemoveTags,
}
impl lurq::app::component::DevtoolsInspectable for Menu {}
#[derive(Clone)]
enum Mutation {
  Status(WorkflowStatus),
  Priority(Priority),
  AddTags(Vec<TagId>),
  RemoveTags(Vec<TagId>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Outcomes(Vec<(TaskId, Result<(), Message>)>);
impl lurq::app::component::DevtoolsInspectable for Outcomes {}
type Action = FutureAction<(Vec<TaskId>, Mutation, bool), Outcomes, Message>;

/// The selection bar's organization group (`EpDpS` in `HI91q`): Set status, Set priority, Add tag
/// and Remove tag, with their menus, the Canceled confirmation and each task's outcome.
pub(super) struct BulkOrganization {
  menu: Signal<Option<Menu>>,
  open: Signal<bool>,
  anchors: [ElementRef; 4],
  tags: Signal<OrganizationValue>,
  confirm: Signal<bool>,
}
impl Component for BulkOrganization {
  type Props = BulkProps;
  fn create(ctx: &mut Ctx) -> Self {
    let bar = Self {
      menu: ctx.signal(None),
      open: ctx.signal(false),
      anchors: std::array::from_fn(|_| ElementRef::default()),
      tags: ctx.signal(OrganizationValue::default()),
      confirm: ctx.signal(false),
    };
    let props = ctx.props::<BulkProps>().clone();
    let action = action(ctx, &props);
    bar.observe(ctx, &action);
    bar
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<BulkProps>().clone();
    let action = action(ctx, &props);
    let selected = props.selected.get().0;
    let pending = action.state().get().is_pending();
    let row = Row::new()
      .wrap()
      .spacing(ExtraSpacing::Group.role())
      .align_items(Alignment::Center);
    let row = self.buttons(ctx, &props, row, pending);
    let mut column = Column::new().spacing(SpacingSize::Sm).child(row);
    if let Some(menu) = self.menu.get().filter(|_| !selected.is_empty()) {
      let index = match menu {
        Menu::Status => 0,
        Menu::Priority => 1,
        Menu::AddTags => 2,
        Menu::RemoveTags => 3,
      };
      let content = self.menu(ctx, &props, menu, &action);
      column = column.child(
        Popup::new(self.anchors[index].clone(), crate::ui::form::form(None, content))
          .open(self.open.clone())
          .placement(Placement::BottomStart),
      );
    }
    column = column.child(results(ctx, &props, &action));
    column.child(self.confirmation(ctx, &props, &action))
  }
}

impl BulkOrganization {
  fn buttons(&self, ctx: &Ctx, props: &BulkProps, mut row: Row, pending: bool) -> Row {
    let selected = props.selected.get().0;
    for (index, (menu, key)) in [
      (Menu::Status, "tasks.selection.setStatus"),
      (Menu::Priority, "tasks.selection.setPriority"),
      (Menu::AddTags, "tasks.selection.addTag"),
      (Menu::RemoveTags, "tasks.selection.removeTag"),
    ]
    .into_iter()
    .enumerate()
    {
      let open = self.menu.clone();
      let tags = self.tags.clone();
      let show = self.open.clone();
      let can_remove = props
        .tasks
        .tasks
        .iter()
        .any(|task| selected.contains(&task.id()) && !task.organization().tags().is_empty());
      row = row.child(
        ActionButton::new(ctx.t(key))
          .icon(match menu {
            Menu::Status => crate::ui::icon::Icon::Circle,
            Menu::Priority => crate::ui::icon::Icon::SignalHigh,
            _ => crate::ui::icon::Icon::Tag,
          })
          .id(match menu {
            Menu::Status => "bulk-status",
            Menu::Priority => "bulk-priority",
            Menu::AddTags => "bulk-add-tags",
            Menu::RemoveTags => "bulk-remove-tags",
          })
          .in_form()
          .height(Dimension::Px(super::SELECTION_BUTTON_HEIGHT))
          .ref_element(self.anchors[index].clone())
          .on_click(move || {
            show.set(true);
            tags.set(OrganizationValue::default());
            open.set(Some(menu));
          })
          .enabled(!pending && !selected.is_empty() && (menu != Menu::RemoveTags || can_remove))
          .render(ctx),
      );
    }
    row
  }

  /// Registered once, in `create`: a bulk change that has just finished closes its menu and
  /// rereads the tasks; the confirmation stays open while any task failed.
  fn observe(&self, ctx: &mut Ctx, action: &Action) {
    let (confirm, open) = (self.confirm.clone(), self.open.clone());
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    ctx.watch(&action.state(), move |state| {
      if let Some(Outcomes(outcomes)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        confirm.set(!outcomes.iter().all(|(_, result)| result.is_ok()) && confirm.get_untracked());
        open.set(false);
        nav.tasks_changed();
      }
    });
  }

  fn menu(&self, ctx: &mut Ctx, props: &BulkProps, menu: Menu, action: &Action) -> Element {
    let selected: Vec<_> = props.selected.get().0.into_iter().collect();
    let pending = action.state().get().is_pending();
    let mut column = Column::new()
      .width(Dimension::Px(MENU_WIDTH))
      .padding(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .rounded(RadiusSize::Md)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .spacing(SpacingSize::Xs);
    match menu {
      Menu::Status => column = self.status_menu(ctx, props, action, column),
      Menu::Priority => column = self.priority_menu(ctx, props, action, column),
      Menu::AddTags => {
        column = column.child(ctx.mount_keyed::<TagPicker>(
          "bulk-tag-picker",
          TagPickerProps {
            root: props.root.clone(),
            appearance: crate::tasks::organization::PickerAppearance::Field,
            selected: self.tags.clone(),
            enabled: !pending,
            inline: true,
          },
        ));
        let (action, tags) = (action.clone(), self.tags.clone());
        column = column.child(
          ActionButton::new(ctx.t_args(
            "tasks.selection.addToCount",
            [("count", selected.len().to_string().as_str())],
          ))
          .icon(crate::ui::icon::Icon::Plus)
          .id("bulk-tags-apply")
          .on_click(move || action.run((selected.clone(), Mutation::AddTags(tags.get().0.tags().to_vec()), false)))
          .enabled(!pending && !self.tags.get().0.tags().is_empty())
          .render(ctx),
        );
      }
      Menu::RemoveTags => column = self.remove_menu(ctx, props, action, column),
    }
    column.into()
  }

  fn status_menu(&self, ctx: &Ctx, props: &BulkProps, action: &Action, mut column: Column) -> Column {
    let selected: Vec<_> = props.selected.get().0.into_iter().collect();
    let pending = action.state().get().is_pending();
    for status in WorkflowStatus::ALL {
      let (apply, ids, confirm) = (action.clone(), selected.clone(), self.confirm.clone());
      let active = active_count(props);
      let checked = props
        .tasks
        .tasks
        .iter()
        .filter(|task| selected.contains(&task.id()))
        .all(|task| task.organization().status() == status);
      column = column.child(
        crate::ui::menu_option::menu_option(&ctx.t(workflow_copy(status).1), checked, !pending, move || {
          if status == WorkflowStatus::Canceled && active > 0 {
            confirm.set(true);
          } else {
            apply.run((ids.clone(), Mutation::Status(status), false));
          }
        })
        .id(format!("bulk-status-{status:?}")),
      );
    }
    column
  }
  fn priority_menu(&self, ctx: &Ctx, props: &BulkProps, action: &Action, mut column: Column) -> Column {
    let selected: Vec<_> = props.selected.get().0.into_iter().collect();
    for priority in Priority::ALL {
      let (apply, ids) = (action.clone(), selected.clone());
      let checked = props
        .tasks
        .tasks
        .iter()
        .filter(|task| selected.contains(&task.id()))
        .all(|task| task.organization().priority() == priority);
      column = column.child(
        crate::ui::menu_option::menu_option(
          &ctx.t(priority_copy(priority).1),
          checked,
          !action.state().get().is_pending(),
          move || apply.run((ids.clone(), Mutation::Priority(priority), false)),
        )
        .id(format!("bulk-priority-{priority:?}")),
      );
    }
    column
  }
  fn remove_menu(&self, ctx: &Ctx, props: &BulkProps, action: &Action, mut column: Column) -> Column {
    let selected: Vec<_> = props.selected.get().0.into_iter().collect();
    let pending = action.state().get().is_pending();
    for tag in &*props.tasks.tags {
      let count = props
        .tasks
        .tasks
        .iter()
        .filter(|task| selected.contains(&task.id()) && task.organization().tags().contains(&tag.id()))
        .count();
      if count == 0 {
        continue;
      }
      let (tags, id) = (self.tags.clone(), tag.id());
      column = column.child(
        crate::ui::menu_option::menu_option(
          &ctx.t_args(
            "tasks.selection.tagMembership",
            [
              ("tag", tag.name().as_str()),
              ("count", count.to_string().as_str()),
              ("total", selected.len().to_string().as_str()),
            ],
          ),
          self.tags.get().0.tags().contains(&id),
          !pending,
          move || {
            tags.update(|value| {
              value.0 = if value.0.tags().contains(&id) {
                value.0.clone().remove_tags(&[id])
              } else {
                value.0.clone().add_tags(&[id])
              };
            })
          },
        )
        .id(format!("bulk-remove-tag-{id}")),
      );
    }
    let (apply, tags) = (action.clone(), self.tags.clone());
    column.child(
      ActionButton::new(ctx.t_args(
        "tasks.selection.removeFromCount",
        [("count", selected.len().to_string().as_str())],
      ))
      .icon(crate::ui::icon::Icon::Minus)
      .id("bulk-tags-remove-apply")
      .on_click(move || {
        apply.run((
          selected.clone(),
          Mutation::RemoveTags(tags.get().0.tags().to_vec()),
          false,
        ))
      })
      .enabled(!pending && !self.tags.get().0.tags().is_empty())
      .render(ctx),
    )
  }

  fn confirmation(&self, ctx: &mut Ctx, props: &BulkProps, action: &Action) -> Element {
    if !self.confirm.get() {
      return Modal::new(Column::new()).open_when(false).into();
    }
    let selected: Vec<_> = props.selected.get().0.into_iter().collect();
    let pending = action.state().get().is_pending();
    let close = self.confirm.clone();
    let apply = action.clone();
    let mut content = Column::new()
      .width(Dimension::Px(CONFIRM_WIDTH))
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Lg)
      .background(ExtraColor::Surface.role())
      .rounded(RadiusSize::Lg)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(Text::new(&ctx.t("tasks.cancel.title")).variant(TypographyStyle::Title))
      .child(
        Text::new(&ctx.t_args(
          "tasks.cancel.bulkExplanation",
          [
            ("tasks", selected.len().to_string().as_str()),
            ("runs", active_count(props).to_string().as_str()),
          ],
        ))
        .variant(TypographyStyle::Description),
      )
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(
            ActionButton::new(ctx.t("tasks.cancel.keep"))
              .id("bulk-cancel-keep")
              .in_form()
              .on_click(move || close.set(false))
              .enabled(!pending)
              .render(ctx),
          )
          .child(
            ActionButton::new(ctx.t("tasks.cancel.confirm"))
              .id("bulk-cancel-stop")
              .in_form()
              .on_click(move || apply.run((selected.clone(), Mutation::Status(WorkflowStatus::Canceled), true)))
              .enabled(!pending)
              .render(ctx),
          ),
      );
    if let Some(Outcomes(outcomes)) = action.state().get().data {
      for (_, result) in outcomes {
        if let Err(error) = result {
          content = content.child(field_error(&message_text(ctx, &error)));
        }
      }
    }
    Modal::new(backdrop(ctx, form(None, content)))
      .target(ModalTarget::Root)
      .open(self.confirm.clone())
      .dismiss_on_escape(!pending)
      .into()
  }
}

// hFH4D: bulk cancellation confirmation.
const CONFIRM_WIDTH: f32 = 620.0;
const MENU_WIDTH: f32 = 320.0;
fn active_count(props: &BulkProps) -> usize {
  props.tasks.runs_not_ended(&props.selected.get().0)
}

async fn apply(
  root: &std::path::Path,
  task: TaskId,
  mutation: Mutation,
  confirmed: bool,
  runners: &Runners,
) -> Result<(), Message> {
  if confirmed && matches!(mutation, Mutation::Status(WorkflowStatus::Canceled)) {
    let root = root.to_owned();
    let run = background::run(move || ProjectDb::open(&root).and_then(|db| db.active_task_run(task)))
      .await?
      .map_err(|error| messages::store_error(&error))?;
    if let Some(run) = run {
      runners
        .control_run(Command::CancelRun { run })
        .await
        .map_err(|error| crate::tasks::run::control_message(&error))?;
    }
  }
  let root = root.to_owned();
  background::run(move || {
    let mut db = ProjectDb::open(&root)?;
    db.update_task_organizations(&[task], ChangeActor::Person, Timestamp::now(), |value| {
      update(value, &mutation)
    })
  })
  .await?
  .map_err(|error| messages::store_error(&error))
}
fn update(value: TaskOrganization, mutation: &Mutation) -> TaskOrganization {
  match mutation {
    Mutation::Status(status) => value.with_status(*status),
    Mutation::Priority(priority) => value.with_priority(*priority),
    Mutation::AddTags(tags) => value.add_tags(tags),
    Mutation::RemoveTags(tags) => value.remove_tags(tags),
  }
}

fn results(ctx: &Ctx, props: &BulkProps, action: &Action) -> Element {
  let mut column = Column::new().spacing(SpacingSize::Sm);
  if let Some(Outcomes(outcomes)) = action.state().get().data {
    for (id, result) in outcomes {
      let key = props
        .tasks
        .tasks
        .iter()
        .find(|task| task.id() == id)
        .map(|task| task.key().to_string())
        .unwrap_or_else(|| id.to_string());
      let outcome: Element = match result {
        Ok(()) => Text::new(&ctx.t_args("tasks.selection.changed", [("task", key.as_str())]))
          .variant(TypographyStyle::Caption)
          .into(),
        Err(error) => field_error(&ctx.t_args(
          "tasks.selection.failed",
          [("task", key.as_str()), ("reason", &message_text(ctx, &error))],
        )),
      };
      column = column.child(outcome.id(format!("bulk-outcome-{id}")));
    }
  }
  if action.state().get().is_pending() {
    column = column.child(Text::new(&ctx.t("tasks.workflow.saving")).variant(TypographyStyle::Caption));
  }
  column.into()
}

/// Applies one change to each selected task in turn, recording each task's outcome; the call in
/// `create` and every `render` refer to the same action.
fn action(ctx: &mut Ctx, props: &BulkProps) -> Action {
  let root = props.root.clone();
  let runners = ctx.use_context::<Runners>().expect("root provides runners");
  ctx.future_action(move |(tasks, mutation, confirmed): (Vec<TaskId>, Mutation, bool)| {
    let (root, runners) = (root.clone(), runners.clone());
    async move {
      let mut outcomes = Vec::new();
      for task in tasks {
        let result = apply(&root, task, mutation.clone(), confirmed, &runners).await;
        outcomes.push((task, result));
      }
      Ok(Outcomes(outcomes))
    }
  })
}
