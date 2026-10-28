//! A step's Instructions page (`02g · step instructions` `PexNQ`): what its agent is told, in one field that says
//! what is wrong with a placeholder under it. What is typed goes into the draft as it is typed.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::{Ref, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::pipelines::fields;

use super::step_fields::Props;
use crate::ui::{field::FieldInput, message::message_text};

/// The field's height (`Instructions field` `z0Q5E`).
const FIELD_HEIGHT: f32 = 300.0;

#[derive(Clone, PartialEq)]
pub struct InstructionsProps(pub Props);
impl DevtoolsInspectable for InstructionsProps {}

pub struct Instructions {
  text: Signal<String>,
  /// Whether the step is locked, as last shown.
  locked: Ref<bool>,
}

impl Component for Instructions {
  type Props = InstructionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let InstructionsProps(props) = ctx.props::<InstructionsProps>().clone();
    let editing = props.state.get();
    let id = format!("{}:step-instructions", props.step.key);
    let text = editing
      .inputs
      .get(&id)
      .filter(|_| editing.viewing.is_none())
      .cloned()
      .unwrap_or_else(|| props.step.instructions.as_str().to_owned());
    let page = Self {
      text: ctx.signal(text),
      locked: ctx.create_ref(props.locked || props.viewing),
    };
    let (state, key, locked) = (props.state.clone(), props.step.key, page.locked.clone());
    ctx.watch(&page.text, move |text| {
      if !locked.get() {
        state.update(|s| {
          s.inputs.insert(id.clone(), text.clone());
          s.field(&id, fields::parse_instructions(text), |s, value| {
            s.change(&key, |step| step.instructions = value)
          })
        });
      }
    });
    page
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let InstructionsProps(props) = ctx.props::<InstructionsProps>().clone();
    let locked = props.locked || props.viewing;
    self.locked.set(locked);
    let mut field = FieldInput::new("step-instructions", ctx.t("pipelines.instructions"), self.text.clone())
      .text_area()
      .mono()
      .area_height(FIELD_HEIGHT)
      .width(Dimension::Pct(100.0));
    if locked {
      field = field.locked();
    }
    if let Some(error) = props
      .state
      .get()
      .errors
      .get(&format!("{}:step-instructions", props.step.key))
    {
      field = field.error(message_text(ctx, error));
    }
    field.render(ctx)
  }
}
