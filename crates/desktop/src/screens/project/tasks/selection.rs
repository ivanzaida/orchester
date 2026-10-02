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

/// How much of what a box stands for is selected: nothing, part of it or all of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marked {
  None,
  Some,
  All,
}

pub fn checkbox(id: impl Into<String>, checked: bool, toggle: impl Fn() + Send + Sync + 'static) -> Element {
  marked_box(id, if checked { Marked::All } else { Marked::None }, toggle)
}

/// A selection box: square, square-minus for a partial selection, square-check.
pub fn marked_box(id: impl Into<String>, marked: Marked, toggle: impl Fn() + Send + Sync + 'static) -> Element {
  let (glyph, described) = match marked {
    Marked::None => (Icon::Square, "false"),
    Marked::Some => (Icon::SquareMinus, "partly"),
    Marked::All => (Icon::SquareCheck, "true"),
  };
  pressable(surface(), toggle)
    .id(id.into())
    .describe("selected", described)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .child(icon(glyph, IconSize::Option, PaletteColor::TextSecondary))
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
