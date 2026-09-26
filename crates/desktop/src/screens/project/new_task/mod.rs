//! New task (02.09 `l2YA4`): brief and acceptance criteria on the left, pipeline and limits
//! on the right. The task is saved as a draft, incomplete if need be, under a key generated
//! when the form opens: a failed save keeps the form, and retrying cannot create a second task.
//! Create & run checks that the run can start, saves and queues the task and asks the runner to
//! start its run under a second key generated when the form opens, so a retry after a failure
//! never starts a second run; it then opens the task page.

mod brief;
mod plan;

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
  repository::RepositoryLink,
  store::RepositoryStore,
  task::{MissingDetail, TaskDetails},
};
use orchester_store::ProjectDb;

use self::brief::Criterion;
use super::{ProjectNav, Section};
use crate::{
  activity::{Activity, operations},
  background, messages,
  messages::Message,
  runner::Runners,
  tasks::{
    data::{PipelineChoice, save_draft_in},
    form::{FormErrors, TaskForm},
    list::missing_key,
    run::{queue_in, start_messages},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    form::{Submit, form},
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
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
}

impl lurq::app::component::DevtoolsInspectable for NewTaskProps {}

/// The chosen pipeline, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineSelection(pub Option<ResourceId>);

impl lurq::app::component::DevtoolsInspectable for PipelineSelection {}

/// Key of the saved task, to confirm the save happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTask(Arc<str>);

impl lurq::app::component::DevtoolsInspectable for SavedTask {}

type SaveArgs = (PathBuf, IdempotencyKey, TaskDetails);
type SaveAction = FutureAction<SaveArgs, SavedTask, Message>;

/// The task whose run started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartedTask(TaskId);

impl lurq::app::component::DevtoolsInspectable for StartedTask {}

/// Why Create & run did not start the run, one message per reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunProblems(pub(super) Arc<[Message]>);

impl lurq::app::component::DevtoolsInspectable for RunProblems {}

/// The form's two keys: the task batch and the run.
type RunArgs = (PathBuf, IdempotencyKey, IdempotencyKey, TaskDetails);
type RunAction = FutureAction<RunArgs, StartedTask, RunProblems>;

/// Everything a part of the form needs to know about the current values.
struct FormState {
  errors: FormErrors,
  missing: Vec<MissingDetail>,
  ready: Option<TaskDetails>,
  saving: bool,
  save_error: Option<Message>,
  /// Create & run is under way.
  starting: bool,
  run_problems: Option<RunProblems>,
}

impl FormState {
  fn is_missing(&self, detail: MissingDetail) -> bool {
    self.missing.contains(&detail)
  }
}

pub struct NewTaskForm {
  title: Signal<String>,
  brief: Signal<String>,
  criteria: Signal<Vec<Criterion>>,
  pipeline: Signal<PipelineSelection>,
  budget: Signal<String>,
  time_limit: Signal<String>,
  /// Generated once per form: every save attempt of this form uses it.
  batch: IdempotencyKey,
  /// Generated once per form: every Create & run attempt of this form starts this run.
  run: IdempotencyKey,
  nav: ProjectNav,
}

impl Component for NewTaskForm {
  type Props = NewTaskProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let save = save_action(ctx);
    let saved = nav.clone();
    ctx.watch(&save.state(), move |state| {
      if state.data.is_some() {
        saved.tasks_changed();
        saved.show(Section::Tasks);
      }
    });
    let run = run_action(ctx);
    let started = nav.clone();
    ctx.watch(&run.state(), move |state| {
      if state.is_pending() {
        return;
      }
      // The task may have been saved and queued even when the run did not start.
      started.tasks_changed();
      if let Some(StartedTask(task)) = state.data {
        started.show(Section::Task(task));
      }
    });
    Self {
      title: ctx.signal(String::new()),
      brief: ctx.signal(String::new()),
      criteria: ctx.signal(vec![Criterion::new(0)]),
      pipeline: ctx.signal(PipelineSelection(None)),
      budget: ctx.signal(String::new()),
      time_limit: ctx.signal(String::new()),
      batch: IdempotencyKey::generate(),
      run: IdempotencyKey::generate(),
      nav,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<NewTaskProps>().clone();
    let save = save_action(ctx);
    let run = run_action(ctx);
    let link = repository_link(ctx, &props.project);
    let state = self.state(&save, &run);
    let submit = self
      .save_handler(&save, &props.project, state.ready.clone())
      .filter(|_| !state.saving && !state.starting);
    let start = self
      .run_handler(&run, &props.project, state.ready.clone())
      .filter(|_| !state.saving && !state.starting && state.missing.is_empty());
    // The brief's scroll viewport starts `FOCUS_ROOM` early and its content is padded by the
    // same amount, so the content stays at the designed inset and keeps room for focus rings.
    let brief_left = ctx.theme().spacing_value(ExtraSpacing::Indent.role()).resolve(0.0)
      - ctx.theme().spacing_value(FOCUS_ROOM).resolve(0.0);
    let content = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding_top(SpacingSize::Sm)
      .padding_left(Dimension::Px(brief_left))
      .padding_right(ExtraSpacing::Indent.role())
      .padding_bottom(SpacingSize::Section)
      .spacing(ExtraSpacing::Indent.role())
      .child(brief::brief_form(ctx, self, &props.project, link.as_deref(), &state))
      .child(plan::execution_plan(
        ctx,
        self,
        props.pipelines.as_deref(),
        &state,
        submit.clone(),
        start,
      ));
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx, props.project.project.name().as_str(), &self.nav, &state))
      .child(heading(ctx))
      .child(form(submit, content))
  }
}

impl NewTaskForm {
  fn state(&self, save: &SaveAction, run: &RunAction) -> FormState {
    let criteria: Vec<String> = self
      .criteria
      .get()
      .iter()
      .map(|criterion| criterion.text.get())
      .collect();
    let (title, brief, budget, time_limit) = (
      self.title.get(),
      self.brief.get(),
      self.budget.get(),
      self.time_limit.get(),
    );
    let form = TaskForm {
      title: &title,
      brief: &brief,
      criteria: &criteria,
      pipeline: self.pipeline.get().0,
      budget: &budget,
      time_limit: &time_limit,
    };
    let saving = save.state().get();
    let (ready, errors) = match form.details() {
      Ok(details) => (Some(details), FormErrors::default()),
      Err(errors) => (None, errors),
    };
    let starting = run.state().get();
    FormState {
      errors,
      missing: form.missing(),
      ready,
      saving: saving.is_pending(),
      save_error: saving.error.clone().filter(|_| !saving.is_pending()),
      starting: starting.is_pending(),
      run_problems: starting.error.clone().filter(|_| !starting.is_pending()),
    }
  }

  /// Saves the valid form under the form's key; `None` while it cannot be saved.
  fn save_handler(&self, save: &SaveAction, project: &OpenProject, ready: Option<TaskDetails>) -> Submit {
    let details = ready?;
    let save = save.clone();
    let repo_root = project.repo_root.clone();
    let batch = self.batch;
    Some(Arc::new(move || {
      save.run((repo_root.clone(), batch, details.clone()));
    }))
  }
}

impl NewTaskForm {
  /// Queues the complete form and starts its run under the form's keys; `None` while the form
  /// is not valid.
  fn run_handler(&self, run: &RunAction, project: &OpenProject, ready: Option<TaskDetails>) -> Submit {
    let details = ready?;
    let run = run.clone();
    let repo_root = project.repo_root.clone();
    let (batch, key) = (self.batch, self.run);
    Some(Arc::new(move || {
      run.run((repo_root.clone(), batch, key, details.clone()));
    }))
  }
}

/// The project's repository link, once read. Without it the form shows the local folder, which
/// is the repository either way, so a failed read is logged and the folder shown.
fn repository_link(ctx: &mut Ctx, project: &OpenProject) -> Option<Arc<RepositoryLink>> {
  /// The link as the future holds it.
  #[derive(Clone, PartialEq)]
  struct Linked(Option<Arc<RepositoryLink>>);
  impl lurq::app::component::DevtoolsInspectable for Linked {}

  let load = ctx.future(project.repo_root.clone(), |repo_root| async move {
    background::run(move || ProjectDb::open(&repo_root).and_then(|db| db.repository_link()))
      .await?
      .map(|link| Linked(link.map(Arc::new)))
      .map_err(|error| {
        tracing::error!(%error, "cannot read the project's repository link");
        messages::store_error(&error)
      })
  });
  load.state().get().data.and_then(|linked| linked.0)
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let open = ctx.props::<NewTaskProps>().project.clone();
  ctx.future_action(move |(repo_root, batch, details): SaveArgs| {
    let (activity, spec) = (activity.clone(), operations::task_draft(&open, details.title.as_str()));
    async move {
      let save = async move {
        let saved = background::run(move || save_draft_in(&repo_root, batch, details)).await?;
        saved
          .map(|task| SavedTask(task.key().to_string().into()))
          .map_err(|error| {
            tracing::error!(%error, "cannot save the task draft");
            messages::store_error(&error)
          })
      };
      activity.track(spec, save).await
    }
  })
}

fn run_action(ctx: &mut Ctx) -> RunAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |(repo_root, batch, key, details): RunArgs| {
    let runners = runners.clone();
    async move {
      let problems = |messages: Vec<Message>| RunProblems(messages.into());
      let queued = background::run(move || queue_in(&repo_root, batch, details))
        .await
        .map_err(|error| problems(vec![error]))?;
      let task = queued.map_err(|failure| {
        tracing::warn!(?failure, "the task was not queued for a run");
        problems(failure.messages())
      })?;
      runners.start_run(key, task.id()).await.map_err(|error| {
        tracing::warn!(%error, "the run did not start");
        problems(start_messages(&error))
      })?;
      Ok(StartedTask(task.id()))
    }
  })
}

/// Back to the task list, the breadcrumb and the save state (`dWjfr`). Nothing is saved until
/// Save as draft or Create & run, so the state says so rather than claiming a saved draft.
fn topbar(ctx: &Ctx, project: &str, nav: &ProjectNav, form: &FormState) -> Element {
  let nav = nav.clone();
  let back = pressable(surface().id("new-task-back"), move || nav.show(Section::Tasks))
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .child(icon(Icon::ArrowLeft, IconSize::Option, PaletteColor::TextSecondary));
  let state = if form.starting {
    ctx.t("tasks.new.starting")
  } else if form.saving {
    ctx.t("tasks.new.saving")
  } else {
    ctx.t("tasks.new.notSaved")
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(back)
    .child(Text::new(&ctx.t_args("tasks.new.breadcrumb", [("project", project)])).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&state).variant(TypographyStyle::Caption).id("new-task-state"))
    .into()
}

/// Heading (`hkWbp`).
fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Section)
    .padding_horizontal(ExtraSpacing::Indent.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("tasks.new.title")).variant(ExtraTypography::ScreenTitle.role()))
    .child(
      Text::new(&ctx.t("tasks.new.description"))
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

/// Why a value or the save was not accepted; a long reason wraps within its column.
fn error_note(text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("role", "field-error")
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
    .into()
}
