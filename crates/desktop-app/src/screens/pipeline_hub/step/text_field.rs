//! A box of an open step that takes text: its *Instructions* (a multi-line area with the placeholder check under it),
//! its *Step budget* and, when changes go back to an earlier step, its *Retries* (`Instructions` `g323q`, `Step
//! budget` `c468d` in P2 `u2rYEH`). What is typed goes into the edit the panel holds as it is typed; text that is not
//! valid (a placeholder that names no variable, a number that is none) is said under the box, the edit keeps the last
//! valid value and the panel does not offer to save until the text is right.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{Ref, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  name::Name,
  resource::{Step, StepKey},
};
use orchester_workspace::{
  messages::{Message, step_name_error},
  pipelines::fields,
};

use super::{Faulty, keep, kept};
use crate::{
  screens::pipeline_hub::edit::Edit,
  theme::DISABLED_OPACITY,
  ui::{field::FieldInput, message::message_text},
};

/// What typed text becomes of a step once it is valid.
type Change = Box<dyn FnOnce(&mut Step)>;

/// Which box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
  /// The step's name, edited in its head ([`super::name`]).
  Name,
  Instructions,
  Budget,
  Retries,
}

impl Kind {
  /// The words that name the box, in the bar's *Fix … to save*.
  pub(super) const fn label(self) -> &'static str {
    match self {
      Self::Name => "pipeline_hub.stepEdit.name",
      Self::Instructions => "pipeline_hub.step.instructions",
      Self::Budget => "pipeline_hub.step.budget",
      Self::Retries => "pipeline_hub.step.retries",
    }
  }

  /// The box's element id.
  pub(super) const fn name(self) -> &'static str {
    match self {
      Self::Name => "pipeline-hub-step-name",
      Self::Instructions => "pipeline-hub-step-instructions",
      Self::Budget => "pipeline-hub-step-budget",
      Self::Retries => "pipeline-hub-step-retries",
    }
  }

  /// What `step` holds, as the box shows it.
  pub(super) fn shown(self, step: &Step) -> String {
    match self {
      Self::Name => step.name.as_str().to_owned(),
      Self::Instructions => step.instructions.as_str().to_owned(),
      Self::Budget => fields::budget_text(step.budget, ","),
      Self::Retries => step
        .on_failure
        .as_ref()
        .map_or_else(String::new, |route| route.max_retries.to_string()),
    }
  }

  /// Why `text` cannot be what `step` holds, if it cannot.
  pub(super) fn refused(self, text: &str) -> Option<Message> {
    self.apply(text).err()
  }

  /// Makes `text` what `step` holds, or says why it cannot be.
  fn apply(self, text: &str) -> Result<Change, Message> {
    Ok(match self {
      Self::Name => {
        let value = Name::new(text).map_err(|error| step_name_error(&error))?;
        Box::new(move |step| step.name = value)
      }
      Self::Instructions => {
        let value = fields::parse_instructions(text)?;
        Box::new(move |step| step.instructions = value)
      }
      Self::Budget => {
        let value = fields::parse_budget(text, ",")?;
        Box::new(move |step| step.budget = value)
      }
      Self::Retries => {
        let value = fields::parse_retries(text)?;
        Box::new(move |step| {
          if let Some(route) = &mut step.on_failure {
            route.max_retries = value;
          }
        })
      }
    })
  }
}

#[derive(Clone)]
pub(super) struct Props {
  pub edit: Edit,
  pub key: StepKey,
  pub kind: Kind,
  /// What the edit holds now, which the box starts with.
  pub shown: String,
  pub label: String,
  pub placeholder: Option<String>,
  pub unit: Option<String>,
  pub locked: bool,
  pub epoch: u64,
  pub faulty: Faulty,
}

/// The props are equal when they say the same: the edit's handle and the panel's list are its own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.key,
      self.kind,
      &self.shown,
      &self.label,
      &self.placeholder,
      &self.unit,
      self.locked,
    ) == (
      &other.key,
      other.kind,
      &other.shown,
      &other.label,
      &other.placeholder,
      &other.unit,
      other.locked,
    )
  }
}
impl DevtoolsInspectable for Props {}

/// Takes `text` typed into the box of `kind` of step `key`: valid, into `edit` (the panel's, as of the last draw), and
/// any invalid text kept for the step is forgotten; not valid, it is kept in `faulty` and the edit keeps the last valid
/// value. Returns why it is not valid.
pub(super) fn take(text: &str, (key, kind): (&StepKey, Kind), faulty: &Faulty, edit: Option<&Edit>) -> Option<Message> {
  match kind.apply(text) {
    Ok(change) => {
      keep(faulty, (key, kind), None);
      if let Some(edit) = edit {
        edit.change(|current| {
          current.update(key, change);
        });
      }
      None
    }
    Err(message) => {
      keep(faulty, (key, kind), Some(text));
      Some(message)
    }
  }
}

/// The box of `props`, mounted anew for each step and after each save or *Cancel*.
pub(super) fn field(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("{}-{}-{}", props.kind.name(), props.key, props.epoch);
  ctx.mount_keyed::<TextField>(&key, props)
}

struct TextField {
  text: Signal<String>,
  /// Why the text is not valid.
  error: Signal<Option<Message>>,
  /// The panel's edit as of the last draw, which a change goes to.
  edit: Ref<Option<Edit>>,
}

impl Component for TextField {
  type Props = Props;

  /// The box starts with what the edit holds, or, when text that was not valid was typed here and the person came
  /// back to it, with that text and what is wrong with it.
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let typed = kept(&props.faulty, &props.key, props.kind);
    let error = typed.as_deref().and_then(|text| props.kind.refused(text));
    let this = Self {
      text: ctx.signal(typed.unwrap_or_else(|| props.shown.clone())),
      error: ctx.signal(error),
      edit: ctx.create_ref(Some(props.edit.clone())),
    };
    let (error, edit) = (this.error.clone(), this.edit.clone());
    let (kind, key, faulty) = (props.kind, props.key.clone(), props.faulty);
    ctx.watch(&this.text, move |text: &String| {
      error.set(take(text, (&key, kind), &faulty, edit.get().as_ref()));
    });
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.edit.set(Some(props.edit.clone()));
    let mut input = FieldInput::new(props.kind.name(), props.label, self.text.clone())
      .width(Dimension::Pct(100.0))
      .fresh(props.epoch);
    if props.kind == Kind::Instructions {
      input = input.text_area();
    }
    if let Some(placeholder) = props.placeholder {
      input = input.placeholder(placeholder);
    }
    if let Some(unit) = props.unit {
      input = input.unit(unit);
    }
    if props.locked {
      input = input.locked();
    }
    if let Some(message) = self.error.get() {
      input = input.error(message_text(ctx, &message));
    }
    // While a save or its question runs the box takes the kit's disabled look, as the step's other fields (P2f).
    Column::new()
      .width(Dimension::Pct(100.0))
      .opacity(if props.locked { DISABLED_OPACITY } else { 1.0 })
      .child(input.render(ctx))
  }
}
