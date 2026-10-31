//! The form as one component: its fields, the version of the task its edits are made on (`base`), its
//! actions and where they lead (`handlers`), and what one render reads of them ([`View`]). The page is the composer
//! (`super::composer`) in a scroll area over the action bar.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::keyboard_event::KeyboardEvent,
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  limits::ProjectLimits,
  module::Module,
  time::Timestamp,
};
use orchester_workspace::{
  messages::Message,
  tasks::{data::PipelineChoice, estimate::Estimates},
};
use parking_lot::Mutex;

use super::{
  Nav,
  actions::{QueuedTask, RunAction, RunServices, SaveAction, run_action, save_action},
  bar::{Handlers, action_bar},
  base::{Base, Newer, Written},
  chips::Chips,
  conflict::{Choices, Conflict},
  fields::Fields,
  kept::KeptEdits,
  keys::FormKeys,
  sources::Drafted,
  state::{Phase, Subject, View},
};
use crate::{project::ShownProject, routes::Screen, services::Services, theme::TaskFormSpacing};

#[derive(Clone, PartialEq)]
pub struct FormProps {
  pub project: ShownProject,
  pub subject: Subject,
  /// The published pipelines; `None` while the project's tasks are read.
  pub pipelines: Option<Arc<Vec<PipelineChoice>>>,
  /// A shared task's pipeline, by its slug, that this computer has no pipeline for.
  pub missing_pipeline: Option<Arc<str>>,
  /// The project's default pipeline, published, which New task starts with.
  pub default_pipeline: Option<orchester_core::id::ResourceId>,
  /// The project's limits: New task starts with their default run budget and time limit, and the Limits chip says
  /// *Default* while the task holds them.
  pub limits: ProjectLimits,
  /// The project's modules, which the task may go in.
  pub modules: Arc<Vec<Module>>,
  /// Whether the project is shared on a server, which keeps no *Don't merge automatically*.
  pub shared: bool,
  /// The module a new task starts in, as a module's *New task in module* opens the form.
  pub starts_in: Option<orchester_core::id::ModuleId>,
  /// The project's tasks, whose kept edits stay; those of other tasks are removed.
  pub tasks: Arc<[TaskId]>,
  /// When the task was last saved, as the project's tasks were read.
  pub updated_at: Option<Timestamp>,
  /// The locale's digit group separator, for budgets.
  pub separator: Arc<str>,
}

impl DevtoolsInspectable for FormProps {}

/// When the edits the form opened with were kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptAt(pub Timestamp);

impl DevtoolsInspectable for KeptAt {}

pub struct TaskForm {
  save: SaveAction,
  run: RunAction,
  pub(super) fields: Fields,
  /// The version the edits are made on; `None` until the fields are filled.
  pub(super) base: Signal<Option<Base>>,
  /// A newer version stored while the fields hold edits.
  pub(super) newer: Signal<Option<Newer>>,
  /// The fields both versions changed, while the person chooses between them.
  pub(super) conflict: Signal<Option<Conflict>>,
  pub(super) choices: Signal<Choices>,
  /// The last action's failure was met on an earlier version: it is not said any more.
  pub(super) dismissed: Signal<bool>,
  pub(super) kept_at: Signal<Option<KeptAt>>,
  pub(super) seeded: Signal<bool>,
  pub(super) confirming_discard: Signal<bool>,
  pub(super) retrying: Signal<bool>,
  pub(super) queued: Signal<Option<QueuedTask>>,
  /// Generated once per form: every save of a new task names the same task.
  pub(super) batch: IdempotencyKey,
  pub(super) nav: Nav,
  pub(super) kept: KeptEdits,
  /// What this form last wrote to the kept edits.
  pub(super) written: Arc<Mutex<Written>>,
  /// A new task's sources, which Create stores with it; a stored task's are saved as they change.
  pub(super) sources: Signal<Drafted>,
  /// The property chips' popovers.
  pub(super) chips: Chips,
  /// The person asked to run while the run still needed something.
  pub(super) tried_run: Signal<bool>,
  /// The title's input, which has the focus on a new task.
  pub(super) title_input: ElementRef,
  pub(super) title_focused: Signal<bool>,
  /// The composer's column, which the Sources popover stays inside.
  pub(super) column: ElementRef,
}

/// Leaving the form writes the edits whose pause has not ended yet.
impl Drop for TaskForm {
  fn drop(&mut self) {
    self.kept.flush();
  }
}

impl Component for TaskForm {
  type Props = FormProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<FormProps>().clone();
    let services = Services::of(ctx);
    let project = props.project.open.project.id();
    let task = props.subject.task().map(|task| task.id());
    let batch = IdempotencyKey::generate();
    let queued = ctx.signal(None);
    let estimates = Estimates::new(
      services.workspace.clone(),
      services.runners.clone(),
      services.changes.clone(),
    );
    let save = save_action(
      ctx,
      (services.servers.clone(), services.programs.clone()),
      (estimates.clone(), props.project.open.clone()),
    );
    let run = run_action(
      ctx,
      RunServices {
        runners: services.runners.clone(),
        servers: services.servers.clone(),
        programs: services.programs.clone(),
        repo_root: props.project.open.repo_root.clone(),
        queued: queued.clone(),
        estimates: (estimates, props.project.open.clone()),
      },
    );
    let form = Self {
      save: save.clone(),
      run: run.clone(),
      fields: Fields::new(ctx),
      base: ctx.signal(None),
      newer: ctx.signal(None),
      conflict: ctx.signal(None),
      choices: ctx.signal(Choices::default()),
      dismissed: ctx.signal(false),
      kept_at: ctx.signal(None),
      seeded: ctx.signal(false),
      confirming_discard: ctx.signal(false),
      retrying: ctx.signal(false),
      queued,
      batch,
      nav: Nav::new(ctx, project),
      kept: KeptEdits::new(
        services.workspace.clone(),
        services.runtime,
        (project, task),
        &batch.to_string(),
      ),
      written: Arc::default(),
      sources: ctx.signal(Drafted::default()),
      chips: Chips::new(ctx),
      tried_run: ctx.signal(false),
      title_input: ElementRef::default(),
      title_focused: ctx.signal(false),
      column: ElementRef::default(),
    };
    form.follow_save(ctx, &save, &props);
    form.follow_run(ctx, &run, &props);
    form
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FormProps>().clone();
    let (save, run) = (self.save.clone(), self.run.clone());
    let separator = props.separator.clone();
    let screen = if props.subject.is_new() {
      Screen::NewTask
    } else {
      Screen::EditTask
    };
    let root = Column::new()
      .id(format!("screen-{}", screen.name()))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    if !self.seed(ctx, &props, &separator) {
      return root;
    }
    let busy = save.state().get_untracked().is_pending() || run.state().get_untracked().is_pending();
    self.follow_stored(&props, &separator, busy);
    let view = self.view(&props, (&save, &run), &separator);
    self.keep(&view, &props.project.feed);
    let handlers = self.handlers(&props, &view, (&save, &run));
    let keys = self.keys(&view, &handlers);
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Xl)
          .bottom(SpacingSize::Xl)
          .left(TaskFormSpacing::Side.role())
          .right(TaskFormSpacing::Side.role()),
      )
      .align_items(Alignment::Center)
      .child(self.composer(ctx, &props, &view, &separator));
    root
      .describe("phase", phase_name(&view.phase))
      .on_key_down(move |event: KeyboardEvent| keys.down(&event))
      .child(ScrollVertical::new(page).id("task-form-scroll").flex(1.0))
      .child(action_bar(ctx, &view, self.confirming_discard.get(), &handlers))
  }
}

const fn phase_name(phase: &Phase) -> &'static str {
  match phase {
    Phase::Editing => "editing",
    Phase::Saving => "saving",
    Phase::Creating => "creating",
    Phase::Starting(_) => "starting",
    Phase::NotStarted { .. } => "not_started",
  }
}

impl TaskForm {
  /// What Ctrl+Enter and Ctrl+S do now: what the bar's buttons would, while they can be pressed.
  fn keys(&self, view: &View, handlers: &Handlers) -> FormKeys {
    let editing = matches!(view.phase, Phase::Editing);
    let save_ready = if view.subject.is_new() {
      view.create_needs().is_empty()
    } else {
      view.save_changes_needs().is_some_and(|needs| needs.is_empty())
    };
    FormKeys {
      run: handlers.run.clone().filter(|_| editing && view.run_needs().is_empty()),
      offers_run: editing && !matches!(view.subject, Subject::Ran(_)),
      save: handlers.save.clone().filter(|_| editing && save_ready),
      active: !self.confirming_discard.get_untracked() && view.conflict.is_none(),
      tried_run: self.tried_run.clone(),
    }
  }

  /// The task the form is about: New task becomes a draft once Create & run saved it without queuing.
  fn subject(&self, props: &FormProps) -> Subject {
    match (&props.subject, self.base.get().and_then(|base| base.task)) {
      (Subject::New, Some(created)) => Subject::Draft(created),
      (subject, _) => subject.clone(),
    }
  }

  fn view(&self, props: &FormProps, actions: (&SaveAction, &RunAction), separator: &str) -> View {
    let values = self.fields.values();
    let form = values.task_form(separator);
    let (ready, errors) = match form.details() {
      Ok(details) => (Some(details), Default::default()),
      Err(errors) => (None, errors),
    };
    let missing = form.missing();
    let base = self.base.get();
    let stored_module = base.as_ref().and_then(Base::module);
    let stored_branch = base.as_ref().map(Base::base_branch).unwrap_or_default();
    let base = base.map(|base| base.values).unwrap_or_default();
    let module = self.fields.module.get().0;
    let base_branch = self.fields.base_branch.get();
    let (phase, failure) = self.phase(actions);
    View {
      subject: self.subject(props),
      phase,
      changed: values.differ_from(&base, separator) || module != stored_module || !base_branch.keeps(&stored_branch),
      module,
      base_branch,
      values,
      title_error: errors.title,
      budget_error: errors.budget,
      time_error: errors.time_limit,
      ready,
      missing,
      pipelines: props.pipelines.clone(),
      missing_pipeline: props.missing_pipeline.clone(),
      kept_at: self.kept_at.get().map(|at| at.0),
      failure,
      retrying: self.retrying.get(),
      newer: self.newer.get().is_some(),
      conflict: self.conflict.get(),
      choices: self.choices.get(),
      tried_run: self.tried_run.get(),
    }
  }

  /// What the form is doing, and why its last save or Create & run failed before the task was queued.
  fn phase(&self, (save, run): (&SaveAction, &RunAction)) -> (Phase, Option<Arc<[Message]>>) {
    let saving = save.state().get();
    let running = run.state().get();
    let queued = self.queued.get();
    if saving.is_pending() {
      return (Phase::Saving, None);
    }
    if running.is_pending() {
      return (queued.map_or(Phase::Creating, Phase::Starting), None);
    }
    if let Some(queued) = queued {
      let problems = running
        .error
        .as_ref()
        .filter(|problems| problems.queued.is_some())
        .map_or_else(|| Arc::from([]), |problems| problems.messages.clone());
      return (Phase::NotStarted { queued, problems }, None);
    }
    if self.dismissed.get() {
      return (Phase::Editing, None);
    }
    let failure = match (&saving.error, &running.error) {
      (_, Some(problems)) if running.is_rejected() && problems.queued.is_none() => Some(problems.messages.clone()),
      (Some(problems), _) if saving.is_rejected() => Some(problems.0.clone()),
      _ => None,
    };
    (Phase::Editing, failure)
  }
}
