//! Orchestrator (`S03` `X8mFTS`, `docs/design/settings.md`, the person's decision 2): *Let the Orchestrator drive*, the
//! project's own switch ([`switch`]) that replaced the six the Orchestrator had (ORC-909). The group is the person's
//! alone, as its heading says; its foot says what off means. Under it,
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

use super::{SectionStatuses, TabProps, kit::Group, page};
use crate::{
  theme::ExtraSpacing,
  ui::{icon::Icon, setting_rows::marked_words},
};

/// *Watching*'s section, after the switch's.
const WATCHING: usize = 1;

pub(super) struct Orchestrator {
  /// The switch's save status, then *Watching*'s.
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
    let row = ctx.mount_keyed::<switch::Switch>(switch::ID, self.sections.props(&props, 0));
    let only_you = marked_words(
      "settings-orchestrator-may-only-you",
      Icon::Hand,
      &ctx.t("project_settings.orchestrator.onlyYou"),
    );
    let may = Group::new("settings-orchestrator-may", ctx.t("project_settings.orchestrator.may"))
      .end(only_you)
      .row(row)
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
