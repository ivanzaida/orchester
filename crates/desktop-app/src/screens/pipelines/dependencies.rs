//! Compact multi-dependency picker for LWah7; editing remains PipelineEdit's.
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
    let names: Vec<_> = shown
      .steps()
      .iter()
      .enumerate()
      .filter(|(_, s)| props.step.depends_on.contains(&s.key))
      .map(|(index, s)| format!("{:02} {}", index + 1, s.name))
      .collect();
    let label = if names.is_empty() {
      ctx.t("pipelines.onStart").to_string()
    } else {
      names.join(", ")
    };
    let open = self.open.clone();
    let trigger = ActionButton::new("step-after-picker", ButtonKind::Select, label)
      .fill_width()
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
      menu = menu.child(
        Toggle::new(
          format!("step-after-{}", step.key),
          ToggleKind::Checkbox,
          props.step.depends_on.contains(&step.key),
          format!("{:02} {}", index + 1, step.name),
        )
        .on_change(move |on| {
          state.update(|s| {
            s.edit.set_dependency(&key, &dependency, on);
          })
        }),
      );
    }
    Column::new()
      .child(if props.locked { trigger.disabled() } else { trigger })
      .child(popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::Below,
        menu,
      ))
  }
}
