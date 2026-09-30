//! New task (02.09 `l2YA4`): brief and acceptance criteria on the left, pipeline and limits
//! on the right. The task is saved as a draft, incomplete if need be, under a key generated
//! when the form opens: a failed save keeps the form, and retrying cannot create a second task.
//! Create & run checks that the run can start, saves and queues the task and asks the runner to
//! start its run under a second key generated when the form opens, so a retry after a failure
//! never starts a second run; it then opens the task page. Opened on a saved draft, the same form
//! edits it ([`edit`]). A task with a recorded run can be saved for future runs without
//! starting another run from this form.

mod brief;
mod cancellation;
mod edit;
mod persistence;
mod plan;
mod start;
mod state;
use persistence::{repository_link, save_action};

/// Room the brief's scroll content keeps on its left for the outer focus rings of controls on
/// its edge: the ring (`focus-extent`) and the padding of a link inside it.
const FOCUS_ROOM: SpacingSize = SpacingSize::Sm;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId, TaskId},
  limits::ProjectLimits,
  task::{MissingDetail, Task, TaskDetails, TaskStatus},
  tokens::grouped,
};

pub use self::edit::{Editing, Recovering};
use self::{brief::Criterion, start::run_action};
use super::ProjectNav;
use crate::{
  budget::minutes_text,
  messages::Message,
  navigation::route::Page,
  tabs::dirty::EditMark,
  tasks::{
    data::{PipelineChoice, SaveTarget},
    form::{FormErrors, TaskForm},
    list::missing_key,
    run::StartOutcome,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    form::{Submit, form},
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    unit_input::FocusedInput,
  },
  workspace::OpenProject,
};

/// Topbar height (`dWjfr`).
const TOPBAR_HEIGHT: f32 = 64.0;

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

/// Details of the latest draft save that completed successfully.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedDraft(TaskDetails, orchester_core::task::organization::TaskOrganization);

impl lurq::app::component::DevtoolsInspectable for SavedDraft {}

type SaveArgs = (
  PathBuf,
  SaveTarget,
  TaskDetails,
  orchester_core::task::organization::TaskOrganization,
  bool,
);
type SaveAction = FutureAction<SaveArgs, SavedDraft, Message>;

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
    organization: orchester_core::task::organization::TaskOrganization,
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

  /// Whether Save as draft can be pressed: not while a save or Create & run is under way or the
  /// task is queued, and for a draft being edited not while it holds what is stored.
  const fn can_save(&self) -> bool {
    !(self.saving
      || self.starting
      || self.queued
      || (self.editing && self.saved)
      || (self.started && !self.missing.is_empty()))
  }
}

pub struct NewTaskForm {
  organization: super::organization_fields::OrganizationDraft,
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
    ctx.watch(&save.state(), move |state| {
      if state
        .error
        .as_ref()
        .is_some_and(|error| error.key == "tasks.cancel.confirmRequired")
        && !state.is_pending()
      {
        confirm.set(true);
      }
      if state.data.is_some() && !state.is_pending() {
        confirm.set(false);
        saved.tasks_changed();
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
      organization: super::organization_fields::OrganizationDraft::new(ctx),
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
    let editing = props.editing.as_ref().map(|editing| &*editing.task);
    let recovered = props.recovering.as_ref().map(|recovering| &*recovering.task);
    let separator = ctx.t("number.groupSeparator");
    let state = self.state(&save, &run, &separator, (limits, editing), recovered.is_some());
    self.mark_unsaved(&state, editing);
    let target = self.target(editing.or(recovered));
    let submit = self
      .save_handler(&save, &props.project, target, state.ready.clone(), false)
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
    let brief_left = ctx.theme().spacing_value(ExtraSpacing::Indent.role()).resolve(0.0)
      - ctx.theme().spacing_value(FOCUS_ROOM).resolve(0.0);
    let content = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding_top(ExtraSpacing::Inline.role())
      .padding_left(Dimension::Px(brief_left))
      .padding_right(ExtraSpacing::Indent.role())
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
    Column::new()
      .id("task-form")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(topbar(
        ctx,
        props.project.project.name().as_str(),
        &self.nav,
        (&state, editing, recovered),
      ))
      .child(heading(ctx, editing, &state))
      .child(form(submit, content))
      .child(cancellation::confirmation(
        ctx,
        self.cancel_confirmation.clone(),
        &props,
        &save,
        self.save_handler(&save, &props.project, target, state.ready.clone(), true),
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

  /// Saves the valid form under the form's key; `None` while it cannot be saved.
  fn save_handler(
    &self,
    save: &SaveAction,
    project: &OpenProject,
    target: SaveTarget,
    ready: Option<TaskDetails>,
    confirmed: bool,
  ) -> Submit {
    let details = ready?;
    // The organization fields display validation errors and disable submission.
    let organization = self.organization.snapshot().ok()?;
    let save = save.clone();
    let repo_root = project.repo_root.clone();
    Some(Arc::new(move || {
      save.run((
        repo_root.clone(),
        target,
        details.clone(),
        organization.clone(),
        confirmed,
      ));
    }))
  }
}

/// Back to the task list, or to the task being edited, the breadcrumb and the persisted task state
/// (`dWjfr`, `kTm0n`): a draft being edited reads *Unsaved changes* once it differs from the
/// saved one.
fn topbar(
  ctx: &Ctx,
  project: &str,
  nav: &ProjectNav,
  (form, editing, recovered): (&FormState, Option<&Task>, Option<&Task>),
) -> Element {
  let nav = nav.clone();
  let back_to = editing.or(recovered).map_or(Page::TASKS, |task| Page::task(task.id()));
  let back = pressable(surface().id("new-task-back"), move || nav.leave(back_to))
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .child(icon(Icon::ArrowLeft, IconSize::Option, PaletteColor::TextSecondary));
  let state = if form.starting {
    ctx.t("tasks.new.starting")
  } else if form.saving {
    ctx.t(if form.editing {
      "tasks.edit.savingChanges"
    } else {
      "tasks.new.saving"
    })
  } else if let Some(queued) = form.queued_task.as_ref() {
    ctx.t(queued.outcome.status_key())
  } else if form.saved {
    ctx.t(if form.started {
      "tasks.edit.saved"
    } else {
      "tasks.new.saved"
    })
  } else if form.editing {
    ctx.t("tasks.edit.unsaved")
  } else {
    ctx.t("tasks.new.notSaved")
  };
  let state = Text::new(&state).variant(TypographyStyle::Caption).id("new-task-state");
  // Changes to a saved draft that are not saved yet read in `$text` (`ctHXE`).
  let state = if form.editing && !form.saved && !form.saving && !form.starting && !form.queued {
    state.color(PaletteColor::TextPrimary)
  } else {
    state
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(back)
    .child(Text::new(&edit::breadcrumb(ctx, project, editing, recovered)).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(state)
    .into()
}

/// Heading (`hkWbp`; `h79DDd` while editing).
fn heading(ctx: &Ctx, editing: Option<&Task>, state: &FormState) -> Element {
  let (title, description) = edit::heading_texts(ctx, editing);
  let description = if state.starting && state.queued_task.is_some() && !state.retrying {
    ctx.t("tasks.new.firstStartQueuedDescription")
  } else if state.recovered && state.run_problems.is_none() {
    ctx.t("tasks.new.recoveredDescription")
  } else if let Some(queued) = state.queued_task.as_ref() {
    ctx.t(queued.outcome.description_key())
  } else {
    description
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Section)
    .padding_horizontal(ExtraSpacing::Indent.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&title).variant(ExtraTypography::ScreenTitle.role()))
    .child(
      Text::new(&description)
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    )
    .into()
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
