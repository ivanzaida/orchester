//! Read-only selectable source with exact substituted-value marks (fTuZZ).
use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{FormTextRole, PaletteColor},
  },
  components::{InputToken, InputTokens, TextInput},
  core::Signal,
  node::{Element, dimension::Dimension},
};

use crate::theme::instructions;
#[derive(Clone, PartialEq)]
pub struct ResolvedSource {
  pub id: Arc<str>,
  pub text: Arc<str>,
  pub values: Vec<Range<usize>>,
}
impl DevtoolsInspectable for ResolvedSource {}
impl ResolvedSource {
  pub fn render(self, ctx: &mut Ctx) -> Element {
    ctx.mount_keyed::<Source>(&format!("resolved-source-{}", self.id), self)
  }
}
struct Source {
  text: Signal<String>,
}
impl Component for Source {
  type Props = ResolvedSource;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      text: ctx.signal(ctx.props::<ResolvedSource>().text.to_string()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ResolvedSource>().clone();
    if self.text.get_untracked() != props.text.as_ref() {
      self.text.set(props.text.to_string());
    }
    let values = props
      .values
      .iter()
      .filter(|range| !props.text[(*range).clone()].contains(['\r', '\n']))
      .map(|range| InputToken::new(range.clone(), instructions::token_style(false)))
      .collect();
    let tokens = InputTokens::new(&props.text, values).expect("workspace returned disjoint UTF-8 ranges");
    let theme = ctx.theme();
    let style = FormTextRole {
      typography: instructions::prompt_text(),
      color: PaletteColor::TextSecondary,
    }
    .resolve(&theme.typography(), &theme.palette());
    TextInput::styled(self.text.clone(), style)
      .id(props.id.to_string())
      .multiline()
      .rows(1, usize::MAX)
      .tokens(tokens)
      .width(Dimension::Pct(100.0))
      .min_width(Dimension::Px(0.0))
      .on_input(|event: lurq::app::events::TextInputEvent| event.prevent_default())
  }
}
