//! Orchestrator (`S03` `X8mFTS`, `docs/design/settings.md`, the person's decision 2): *The Orchestrator may* — publish
//! drafts, answer agents, merge runs, merge finished runs on its own (further in: it needs merge runs and answer agents
//! too), continue interrupted runs and write with git — the project's own switches ([`switch`]), moved from Settings ›
//! Automation. The group is the person's alone, as its heading says; its foot says what off means. Under it,
//! *Watching* holds the default order for watching and when a run without progress is reported (07 `DE2Pj`,
//! [`watching`]), then *Also needed · set for this computer* shows *Let automation answer approvals* read-only
//! ([`computer`]).
mod computer;
mod switch;
mod watching;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::workspace::OrchestratorConsent;

use super::{SectionStatuses, TabProps, kit::Group, page};
use crate::{
  theme::ExtraSpacing,
  ui::{icon::Icon, setting_rows::marked_words},
};

/// *Watching*'s section, after the switches'.
const WATCHING: usize = OrchestratorConsent::ALL.len();

pub(super) struct Orchestrator {
  /// Each switch's save status, then *Watching*'s.
  sections: SectionStatuses,
}

impl Component for Orchestrator {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let page = ctx.props::<TabProps>().status.clone();
    Self {
      sections: SectionStatuses::new(ctx, WATCHING + 1, &page),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let rows: Vec<Element> = OrchestratorConsent::ALL
      .into_iter()
      .enumerate()
      .map(|(index, consent)| {
        ctx.mount_keyed::<switch::Switch>(
          switch::id(consent),
          switch::Props {
            tab: self.sections.props(&props, index),
            consent,
          },
        )
      })
      .collect();
    let only_you = marked_words(
      "settings-orchestrator-may-only-you",
      Icon::Hand,
      &ctx.t("project_settings.orchestrator.onlyYou"),
    );
    let may = Group::new("settings-orchestrator-may", ctx.t("project_settings.orchestrator.may"))
      .end(only_you)
      .rows(rows)
      .foot(&ctx.t("project_settings.orchestrator.foot"));
    let watching =
      ctx.mount_keyed::<watching::Watching>("settings-orchestrator-watching", self.sections.props(&props, WATCHING));
    let computer = ctx.mount_keyed::<computer::ThisComputer>("settings-orchestrator-computer", props);
    page::scroll(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Roomy.role())
        .child(may)
        .child(watching)
        .child(computer),
    )
  }
}
