//! New task (02.09 `l2YA4`): brief and acceptance criteria on the left, pipeline and limits
//! on the right. The task is saved as a draft, incomplete if need be, under a key generated
//! when the form opens: a failed save keeps the form, and retrying cannot create a second task.
//! Running a task does not exist yet, so Create & run is unavailable.

mod brief;
mod plan;

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
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  task::{MissingDetail, TaskDetails},
};

use self::brief::Criterion;
use super::{ProjectNav, Section};
use crate::{
  activity::{Activity, operations},
  background, messages,
  messages::Message,
  tasks::{
    data::{PipelineChoice, save_draft_in},
    form::{FormErrors, TaskForm},
    list::missing_key,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
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

/// Everything a part of the form needs to know about the current values.
struct FormState {
  errors: FormErrors,
  missing: Vec<MissingDetail>,
  ready: Option<TaskDetails>,
  saving: bool,
  save_error: Option<Message>,
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
    Self {
      title: ctx.signal(String::new()),
      brief: ctx.signal(String::new()),
      criteria: ctx.signal(vec![Criterion::new(0)]),
      pipeline: ctx.signal(PipelineSelection(None)),
      budget: ctx.signal(String::new()),
      time_limit: ctx.signal(String::new()),
      batch: IdempotencyKey::generate(),
      nav,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<NewTaskProps>().clone();
    let save = save_action(ctx);
    let state = self.state(&save);
    let run_save = self.save_handler(&save, &props.project, state.ready.clone());
    let content = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding_top(SpacingSize::Sm)
      .padding_horizontal(ExtraSpacing::Indent.role())
      .padding_bottom(SpacingSize::Section)
      .spacing(ExtraSpacing::Indent.role())
      .child(brief::brief_form(ctx, self, &props.project, &state))
      .child(plan::execution_plan(
        ctx,
        self,
        props.pipelines.as_deref(),
        &state,
        run_save,
      ));
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(topbar(
        ctx,
        props.project.project.name().as_str(),
        &self.nav,
        state.saving,
      ))
      .child(heading(ctx))
      .child(content)
  }
}

impl NewTaskForm {
  fn state(&self, save: &SaveAction) -> FormState {
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
    FormState {
      errors,
      missing: form.missing(),
      ready,
      saving: saving.is_pending(),
      save_error: saving.error.clone().filter(|_| !saving.is_pending()),
    }
  }

  /// Saves the valid form under the form's key; `None` while it cannot be saved.
  fn save_handler(
    &self,
    save: &SaveAction,
    project: &OpenProject,
    ready: Option<TaskDetails>,
  ) -> Option<Box<dyn Fn() + Send + Sync>> {
    let details = ready?;
    let save = save.clone();
    let repo_root = project.repo_root.clone();
    let batch = self.batch;
    Some(Box::new(move || {
      save.run((repo_root.clone(), batch, details.clone()));
    }))
  }
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

/// Back to the task list, the breadcrumb and the save state (`dWjfr`). Nothing is saved
/// until Save as draft, so the state says so rather than claiming a saved draft.
fn topbar(ctx: &Ctx, project: &str, nav: &ProjectNav, saving: bool) -> Element {
  let nav = nav.clone();
  let back = Row::new()
    .id("new-task-back")
    .cursor(CursorIcon::Pointer)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |_| nav.show(Section::Tasks))
    .child(icon(Icon::ArrowLeft, IconSize::Option, PaletteColor::TextSecondary));
  let state = if saving {
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

/// Why a value or the save was not accepted.
fn error_note(text: &str) -> Element {
  Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("role", "field-error")
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}
