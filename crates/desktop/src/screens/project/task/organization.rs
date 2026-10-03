//! The task page's status, priority and tag pickers (`gNExS`, `Sd5qt`) and the confirmation that
//! Canceled stops the task's active run (`N1vwaM`). A choice saves at once; the pickers then show
//! what is stored.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Text},
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  id::{RunId, TagId},
  task::organization::{ChangeActor, OrganizationError, Priority, TaskOrganization, WorkflowStatus},
  time::Timestamp,
};
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
    cancel_run::{Stop, stop_named_run},
    detail::TaskDetail,
    organization::{
      OrganizationValue, PickerAppearance, PriorityChoice, WorkflowChoice, priority_select, workflow_select,
    },
  },
  theme::ExtraSpacing,
  ui::{
    backdrop::backdrop,
    cancel_dialog::{CancelDialog, cancel_dialog},
    field::field_error,
    form::form,
    message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct OrganizationProps {
  pub root: PathBuf,
  pub detail: Arc<TaskDetail>,
}
impl lurq::app::component::DevtoolsInspectable for OrganizationProps {}
#[derive(Clone)]
enum Mutation {
  /// A status, with whether the person confirmed stopping a run for Canceled.
  Status(WorkflowStatus, Stop),
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
  confirm: Signal<bool>,
  /// The active run the store reported when Canceled was refused: the one the confirmation names
  /// and stops, even before the page has read it.
  asked: Signal<AskedRun>,
}

/// The active run the store named when it refused Canceled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AskedRun(Option<RunId>);
impl lurq::app::component::DevtoolsInspectable for AskedRun {}

impl Component for OrganizationHeader {
  type Props = OrganizationProps;
  fn create(ctx: &mut Ctx) -> Self {
    let header = Self {
      status: ctx.signal(WorkflowChoice(WorkflowStatus::Backlog)),
      priority: ctx.signal(PriorityChoice(Priority::None)),
      value: ctx.signal(OrganizationValue::default()),
      shown: ctx.signal(None),
      stored: ctx.signal(OrganizationValue::default()),
      confirm: ctx.signal(false),
      asked: ctx.signal(AskedRun(None)),
    };
    let action = action(ctx, &header.asked);
    header.observe(ctx, &action);
    header
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OrganizationProps>().clone();
    let action = action(ctx, &self.asked);
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
          .child(Text::new(&run_suffix(ctx, &props.detail)).variant(TypographyStyle::Description))
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
    // While the confirmation is open it shows the save and its failure itself (`Fhzcc`), so the
    // header keeps its height.
    let confirming = self.confirm.get();
    if state.is_pending() && !confirming {
      column = column.child(Text::new(&ctx.t("tasks.workflow.saving")).variant(TypographyStyle::Caption));
    }
    // Refused because the task has an active run: the confirmation that opened says so.
    if let Some(error) = state
      .error
      .filter(|error| error.key != messages::CANCEL_CONFIRM_REQUIRED && !confirming)
    {
      column = column.child(field_error(&message_text(ctx, &error)));
    }
    column.child(self.confirmation(ctx, &props, &action))
  }
}

/// Saves one choice for the task the page shows. The call in the header's `create` and every
/// `render` refer to the same action; each render points it at the current task.
fn action(ctx: &mut Ctx, asked: &Signal<AskedRun>) -> Action {
  let props = ctx.props::<OrganizationProps>();
  let (root, task) = (props.root.clone(), props.detail.task.id());
  let runners = ctx.use_context::<Runners>().expect("root provides runners");
  let asked = asked.clone();
  ctx.future_action(move |mutation: Mutation| {
    let (root, runners, asked) = (root.clone(), runners.clone(), asked.clone());
    async move {
      // Canceled stops only the run the confirmation named; otherwise it names the active run.
      if let Mutation::Status(WorkflowStatus::Canceled, stop) = mutation
        && let Err(refused) = stop_named_run(&root, task, stop, &runners).await
      {
        if let Some(run) = refused.run() {
          asked.set(AskedRun(Some(run)));
        }
        return Err(refused.message());
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
      .map_err(|error| {
        // A run that started since the check: the store names it, and the confirmation stops it.
        if let orchester_store::StoreError::Organization(OrganizationError::StopRunFirst(run)) = &error {
          asked.set(AskedRun(Some(*run)));
        }
        messages::store_error(&error)
      })
    }
  })
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

  /// Seeds the pickers when the detail shows another organization.
  fn follow(&self, detail: &TaskDetail) {
    let current = detail.task.organization();
    if self.shown.get_untracked().as_ref().map(|shown| &shown.0) != Some(current) {
      self.shown.set(Some(OrganizationValue(current.clone())));
      self.choices().show_stored(current.clone());
    }
  }

  /// Registered once, in `create`: each observer compares a choice with what is stored when the
  /// choice is made.
  fn observe(&self, ctx: &mut Ctx, action: &Action) {
    self.observe_saves(ctx, action);
    // Whether Canceled has a run to stop is the store's to say: the detail may not show yet that
    // a run started or ended. A refused Canceled opens the confirmation (`observe_saves`).
    let (choices, submit) = (self.choices(), action.clone());
    ctx.watch(&self.status, move |choice| {
      if choice.0 != choices.stored.get_untracked().0.status() {
        submit.run(Mutation::Status(choice.0, Stop::Unconfirmed));
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
    // The run the store named; the page shows its number and branch once it has read it.
    let named = self.asked.get().0;
    let run = props
      .detail
      .run
      .as_ref()
      .map(|run| &run.run)
      .filter(|run| Some(run.id()) == named);
    let state = action.state().get();
    let pending = state.is_pending();
    let error = state
      .error
      .filter(|error| error.key != messages::CANCEL_CONFIRM_REQUIRED)
      .map(|error| message_text(ctx, &error).to_string());
    let (close, submit) = (self.confirm.clone(), action.clone());
    let dialog = single_cancel(
      ctx,
      (&props.detail.task, run),
      ("task-cancel-confirmation", "task-cancel-keep", "task-cancel-stop"),
      pending,
      error.as_deref(),
    );
    let content = cancel_dialog(
      ctx,
      dialog,
      move || close.set(false),
      move || submit.run(Mutation::Status(WorkflowStatus::Canceled, Stop::Confirmed(named))),
    );
    Modal::new(backdrop(ctx, form(None, content)))
      .target(ModalTarget::Root)
      .open(self.confirm.clone())
      .dismiss_on_escape(!pending)
      .into()
  }
}

/// What the confirmation for one task says (`N1vwaM`): its active run stops, its work stays on its
/// branch. Without the run (not read yet, or it ended meanwhile) only the question remains.
pub(in crate::screens::project) fn single_cancel<'a>(
  ctx: &Ctx,
  (task, run): (&orchester_core::task::Task, Option<&orchester_core::run::Run>),
  (id, keep, stop): (&'static str, &'static str, &'static str),
  pending: bool,
  error: Option<&'a str>,
) -> CancelDialog<'a> {
  CancelDialog {
    id,
    title: ctx.t("tasks.cancel.title"),
    explanation: run.map(|run| {
      ctx.t_args(
        "tasks.cancel.explanation",
        [
          ("task", task.key().to_string().as_str()),
          ("title", task.details().title.as_str()),
          ("run", run.number().get().to_string().as_str()),
        ],
      )
    }),
    notice: run.map(|run| {
      (
        ctx.t("tasks.cancel.workKept"),
        ctx.t_args("tasks.cancel.workKeptDetail", [("branch", run.branch().as_str())]),
      )
    }),
    keep: (ctx.t("tasks.cancel.keep"), keep),
    confirm: (ctx.t("tasks.cancel.confirm"), stop),
    pending,
    error,
  }
}

/// What the header says after the workflow status about the task's run (`Fhzcc`, `tUwII`): its
/// state, "Canceled run" in the workflow's spelling, or that it has none.
fn run_suffix(ctx: &Ctx, detail: &TaskDetail) -> Arc<str> {
  let run = match (&detail.run, detail.phase()) {
    (None, _) => ctx.t("tasks.workflow.noActiveRun"),
    (Some(_), orchester_core::run::TaskPhase::Cancelled) => ctx.t("tasks.workflow.canceledRun"),
    (Some(_), phase) => ctx.t(super::super::task_rows::status_copy(phase).1),
  };
  ctx.t_args("tasks.workflow.runSuffix", [("run", run.as_ref())])
}
