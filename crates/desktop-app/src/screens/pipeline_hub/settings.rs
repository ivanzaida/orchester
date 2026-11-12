//! *Pipeline* under the flow (`Pipeline` `rsIh5` in `Pipelines · v2` P1): the pipeline's own settings as rows in one
//! outline, edited in place. They change only the edit the panel holds ([`super::edit`]); *Save* stores it.
//!
//! The rows are *Description* (a box), *Slug* (fixed, with a lock: `Slug` `Sni1T` in P17 `LzGvn`), *Starts* and *When a
//! step fails* (what every run does today, shown as their value: nothing else can be chosen), *Sources for every step*
//! ([`super::sources`]) and *A run delivers* (a select of what a passing run leaves, with what the chosen one means
//! under its label). The old Settings tab's *Who can use it* is not here: it only ever said *This project*, and a
//! pipeline stores no access setting (P17).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::resource::{Deliverable, PipelineEdit};
use orchester_workspace::pipelines::sources::Listing;

use super::{
  edit::Edit,
  sources::{self, Props as SourcesProps},
};
use crate::{
  project::ShownProject,
  screens::{
    pipeline_hub::pick::{self, Pick},
    team_hub::rows::{self, Sub},
  },
  theme::{IconSize, PipelinesText},
  ui::{
    field::FieldInput,
    icon::{Icon, icon},
  },
};

/// What the section needs of the panel.
pub(super) struct Settings<'a> {
  pub project: &'a ShownProject,
  pub edit: &'a Edit,
  /// What the panel shows: the edit, or the stored pipeline.
  pub current: &'a PipelineEdit,
  /// The pipeline's slug, which neither an edit nor a rename changes.
  pub slug: &'a str,
  /// What the description's box holds.
  pub description: &'a Signal<String>,
  /// A save is under way: nothing is changed now.
  pub locked: bool,
  /// How many saves stored the edit: the box is mounted anew after each, so it no longer has the focus.
  pub saved: u64,
  /// The *A run delivers* row, which the Deliver chip of an open step brings into view.
  pub delivers: &'a ElementRef,
}

pub(super) fn section(ctx: &mut Ctx, settings: &Settings<'_>) -> Column {
  let list = vec![
    description(ctx, settings),
    slug(ctx, settings.slug),
    rows::row(
      "pipeline-hub-starts",
      &ctx.t("pipeline_hub.settings.starts"),
      None,
      rows::words("pipeline-hub-starts-value", &ctx.t("pipeline_hub.settings.onTaskStart")),
    ),
    rows::row(
      "pipeline-hub-failure",
      &ctx.t("pipeline_hub.settings.failure"),
      Some(Sub::quiet(ctx.t("pipeline_hub.settings.failureSub"))),
      rows::words(
        "pipeline-hub-failure-value",
        &ctx.t("pipeline_hub.settings.failureValue"),
      ),
    ),
    pipeline_sources(ctx, settings),
    delivers(ctx, settings),
  ];
  rows::section(
    "pipeline-hub-settings",
    &ctx.t("pipeline_hub.settings.title"),
    Vec::new(),
    list,
  )
}

/// *Description*: the box's text is the edit's description (the panel follows it, see `PipelinePanel::follow_typed`).
fn description(ctx: &mut Ctx, settings: &Settings<'_>) -> lurq::components::Row {
  let label = ctx.t("pipeline_hub.settings.description");
  let mut input = FieldInput::new("pipeline-hub-description", label.clone(), settings.description.clone())
    .placeholder(ctx.t("pipeline_hub.settings.descriptionNone"))
    .fresh(settings.saved);
  if settings.locked {
    input = input.locked();
  }
  rows::row("pipeline-hub-description-row", &label, None, input.render(ctx))
}

/// *Slug*: how tools, Orchester's MCP server, tasks and runs name the pipeline. It was made from the name when the
/// pipeline was created and never changes (a rename keeps it), so it is plain text with a lock, not a field.
fn slug(ctx: &mut Ctx, slug: &str) -> Row {
  let value = Row::new()
    .id("pipeline-hub-slug-value")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(slug)
        .id("pipeline-hub-slug")
        .variant(PipelinesText::Slug.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(icon(Icon::Lock, IconSize::Base, PaletteColor::TextMuted).id("pipeline-hub-slug-locked"));
  rows::row(
    "pipeline-hub-slug-row",
    &ctx.t("pipeline_hub.settings.slug"),
    Some(Sub::quiet(ctx.t("pipeline_hub.settings.slugSub"))),
    value,
  )
}

/// *Sources for every step*: the pills and *Add*.
fn pipeline_sources(ctx: &mut Ctx, settings: &Settings<'_>) -> lurq::components::Row {
  let value = sources::view(
    ctx,
    SourcesProps {
      root: settings.project.open.repo_root.clone(),
      project: settings.project.open.project.name().as_str().into(),
      listing: Listing::Pipeline,
      current: settings.current.clone(),
      edit: settings.edit.clone(),
      locked: settings.locked,
    },
  );
  rows::row(
    "pipeline-hub-sources-row",
    &ctx.t("pipeline_hub.settings.sources"),
    None,
    value,
  )
}

/// *A run delivers*: what a run that passes every step leaves.
fn delivers(ctx: &mut Ctx, settings: &Settings<'_>) -> lurq::components::Row {
  let shown = settings.current.delivers();
  let options = Deliverable::ALL
    .into_iter()
    .map(|choice| (choice.as_str().to_owned(), ctx.t(delivers_words(choice).0).to_string()))
    .collect();
  let edit = settings.edit.clone();
  let on_pick: Pick = Arc::new(move |picked| {
    if let Some(choice) = Deliverable::ALL.into_iter().find(|choice| choice.as_str() == picked) {
      edit.change(|current| current.set_delivers(choice));
    }
  });
  let label = ctx.t("pipeline_hub.settings.delivers");
  let select = pick::pick(
    ctx,
    pick::Props {
      id: "pipeline-hub-delivers-pick".to_owned(),
      name: label.to_string(),
      value: shown.as_str().to_owned(),
      options,
      placeholder: None,
      mono: false,
      locked: settings.locked,
      choosing: false,
      on_pick,
    },
  );
  rows::row(
    "pipeline-hub-delivers",
    &label,
    Some(Sub::quiet(ctx.t(delivers_words(shown).1))),
    select,
  )
  .ref_element(settings.delivers.clone())
}

/// The words of what `delivers` means: its name and what it leaves.
pub(super) const fn delivers_words(delivers: Deliverable) -> (&'static str, &'static str) {
  match delivers {
    Deliverable::Changes => (
      "pipeline_hub.settings.deliverOption.changes",
      "pipeline_hub.settings.deliverOption.changesSub",
    ),
    Deliverable::Report => (
      "pipeline_hub.settings.deliverOption.report",
      "pipeline_hub.settings.deliverOption.reportSub",
    ),
    Deliverable::Outside => (
      "pipeline_hub.settings.deliverOption.outside",
      "pipeline_hub.settings.deliverOption.outsideSub",
    ),
    Deliverable::ChangesOrReport => (
      "pipeline_hub.settings.deliverOption.changesOrReport",
      "pipeline_hub.settings.deliverOption.changesOrReportSub",
    ),
  }
}
