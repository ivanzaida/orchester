use std::{collections::HashSet, path::PathBuf};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{TagId, TaskId},
  task::organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;

use super::{
  ProjectNav,
  tag_picker::{TagPicker, TagPickerProps},
};
use crate::{
  background,
  messages::{self, Message},
  runner::Runners,
  tasks::{
    data::ProjectTasks,
    organization::{OrganizationValue, priority_copy, workflow_copy},
  },
  theme::ExtraColor,
  ui::{backdrop::backdrop, button::ActionButton, field::field_error, form::form, message_text},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SelectedIds(pub HashSet<TaskId>);
impl lurq::app::component::DevtoolsInspectable for SelectedIds {}
#[derive(Clone)]
pub(super) struct TaskSelection(pub Signal<SelectedIds>);
#[derive(Clone)]
pub(super) struct SelectionProps {
  pub root: PathBuf,
  pub tasks: ProjectTasks,
  pub selected: Signal<SelectedIds>,
}
impl PartialEq for SelectionProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.tasks == other.tasks
  }
}
impl lurq::app::component::DevtoolsInspectable for SelectionProps {}
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

pub(super) struct SelectionBar {
  menu: Signal<Option<Menu>>,
  tags: Signal<OrganizationValue>,
  confirm: Signal<bool>,
}
impl Component for SelectionBar {
  type Props = SelectionProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      menu: ctx.signal(None),
      tags: ctx.signal(OrganizationValue::default()),
      confirm: ctx.signal(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SelectionProps>().clone();
    let action = self.action(ctx, &props);
    let selected = props.selected.get().0;
    let pending = action.state().get().is_pending();
    let mut row = Row::new()
      .spacing(SpacingSize::Sm)
      .padding(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .child(
        Text::new(&ctx.t_args(
          "tasks.selection.count",
          [("count", selected.len().to_string().as_str())],
        ))
        .variant(TypographyStyle::Caption),
      );
    for (menu, key) in [
      (Menu::Status, "tasks.selection.setStatus"),
      (Menu::Priority, "tasks.selection.setPriority"),
      (Menu::AddTags, "tasks.selection.addTag"),
      (Menu::RemoveTags, "tasks.selection.removeTag"),
    ] {
      let open = self.menu.clone();
      let tags = self.tags.clone();
      let can_remove = props
        .tasks
        .tasks
        .iter()
        .any(|task| selected.contains(&task.id()) && !task.organization().tags().is_empty());
      row = row.child(
        ActionButton::new(ctx.t(key))
          .id(match menu {
            Menu::Status => "bulk-status",
            Menu::Priority => "bulk-priority",
            Menu::AddTags => "bulk-add-tags",
            Menu::RemoveTags => "bulk-remove-tags",
          })
          .on_click(move || {
            tags.set(OrganizationValue::default());
            open.set(Some(menu));
          })
          .enabled(!pending && !selected.is_empty() && (menu != Menu::RemoveTags || can_remove))
          .render(ctx),
      );
    }
    let clear = props.selected.clone();
    row = row.child(
      ActionButton::new(ctx.t("tasks.selection.clear"))
        .on_click(move || clear.set(SelectedIds::default()))
        .enabled(!pending)
        .render(ctx),
    );
    let mut column = Column::new().spacing(SpacingSize::Sm).child(row);
    if let Some(menu) = self.menu.get().filter(|_| !selected.is_empty()) {
      column = column.child(self.menu(ctx, &props, menu, &action));
    }
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
    if pending {
      column = column.child(Text::new(&ctx.t("tasks.workflow.saving")).variant(TypographyStyle::Caption));
    }
    column.child(self.confirmation(ctx, &props, &action))
  }
}

impl SelectionBar {
  fn action(&self, ctx: &mut Ctx, props: &SelectionProps) -> Action {
    let root = props.root.clone();
    let runners = ctx.use_context::<Runners>().expect("root provides runners");
    let action = ctx.future_action(move |(tasks, mutation, confirmed): (Vec<TaskId>, Mutation, bool)| {
      let (root, runners) = (root.clone(), runners.clone());
      async move {
        let mut outcomes = Vec::new();
        for task in tasks {
          let result = apply(&root, task, mutation.clone(), confirmed, &runners).await;
          outcomes.push((task, result));
        }
        Ok(Outcomes(outcomes))
      }
    });
    let (confirm, nav) = (
      self.confirm.clone(),
      ctx.use_context::<ProjectNav>().expect("shell provides navigation"),
    );
    ctx.watch(&action.state(), move |state| {
      if let Some(Outcomes(outcomes)) = &state.data
        && !state.is_pending()
      {
        confirm.set(!outcomes.iter().all(|(_, result)| result.is_ok()) && confirm.get_untracked());
        nav.tasks_changed();
      }
    });
    action
  }

  fn menu(&self, ctx: &mut Ctx, props: &SelectionProps, menu: Menu, action: &Action) -> Element {
    let selected: Vec<_> = props.selected.get().0.into_iter().collect();
    let pending = action.state().get().is_pending();
    let mut column = Column::new().spacing(SpacingSize::Xs);
    match menu {
      Menu::Status => {
        for status in WorkflowStatus::ALL {
          let (action, ids, confirm) = (action.clone(), selected.clone(), self.confirm.clone());
          let active = active_count(props);
          column = column.child(
            ActionButton::new(ctx.t(workflow_copy(status).1))
              .on_click(move || {
                if status == WorkflowStatus::Canceled && active > 0 {
                  confirm.set(true);
                } else {
                  action.run((ids.clone(), Mutation::Status(status), false));
                }
              })
              .enabled(!pending)
              .render(ctx)
              .id(format!("bulk-status-{status:?}")),
          );
        }
      }
      Menu::Priority => {
        for priority in Priority::ALL {
          let (action, ids) = (action.clone(), selected.clone());
          column = column.child(
            ActionButton::new(ctx.t(priority_copy(priority).1))
              .on_click(move || action.run((ids.clone(), Mutation::Priority(priority), false)))
              .enabled(!pending)
              .render(ctx)
              .id(format!("bulk-priority-{priority:?}")),
          );
        }
      }
      Menu::AddTags => {
        column = column.child(ctx.mount_keyed::<TagPicker>(
          "bulk-tag-picker",
          TagPickerProps {
            root: props.root.clone(),
            selected: self.tags.clone(),
          },
        ));
        let (action, tags) = (action.clone(), self.tags.clone());
        column = column.child(
          ActionButton::new(ctx.t("tasks.selection.addTag"))
            .on_click(move || action.run((selected.clone(), Mutation::AddTags(tags.get().0.tags().to_vec()), false)))
            .enabled(!pending && !self.tags.get().0.tags().is_empty())
            .render(ctx),
        );
      }
      Menu::RemoveTags => {
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
          let (action, ids, id) = (action.clone(), selected.clone(), tag.id());
          column = column.child(
            ActionButton::new(ctx.t_args(
              "tasks.selection.removeTagCount",
              [("tag", tag.name().as_str()), ("count", count.to_string().as_str())],
            ))
            .on_click(move || action.run((ids.clone(), Mutation::RemoveTags(vec![id]), false)))
            .enabled(!pending)
            .render(ctx),
          );
        }
      }
    }
    column.into()
  }

  fn confirmation(&self, ctx: &mut Ctx, props: &SelectionProps, action: &Action) -> Element {
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
fn active_count(props: &SelectionProps) -> usize {
  let selected = props.selected.get().0;
  props
    .tasks
    .phases
    .iter()
    .filter(|(id, latest)| {
      selected.contains(id)
        && latest.phase().is_some_and(|phase| {
          matches!(
            phase,
            orchester_core::run::TaskPhase::Queued
              | orchester_core::run::TaskPhase::Running
              | orchester_core::run::TaskPhase::Waiting
              | orchester_core::run::TaskPhase::Paused
              | orchester_core::run::TaskPhase::StoppedAtLimit
              | orchester_core::run::TaskPhase::Interrupted
          )
        })
    })
    .count()
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
