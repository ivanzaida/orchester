//! Row selection is separate from the row's task link.
use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::{Element, Style},
};
use orchester_core::id::TaskId;

use crate::{
  tasks::browse::Selection,
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

#[derive(Clone)]
pub struct TaskSelection(pub Signal<Selection>);

pub fn checkbox(id: impl Into<String>, checked: bool, toggle: impl Fn() + Send + Sync + 'static) -> Element {
  pressable(surface(), toggle)
    .id(id.into())
    .describe("selected", if checked { "true" } else { "false" })
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .child(icon(
      if checked { Icon::SquareCheck } else { Icon::Square },
      IconSize::Option,
      PaletteColor::TextSecondary,
    ))
    .into()
}

pub fn row_checkbox(ctx: &mut Ctx, task: TaskId, key: &str) -> Element {
  let selection = ctx
    .use_context::<TaskSelection>()
    .expect("the task list provides its selection")
    .0;
  let checked = selection.get().0.contains(&task);
  checkbox(format!("task-select-{key}"), checked, move || {
    selection.update(|value| value.toggle(task))
  })
}
