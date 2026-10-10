//! Text and select inputs for the step inspector. Parsing is shared workspace logic.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, SelectOption},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{Step, StepKind};
use orchester_workspace::pipelines::fields::{self, Executor, ModelChoice, RouteChoice};

use super::{parts, state::State};
use crate::ui::{
  field::{FieldInput, SelectField},
  inspector::property_of,
  message::message_text,
};

#[derive(Clone)]
pub struct Props {
  pub state: State,
  pub step: Step,
  pub locked: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.step == other.step && self.locked == other.locked
  }
}
impl DevtoolsInspectable for Props {}
pub struct Fields {
  name: Signal<String>,
  instructions: Signal<String>,
  retries: Signal<String>,
  budget: Signal<String>,
  executor: Signal<Executor>,
  model: Signal<ModelChoice>,
  route: Signal<RouteChoice>,
}
impl Component for Fields {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>();
    let step = props.step.clone();
    let editing = props.state.get();
    let inputs = if editing.viewing.is_some() {
      Default::default()
    } else {
      editing.inputs
    };
    let text = |id: &str, fallback: String| inputs.get(&format!("{}:{id}", step.key)).cloned().unwrap_or(fallback);
    Self {
      name: ctx.signal(text("step-name", step.name.as_str().to_owned())),
      instructions: ctx.signal(text("step-instructions", step.instructions.as_str().to_owned())),
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
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.watch(ctx, &props);
    let mut body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(Self::text(
        ctx,
        &props,
        "step-name",
        "pipelines.stepName",
        self.name.clone(),
        false,
      ))
      .child(parts::section(ctx, "pipelines.executor"))
      .child(self.executor(ctx, &props));
    if matches!(props.step.kind, StepKind::Agent { .. }) {
      let editing = props.state.get();
      let models = fields::model_choices(
        editing.opened.assignables.profiles.iter().map(|p| &p.model),
        self.model.get().0.as_ref(),
      );
      let mut options = vec![SelectOption::new(ModelChoice(None), ctx.t("pipelines.inherit"))];
      options.extend(
        models
          .into_iter()
          .map(|m| SelectOption::new(ModelChoice(Some(m.clone())), m.as_str().to_owned())),
      );
      let options = locked_options(options, props.locked);
      let select = SelectField::new("step-model", ctx.t("pipelines.model"), self.model.clone(), options).render(ctx);
      body = body.child(property_of("step-model-row", &ctx.t("pipelines.model"), select));
    }
    body = body.child(Self::text(
      ctx,
      &props,
      "step-instructions",
      "pipelines.instructions",
      self.instructions.clone(),
      true,
    ));
    body
      .child(parts::section(ctx, "pipelines.flow"))
      .child(self.flow(ctx, &props))
      .child(Self::text(
        ctx,
        &props,
        "step-budget",
        "pipelines.budget",
        self.budget.clone(),
        false,
      ))
  }
}
impl Fields {
  fn text(ctx: &mut Ctx, props: &Props, id: &str, label: &str, value: Signal<String>, multiline: bool) -> Element {
    let title = ctx.t(label);
    let mut field = FieldInput::new(id, title.clone(), value);
    if multiline {
      field = field.text_area();
    }
    if props.locked {
      field = field.locked();
    }
    if let Some(error) = props.state.get().errors.get(&format!("{}:{id}", props.step.key)) {
      field = field.error(message_text(ctx, error));
    }
    let input = field.render(ctx);
    property_of(&format!("{id}-row"), &title, input).into()
  }
  fn executor(&self, ctx: &Ctx, props: &Props) -> Element {
    let editing = props.state.get();
    let mut options = vec![SelectOption::new(Executor::Unset, ctx.t("pipelines.unassigned"))];
    options.extend(
      editing
        .opened
        .assignables
        .profiles
        .iter()
        .map(|p| SelectOption::new(Executor::Profile(p.id), p.name.clone())),
    );
    options.extend(editing.opened.assignables.tools.iter().map(|t| {
      let option = SelectOption::new(Executor::Tool(t.id), t.name.clone());
      if t.runnable {
        option
      } else {
        option.detail(ctx.t("pipelines.toolUnavailable")).disabled(true)
      }
    }));
    let options = locked_options(options, props.locked);
    property_of(
      "step-executor-row",
      &ctx.t("pipelines.profile"),
      SelectField::new(
        "step-executor",
        ctx.t("pipelines.executor"),
        self.executor.clone(),
        options,
      )
      .render(ctx),
    )
    .into()
  }
  fn flow(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let editing = props.state.get();
    let mut body = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Sm);
    let mut dependencies = Column::new().spacing(SpacingSize::Xs);
    for step in editing.shown().steps().iter().filter(|s| s.key != props.step.key) {
      let (state, key, dependency) = (props.state.clone(), props.step.key.clone(), step.key.clone());
      let toggle = crate::ui::toggle::Toggle::new(
        format!("step-after-{}", step.key),
        crate::ui::toggle::ToggleKind::Checkbox,
        props.step.depends_on.contains(&step.key),
        step.name.as_str(),
      );
      dependencies = dependencies.child(if props.locked {
        toggle
      } else {
        toggle.on_change(move |on| {
          state.update(|s| {
            s.edit.set_dependency(&key, &dependency, on);
          })
        })
      });
    }
    body = body.child(property_of(
      "step-dependencies",
      &ctx.t("pipelines.after"),
      dependencies,
    ));
    let mut options = vec![SelectOption::new(RouteChoice(None), ctx.t("pipelines.failRun"))];
    options.extend(
      editing
        .shown()
        .steps()
        .iter()
        .filter(|s| s.key != props.step.key)
        .map(|s| SelectOption::new(RouteChoice(Some(s.key.clone())), s.name.as_str().to_owned())),
    );
    let options = locked_options(options, props.locked);
    body = body.child(property_of(
      "step-failure-row",
      &ctx.t("pipelines.failure"),
      SelectField::new("step-failure", ctx.t("pipelines.failure"), self.route.clone(), options).render(ctx),
    ));
    if props.step.on_failure.is_some() {
      body = body.child(Self::text(
        ctx,
        props,
        "step-retries",
        "pipelines.retries",
        self.retries.clone(),
        false,
      ));
    }
    body.into()
  }
  fn watch(&self, ctx: &mut Ctx, props: &Props) {
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), props.locked);
    ctx.watch(&self.name, move |text| {
      if !locked {
        state.update(|s| {
          let id = format!("{key}:step-name");
          s.inputs.insert(id.clone(), text.clone());
          s.field(&id, fields::parse_name(text), |s, value| {
            s.change(&key, |step| step.name = value)
          })
        });
      }
    });
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), props.locked);
    ctx.watch(&self.instructions, move |text| {
      if !locked {
        state.update(|s| {
          let id = format!("{key}:step-instructions");
          s.inputs.insert(id.clone(), text.clone());
          s.field(&id, fields::parse_instructions(text), |s, value| {
            s.change(&key, |step| step.instructions = value)
          })
        });
      }
    });
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), props.locked);
    ctx.watch(&self.budget, move |text| {
      if !locked {
        state.update(|s| {
          let id = format!("{key}:step-budget");
          s.inputs.insert(id.clone(), text.clone());
          s.field(&id, fields::parse_budget(text, ","), |s, value| {
            s.change(&key, |step| step.budget = value)
          })
        });
      }
    });
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), props.locked);
    ctx.watch(&self.retries, move |text| {
      if !locked {
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
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), props.locked);
    ctx.watch(&self.executor, move |value| {
      if !locked {
        state.update(|s| s.change(&key, |step| value.apply(step)));
      }
    });
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), props.locked);
    ctx.watch(&self.model, move |value| {
      if !locked {
        state.update(|s| s.change(&key, |step| value.clone().apply(step)));
      }
    });
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), props.locked);
    ctx.watch(&self.route, move |value| {
      if !locked {
        state.update(|s| s.change(&key, |step| value.clone().apply(step)));
      }
    });
  }
}
fn locked_options<T>(options: Vec<SelectOption<T>>, locked: bool) -> Vec<SelectOption<T>> {
  options
    .into_iter()
    .map(|option| if locked { option.disabled(true) } else { option })
    .collect()
}
