//! Context (`03 · Context` `ux0Kh`, empty `09` `mE5oU`): what agents get besides the run's worktree — the project's
//! sources ([`super::sources`]) and the code index with its search tool ([`super::index`]).
use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{SectionStatuses, TabProps, index, page, sources};
use crate::theme::ExtraSpacing;

/// The tab, the sources' and the code index's save statuses said apart.
pub(super) struct Context(SectionStatuses);

impl Component for Context {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let page = ctx.props::<TabProps>().status.clone();
    Self(SectionStatuses::new(ctx, 2, &page))
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let sources = ctx.mount_keyed::<sources::Sources>("project-settings-sources", self.0.props(&props, 0));
    let index = ctx.mount_keyed::<index::CodeIndex>("project-settings-index", self.0.props(&props, 1));
    page::scroll(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Roomy.role())
        .child(sources)
        .child(index),
    )
  }
}
