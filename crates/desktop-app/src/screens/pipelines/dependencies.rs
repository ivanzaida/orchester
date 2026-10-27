//! Compact multi-dependency picker for LWah7; editing remains PipelineEdit's. A step on a cycle keeps the box's
//! outline in `danger` (`ok2Zu` in `g6Ecy`).
use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::{ElementRef, Signal},
  node::Element,
};

use super::step_fields::Props;
use crate::ui::{
  button::{ActionButton, ButtonKind},
  popover::{PopoverSide, popover, popover_panel},
  toggle::{Toggle, ToggleKind},
};

pub struct Dependencies {
  open: Signal<bool>,
  anchor: ElementRef,
}
impl Component for Dependencies {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::new(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let editing = props.state.get();
    let shown = editing.shown();
    let open = self.open.clone();
    let trigger = ActionButton::new("step-after-picker", ButtonKind::Select, label(ctx, &shown, &props.step))
      .fill_width()
      .bounded_label()
      .invalid(on_cycle(&shown, &props.step.key))
      .element_ref(self.anchor.clone())
      .on_click(move || open.update(|v| *v = !*v));
    let mut menu = popover_panel("step-after-menu", 260.0);
    for (index, step) in shown
      .steps()
      .iter()
      .enumerate()
      .filter(|(_, s)| s.key != props.step.key)
    {
      let (state, key, dependency) = (props.state.clone(), props.step.key.clone(), step.key.clone());
      let toggle = Toggle::new(
        format!("step-after-{}", step.key),
        ToggleKind::Checkbox,
        props.step.depends_on.contains(&step.key),
        format!("{:02} {}", index + 1, step.name),
      );
      menu = menu.child(if props.locked {
        toggle
      } else {
        toggle.on_change(move |on| {
          state.update(|s| {
            s.edit.set_dependency(&key, &dependency, on);
          })
        })
      });
    }
    Column::new()
      .width(lurq::node::dimension::Dimension::Px(super::step_fields::FIELD_WIDTH))
      .child(if props.locked { trigger.disabled() } else { trigger })
      .child(popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::Below,
        menu,
      ))
  }
}

/// The steps `step` runs after, by number and name, or *On task start* when it runs first.
pub fn label(
  ctx: &Ctx,
  shown: &orchester_core::resource::PipelineEdit,
  step: &orchester_core::resource::Step,
) -> String {
  let names: Vec<_> = shown
    .steps()
    .iter()
    .enumerate()
    .filter(|(_, s)| step.depends_on.contains(&s.key))
    .map(|(index, s)| format!("{:02} {}", index + 1, s.name))
    .collect();
  if names.is_empty() {
    ctx.t("pipelines.onTaskStart").to_string()
  } else {
    names.join(", ")
  }
}

/// Whether the step keyed `key` waits for itself through the steps it runs after.
pub fn on_cycle(shown: &orchester_core::resource::PipelineEdit, key: &orchester_core::resource::StepKey) -> bool {
  shown.cycle().is_some_and(|cycle| cycle.contains(key))
}
