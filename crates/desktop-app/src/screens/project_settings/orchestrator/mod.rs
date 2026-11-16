//! Orchestrator (`S03` `X8mFTS`, `docs/design/settings.md`, the person's decision 2): *Let the Orchestrator drive*, the
//! project's own switch ([`switch`]) that replaced the six the Orchestrator had (ORC-909). The group is the person's
//! alone, as its heading says; its foot says what off means. Under it,
//! *Watching* holds the default order for watching and when a run without progress is reported (07 `DE2Pj`,
//! [`watching`]), then *Leading to the end* the ceilings a new subscription that leads its target to the end takes
//! (`S01` `pK4S5` on the board `Orchestrator autonomy · frames to approve`, [`leading`]), which ends the tab: the chat
//! no longer depends on *Let automation answer approvals*, a setting for outside MCP clients alone, so the tab no
//! longer shows it (`docs/plan/orchestrator-autonomy/OA-11-…`).
mod leading;
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

/// *Watching*'s section, after the switch's, and *Leading to the end*'s after it.
const WATCHING: usize = 1;
const LEADING: usize = 2;

pub(super) struct Orchestrator {
  /// The switch's save status, then *Watching*'s, then *Leading to the end*'s.
  sections: SectionStatuses,
}

impl Component for Orchestrator {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let page = ctx.props::<TabProps>().status.clone();
    Self {
      sections: SectionStatuses::new(ctx, LEADING + 1, &page),
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
    let leading =
      ctx.mount_keyed::<leading::Leading>("settings-orchestrator-leading", self.sections.props(&props, LEADING));
    page::scroll(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Roomy.role())
        .child(may)
        .child(watching)
        .child(leading),
    )
  }
}
