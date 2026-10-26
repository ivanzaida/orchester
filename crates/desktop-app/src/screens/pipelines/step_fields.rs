//! The step column's Step and Flow groups (`Step` `E3MuZQ`, `Flow` `Ltw9a` in `IsU5u`): who runs the step, its
//! model and its instructions, which open on a page of their own; what it runs after, where requested changes go
//! with the retries once a step is chosen, its budget, and what happens when it fails. A viewed version shows the
//! values as words (`05` `AaaH0`). Parsing is shared workspace logic.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, SelectOption},
  core::{Ref, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{Step, StepKey};
use orchester_workspace::pipelines::fields::{self, Executor, ModelChoice, RouteChoice};

use super::state::{State, StepPage, View};
use crate::ui::{
  field::{FieldInput, SelectField},
  message::message_text,
  pressable::Press,
  row_group::{GroupRow, RowGroup},
};

/// A control's width at a row's end (`Select` `s3R8u`).
pub const FIELD_WIDTH: f32 = 220.0;

#[derive(Clone)]
pub struct Props {
  pub state: State,
  pub step: Step,
  pub locked: bool,
  pub navigation_locked: bool,
  /// A published version is viewed: its values show as words.
  pub viewing: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.step == other.step
      && self.locked == other.locked
      && self.navigation_locked == other.navigation_locked
      && self.viewing == other.viewing
  }
}
impl DevtoolsInspectable for Props {}
pub struct Fields {
  retries: Signal<String>,
  budget: Signal<String>,
  executor: Signal<Executor>,
  model: Signal<ModelChoice>,
  route: Signal<RouteChoice>,
  /// Whether the step is locked, as last shown.
  locked: Ref<bool>,
}
impl Component for Fields {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let step = props.step.clone();
    let editing = props.state.get();
    let inputs = if editing.viewing.is_some() {
      Default::default()
    } else {
      editing.inputs
    };
    let text = |id: &str, fallback: String| inputs.get(&format!("{}:{id}", step.key)).cloned().unwrap_or(fallback);
    let fields = Self {
      retries: ctx.signal(text(
        "step-retries",
        step
          .on_failure
          .as_ref()
          .map(|r| r.max_retries.to_string())
          .unwrap_or_else(|| "1".to_owned()),
      )),
      budget: ctx.signal(text("step-budget", fields::budget_text(step.budget, ","))),
      executor: ctx.signal(Executor::of(&step)),
      model: ctx.signal(ModelChoice::of(&step)),
      route: ctx.signal(RouteChoice::of(&step)),
      locked: ctx.create_ref(props.locked),
    };
    fields.watch(ctx, &props.state, &step.key);
    fields
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.locked.set(props.locked);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Lg)
      .child(self.step_group(ctx, &props))
      .child(self.flow(ctx, &props))
  }
}
impl Fields {
  fn step_group(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let (executor, notices) = super::step_executor::executor(ctx, props, self.executor.clone());
    let mut group = RowGroup::new("step-group", ctx.t("pipelines.stepGroup")).row(executor);
    if let Some(model) = super::step_executor::model(ctx, props, &self.model) {
      group = group.row(model);
    }
    if super::step_executor::unavailable_tool(props).is_none() {
      group = group.row(instructions_row(ctx, props));
    }
    for notice in notices {
      group = group.after(notice);
    }
    group.into()
  }

  fn text(ctx: &mut Ctx, props: &Props, id: &str, label: &str, value: Signal<String>) -> GroupRow {
    let title = ctx.t(label);
    let row = GroupRow::new(format!("{id}-row"), title.clone());
    if props.viewing {
      let shown = value.get();
      let shown = if shown.is_empty() {
        ctx.t("pipelines.noLimit").to_string()
      } else {
        shown
      };
      return row.value(&shown);
    }
    let mut field = FieldInput::new(id, title, value).width(Dimension::Px(FIELD_WIDTH));
    if id == "step-budget" {
      field = field
        .placeholder(ctx.t("pipelines.noLimit"))
        .unit(ctx.t("pipelines.tokens"));
    }
    if props.locked {
      field = field.locked();
    }
    if let Some(error) = props.state.get().errors.get(&format!("{}:{id}", props.step.key)) {
      field = field.error(message_text(ctx, error));
    }
    row.end(field.render(ctx))
  }

  fn flow(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let editing = props.state.get();
    let shown = editing.shown();
    let after = GroupRow::new("step-dependencies", ctx.t("pipelines.after"));
    let after = if props.viewing {
      after.value(&super::dependencies::label(ctx, &shown, &props.step))
    } else {
      after.end(ctx.mount_keyed::<super::dependencies::Dependencies>("step-dependencies-picker", props.clone()))
    };
    let failure = GroupRow::new("step-failure-row", ctx.t("pipelines.failure"));
    let failure = if props.viewing {
      failure.value(&route_label(ctx, &shown, &props.step))
    } else {
      let mut options = vec![SelectOption::new(RouteChoice(None), ctx.t("pipelines.failRun"))];
      options.extend(
        shown
          .steps()
          .iter()
          .enumerate()
          .filter(|(_, s)| s.key != props.step.key)
          .map(|(index, s)| {
            SelectOption::new(RouteChoice(Some(s.key.clone())), format!("{:02} {}", index + 1, s.name))
          }),
      );
      let select = SelectField::new(
        "step-failure",
        ctx.t("pipelines.failure"),
        self.route.clone(),
        locked_options(options, props.locked),
      )
      .width(Dimension::Px(FIELD_WIDTH));
      failure.end(select.render(ctx))
    };
    let mut group = RowGroup::new("step-flow", ctx.t("pipelines.flow"))
      .row(after)
      .row(failure);
    if props.step.on_failure.is_some() {
      group = group.row(Self::text(
        ctx,
        props,
        "step-retries",
        "pipelines.retries",
        self.retries.clone(),
      ));
    }
    group
      .row(Self::text(
        ctx,
        props,
        "step-budget",
        "pipelines.budget",
        self.budget.clone(),
      ))
      .row(GroupRow::new("step-stopped", ctx.t("pipelines.stopped")).value(&ctx.t("pipelines.stoppedSummary")))
      .into()
  }

  /// Takes what is typed and chosen into the draft, once per step's fields, unless they are locked now. The
  /// step's key is part of the column's mount key, so it holds for the fields' life.
  fn watch(&self, ctx: &mut Ctx, draft: &State, step_key: &StepKey) {
    let (state, key, locked) = (draft.clone(), step_key.clone(), self.locked.clone());
    ctx.watch(&self.budget, move |text| {
      if !locked.get() {
        state.update(|s| {
          let id = format!("{key}:step-budget");
          s.inputs.insert(id.clone(), text.clone());
          s.field(&id, fields::parse_budget(text, ","), |s, value| {
            s.change(&key, |step| step.budget = value)
          })
        });
      }
    });
    let (state, key, locked) = (draft.clone(), step_key.clone(), self.locked.clone());
    ctx.watch(&self.retries, move |text| {
      if !locked.get() {
        state.update(|s| {
          let id = format!("{key}:step-retries");
          s.inputs.insert(id.clone(), text.clone());
          s.field(&id, fields::parse_retries(text), |s, value| {
            s.change(&key, |step| {
              if let Some(route) = &mut step.on_failure {
                route.max_retries = value;
              }
            })
          })
        });
      }
    });
    self.watch_choices(ctx, draft, step_key);
  }
  fn watch_choices(&self, ctx: &mut Ctx, draft: &State, step_key: &StepKey) {
    let (state, key, locked) = (draft.clone(), step_key.clone(), self.locked.clone());
    let model = self.model.clone();
    ctx.watch(&self.executor, move |value| {
      if !locked.get() {
        let mut selected = None;
        state.update(|s| {
          s.change(&key, |step| {
            value.apply(step);
            selected = Some(ModelChoice::of(step));
          });
          s.errors.remove(&format!("{key}:step-permissions"));
        });
        // Signal watchers can run immediately and write to state themselves.
        if let Some(selected) = selected {
          model.set(selected);
        }
      }
    });
    let (state, key, locked) = (draft.clone(), step_key.clone(), self.locked.clone());
    ctx.watch(&self.model, move |value| {
      if !locked.get() {
        state.update(|s| s.change(&key, |step| value.clone().apply(step)));
      }
    });
    self.watch_route(ctx, draft, step_key);
  }
  fn watch_route(&self, ctx: &mut Ctx, draft: &State, step_key: &StepKey) {
    let (state, key, locked) = (draft.clone(), step_key.clone(), self.locked.clone());
    let retries = self.retries.clone();
    ctx.watch(&self.route, move |value| {
      if !locked.get() {
        let mut restored = None;
        state.update(|s| {
          s.change(&key, |step| {
            let had_route = step.on_failure.is_some();
            value.clone().apply(step);
            if !had_route {
              restored = step.on_failure.as_ref().map(|route| route.max_retries.to_string());
            }
          });
          let id = format!("{key}:step-retries");
          if value.0.is_none() {
            s.inputs.remove(&id);
            s.errors.remove(&id);
          } else if let Some(text) = &restored {
            // A newly enabled route uses the domain's retry count, including after remounting.
            s.inputs.insert(id.clone(), text.clone());
            s.errors.remove(&id);
          }
        });
        if let Some(text) = restored {
          retries.set(text);
        }
      }
    });
  }
}

/// The Instructions row: their first line, and a press opens their page (`rXnGJ`).
fn instructions_row(ctx: &Ctx, props: &Props) -> GroupRow {
  let editing = props.state.get();
  let typed = editing
    .inputs
    .get(&format!("{}:step-instructions", props.step.key))
    .filter(|_| editing.viewing.is_none())
    .cloned()
    .unwrap_or_else(|| props.step.instructions.as_str().to_owned());
  let first = typed.lines().find(|line| !line.trim().is_empty()).map(str::to_owned);
  let row = GroupRow::new("step-instructions-row", ctx.t("pipelines.instructions"));
  let row = match first {
    Some(line) => row.hint(line),
    None => row.hint(ctx.t("pipelines.noInstructions")),
  };
  let (views, key) = (editing.views, props.step.key.clone());
  row.opens_page(Press::run(move || {
    views.choose(View::StepPage(key.clone(), StepPage::Instructions));
  }))
}

/// Where requested changes go, as words: *Fail the run* or the step's number and name.
fn route_label(ctx: &Ctx, shown: &orchester_core::resource::PipelineEdit, step: &Step) -> String {
  step
    .on_failure
    .as_ref()
    .and_then(|route| {
      shown
        .steps()
        .iter()
        .enumerate()
        .find(|(_, s)| s.key == route.return_to)
        .map(|(index, s)| format!("{:02} {}", index + 1, s.name))
    })
    .unwrap_or_else(|| ctx.t("pipelines.failRun").to_string())
}

pub(super) fn locked_options<T>(options: Vec<SelectOption<T>>, locked: bool) -> Vec<SelectOption<T>> {
  options
    .into_iter()
    .map(|option| if locked { option.disabled(true) } else { option })
    .collect()
}
