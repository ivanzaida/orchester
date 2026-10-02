//! The task page's status, priority and tag pickers (`gNExS`, `Sd5qt`) and the confirmation that
//! Canceled stops the task's active run (`N1vwaM`). A choice saves at once; the pickers then show
//! what is stored.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Text},
  core::Signal,
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
    detail::TaskDetail,
    organization::{
      OrganizationValue, PickerAppearance, PriorityChoice, WorkflowChoice, priority_select, workflow_select,
    },
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{backdrop::backdrop, button::ActionButton, field::field_error, form::form, message_text},
};

#[derive(Clone, PartialEq)]
pub(super) struct OrganizationProps {
  pub root: PathBuf,
  pub detail: Arc<TaskDetail>,
}
impl lurq::app::component::DevtoolsInspectable for OrganizationProps {}
#[derive(Clone)]
enum Mutation {
  Status(WorkflowStatus, bool),
  Priority(Priority),
  Tags(Vec<TagId>),
}
type Action = FutureAction<Mutation, OrganizationValue, Message>;

pub(super) struct OrganizationHeader {
  status: Signal<WorkflowChoice>,
  priority: Signal<PriorityChoice>,
  value: Signal<OrganizationValue>,
  /// The organization the task's detail last showed; when the detail changes, the pickers follow.
  shown: Signal<Option<OrganizationValue>>,
  /// What is stored: the detail's organization, or what the latest change saved before the detail
  /// is read again. A choice equal to it saves nothing.
  stored: Signal<OrganizationValue>,
  /// The task has a run that has not ended, which Canceled stops.
  active: Signal<bool>,
  confirm: Signal<bool>,
}
impl Component for OrganizationHeader {
  type Props = OrganizationProps;
  fn create(ctx: &mut Ctx) -> Self {
    let header = Self {
      status: ctx.signal(WorkflowChoice(WorkflowStatus::Backlog)),
      priority: ctx.signal(PriorityChoice(Priority::None)),
      value: ctx.signal(OrganizationValue::default()),
      shown: ctx.signal(None),
      stored: ctx.signal(OrganizationValue::default()),
      active: ctx.signal(false),
      confirm: ctx.signal(false),
    };
    let action = action(ctx);
    header.observe(ctx, &action);
    header
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OrganizationProps>().clone();
    let action = action(ctx);
    self.follow(&props.detail);
    let current = props.detail.task.organization();
    let state = action.state().get();
    let mut column = Column::new()
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .wrap()
          .spacing(ExtraSpacing::Stack.role())
          .align_items(Alignment::Center)
          .child(workflow_select(
            ctx,
            self.status.clone(),
            "task-header-status",
            !state.is_pending(),
            PickerAppearance::Header,
          ))
          .with_children(props.detail.run.as_ref().map(|_| {
            Text::new(
              &ctx.t_args(
                "tasks.workflow.runSuffix",
                [(
                  "run",
                  ctx
                    .t(super::super::task_rows::status_copy(props.detail.phase()).1)
                    .as_ref(),
                )],
              ),
            )
            .variant(TypographyStyle::Description)
          }))
          .child(priority_select(
            ctx,
            self.priority.clone(),
            "task-header-priority",
            !state.is_pending(),
            PickerAppearance::Header,
          ))
          .child(ctx.mount_keyed::<TagPicker>(
            "task-header-tags",
            TagPickerProps {
              root: props.root.clone(),
              inline: false,
              appearance: PickerAppearance::Header,
              selected: self.value.clone(),
              enabled: !state.is_pending(),
            },
          ))
          // Flagged with a reason, or held by linked blocking tasks that have not finished.
          .with_children((current.blocked_reason().is_some() || !props.detail.blocked_by.is_empty()).then(|| {
            crate::ui::status_label::status_label(crate::ui::icon::Icon::CircleMinus, &ctx.t("tasks.blocked.label"))
              .id("task-header-blocked")
          })),
      );
    if state.is_pending() {
      column = column.child(Text::new(&ctx.t("tasks.workflow.saving")).variant(TypographyStyle::Caption));
    }
    // Refused because the task has an active run: the confirmation that opened says so.
    if let Some(error) = state
      .error
      .filter(|error| error.key != messages::CANCEL_CONFIRM_REQUIRED)
    {
      column = column.child(field_error(&message_text(ctx, &error)));
    }
    column.child(self.confirmation(ctx, &props, &action))
  }
}

/// Saves one choice for the task the page shows. The call in the header's `create` and every
/// `render` refer to the same action; each render points it at the current task.
fn action(ctx: &mut Ctx) -> Action {
  let props = ctx.props::<OrganizationProps>();
  let (root, task) = (props.root.clone(), props.detail.task.id());
  let runners = ctx.use_context::<Runners>().expect("root provides runners");
  ctx.future_action(move |mutation: Mutation| {
    let (root, runners) = (root.clone(), runners.clone());
    async move {
      if matches!(mutation, Mutation::Status(WorkflowStatus::Canceled, true)) {
        stop_active_run(&root, task, &runners).await?;
      }
      background::run(move || {
        let mut db = ProjectDb::open(&root)?;
        let value = db.task_organization(task)?;
        let value = match mutation {
          Mutation::Status(status, _) => value.with_status(status),
          Mutation::Priority(priority) => value.with_priority(priority),
          Mutation::Tags(tags) => value.with_tags(tags),
        };
        db.set_task_organization(task, &value, ChangeActor::Person, Timestamp::now())?;
        Ok::<_, orchester_store::StoreError>(OrganizationValue(value))
      })
      .await?
      .map_err(|error| messages::store_error(&error))
    }
  })
}

/// Cancels the task's active run through the runner, as the run's own Cancel does; the run's work
/// stays on its branch.
async fn stop_active_run(root: &std::path::Path, task: TaskId, runners: &Runners) -> Result<(), Message> {
  let root = root.to_path_buf();
  let run = background::run(move || ProjectDb::open(&root).and_then(|db| db.active_task_run(task)))
    .await?
    .map_err(|error| messages::store_error(&error))?;
  if let Some(run) = run {
    runners
      .control_run(Command::CancelRun { run })
      .await
      .map_err(|error| crate::tasks::run::control_message(&error))?;
  }
  Ok(())
}

/// The header's signals, shared with the observers registered in `create`.
#[derive(Clone)]
struct Choices {
  status: Signal<WorkflowChoice>,
  priority: Signal<PriorityChoice>,
  value: Signal<OrganizationValue>,
  stored: Signal<OrganizationValue>,
  confirm: Signal<bool>,
}

impl Choices {
  /// Records `stored` as what is stored before the pickers show it, so showing it saves nothing.
  fn show_stored(&self, stored: TaskOrganization) {
    self.stored.set(OrganizationValue(stored.clone()));
    self.status.set(WorkflowChoice(stored.status()));
    self.priority.set(PriorityChoice(stored.priority()));
    self.value.set(OrganizationValue(stored));
  }
}

impl OrganizationHeader {
  fn choices(&self) -> Choices {
    Choices {
      status: self.status.clone(),
      priority: self.priority.clone(),
      value: self.value.clone(),
      stored: self.stored.clone(),
      confirm: self.confirm.clone(),
    }
  }

  /// Seeds the pickers when the detail shows another organization, and notes whether the task
  /// has an active run.
  fn follow(&self, detail: &TaskDetail) {
    let current = detail.task.organization();
    if self.shown.get_untracked().as_ref().map(|shown| &shown.0) != Some(current) {
      self.shown.set(Some(OrganizationValue(current.clone())));
      self.choices().show_stored(current.clone());
    }
    let active = detail.run.as_ref().is_some_and(|run| !run.run.status().is_terminal());
    if self.active.get_untracked() != active {
      self.active.set(active);
    }
  }

  /// Registered once, in `create`: each observer compares a choice with what is stored when the
  /// choice is made.
  fn observe(&self, ctx: &mut Ctx, action: &Action) {
    self.observe_saves(ctx, action);
    let (choices, submit, active) = (self.choices(), action.clone(), self.active.clone());
    ctx.watch(&self.status, move |choice| {
      let expected = choices.stored.get_untracked().0.status();
      if choice.0 == expected {
        return;
      }
      if choice.0 == WorkflowStatus::Canceled && active.get_untracked() {
        choices.confirm.set(true);
        choices.status.set(WorkflowChoice(expected));
      } else {
        submit.run(Mutation::Status(choice.0, false));
      }
    });
    let (submit, stored) = (action.clone(), self.stored.clone());
    ctx.watch(&self.priority, move |choice| {
      if choice.0 != stored.get_untracked().0.priority() {
        submit.run(Mutation::Priority(choice.0));
      }
    });
    let (submit, stored) = (action.clone(), self.stored.clone());
    ctx.watch(&self.value, move |value| {
      if value.0.tags() != stored.get_untracked().0.tags() {
        submit.run(Mutation::Tags(value.0.tags().to_vec()));
      }
    });
  }

  /// A change that has just been saved closes the confirmation and becomes what is stored. A failed
  /// change keeps the data of the last one that succeeded: it leaves the confirmation open with its
  /// error and puts the pickers back on what is stored. Canceled refused for an active run the
  /// detail did not show yet asks as well.
  fn observe_saves(&self, ctx: &mut Ctx, action: &Action) {
    let choices = self.choices();
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    ctx.watch(&action.state(), move |state| {
      if state.is_rejected() {
        if state
          .error
          .as_ref()
          .is_some_and(|error| error.key == messages::CANCEL_CONFIRM_REQUIRED)
        {
          choices.confirm.set(true);
        }
        choices.show_stored(choices.stored.get_untracked().0);
        return;
      }
      let Some(saved) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
        return;
      };
      choices.confirm.set(false);
      choices.show_stored(saved.0.clone());
      nav.tasks_changed();
    });
  }

  fn confirmation(&self, ctx: &mut Ctx, props: &OrganizationProps, action: &Action) -> Element {
    if !self.confirm.get() {
      return Modal::new(Column::new()).open_when(false).into();
    }
    let Some(run) = &props.detail.run else {
      return Modal::new(Column::new()).open_when(false).into();
    };
    let pending = action.state().get().is_pending();
    let close = self.confirm.clone();
    let submit = action.clone();
    let mut content = Column::new()
      .id("task-cancel-confirmation")
      .width(Dimension::Px(CONFIRM_WIDTH))
      .padding(SpacingSize::Section)
      .spacing(SpacingSize::Lg)
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(Text::new(&ctx.t("tasks.cancel.title")).variant(ExtraTypography::DialogTitle.role()))
      .child(
        Text::new(&ctx.t_args(
          "tasks.cancel.explanation",
          [
            ("task", props.detail.task.key().to_string().as_str()),
            ("run", run.run.number().to_string().as_str()),
            ("branch", run.run.branch().as_str()),
          ],
        ))
        .variant(ExtraTypography::Explanation.role()),
      )
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(
            ActionButton::new(ctx.t("tasks.cancel.keep"))
              .id("task-cancel-keep")
              .in_form()
              .on_click(move || close.set(false))
              .enabled(!pending)
              .render(ctx),
          )
          .child(
            ActionButton::new(ctx.t(if pending {
              "tasks.cancel.stopping"
            } else {
              "tasks.cancel.confirm"
            }))
            .id("task-cancel-stop")
            .in_form()
            .on_click(move || submit.run(Mutation::Status(WorkflowStatus::Canceled, true)))
            .enabled(!pending)
            .render(ctx),
          ),
      );
    if let Some(error) = action
      .state()
      .get()
      .error
      .filter(|error| error.key != messages::CANCEL_CONFIRM_REQUIRED)
    {
      content = content.child(field_error(&message_text(ctx, &error)));
    }
    Modal::new(backdrop(ctx, form(None, content)))
      .target(ModalTarget::Root)
      .open(self.confirm.clone())
      .dismiss_on_escape(!pending)
      .into()
  }
}

// Confirmation N1vwaM.
const CONFIRM_WIDTH: f32 = 620.0;
