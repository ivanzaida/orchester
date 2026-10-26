//! Context (`03 · Context` `ux0Kh`, empty `09` `mE5oU`): what agents get besides the run's worktree — the project's
//! sources ([`super::sources`]) and the code index with its search tool ([`super::index`]).
use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{TabProps, index, page, sources};
use crate::theme::ExtraSpacing;

pub(super) struct Context;

impl Component for Context {
  type Props = TabProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let sources = ctx.mount_keyed::<sources::Sources>("project-settings-sources", props.clone());
    let index = ctx.mount_keyed::<index::CodeIndex>("project-settings-index", props);
    page::scroll(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Roomy.role())
        .child(sources)
        .child(index),
    )
  }
}
