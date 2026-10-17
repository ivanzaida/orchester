//! The form as one component: its fields, the version of the task its edits are made on (`base`), its
//! actions and where they lead (`handlers`), and what one render reads of them ([`View`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  time::Timestamp,
};
use orchester_workspace::{messages::Message, tasks::data::PipelineChoice};
use parking_lot::Mutex;

use super::{
  FORM_WIDTH, Nav,
  actions::{QueuedTask, RunAction, RunServices, SaveAction, run_action, save_action},
  bar::action_bar,
  base::{Base, Newer, Written},
  brief::{BriefFields, brief},
  conflict::{Choices, Conflict, conflict_parts},
  fields::Fields,
  head::{head, no_run_notice},
  kept::KeptEdits,
  limits::{LimitFields, limits_section},
  plan::pipeline_section,
  state::{Phase, Subject, View},
  unbuilt::{context_section, instructions_section},
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
    let save = save_action(ctx, services.servers.clone());
    let run = run_action(
      ctx,
      RunServices {
        runners: services.runners.clone(),
        servers: services.servers.clone(),
        repo_root: props.project.open.repo_root.clone(),
        queued: queued.clone(),
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
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(TaskFormSpacing::PageTop.role())
          .bottom(TaskFormSpacing::PageBottom.role())
          .left(SpacingSize::Xl)
          .right(SpacingSize::Xl),
      )
      .align_items(Alignment::Center)
      .child(self.form(ctx, &props, &view, &separator));
    root
      .describe("phase", phase_name(&view.phase))
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
  /// The page's sections, in the design's order, or the fields to choose between while a newer version
  /// conflicts with the edits.
  fn form(&self, ctx: &mut Ctx, props: &FormProps, view: &View, separator: &str) -> Column {
    let page = Column::new()
      .id("task-form")
      .width(Dimension::Px(FORM_WIDTH))
      .spacing(SpacingSize::Section)
      .child(head(ctx, view, &self.nav));
    if let Some(conflict) = &view.conflict {
      let pipelines = props.pipelines.as_deref().map(Vec::as_slice);
      return page.with_children(conflict_parts(ctx, conflict, &self.choices, pipelines));
    }
    let brief_fields = BriefFields {
      title: &self.fields.title,
      brief: &self.fields.brief,
      criteria: &self.fields.criteria,
    };
    let limit_fields = LimitFields {
      budget: &self.fields.budget,
      time_limit: &self.fields.time_limit,
    };
    page
      .with_children(no_run_notice(ctx, view))
      .child(brief(ctx, view, &props.project.open, &brief_fields))
      .child(context_section(ctx))
      .child(pipeline_section(ctx, view, &self.fields.pipeline, &self.nav))
      .child(limits_section(ctx, view, &limit_fields, separator))
      .child(instructions_section(ctx))
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
    let base = self.base.get().map(|base| base.values).unwrap_or_default();
    let (phase, failure) = self.phase(actions);
    View {
      subject: self.subject(props),
      phase,
      changed: values.differ_from(&base, separator),
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
