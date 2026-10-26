//! K0WLeG: command narrowing stays compact until its selector is opened.
use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, ScrollVertical},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::StepKind;
use orchester_workspace::pipelines::narrowing;

use super::{step_access::command_summary, step_fields::Props, step_permissions::command_rows};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  popover::{PopoverSide, popover, popover_panel},
  tooltip::Tooltip,
};

pub struct Commands {
  open: Signal<bool>,
  anchor: ElementRef,
}
impl Component for Commands {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let editing = props.state.get();
    let StepKind::Agent {
      profile: Some(profile), ..
    } = props.step.kind
    else {
      return Column::new();
    };
    let Some(choice) = editing.opened.assignables.profile(profile) else {
      return Column::new();
    };
    let open = self.open.clone();
    let trigger = ActionButton::new(
      "step-command-picker",
      ButtonKind::Select,
      command_summary(ctx, &props.step),
    )
    .bounded_label()
    .element_ref(self.anchor.clone())
    .on_click(move || open.update(|v| *v = !*v));
    let trigger: Element = if narrowing::commands(choice).is_empty() {
      let reason = ctx.t("pipelines.commandsUnavailable");
      Tooltip::new("step-command-reason", reason.clone()).wrap(ctx, trigger.unavailable(reason))
    } else if props.locked {
      trigger.disabled().into()
    } else {
      trigger.into()
    };
    let menu = popover_panel("step-command-menu", 300.0).child(
      ScrollVertical::new(command_rows(&props.state, &props.step, choice, props.locked))
        .max_height(Dimension::Px(260.0)),
    );
    Column::new()
      .width(Dimension::Px(super::step_fields::FIELD_WIDTH))
      .child(trigger)
      .child(popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::Below,
        menu,
      ))
  }
}
