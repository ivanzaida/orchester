//! The form as one component: its fields, what it holds compared with what is stored, its actions and
//! where they lead (`handlers`). It fills its fields once, from the task it edits or from edits kept on
//! this computer (`kept`), and follows the stored task afterwards while the person changed nothing.

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
use orchester_core::{id::IdempotencyKey, task::Task, time::Timestamp};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{data::PipelineChoice, run::StartOutcome},
};
use parking_lot::Mutex;

use super::{
  FORM_WIDTH, Nav,
  actions::{QueuedTask, RunAction, RunServices, SaveAction, Saved, run_action, save_action},
  bar::action_bar,
  brief::{BriefFields, brief},
  criteria::{Criterion, texts},
  head::{head, no_run_notice},
  kept::{Kept, KeptEdits},
  limits::{LimitFields, limits_section},
  plan::{Pick, pipeline_section},
  state::{Phase, Subject, View},
  unbuilt::{context_section, instructions_section},
  values::Values,
};
use crate::{project::ShownProject, routes::Screen, services::Services, theme::TaskFormSpacing};

#[derive(Clone, PartialEq)]
pub struct FormProps {
  pub project: ShownProject,
  pub subject: Subject,
  /// The published pipelines; `None` while the project's tasks are read.
  pub pipelines: Option<Arc<Vec<PipelineChoice>>>,
}

impl DevtoolsInspectable for FormProps {}

/// What reading the kept edits found.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KeptRead(Option<Kept>);

impl DevtoolsInspectable for KeptRead {}

/// When the edits the form opened with were kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptAt(pub Timestamp);

impl DevtoolsInspectable for KeptAt {}

/// The form's fields.
#[derive(Clone)]
pub struct Fields {
  pub title: Signal<String>,
  pub brief: Signal<String>,
  pub criteria: Signal<Vec<Criterion>>,
  pub pipeline: Signal<Pick>,
  pub budget: Signal<String>,
  pub time_limit: Signal<String>,
}

impl Fields {
  fn new(ctx: &mut Ctx) -> Self {
    Self {
      title: ctx.signal(String::new()),
      brief: ctx.signal(String::new()),
      criteria: ctx.signal(Vec::new()),
      pipeline: ctx.signal(Pick(None)),
      budget: ctx.signal(String::new()),
      time_limit: ctx.signal(String::new()),
    }
  }

  /// The fields' values; reading them subscribes the render to every field.
  fn values(&self) -> Values {
    Values {
      title: self.title.get(),
      brief: self.brief.get(),
      criteria: texts(&self.criteria.get()),
      pipeline: self.pipeline.get().0,
      budget: self.budget.get(),
      time_limit: self.time_limit.get(),
    }
  }

  fn values_untracked(&self) -> Values {
    Values {
      title: self.title.get_untracked(),
      brief: self.brief.get_untracked(),
      criteria: self
        .criteria
        .get_untracked()
        .iter()
        .map(|row| row.text.get_untracked())
        .collect(),
      pipeline: self.pipeline.get_untracked().0,
      budget: self.budget.get_untracked(),
      time_limit: self.time_limit.get_untracked(),
    }
  }

  pub fn fill(&self, values: &Values) {
    self.title.set(values.title.clone());
    self.brief.set(values.brief.clone());
    self.criteria.set(Criterion::rows(&values.criteria));
    self.pipeline.set(Pick(values.pipeline));
    self.budget.set(values.budget.clone());
    self.time_limit.set(values.time_limit.clone());
  }
}

pub struct TaskForm {
  pub(super) fields: Fields,
  /// What is stored, as the fields were last filled from it.
  pub(super) baseline: Signal<Option<Values>>,
  pub(super) kept_at: Signal<Option<KeptAt>>,
  seeded: Signal<bool>,
  pub(super) confirming_discard: Signal<bool>,
  pub(super) retrying: Signal<bool>,
  pub(super) queued: Signal<Option<QueuedTask>>,
  /// The task the form's last save stored, until the project's tasks are read again.
  pub(super) saved: Signal<Option<Saved>>,
  /// Generated once per form: every save of a new task names the same task.
  pub(super) batch: IdempotencyKey,
  pub(super) nav: Nav,
  pub(super) kept: KeptEdits,
  /// What this form last wrote to the kept edits.
  pub(super) written: Arc<Mutex<Written>>,
}

/// What a form last wrote to its kept edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Written {
  /// The values kept, or that none are.
  #[default]
  Nothing,
  Values(Values),
  /// The form created its task or started its run and leaves: nothing is kept again.
  Finished,
}

impl Component for TaskForm {
  type Props = FormProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<FormProps>().clone();
    let services = Services::of(ctx);
    let project = props.project.open.project.id();
    let task = props.subject.task().map(|task| task.id());
    let form = Self {
      fields: Fields::new(ctx),
      baseline: ctx.signal(None),
      kept_at: ctx.signal(None),
      seeded: ctx.signal(false),
      confirming_discard: ctx.signal(false),
      retrying: ctx.signal(false),
      queued: ctx.signal(None),
      saved: ctx.signal(None),
      batch: IdempotencyKey::generate(),
      nav: Nav::new(ctx, project),
      kept: KeptEdits::new(services.workspace.clone(), services.runtime, project, task),
      written: Arc::default(),
    };
    let (save, run) = form.actions(ctx, &props);
    form.follow_save(ctx, &save, &props);
    form.follow_run(ctx, &run, &props);
    form
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FormProps>().clone();
    let (save, run) = self.actions(ctx, &props);
    let separator = ctx.t("task_form.number.groupSeparator");
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
    self.follow_stored(&props, &separator);
    let view = self.view(&props, (&save, &run), &separator);
    self.keep(&view);
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
  /// The save and run actions, in the same order every time: they are the component's first futures.
  fn actions(&self, ctx: &mut Ctx, props: &FormProps) -> (SaveAction, RunAction) {
    let services = Services::of(ctx);
    let save = save_action(ctx, services.servers.clone());
    let run = run_action(
      ctx,
      RunServices {
        runners: services.runners,
        servers: services.servers,
        repo_root: props.project.open.repo_root.clone(),
        queued: self.queued.clone(),
      },
    );
    (save, run)
  }

  /// The page's sections, in the design's order; Organization is not built yet (D4).
  fn form(&self, ctx: &mut Ctx, props: &FormProps, view: &View, separator: &str) -> Column {
    let brief_fields = BriefFields {
      title: &self.fields.title,
      brief: &self.fields.brief,
      criteria: &self.fields.criteria,
    };
    let limit_fields = LimitFields {
      budget: &self.fields.budget,
      time_limit: &self.fields.time_limit,
    };
    Column::new()
      .id("task-form")
      .width(Dimension::Px(FORM_WIDTH))
      .spacing(SpacingSize::Section)
      .child(head(ctx, view, &self.nav))
      .with_children(no_run_notice(ctx, view))
      .child(brief(ctx, view, &props.project.open, &brief_fields))
      .child(context_section(ctx))
      .child(pipeline_section(ctx, view, &self.fields.pipeline, &self.nav))
      .child(limits_section(ctx, view, &limit_fields, separator))
      .child(instructions_section(ctx))
  }

  /// The task as last stored: what the form's last save stored, when newer than what was last read.
  /// New task has none.
  pub(super) fn stored_task(&self, props: &FormProps) -> Option<Arc<Task>> {
    let task = props.subject.task()?;
    let saved = self
      .saved
      .get()
      .map(|saved| saved.0)
      .filter(|saved| saved.id() == task.id() && saved.revision().get() > task.revision().get());
    Some(saved.unwrap_or_else(|| task.clone()))
  }

  fn stored(&self, props: &FormProps, separator: &str) -> Values {
    self
      .stored_task(props)
      .map(|task| Values::of_details(task.details(), separator))
      .unwrap_or_default()
  }

  /// Fills the fields once the kept edits are read: from them when they differ from what is stored,
  /// else from what is stored. Whether the fields are filled.
  fn seed(&self, ctx: &mut Ctx, props: &FormProps, separator: &str) -> bool {
    let kept = self.kept.clone();
    let read = ctx.future((), move |()| {
      let kept = kept.clone();
      async move { background::run(move || KeptRead(kept.read())).await }
    });
    if self.seeded.get_untracked() {
      return true;
    }
    let state = read.state().get();
    let kept = match (state.data, state.error) {
      (Some(KeptRead(kept)), _) => kept,
      // Background work that did not finish loses the kept edits; the form opens on what is stored.
      (None, Some(_)) => None,
      (None, None) => return false,
    };
    let stored = self.stored(props, separator);
    self.baseline.set(Some(stored.clone()));
    match props.subject.task().filter(|_| props.subject.awaits_first_run()) {
      Some(task) => {
        self.queued.set(Some(QueuedTask {
          task: task.id(),
          key: task.key().clone(),
          outcome: StartOutcome::ConfirmedNotStarted,
        }));
        self.fields.fill(&stored);
      }
      None => match kept.filter(|kept| kept.values.differ_from(&stored, separator)) {
        Some(kept) => {
          self.fields.fill(&kept.values);
          self.kept_at.set(Some(KeptAt(kept.at)));
          *self.written.lock() = Written::Values(kept.values);
        }
        None => self.fields.fill(&stored),
      },
    }
    self.seeded.set(true);
    true
  }

  /// When what is stored changed (a save, or a change made elsewhere) while the fields still hold what
  /// was stored before, the fields show the stored task again.
  fn follow_stored(&self, props: &FormProps, separator: &str) {
    let stored = self.stored(props, separator);
    let baseline = self.baseline.get_untracked();
    if baseline.as_ref() == Some(&stored) {
      return;
    }
    let unchanged = baseline.is_none_or(|baseline| !self.fields.values_untracked().differ_from(&baseline, separator));
    if unchanged {
      self.fields.fill(&stored);
    }
    self.baseline.set(Some(stored));
  }

  /// Keeps the fields' values on this computer while they differ from what is stored, and clears them
  /// once they do not.
  fn keep(&self, view: &View) {
    if view.queued() {
      return;
    }
    let wanted = if view.changed {
      Written::Values(view.values.clone())
    } else {
      Written::Nothing
    };
    let mut written = self.written.lock();
    if *written == Written::Finished || *written == wanted {
      return;
    }
    let values = match &wanted {
      Written::Values(values) => Some((values, Timestamp::now())),
      Written::Nothing | Written::Finished => None,
    };
    self.kept.write(values);
    *written = wanted;
  }

  /// Clears the kept edits once the form created its task or queued it: they are stored now.
  pub(super) fn forget_kept(kept: &KeptEdits, written: &Mutex<Written>) {
    kept.write(None);
    *written.lock() = Written::Finished;
  }

  fn view(&self, props: &FormProps, actions: (&SaveAction, &RunAction), separator: &str) -> View {
    let values = self.fields.values();
    let form = values.task_form(separator);
    let (ready, errors) = match form.details() {
      Ok(details) => (Some(details), Default::default()),
      Err(errors) => (None, errors),
    };
    let missing = form.missing();
    let baseline = self.baseline.get().unwrap_or_default();
    let (phase, failure) = self.phase(actions);
    View {
      subject: props.subject.clone(),
      phase,
      changed: values.differ_from(&baseline, separator),
      values,
      title_error: errors.title,
      budget_error: errors.budget,
      time_error: errors.time_limit,
      ready,
      missing,
      pipelines: props.pipelines.clone(),
      kept_at: self.kept_at.get().map(|at| at.0),
      failure,
      retrying: self.retrying.get(),
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
    let failure = match (&saving.error, &running.error) {
      (_, Some(problems)) if running.is_rejected() && problems.queued.is_none() => Some(problems.messages.clone()),
      (Some(problems), _) if saving.is_rejected() => Some(problems.0.clone()),
      _ => None,
    };
    (Phase::Editing, failure)
  }
}
