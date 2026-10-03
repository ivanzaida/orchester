//! New task (02.09 `l2YA4`): brief and acceptance criteria on the left, pipeline and limits
//! on the right. Create saves the task as a draft without running it, incomplete if need be, under
//! a key generated when the form opens, and returns to the task list, where the new task is
//! highlighted: a failed save keeps the form, and retrying cannot create a second task.
//! Create & run checks that the run can start, saves and queues the task and asks the runner to
//! start its run under a second key generated when the form opens, so a retry after a failure
//! never starts a second run; it then opens the task page. Opened on a saved draft, the same form
//! edits it ([`edit`]). A task with a recorded run can be saved for future runs without
//! starting another run from this form.

mod brief;
mod cancellation;
mod edit;
mod header;
mod persistence;
mod plan;
mod start;
mod state;

/// Room the brief's scroll content keeps on its left for the outer focus rings of controls on
/// its edge: the ring (`focus-extent`) and the padding of a link inside it.
const FOCUS_ROOM: SpacingSize = SpacingSize::Sm;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId, TaskId},
  limits::ProjectLimits,
  task::{MissingDetail, Task, TaskDetails, organization::TaskOrganization},
  tokens::grouped,
};

pub use self::edit::{Editing, Recovering};
use self::{
  brief::Criterion,
  persistence::{repository_link, save_action},
  start::run_action,
};
use super::{ProjectNav, organization_fields::OrganizationDraft};
use crate::{
  budget::minutes_text,
  messages::Message,
  navigation::route::Page,
  tabs::dirty::EditMark,
  tasks::{
    data::{PipelineChoice, SaveTarget},
    form::FormErrors,
    list::missing_key,
    run::StartOutcome,
  },
  theme::{ExtraSpacing, gutter_width},
  ui::{
    form::form,
    icon::{Icon, IconSize, icon},
    unit_input::FocusedInput,
  },
  workspace::OpenProject,
};

#[derive(Clone, Debug, PartialEq)]
pub struct NewTaskProps {
  pub project: OpenProject,
  /// Published pipelines, once the project's data is read.
  pub pipelines: Option<Arc<Vec<PipelineChoice>>>,
  /// The project's limits, once read: the defaults the form starts with and the maximum budget.
  pub limits: Option<ProjectLimits>,
  /// The saved task the form edits (Edit task); `None` for a new task.
  pub editing: Option<Editing>,
  /// A queued task reopened after the form was closed, with its pinned published pipeline.
  pub recovering: Option<Recovering>,
}

impl lurq::app::component::DevtoolsInspectable for NewTaskProps {}

/// The chosen pipeline, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineSelection(pub Option<ResourceId>);

impl lurq::app::component::DevtoolsInspectable for PipelineSelection {}

/// The latest draft save that completed successfully.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedDraft {
  details: TaskDetails,
  organization: TaskOrganization,
  /// The task Create made; `None` when the form saved over a task it edits.
  created: Option<TaskId>,
}

impl lurq::app::component::DevtoolsInspectable for SavedDraft {}

/// Where to save, what: the details and the organization as the form loaded it and as it is now
/// (only what the person changed is saved), and the run the person confirmed stopping for
/// Canceled.
type SaveArgs = (
  PathBuf,
  SaveTarget,
  TaskDetails,
  (TaskOrganization, TaskOrganization),
  Option<orchester_core::id::RunId>,
);
type SaveAction = FutureAction<SaveArgs, SavedDraft, Message>;
/// A save, with the run the person confirmed stopping, if any.
type SaveRequest = Arc<dyn Fn(Option<orchester_core::id::RunId>) + Send + Sync>;

/// The task whose run started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartedTask(TaskId);

impl lurq::app::component::DevtoolsInspectable for StartedTask {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct QueuedTask {
  task: TaskId,
  outcome: StartOutcome,
  pipeline: Arc<PipelineChoice>,
}

impl lurq::app::component::DevtoolsInspectable for QueuedTask {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StartingPipeline(Arc<PipelineChoice>);

impl lurq::app::component::DevtoolsInspectable for StartingPipeline {}

/// Why the start request failed, and any task already queued for retry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunProblems {
  pub(super) messages: Arc<[Message]>,
  queued: Option<QueuedTask>,
}

impl lurq::app::component::DevtoolsInspectable for RunProblems {}

/// A new task must be queued once; a retry starts the same queued task under the same run key.
#[derive(Clone)]
enum RunArgs {
  New {
    repo_root: PathBuf,
    target: SaveTarget,
    details: Box<TaskDetails>,
    /// As loaded and as edited.
    organization: (TaskOrganization, TaskOrganization),
    pipeline: Arc<PipelineChoice>,
  },
  Retry {
    queued: QueuedTask,
  },
}
type RunAction = FutureAction<RunArgs, StartedTask, RunProblems>;

/// Everything a part of the form needs to know about the current values.
struct FormState {
  errors: FormErrors,
  missing: Vec<MissingDetail>,
  ready: Option<TaskDetails>,
  saving: bool,
  /// The form holds what is stored: the latest save, or the task being edited.
  saved: bool,
  /// The form edits a saved task (Edit task).
  editing: bool,
  /// An existing run fixes its own prompt; this form changes future runs only.
  started: bool,
  queued: bool,
  recovered: bool,
  save_error: Option<Message>,
  /// Create & run is under way.
  starting: bool,
  /// This pending request was started from Retry run, not the first Create & run.
  retrying: bool,
  /// The selected published version while the first request has not queued the task yet.
  starting_pipeline: Option<Arc<PipelineChoice>>,
  queued_task: Option<QueuedTask>,
  run_problems: Option<RunProblems>,
}

impl FormState {
  fn is_missing(&self, detail: MissingDetail) -> bool {
    self.missing.contains(&detail)
  }

  /// The task is queued and no run is recorded for it: its details are locked until the runner
  /// starts its first run.
  const fn awaits_first_run(&self) -> bool {
    self.queued && !self.started
  }

  /// Whether Create, or Save changes for a draft being edited, can be pressed: not while a save or
  /// Create & run is under way or the task is queued, and for a draft being edited not while it
  /// holds what is stored.
  const fn can_save(&self) -> bool {
    !(self.saving
      || self.starting
      || self.queued
      || (self.editing && self.saved)
      || (self.started && !self.missing.is_empty()))
  }
}

pub struct NewTaskForm {
  organization: OrganizationDraft,
  /// The confirmation that saving Canceled stops the task's active run is open.
  cancel_confirmation: Signal<bool>,
  title: Signal<String>,
  brief: Signal<String>,
  criteria: Signal<Vec<Criterion>>,
  pipeline: Signal<PipelineSelection>,
  budget: Signal<String>,
  time_limit: Signal<String>,
  /// Which limit input has the focus, for its focused stroke.
  focused_input: FocusedInput,
  /// The fields were filled in, from the project's defaults or the draft being edited; they are
  /// filled in once.
  seeded: Signal<bool>,
  /// Generated once per form: every save attempt of this form uses it.
  batch: IdempotencyKey,
  queued_task: Signal<Option<QueuedTask>>,
  retrying: Signal<bool>,
  starting_pipeline: Signal<Option<StartingPipeline>>,
  nav: ProjectNav,
  /// The edits of a draft being edited that are not saved, which leaving the form asks about.
  unsaved: Option<EditMark>,
}

impl Component for NewTaskForm {
  type Props = NewTaskProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let save = save_action(ctx);
    let saved = nav.clone();
    let cancel_confirmation = ctx.signal(false);
    let confirm = cancel_confirmation.clone();
    let organization = OrganizationDraft::new(ctx);
    let loaded = organization.loaded.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_rejected() {
        if state.error.as_ref().is_some_and(persistence::needs_cancel_confirmation) {
          confirm.set(true);
        }
        return;
      }
      // A failed save keeps the data of the last one that succeeded: only a save that has just
      // succeeded closes the confirmation and reports the change.
      let Some(draft) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
        return;
      };
      confirm.set(false);
      // The next save writes what the person changes from here.
      loaded.set(crate::tasks::organization::OrganizationValue(
        draft.organization.clone(),
      ));
      saved.tasks_changed();
      if let Some(task) = draft.created {
        saved.show_created(task);
      }
    });
    let queued_task = ctx.signal(None);
    let retrying = ctx.signal(false);
    let starting_pipeline = ctx.signal(None);
    let run = run_action(ctx, queued_task.clone());
    let queued_on_failure = queued_task.clone();
    let started = nav.clone();
    ctx.watch(&run.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if let Some(task) = state.error.as_ref().and_then(|problems| problems.queued.clone()) {
        queued_on_failure.set(Some(task));
      }
      // The task may have been saved and queued even when the start outcome is unknown.
      started.tasks_changed();
      if let Some(StartedTask(task)) = state.data {
        started.finish(Page::task(task));
      }
    });
    Self {
      organization,
      cancel_confirmation,
      title: ctx.signal(String::new()),
      brief: ctx.signal(String::new()),
      criteria: ctx.signal(vec![Criterion::new(0)]),
      pipeline: ctx.signal(PipelineSelection(None)),
      budget: ctx.signal(String::new()),
      time_limit: ctx.signal(String::new()),
      focused_input: ctx.signal(None),
      seeded: ctx.signal(false),
      batch: IdempotencyKey::generate(),
      queued_task,
      retrying,
      starting_pipeline,
      nav,
      unsaved: EditMark::of(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<NewTaskProps>().clone();
    let save = save_action(ctx);
    let run = run_action(ctx, self.queued_task.clone());
    let link = repository_link(ctx, &props.project);
    let limits = props.limits.unwrap_or_default();
    self.seed(ctx, props.limits, props.editing.as_ref(), props.recovering.as_ref());
    let edited = props.editing.as_ref();
    let editing = edited.map(|editing| &*editing.task);
    let recovered = props.recovering.as_ref().map(|recovering| &*recovering.task);
    let separator = ctx.t("number.groupSeparator");
    let state = self.state(&save, &run, &separator, (limits, edited), recovered.is_some());
    self.mark_unsaved(&state, editing);
    let target = self.target(editing.or(recovered));
    let submit = self
      .save_request(&save, &props.project, target, state.ready.clone())
      .map(|request| -> Arc<dyn Fn() + Send + Sync> { Arc::new(move || request(None)) })
      .filter(|_| state.can_save());
    let start = (!state.started)
      .then(|| {
        self.run_handler(
          &run,
          (&props.project, target, props.pipelines.as_deref()),
          state.ready.clone(),
          state.queued_task.clone(),
        )
      })
      .flatten()
      .filter(|_| {
        self.organization.ready.get()
          && !state.saving
          && !state.starting
          && (state.queued_task.is_some() || state.missing.is_empty())
      });
    // The brief's scroll viewport starts `FOCUS_ROOM` early and its content is padded by the
    // same amount, so the content stays at the designed inset and keeps room for focus rings.
    let indent = ctx.theme().spacing_value(ExtraSpacing::Indent.role()).resolve(0.0);
    let brief_left = indent - ctx.theme().spacing_value(FOCUS_ROOM).resolve(0.0);
    let content = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding_top(ExtraSpacing::Inline.role())
      .padding_left(Dimension::Px(brief_left))
      // The plan's scroll gutter sits in the right padding, so its content ends at the designed
      // inset (`HVSv5`).
      .padding_right(Dimension::Px(indent - gutter_width()))
      .padding_bottom(SpacingSize::Section)
      .spacing(ExtraSpacing::Indent.role())
      .child(brief::brief_form(ctx, self, &props.project, link.as_deref(), &state))
      .child(plan::execution_plan(
        ctx,
        self,
        (props.pipelines.as_deref(), limits),
        &state,
        submit.clone(),
        start,
      ));
    // A draft this form queued reads as the queued task of D5 (`WOdDe`, `T6bVyU`) until its run
    // starts, whether the form was opened on the draft or reopened on the queued task.
    let (still_editing, retried) = match editing {
      Some(task) if state.awaits_first_run() => (None, Some(task)),
      _ => (editing, recovered),
    };
    Column::new()
      .id("task-form")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(header::topbar(
        ctx,
        props.project.project.name().as_str(),
        &self.nav,
        (&state, still_editing, retried),
      ))
      .child(header::heading(ctx, still_editing, &state))
      .child(form(submit, content))
      .child(cancellation::confirmation(
        ctx,
        self.cancel_confirmation.clone(),
        &props,
        &save,
        self.save_request(&save, &props.project, target, state.ready.clone()),
      ))
  }
}

impl NewTaskForm {
  /// Fills in the draft being edited, or else the project's default run budget and time limit
  /// once they are read, unless something was typed there already.
  fn seed(&self, ctx: &Ctx, limits: Option<ProjectLimits>, editing: Option<&Editing>, recovering: Option<&Recovering>) {
    if self.seeded.get_untracked() {
      return;
    }
    if let Some(editing) = editing {
      self.seeded.set(true);
      edit::open(ctx, self, editing);
      return;
    }
    if let Some(recovering) = recovering {
      self.seeded.set(true);
      edit::seed(ctx, self, &recovering.task);
      self.queued_task.set(Some(QueuedTask {
        task: recovering.task.id(),
        outcome: StartOutcome::ConfirmedNotStarted,
        pipeline: recovering.pipeline.clone(),
      }));
      return;
    }
    let Some(limits) = limits else {
      return;
    };
    self.seeded.set(true);
    if self.budget.get_untracked().is_empty() {
      let separator = ctx.t("number.groupSeparator");
      self.budget.set(grouped(limits.default_run_budget().get(), &separator));
    }
    if self.time_limit.get_untracked().is_empty() {
      self.time_limit.set(minutes_text(limits.default_time_limit()));
    }
  }

  /// Where the form saves: over the draft it edits, or as a new draft under the form's key.
  fn target(&self, editing: Option<&Task>) -> SaveTarget {
    editing.map_or(SaveTarget::New(self.batch), |task| SaveTarget::Task(task.id()))
  }

  /// Records whether a draft being edited holds changes that are not saved, so leaving the form
  /// or closing its tab asks first.
  fn mark_unsaved(&self, state: &FormState, editing: Option<&Task>) {
    if let (Some(mark), Some(task)) = (&self.unsaved, editing) {
      let unsaved = !state.saved && !state.queued;
      mark.set_asking(unsaved.then(|| edit::leave_question(task)));
    }
  }

  /// Saves the valid form: Create under the form's key, or over the task it edits, stopping the
  /// run the person confirmed for Canceled; `None` while it cannot be saved.
  fn save_request(
    &self,
    save: &SaveAction,
    project: &OpenProject,
    target: SaveTarget,
    ready: Option<TaskDetails>,
  ) -> Option<SaveRequest> {
    let details = ready?;
    // The organization fields explain their own errors; the form cannot be saved meanwhile.
    let organization = self.organization.change()?;
    let save = save.clone();
    let repo_root = project.repo_root.clone();
    Some(Arc::new(move |named| {
      save.run((repo_root.clone(), target, details.clone(), organization.clone(), named));
    }))
  }
}

/// The detail is needed before the task can run; a draft may still be saved without it.
fn missing_note(ctx: &Ctx, state: &FormState, detail: MissingDetail) -> Option<Element> {
  state.is_missing(detail).then(|| {
    Row::new()
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .describe("role", "missing-detail")
      .child(icon(Icon::CircleDot, IconSize::Small, PaletteColor::TextMuted))
      .child(Text::new(&ctx.t(missing_key(detail))).variant(TypographyStyle::Caption))
      .into()
  })
}
