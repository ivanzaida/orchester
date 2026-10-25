//! What a pipeline's runs deliver (ORC-93), in Pipeline settings › Delivery: repository changes, a report, work done
//! outside the repository through MCP tools, or changes or a report, one option card each as *Starts* chooses its
//! trigger (`Section · Starts` `lLbSl` in `x4IIjM`; the design has no frame of this field yet). Choosing one changes
//! the draft, which the editor saves; the published version keeps what it delivers until the next one is published.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::Deliverable;

use super::state::State;
use crate::ui::{
  form_section::{self, Label, Place},
  icon::Icon,
  option_card::OptionCard,
};

/// The width of the option cards, as Starts' (`lLbSl`).
const CHOICES_WIDTH: f32 = 300.0;

/// Delivery with *What a run delivers*; `locked` while a published version is viewed or the draft is saved.
pub fn settings_section(ctx: &Ctx, state: &State, locked: bool) -> Element {
  let editing = state.get();
  let shown = editing.shown().delivers();
  let published = editing.opened.latest.as_ref().map(|latest| latest.body.delivers());
  let changed = editing.viewing.is_none() && published.is_some_and(|published| published != shown);
  let choices = Deliverable::ALL.into_iter().fold(
    Column::new()
      .id("pipeline-delivers")
      .width(Dimension::Px(CHOICES_WIDTH))
      .spacing(SpacingSize::Sm),
    |choices, delivers| choices.child(card(ctx, state, (delivers, shown), locked)),
  );
  form_section::section(
    "pipeline-settings-delivery",
    &ctx.t("pipelines.delivery"),
    changed.then(|| ctx.t("pipelines.changed")).as_deref(),
    vec![form_section::side_row(
      Label::new("pipeline-settings-delivers", ctx.t("pipelines.delivers.label"))
        .description(ctx.t("pipelines.delivers.help")),
      choices,
      Place::Last,
      true,
    )],
  )
  .into()
}

/// The option card of `delivers`, chosen when it is `shown`.
fn card(ctx: &Ctx, state: &State, (delivers, shown): (Deliverable, Deliverable), locked: bool) -> OptionCard {
  let (glyph, title, help) = words(delivers);
  let state = state.clone();
  let card = OptionCard::new(
    format!("pipeline-delivers-{}", delivers.as_str()),
    (glyph, ctx.t(title)),
    ctx.t(help),
    delivers == shown,
    move || state.update(|editing| editing.edit.set_delivers(delivers)),
  );
  if locked { card.waiting() } else { card }
}

/// The glyph, title and explanation of `delivers`.
const fn words(delivers: Deliverable) -> (Icon, &'static str, &'static str) {
  match delivers {
    Deliverable::Changes => (
      Icon::GitMerge,
      "pipelines.delivers.changes",
      "pipelines.delivers.changesHelp",
    ),
    Deliverable::Report => (
      Icon::FileText,
      "pipelines.delivers.report",
      "pipelines.delivers.reportHelp",
    ),
    Deliverable::Outside => (
      Icon::Plug,
      "pipelines.delivers.outside",
      "pipelines.delivers.outsideHelp",
    ),
    Deliverable::ChangesOrReport => (
      Icon::ListChecks,
      "pipelines.delivers.changesOrReport",
      "pipelines.delivers.changesOrReportHelp",
    ),
  }
}
