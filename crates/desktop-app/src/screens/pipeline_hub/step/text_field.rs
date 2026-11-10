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
  core::{Ref, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{Step, StepKey};
use orchester_workspace::{messages::Message, pipelines::fields};

use super::{Faulty, mark};
use crate::{
  screens::pipeline_hub::edit::Edit,
  ui::{field::FieldInput, message::message_text},
};

/// What typed text becomes of a step once it is valid.
type Change = Box<dyn FnOnce(&mut Step)>;

/// Which box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
  Instructions,
  Budget,
  Retries,
}

impl Kind {
  /// The field's name in [`Faulty`] and the box's element id.
  const fn name(self) -> &'static str {
    match self {
      Self::Instructions => "pipeline-hub-step-instructions",
      Self::Budget => "pipeline-hub-step-budget",
      Self::Retries => "pipeline-hub-step-retries",
    }
  }

  /// What `step` holds, as the box shows it.
  pub(super) fn shown(self, step: &Step) -> String {
    match self {
      Self::Instructions => step.instructions.as_str().to_owned(),
      Self::Budget => fields::budget_text(step.budget, ","),
      Self::Retries => step
        .on_failure
        .as_ref()
        .map_or_else(String::new, |route| route.max_retries.to_string()),
    }
  }

  /// Makes `text` what `step` holds, or says why it cannot be.
  fn apply(self, text: &str) -> Result<Change, Message> {
    Ok(match self {
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
  kind: Kind,
  faulty: Faulty,
}

impl Component for TextField {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let this = Self {
      text: ctx.signal(props.shown.clone()),
      error: ctx.signal(None),
      edit: ctx.create_ref(Some(props.edit.clone())),
      kind: props.kind,
      faulty: props.faulty.clone(),
    };
    let (error, edit) = (this.error.clone(), this.edit.clone());
    let (kind, key, faulty) = (props.kind, props.key.clone(), props.faulty.clone());
    ctx.watch(&this.text, move |text: &String| match kind.apply(text) {
      Ok(change) => {
        error.set(None);
        mark(&faulty, kind.name(), false);
        if let Some(edit) = edit.get() {
          edit.change(|current| {
            current.update(&key, change);
          });
        }
      }
      Err(message) => {
        error.set(Some(message));
        mark(&faulty, kind.name(), true);
      }
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
    input.render(ctx)
  }
}

impl Drop for TextField {
  fn drop(&mut self) {
    // The box is gone with its error, which must not keep the panel from saving.
    mark(&self.faulty, self.kind.name(), false);
  }
}
