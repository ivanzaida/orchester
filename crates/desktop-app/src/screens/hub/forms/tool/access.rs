//! *Access* of an HTTP or database tool (`Access` in A32 `gLLEv`, L20 `Kb6Vk`): for the team, the pipelines and steps a
//! team run may call it from, a list with *Add* as a profile's lists are ([`given`]); then the environment whose secret
//! values it uses, whether a person approves a call first, how long access lasts (with the minutes, when it lasts
//! minutes) and what a denied call does, each a choice. It becomes the tool's access policy ([`AccessText`]). A local
//! project has no pipelines to choose: a local run's agents get a tool from their profile.

use std::{collections::BTreeMap, fmt, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{AccessPolicy, AllowedSteps, ApprovalMode, DeniedBehavior, StepKey},
  secret::Environment,
};
use orchester_workspace::{
  agents::team_tool::CallingPipeline,
  tools::form::{AccessErrors, AccessText},
};

use super::{
  super::Form,
  choice::{chosen, segments, text_of},
};
use crate::{
  screens::{
    hub::given,
    team_hub::rows::{self, Sub},
  },
  theme::HubText,
  ui::{field::FieldInput, message::message_text},
};

/// The minutes' field beside *Minutes* (`Minutes` in A34 `WUuEj`).
const MINUTES_WIDTH: f32 = 64.0;

const ENVIRONMENTS: [(&str, Environment); 3] = [
  ("development", Environment::Development),
  ("staging", Environment::Staging),
  ("production", Environment::Production),
];
const APPROVALS: [(&str, ApprovalMode); 3] = [
  ("every", ApprovalMode::EveryCall),
  ("once", ApprovalMode::OncePerRun),
  ("never", ApprovalMode::Never),
];
const DENIALS: [(&str, DeniedBehavior); 2] = [
  ("wait", DeniedBehavior::AwaitChangedPlan),
  ("fail", DeniedBehavior::FailStep),
];
const UNTIL_STEP_ENDS: &str = "step";
const MINUTES: &str = "minutes";

/// The words of each choice's options, by value.
const ENVIRONMENT_WORDS: [(&str, &str); 3] = [
  ("development", "team_new.tool.access.environment.development"),
  ("staging", "team_new.tool.access.environment.staging"),
  ("production", "team_new.tool.access.environment.production"),
];
const APPROVAL_WORDS: [(&str, &str); 3] = [
  ("every", "team_new.tool.access.approval.every"),
  ("once", "team_new.tool.access.approval.once"),
  ("never", "team_new.tool.access.approval.never"),
];
const WINDOW_WORDS: [(&str, &str); 2] = [
  (UNTIL_STEP_ENDS, "team_new.tool.access.window.step"),
  (MINUTES, "team_new.tool.access.window.minutes"),
];
const DENIED_WORDS: [(&str, &str); 2] = [
  ("wait", "team_new.tool.access.denied.wait"),
  ("fail", "team_new.tool.access.denied.fail"),
];

/// The pipelines a team run calls the tool from, with their steps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Callers(BTreeMap<ResourceId, AllowedSteps>);
impl DevtoolsInspectable for Callers {}

/// What a team's tool's access chooses from: the team's pipelines, which may call it, and the access an edit starts
/// from, whose callers are not new.
#[derive(Clone, Copy)]
pub(super) struct Team<'a> {
  pub pipelines: &'a [CallingPipeline],
  pub base: Option<&'a AccessPolicy>,
}

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(super) struct AccessFields {
  callers: Signal<Callers>,
  environment: Signal<String>,
  approval: Signal<String>,
  window: Signal<String>,
  minutes: Signal<String>,
  denied: Signal<String>,
}

impl AccessFields {
  /// A new tool's access ([`AccessText::default`]).
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let fields = Self {
      callers: ctx.signal(Callers::default()),
      environment: ctx.signal(String::new()),
      approval: ctx.signal(String::new()),
      window: ctx.signal(String::new()),
      minutes: ctx.signal(String::new()),
      denied: ctx.signal(String::new()),
    };
    fields.load(AccessText::default());
    fields
  }

  /// Every field takes the value `text` has.
  pub(super) fn load(&self, text: AccessText) {
    self.callers.set(Callers(text.callers));
    self.environment.set(
      text
        .environment
        .map_or("", |environment| text_of(environment, &ENVIRONMENTS))
        .to_owned(),
    );
    self.approval.set(text_of(text.approval, &APPROVALS).to_owned());
    self
      .window
      .set(if text.by_minutes { MINUTES } else { UNTIL_STEP_ENDS }.to_owned());
    self.minutes.set(text.minutes);
    self.denied.set(text_of(text.denied, &DENIALS).to_owned());
  }

  /// The access as typed.
  pub(super) fn text(&self) -> AccessText {
    AccessText {
      callers: self.callers.get().0,
      environment: self.environment(),
      approval: chosen(&self.approval, &APPROVALS),
      by_minutes: self.window.get() == MINUTES,
      minutes: self.minutes.get(),
      denied: chosen(&self.denied, &DENIALS),
    }
  }

  /// The environment chosen; `None` while the policy named none of the three.
  pub(super) fn environment(&self) -> Option<Environment> {
    let held = self.environment.get();
    ENVIRONMENTS
      .iter()
      .find(|(text, _)| *text == held)
      .map(|(_, environment)| *environment)
  }

  /// The section, saying `errors`; for the team, with the pipelines that call it from `pipelines`.
  pub(super) fn section<B>(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, B>,
    (errors, team): (&AccessErrors, Option<Team<'_>>),
  ) -> Element {
    let mut list = Vec::new();
    if let Some(Team { pipelines, base }) = team {
      list.push(self.callers_row(ctx, form, (pipelines, base)));
    }
    list.push(self.environment_row(ctx, form, errors));
    list.push(choice_row(
      ctx,
      form,
      (
        "approval",
        "team_new.tool.access.approval.label",
        "team_new.tool.access.approval.sub",
      ),
      (&self.approval, &APPROVAL_WORDS),
    ));
    list.push(self.window_row(ctx, form, errors));
    list.push(choice_row(
      ctx,
      form,
      (
        "denied",
        "team_new.tool.access.denied.label",
        "team_new.tool.access.denied.sub",
      ),
      (&self.denied, &DENIED_WORDS),
    ));
    // A team's new tool says who may call it; its edit, that this is the same on every computer (A32, A33).
    let hint = match (team.is_some(), form.creates()) {
      (true, true) => "team_new.tool.access.teamHint",
      (true, false) => "team_new.tool.access.everyComputer",
      (false, _) => "team_new.tool.access.localHint",
    };
    let id = format!("{}-access", form.ids.sections);
    let hint = Text::new(&ctx.t(hint))
      .id(format!("{id}-hint"))
      .variant(HubText::Hint.role())
      .nowrap();
    rows::section(&id, &ctx.t("team_new.tool.access.title"), vec![hint.into()], list).into()
  }

  fn environment_row<B>(&self, ctx: &Ctx, form: &Form<'_, B>, errors: &AccessErrors) -> Row {
    let value = segments(
      &form.value_id("environment"),
      &self.environment,
      worded(ctx, &ENVIRONMENT_WORDS),
      form.locked,
    );
    let sub = match &errors.environment {
      Some(error) => Sub {
        words: message_text(ctx, error),
        color: PaletteColor::Danger,
      },
      None => Sub::quiet(ctx.t("team_new.tool.access.environmentSub")),
    };
    rows::row(
      &format!("{}-environment", form.ids.rows),
      &ctx.t("team_new.tool.access.environment.label"),
      Some(sub),
      value,
    )
  }

  /// *Access lasts*: until the step ends, or the minutes typed beside *Minutes*.
  fn window_row<B>(&self, ctx: &mut Ctx, form: &Form<'_, B>, errors: &AccessErrors) -> Row {
    let choice = segments(
      &form.value_id("window"),
      &self.window,
      worded(ctx, &WINDOW_WORDS),
      form.locked,
    );
    let mut value = Row::new()
      .id(form.value_id("window-value"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(choice);
    let error = errors.minutes.as_ref().map(|error| message_text(ctx, error));
    if self.window.get() == MINUTES {
      let mut minutes = FieldInput::new(
        format!("{}-minutes-input", form.ids.rows),
        ctx.t("team_new.tool.access.window.minutesField"),
        self.minutes.clone(),
      )
      .width(Dimension::Px(MINUTES_WIDTH));
      if error.is_some() {
        minutes = minutes.invalid();
      }
      if form.locked {
        minutes = minutes.locked();
      }
      value = value.child(minutes.render(ctx));
    }
    let sub = match error {
      Some(words) => Sub {
        words,
        color: PaletteColor::Danger,
      },
      None => Sub::quiet(ctx.t("team_new.tool.access.window.sub")),
    };
    rows::row(
      &format!("{}-window", form.ids.rows),
      &ctx.t("team_new.tool.access.window.label"),
      Some(sub),
      value,
    )
  }

  /// *Pipelines that call it*: each pipeline with all its steps, or the steps chosen, in tags, and *Add*, a menu of
  /// every team pipeline and its steps; `base` is the access an edit starts from, whose tags are not new.
  fn callers_row<B>(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, B>,
    (pipelines, base): (&[CallingPipeline], Option<&AccessPolicy>),
  ) -> Row {
    let options = Arc::new(options(ctx, pipelines));
    let callers = self.callers.get().0;
    let (chosen, offered) = options
      .iter()
      .enumerate()
      .map(|(index, option)| ((Caller(index), option.words.clone()), option.chosen_by(&callers)))
      .partition::<Vec<_>, _>(|(_, chosen)| *chosen);
    let held = self.callers.clone();
    let on_toggle: Arc<dyn Fn(Caller, bool) + Send + Sync> = {
      let options = options.clone();
      Arc::new(move |Caller(index), choose| {
        if let Some(option) = options.get(index) {
          held.update(|Callers(callers)| option.toggle(callers, choose));
        }
      })
    };
    let before = base.map(|base| {
      options
        .iter()
        .enumerate()
        .filter(|(_, option)| option.chosen_by(base.callers()))
        .map(|(index, _)| Caller(index))
        .collect()
    });
    let list = given::List {
      id: format!("{}-callers", form.ids.rows),
      what: "pipeline",
      label: ctx.t("team_new.tool.access.callers"),
      sub: Some(Sub::quiet(ctx.t("team_new.tool.access.callersSub"))),
      chosen: chosen.into_iter().map(|(item, _)| item).collect(),
      before,
      offered: offered.into_iter().map(|(item, _)| item).collect(),
      on_toggle,
      locked: form.locked,
    };
    given::row(ctx, list)
  }
}

/// An entry of *Pipelines that call it*, by its place among the entries the menu lists.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Caller(usize);

impl fmt::Display for Caller {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(f)
  }
}

/// One entry: a pipeline with all its steps (`step` none), or one of its steps; and its words.
struct CallerOption {
  pipeline: ResourceId,
  step: Option<StepKey>,
  words: String,
}

impl CallerOption {
  fn chosen_by(&self, callers: &BTreeMap<ResourceId, AllowedSteps>) -> bool {
    match (callers.get(&self.pipeline), &self.step) {
      (Some(AllowedSteps::All), None) => true,
      (Some(AllowedSteps::Only(steps)), Some(step)) => steps.contains(step),
      _ => false,
    }
  }

  /// `callers` with this entry chosen or not: a pipeline's steps are all of them, or those chosen one by one.
  fn toggle(&self, callers: &mut BTreeMap<ResourceId, AllowedSteps>, choose: bool) {
    match (&self.step, choose) {
      (None, true) => {
        callers.insert(self.pipeline, AllowedSteps::All);
      }
      (Some(step), true) => match callers.get_mut(&self.pipeline) {
        Some(AllowedSteps::Only(steps)) => {
          steps.insert(step.clone());
        }
        _ => {
          callers.insert(self.pipeline, AllowedSteps::Only([step.clone()].into()));
        }
      },
      (None, false) => {
        callers.remove(&self.pipeline);
      }
      (Some(step), false) => {
        if let Some(AllowedSteps::Only(steps)) = callers.get_mut(&self.pipeline) {
          steps.remove(step);
          if steps.is_empty() {
            callers.remove(&self.pipeline);
          }
        }
      }
    }
  }
}

/// Every pipeline of `pipelines`, then each of its steps numbered as the pipeline orders them.
fn options(ctx: &Ctx, pipelines: &[CallingPipeline]) -> Vec<CallerOption> {
  let mut options = Vec::new();
  for pipeline in pipelines {
    options.push(CallerOption {
      pipeline: pipeline.id,
      step: None,
      words: ctx
        .t_args("team_new.tool.access.allSteps", [("pipeline", pipeline.name.as_str())])
        .to_string(),
    });
    for (index, (key, name)) in pipeline.steps.iter().enumerate() {
      let number = format!("{:02}", index + 1);
      options.push(CallerOption {
        pipeline: pipeline.id,
        step: Some(key.clone()),
        words: ctx
          .t_args(
            "team_new.tool.access.step",
            [
              ("pipeline", pipeline.name.as_str()),
              ("number", number.as_str()),
              ("step", name.as_str()),
            ],
          )
          .to_string(),
      });
    }
  }
  options
}

/// The options of a choice, each with its words.
fn worded(ctx: &Ctx, options: &[(&'static str, &'static str)]) -> Vec<(&'static str, Arc<str>)> {
  options.iter().map(|(value, words)| (*value, ctx.t(words))).collect()
}

/// A row of the choice of `part`, labelled `label` over `sub`, between `options`.
fn choice_row<B>(
  ctx: &Ctx,
  form: &Form<'_, B>,
  (part, label, sub): (&str, &str, &str),
  (value, options): (&Signal<String>, &[(&'static str, &'static str)]),
) -> Row {
  let control = segments(&form.value_id(part), value, worded(ctx, options), form.locked);
  form.shown(part, (ctx.t(label), Some(ctx.t(sub))), control)
}

/// The words of `environment`.
pub(super) const fn environment_key(environment: Environment) -> &'static str {
  match environment {
    Environment::Development => ENVIRONMENT_WORDS[0].1,
    Environment::Staging => ENVIRONMENT_WORDS[1].1,
    Environment::Production => ENVIRONMENT_WORDS[2].1,
  }
}
