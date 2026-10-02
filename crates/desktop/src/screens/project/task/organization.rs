use std::{path::PathBuf, sync::Arc};

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
  id::TagId,
  task::organization::{ChangeActor, Priority, WorkflowStatus},
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
  theme::{ExtraColor, ExtraTypography},
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
  source: Signal<Option<OrganizationValue>>,
  confirm: Signal<bool>,
}
impl Component for OrganizationHeader {
  type Props = OrganizationProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      status: ctx.signal(WorkflowChoice(WorkflowStatus::Backlog)),
      priority: ctx.signal(PriorityChoice(Priority::None)),
      value: ctx.signal(OrganizationValue::default()),
      source: ctx.signal(None),
      confirm: ctx.signal(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OrganizationProps>().clone();
    let current = props.detail.task.organization();
    if self.source.get_untracked().as_ref().map(|source| &source.0) != Some(current) {
      self.source.set(Some(OrganizationValue(current.clone())));
      self.status.set(WorkflowChoice(current.status()));
      self.priority.set(PriorityChoice(current.priority()));
      self.value.set(OrganizationValue(current.clone()));
    }
    let action = self.action(ctx, &props);
    self.watch_choices(ctx, &props, &action);
    let state = action.state().get();
    let mut column = Column::new()
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(Column::new().width(Dimension::Px(STATUS_WIDTH)).child(workflow_select(
            ctx,
            self.status.clone(),
            "task-header-status",
            !state.is_pending(),
            PickerAppearance::Header,
          )))
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
            .variant(TypographyStyle::Caption)
          }))
          .child(
            Column::new()
              .width(Dimension::Px(PRIORITY_WIDTH))
              .child(priority_select(
                ctx,
                self.priority.clone(),
                "task-header-priority",
                !state.is_pending(),
                PickerAppearance::Header,
              )),
          )
          .child(ctx.mount_keyed::<TagPicker>(
            "task-header-tags",
            TagPickerProps {
              root: props.root.clone(),
              appearance: PickerAppearance::Header,
              selected: self.value.clone(),
              enabled: !state.is_pending(),
            },
          ))
          .with_children(current.blocked_reason().is_some().then(|| {
            crate::ui::status_label::status_label(crate::ui::icon::Icon::CircleMinus, &ctx.t("tasks.blocked.label"))
          })),
      );
    if state.is_pending() {
      column = column.child(Text::new(&ctx.t("tasks.workflow.saving")).variant(TypographyStyle::Caption));
    }
    if let Some(error) = state.error {
      column = column.child(field_error(&message_text(ctx, &error)));
    }
    column.child(self.confirmation(ctx, &props, &action))
  }
}

impl OrganizationHeader {
  fn action(&self, ctx: &mut Ctx, props: &OrganizationProps) -> Action {
    let (root, task) = (props.root.clone(), props.detail.task.id());
    let runners = ctx.use_context::<Runners>().expect("root provides runners");
    let action = ctx.future_action(move |mutation: Mutation| {
      let (root, runners) = (root.clone(), runners.clone());
      async move {
        if matches!(mutation, Mutation::Status(WorkflowStatus::Canceled, true)) {
          let read_root = root.clone();
          let run = background::run(move || ProjectDb::open(&read_root).and_then(|db| db.active_task_run(task)))
            .await?
            .map_err(|error| messages::store_error(&error))?;
          if let Some(run) = run {
            runners
              .control_run(Command::CancelRun { run })
              .await
              .map_err(|error| crate::tasks::run::control_message(&error))?;
          }
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
    });
    let confirm = self.confirm.clone();
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    ctx.watch(&action.state(), move |state| {
      if state.data.is_some() && !state.is_pending() {
        confirm.set(false);
        nav.tasks_changed();
      }
    });
    action
  }

  fn watch_choices(&self, ctx: &mut Ctx, props: &OrganizationProps, action: &Action) {
    let current = props.detail.task.organization().clone();
    let active = props
      .detail
      .run
      .as_ref()
      .is_some_and(|run| !run.run.status().is_terminal());
    let (confirm, submit, status) = (self.confirm.clone(), action.clone(), self.status.clone());
    let expected = current.status();
    ctx.watch(&self.status, move |choice| {
      if choice.0 == expected {
        return;
      }
      if choice.0 == WorkflowStatus::Canceled && active {
        confirm.set(true);
        status.set(WorkflowChoice(expected));
      } else {
        submit.run(Mutation::Status(choice.0, false));
      }
    });
    let submit = action.clone();
    let expected = current.priority();
    ctx.watch(&self.priority, move |choice| {
      if choice.0 != expected {
        submit.run(Mutation::Priority(choice.0));
      }
    });
    let submit = action.clone();
    let expected = current.tags().to_vec();
    ctx.watch(&self.value, move |value| {
      if value.0.tags() != expected {
        submit.run(Mutation::Tags(value.0.tags().to_vec()));
      }
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
    if let Some(error) = action.state().get().error {
      content = content.child(field_error(&message_text(ctx, &error)));
    }
    Modal::new(backdrop(ctx, form(None, content)))
      .target(ModalTarget::Root)
      .open(self.confirm.clone())
      .dismiss_on_escape(!pending)
      .into()
  }
}

// Header controls gNExS/Sd5qt; confirmation N1vwaM.
const STATUS_WIDTH: f32 = 128.0;
const PRIORITY_WIDTH: f32 = 112.0;
const CONFIRM_WIDTH: f32 = 620.0;
