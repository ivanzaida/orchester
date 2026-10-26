//! Runs (`02 · Runs` `b7Ya1u`): Auto mode (whether new runs use it, its reviewer, and *Approved for every run* with a
//! count that opens the project's rules, [`super::rules`]), Limits (the run and daily budgets, the time limit and how
//! many agents and QA apps work at once) and Environment (its variables' count and Edit, which opens today's editor
//! with its confirmation), each saving as a person leaves a field.
mod auto_mode;
mod environment;
mod limits;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::tasks::outside::Outside;

use super::{
  TabProps, page,
  work::{self, Read},
};
use crate::theme::ExtraSpacing;

pub(super) struct Runs {
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
  own: work::OwnSaves,
}

impl Component for Runs {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      own: work::OwnSaves::new(
        ctx,
        &[
          Outside::AutoMode,
          Outside::Runs,
          Outside::Connections,
          Outside::ProjectSettings,
        ],
      ),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let revision = self.revision.get() + self.own.count(&props.project.feed);
    let state = work::load(ctx, props.project.open.repo_root.clone(), revision, &self.own);
    let Some(read) = state.data.clone().or_else(|| self.stored.get()) else {
      return page::scroll(work::pending(ctx, &state, &self.revision));
    };
    self.stored.set(Some(read.clone()));
    let auto = ctx.mount_keyed::<auto_mode::AutoMode>(
      "settings-auto-mode",
      auto_mode::Props {
        tab: props.clone(),
        settings: read.0,
        parent_read: self.stored.clone(),
        refresh: self.revision.clone(),
        own: self.own.clone(),
      },
    );
    let limits = ctx.mount_keyed::<limits::Limits>("project-settings-limits", props.clone());
    let environment = ctx.mount_keyed::<environment::Environment>("project-settings-environment", props);
    page::scroll(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Roomy.role())
        .child(auto)
        .child(limits)
        .child(environment),
    )
  }
}
