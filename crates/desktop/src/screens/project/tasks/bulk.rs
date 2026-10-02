//! Bulk Set status, Set priority, Add tag and Remove tag for the selected tasks (ORC-39 `HI91q`).

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Modal, ModalTarget, Popup, Row},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
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
  theme::ExtraSpacing,
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    cancel_dialog::{CancelDialog, cancel_dialog},
    form::form,
    message_text,
  },
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Mutation {
  Status(WorkflowStatus),
  Priority(Priority),
  AddTags(Vec<TagId>),
  RemoveTags(Vec<TagId>),
}
/// The change a bulk action made and how it went for each selected task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Outcomes {
  pub mutation: Mutation,
  pub results: Vec<(TaskId, Result<(), Message>)>,
}
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
    // The organization group (`EpDpS`): 32 high, 12 apart.
    let row = Row::new()
      .wrap()
      .spacing(ExtraSpacing::Stack.role())
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
    let state = action.state().get();
    column = column.child(super::outcome::outcome(
      ctx,
      &props.tasks,
      state.data.as_ref(),
      state.is_pending(),
    ));
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
      if let Some(Outcomes { results: outcomes, .. }) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
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
      let (apply, ids, confirm, menu) = (
        action.clone(),
        selected.clone(),
        self.confirm.clone(),
        self.open.clone(),
      );
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
            // The menu closes: the confirmation takes its place.
            menu.set(false);
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

  /// Bulk Canceled with active runs (`hFH4D`); the runs' failures show in it.
  fn confirmation(&self, ctx: &mut Ctx, props: &BulkProps, action: &Action) -> Element {
    if !self.confirm.get() {
      return Modal::new(Column::new()).open_when(false).into();
    }
    let selected = props.selected.get().0;
    let text = super::bulk_cancel::bulk_cancel_text(ctx, &props.tasks, &selected);
    let selected: Vec<_> = selected.into_iter().collect();
    let state = action.state().get();
    let pending = state.is_pending();
    let failures: Vec<String> = state
      .data
      .iter()
      .flat_map(|outcomes| outcomes.results.iter())
      .filter_map(|(_, result)| result.as_ref().err())
      .map(|error| message_text(ctx, error).to_string())
      .collect();
    let error = (!failures.is_empty()).then(|| failures.join(" "));
    let (close, apply) = (self.confirm.clone(), action.clone());
    let dialog = CancelDialog {
      id: "bulk-cancel-confirmation",
      title: text.title,
      explanation: Some(text.explanation),
      notice: Some((text.heading, text.notice)),
      keep: (ctx.t("tasks.cancel.bulkKeep"), "bulk-cancel-keep"),
      confirm: (text.confirm, "bulk-cancel-stop"),
      pending,
      error: error.as_deref(),
    };
    let content = cancel_dialog(
      ctx,
      dialog,
      move || close.set(false),
      move || apply.run((selected.clone(), Mutation::Status(WorkflowStatus::Canceled), true)),
    );
    Modal::new(backdrop(ctx, form(None, content)))
      .target(ModalTarget::Root)
      .open(self.confirm.clone())
      .dismiss_on_escape(!pending)
      .into()
  }
}

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

/// Applies one change to each selected task in turn, recording each task's outcome; the call in
/// `create` and every `render` refer to the same action.
fn action(ctx: &mut Ctx, props: &BulkProps) -> Action {
  let root = props.root.clone();
  let runners = ctx.use_context::<Runners>().expect("root provides runners");
  ctx.future_action(move |(tasks, mutation, confirmed): (Vec<TaskId>, Mutation, bool)| {
    let (root, runners) = (root.clone(), runners.clone());
    async move {
      let mut results = Vec::new();
      for task in tasks {
        let result = apply(&root, task, mutation.clone(), confirmed, &runners).await;
        results.push((task, result));
      }
      Ok(Outcomes { mutation, results })
    }
  })
}
