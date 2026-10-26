//! What a pipeline's runs deliver (ORC-93), in Settings › Delivery (`Delivery` `GGbK4` in `Wq6tZ`): repository
//! changes, a report, work done outside the repository through MCP tools, or changes or a report, one radio row each
//! with what it means, under the section's help. Choosing one changes the draft, which the editor saves; the
//! published version keeps what it delivers until the next one is published.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::Deliverable;

use super::state::State;
use crate::ui::{
  pressable::Press,
  row_group::{GroupRow, RowGroup},
  toggle::radio_mark,
};

/// Delivery with *What a run delivers*; `locked` while a published version is viewed or the draft is saved,
/// `changed` while the draft delivers something else than the published version.
pub fn section(ctx: &Ctx, state: &State, (locked, changed): (bool, bool)) -> Element {
  let editing = state.get();
  let shown = editing.shown().delivers();
  let mut group =
    RowGroup::new("pipeline-settings-delivery", ctx.t("pipelines.delivery")).lead(ctx.t("pipelines.delivers.help"));
  if changed {
    group = group.end(super::settings::draft_tag(ctx, &editing).id("pipeline-delivers-changed"));
  }
  Deliverable::ALL
    .into_iter()
    .fold(group, |group, delivers| {
      group.row(row(ctx, state, (delivers, shown), locked))
    })
    .into()
}

/// The row of `delivers`, chosen when it is `shown`.
fn row(ctx: &Ctx, state: &State, (delivers, shown): (Deliverable, Deliverable), locked: bool) -> GroupRow {
  let (title, help) = words(delivers);
  let row = GroupRow::new(format!("pipeline-delivers-{}", delivers.as_str()), ctx.t(title))
    .leading(radio_mark(delivers == shown))
    .hint(ctx.t(help));
  if locked {
    return row;
  }
  let state = state.clone();
  row.press(Press::run(move || {
    state.update(|editing| editing.edit.set_delivers(delivers))
  }))
}

/// The title and explanation of `delivers`.
const fn words(delivers: Deliverable) -> (&'static str, &'static str) {
  match delivers {
    Deliverable::Changes => ("pipelines.delivers.changes", "pipelines.delivers.changesHelp"),
    Deliverable::Report => ("pipelines.delivers.report", "pipelines.delivers.reportHelp"),
    Deliverable::Outside => ("pipelines.delivers.outside", "pipelines.delivers.outsideHelp"),
    Deliverable::ChangesOrReport => (
      "pipelines.delivers.changesOrReport",
      "pipelines.delivers.changesOrReportHelp",
    ),
  }
}
