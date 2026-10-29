//! Markdown source editor: mono lines in a box as high as its parent leaves, scrolling inside it, 10 over and under
//! them and 12 at their sides, with no line numbers (`Area` `IaAfU` in 03b `QbDWX`).
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, ScrollVertical, TextInput},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::theme::ExtraSpacing;
#[derive(Clone)]
pub struct CodeEditor {
  id: Arc<str>,
  name: Arc<str>,
  value: Signal<String>,
  locked: bool,
}
impl PartialEq for CodeEditor {
  fn eq(&self, o: &Self) -> bool {
    self.id == o.id && self.name == o.name && self.value.id() == o.value.id() && self.locked == o.locked
  }
}
impl DevtoolsInspectable for CodeEditor {}
impl CodeEditor {
  pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>, value: Signal<String>) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      value,
      locked: false,
    }
  }
  pub fn locked(mut self, locked: bool) -> Self {
    self.locked = locked;
    self
  }
  pub fn render(self, ctx: &mut Ctx) -> Element {
    ctx.mount_keyed::<Editor>(&format!("code-editor-{}", self.id), self)
  }
}
struct Editor {
  frame: ElementRef,
  height: Signal<f32>,
}
impl Component for Editor {
  type Props = CodeEditor;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      frame: ctx.element_ref(),
      height: ctx.signal(0.0),
    }
  }
  fn after_layout(&self) {
    if !self.frame.is_attached() {
      return;
    }
    // The input fills the box under its padding, so a press anywhere in it places the caret.
    let height = (self.frame.bounds().height - 2.0 * ExtraSpacing::Loose.value()).max(0.0);
    if (self.height.get_untracked() - height).abs() > 0.5 {
      self.height.set(height);
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<CodeEditor>().clone();
    let theme = ctx.theme();
    let style = FormTextRole {
      typography: crate::theme::skills::source(),
      color: PaletteColor::TextPrimary,
    }
    .resolve(&theme.typography(), &theme.palette());
    let mut input = TextInput::styled(p.value, style)
      .id(p.id.to_string())
      .describe("a11y_name", p.name)
      .multiline()
      .rows(1, usize::MAX)
      .min_height(Dimension::Px(self.height.get()))
      .width(Dimension::Pct(100.0));
    if p.locked {
      input = input
        .on_input(|e: lurq::app::events::TextInputEvent| e.prevent_default())
        .focusable(false);
    }
    Column::new()
      .id(format!("{}-box", p.id))
      .ref_element(self.frame.clone())
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .background(PaletteColor::SurfaceInput)
      .rounded(RadiusSize::Md)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .clip()
      .child(
        ScrollVertical::new(
          Column::new()
            .width(Dimension::Pct(100.0))
            .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
            .child(input),
        )
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0)),
      )
  }
}
