//! The pipeline being made in the hub's list (`Section · New` `ddYrV`, `Item · New pipeline` `arETz` in P14 `Rszkt`):
//! first, under *New · not saved*, chosen, with what is typed as its name (*New pipeline* while nothing is) over where it
//! starts from and that nothing is saved.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Button, Column},
  node::{Element, dimension::Dimension},
};

use super::{NewPipeline, words};
use crate::{
  screens::{
    hub::{
      agent_row::{AgentRow, agent_row},
      section::section_row,
    },
    pipeline_hub::model::Listing,
  },
  theme::{HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    tooltip::Truncated,
  },
};

/// *New · not saved* and the row of the pipeline being made.
pub(in crate::screens::pipeline_hub) fn section(ctx: &mut Ctx, new: &NewPipeline, listing: &Listing) -> Element {
  let typed = new.name.get();
  let name = if typed.trim().is_empty() {
    ctx.t("pipeline_new.untitled").to_string()
  } else {
    typed.trim().to_owned()
  };
  let source = new
    .copy
    .get()
    .parse()
    .ok()
    .and_then(|id| listing.entry(id))
    .filter(|entry| entry.body.is_some());
  let said = words::line(ctx, new.choice.get(), source, new.steps.get());
  let id = "pipeline-hub-row-new";
  let line = Truncated::new(format!("{id}-line"), said, HubText::RowLine.role()).render(ctx);
  let row = agent_row(AgentRow {
    id,
    chosen: true,
    open: Button::empty(),
    group: None,
    kind: "pipeline".to_owned(),
    mark: icon(Icon::Workflow, IconSize::Lg, PaletteColor::TextSecondary).into(),
    name: (&name, PaletteColor::TextPrimary),
    line,
    tag: None,
    version: None,
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(section_row(
      "pipeline-hub-section-new",
      &ctx.t("pipeline_new.section"),
      (&ctx.t("pipeline_new.notSaved"), PaletteColor::TextMuted),
    ))
    .child(row)
    .into()
}
