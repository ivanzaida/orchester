//! Selected step inspector (`yCO8P`): the step's name, who runs it, its model, instructions,
//! what it runs after, where it returns when changes are requested, retries, budget,
//! repository access and approval, and Duplicate / Remove. Mounted per step: its fields hold
//! the text as typed, and a value that cannot be used stays in the field with the reason under
//! it while the step keeps its last valid value.

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, ScrollVertical},
  core::Signal,
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{RepositoryAccess, Step, StepKey, StepKind};

use super::{EditorHandle, step_executor, step_fields};
use crate::{
  messages::Message,
  pipelines::{
    data::Assignables,
    editor::EditorState,
    fields::{
      AccessChoice, Executor, ModelChoice, RouteChoice, budget_text, parse_budget, parse_instructions, parse_name,
      parse_retries,
    },
  },
  screens::project::ProjectNav,
  ui::unit_input::FocusedInput,
};

/// Field names for [`EditorHandle::set_field_invalid`].
const NAME: &str = "name";
const INSTRUCTIONS: &str = "instructions";
const BUDGET: &str = "budget";
const RETRIES: &str = "retries";

/// Room the inspector's scroll content and actions keep on their sides for the outer focus
/// rings of full-width buttons (`focus-extent`, rounded up to the spacing scale).
const FOCUS_ROOM: SpacingSize = SpacingSize::Sm;

#[derive(Clone, Debug, PartialEq)]
pub struct StepInspectorProps {
  pub step: Step,
  /// Position in the list, from 1.
  pub number: usize,
  /// Every step of the pipeline, for dependencies and failure routes.
  pub steps: Arc<Vec<Step>>,
  pub assignables: Assignables,
}

impl lurq::app::component::DevtoolsInspectable for StepInspectorProps {}

impl StepInspectorProps {
  pub fn new(state: &EditorState, step: Step) -> Self {
    let steps = state.edit().steps();
    Self {
      number: steps
        .iter()
        .position(|candidate| candidate.key == step.key)
        .map_or(0, |index| index + 1),
      step,
      steps: Arc::new(steps.to_vec()),
      assignables: state.assignables().clone(),
    }
  }
}

/// The step the inspector shows, once its fields are filled from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspected(pub Option<StepKey>);

impl lurq::app::component::DevtoolsInspectable for Inspected {}

/// The inspector's fields as the user left them.
#[derive(Clone)]
pub struct Fields {
  pub key: Signal<Inspected>,
  pub name: Signal<String>,
  pub instructions: Signal<String>,
  pub budget: Signal<String>,
  pub retries: Signal<String>,
  pub executor: Signal<Executor>,
  pub model: Signal<ModelChoice>,
  pub route: Signal<RouteChoice>,
  pub access: Signal<AccessChoice>,
  /// Remove asks once more before it removes the step.
  pub confirm_remove: Signal<bool>,
  /// Which of the inspector's unit inputs has the focus.
  pub focused: FocusedInput,
}

pub struct StepInspector {
  fields: Fields,
  handle: EditorHandle,
  nav: ProjectNav,
  /// Where the page is scrolled: kept by the component, so a tab that goes inactive and back
  /// keeps it.
  scroll: ScrollState,
}

impl Component for StepInspector {
  type Props = StepInspectorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let handle = ctx
      .use_context::<EditorHandle>()
      .expect("the editor provides EditorHandle");
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let fields = Fields {
      key: ctx.signal(Inspected(None)),
      name: ctx.signal(String::new()),
      instructions: ctx.signal(String::new()),
      budget: ctx.signal(String::new()),
      retries: ctx.signal(String::new()),
      executor: ctx.signal(Executor::Unset),
      model: ctx.signal(ModelChoice(None)),
      route: ctx.signal(RouteChoice(None)),
      access: ctx.signal(AccessChoice(RepositoryAccess::ReadOnly)),
      confirm_remove: ctx.signal(false),
      focused: ctx.signal(None),
    };
    watch_fields(ctx, &fields, &handle);
    watch_budget(ctx, &fields, &handle);
    Self {
      fields,
      handle,
      nav,
      scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StepInspectorProps>().clone();
    if self.fields.key.get().0.is_none() {
      self.seed(&props.step, &ctx.t("number.groupSeparator"));
    }
    let errors = FieldErrors {
      name: parse_name(&self.fields.name.get()).err(),
      instructions: parse_instructions(&self.fields.instructions.get()).err(),
      budget: parse_budget(&self.fields.budget.get(), &ctx.t("number.groupSeparator")).err(),
      retries: props
        .step
        .on_failure
        .as_ref()
        .and_then(|_| parse_retries(&self.fields.retries.get()).err()),
    };
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(FOCUS_ROOM)
      .spacing(SpacingSize::Md)
      .child(step_fields::header(ctx, &props, &self.fields, errors.name.as_ref()))
      .child(step_executor::executor(ctx, &props, &self.fields))
      .with_children(step_executor::tool(ctx, &props, &self.nav))
      .with_children(step_fields::model(ctx, &props, &self.fields))
      // A tool step's command reads no instructions.
      .with_children(matches!(props.step.kind, StepKind::Agent { .. }).then(|| {
        step_fields::instructions(ctx, &self.fields, errors.instructions.as_ref())
      }))
      .child(step_fields::runs_after(ctx, &props, &self.handle))
      .child(step_fields::route(ctx, &props, &self.fields))
      .child(step_fields::constraints(
        ctx,
        &props,
        &self.fields,
        &errors,
        &self.handle,
      ))
      .child(step_fields::approval(ctx, &props, &self.handle));
    // The scroll viewport starts `FOCUS_ROOM` early on both sides and its content is padded by
    // the same amount, so the fields stay at the designed inset and keep room for focus rings.
    let inset =
      ctx.theme().spacing_value(SpacingSize::Lg).resolve(0.0) - ctx.theme().spacing_value(FOCUS_ROOM).resolve(0.0);
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(Dimension::Px(inset))
      .spacing(SpacingSize::Md)
      .child(
        ScrollVertical::new(content)
          .with_scroll_state(self.scroll.clone())
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      )
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding_horizontal(FOCUS_ROOM)
          .overflow_visible()
          .child(step_fields::actions(ctx, &props, &self.fields, &self.handle)),
      )
  }
}

impl StepInspector {
  /// Fills the fields from the step when the inspector opens; the watches ignore it because
  /// the values are the step's own.
  fn seed(&self, step: &Step, separator: &str) {
    let fields = &self.fields;
    fields.name.set(step.name.as_str().to_owned());
    fields.instructions.set(step.instructions.as_str().to_owned());
    fields.budget.set(budget_text(step.budget, separator));
    fields.retries.set(
      step
        .on_failure
        .as_ref()
        .map_or_else(String::new, |route| route.max_retries.to_string()),
    );
    fields.executor.set(Executor::of(step));
    fields.model.set(ModelChoice::of(step));
    fields.route.set(RouteChoice::of(step));
    fields.access.set(AccessChoice::of(step));
    fields.key.set(Inspected(Some(step.key.clone())));
  }
}

/// Why typed values cannot be used, per field.
pub struct FieldErrors {
  pub name: Option<Message>,
  pub instructions: Option<Message>,
  pub budget: Option<Message>,
  pub retries: Option<Message>,
}

/// Applies each field's value to the step once the inspector is seeded; typed text that cannot
/// be used marks its field invalid instead.
fn watch_fields(ctx: &mut Ctx, fields: &Fields, handle: &EditorHandle) {
  let typed =
    |ctx: &mut Ctx, signal: &Signal<String>, field: &'static str, apply: fn(&str, &mut Step) -> Result<(), Message>| {
      let (key, handle) = (fields.key.clone(), handle.clone());
      ctx.watch(signal, move |text| {
        let Inspected(Some(step)) = key.get() else {
          return;
        };
        let text = text.clone();
        let applied = handle.edit(|edit| {
          let mut result = Ok(());
          edit.update(&step, |target| result = apply(&text, target));
          result
        });
        handle.set_field_invalid(&step, field, matches!(applied, Some(Err(_))));
      });
    };
  typed(ctx, &fields.name, NAME, |text, step| {
    step.name = parse_name(text)?;
    Ok(())
  });
  typed(ctx, &fields.instructions, INSTRUCTIONS, |text, step| {
    step.instructions = parse_instructions(text)?;
    Ok(())
  });
  typed(ctx, &fields.retries, RETRIES, |text, step| {
    if let Some(route) = &mut step.on_failure {
      route.max_retries = parse_retries(text)?;
    }
    Ok(())
  });
  chosen(ctx, &fields.executor, fields, handle, |choice, step| choice.apply(step));
  chosen(ctx, &fields.model, fields, handle, |choice, step| {
    choice.clone().apply(step)
  });
  watch_route(ctx, fields, handle);
  chosen(ctx, &fields.access, fields, handle, |choice, step| choice.apply(step));
}

/// Applies the step budget as typed: whole tokens grouped for the locale, which the watch reads
/// when the text changes.
fn watch_budget(ctx: &mut Ctx, fields: &Fields, handle: &EditorHandle) {
  let (key, handle) = (fields.key.clone(), handle.clone());
  let separator = ctx.t("number.groupSeparator");
  ctx.watch(&fields.budget, move |text| {
    let Inspected(Some(step)) = key.get() else {
      return;
    };
    let budget = parse_budget(text, &separator);
    let applied = handle.edit(|edit| {
      let mut result = Ok(());
      edit.update(&step, |target| match &budget {
        Ok(budget) => target.budget = *budget,
        Err(error) => result = Err(error.clone()),
      });
      result
    });
    handle.set_field_invalid(&step, BUDGET, matches!(applied, Some(Err(_))));
  });
}

/// A new route shows the retries it allows, so the field is not left empty.
fn watch_route(ctx: &mut Ctx, fields: &Fields, handle: &EditorHandle) {
  let (key, handle, retries) = (fields.key.clone(), handle.clone(), fields.retries.clone());
  ctx.watch(&fields.route, move |choice| {
    let Inspected(Some(step)) = key.get() else {
      return;
    };
    let allowed = handle.edit(|edit| {
      let mut allowed = None;
      edit.update(&step, |target| {
        choice.clone().apply(target);
        allowed = target.on_failure.as_ref().map(|route| route.max_retries);
      });
      allowed
    });
    if let Some(Some(allowed)) = allowed
      && parse_retries(&retries.get()).is_err()
    {
      retries.set(allowed.to_string());
    }
  });
}

fn chosen<T: Clone + PartialEq + Send + Sync + lurq::core::SignalValue + 'static>(
  ctx: &mut Ctx,
  signal: &Signal<T>,
  fields: &Fields,
  handle: &EditorHandle,
  apply: fn(&T, &mut Step),
) {
  let (key, handle) = (fields.key.clone(), handle.clone());
  ctx.watch(signal, move |choice| {
    if let Inspected(Some(step)) = key.get() {
      handle.edit(|edit| edit.update(&step, |target| apply(choice, target)));
    }
  });
}
