//! A published version shown read-only in the panel, opened from the list under *Versions* ([`super::versions`]): in
//! the place of the pipeline's body a notice names the version, when it was published and what holds it, with *Restore*
//! and *Back to editing*; under it the version's steps as the flow they run in, without any card that opens, and its
//! settings as plain values. Nothing in it changes: *Restore* puts the version into the panel's edit
//! ([`super::edit::Edit::restore`]), which *Save* then keeps as the next version.
//!
//! Pencil's P12 designs the list only; this view is built from the kit's notice, rows and the flow's read-only mode and
//! has no frame of its own.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::{Pipeline, PipelineEdit, VersionNumber},
  time::Timestamp,
};
use orchester_workspace::pipelines::{
  data::Assignables,
  flow::FlowRead,
  history::{PublishedVersion, Use},
};

use super::{flow, model::Entry, settings, versions};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  screens::team_hub::rows::{self, Sub},
  theme::{ExtraSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// A version the panel shows read-only: what the list read of it when the person pressed its row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Viewed {
  pub number: VersionNumber,
  pub published_at: Timestamp,
  pub body: Arc<Pipeline>,
  /// What holds it, as the list said.
  pub held_by: Use,
}

impl lurq::app::component::DevtoolsInspectable for Viewed {}

pub(super) type ViewedSignal = lurq::core::Signal<Option<Viewed>>;

impl Viewed {
  pub(super) fn of(version: &PublishedVersion, held_by: Use) -> Self {
    Self {
      number: version.number,
      published_at: version.published_at,
      body: version.body.clone(),
      held_by,
    }
  }
}

/// What *Restore* can do now.
pub(super) enum Restore {
  /// It puts the version into the edit.
  Ready(Arc<dyn Fn() + Send + Sync>),
  /// The panel already holds exactly this version: there is nothing to put in.
  Same,
  /// A save is under way or waits for a decision: nothing changes now.
  Busy,
}

/// What the view needs of the panel.
pub(super) struct Parts<'a> {
  pub project: &'a ShownProject,
  pub entry: &'a Entry,
  pub assignables: &'a Assignables,
  pub now: Timestamp,
  pub viewed: &'a Viewed,
  pub restore: Restore,
  /// *Back to editing*.
  pub back: Arc<dyn Fn() + Send + Sync>,
}

/// The panel's body for the version: the notice, *Steps*, the flow and the settings.
pub(super) fn parts(ctx: &mut Ctx, parts: &Parts<'_>) -> Vec<Element> {
  let Parts {
    project,
    entry,
    assignables,
    viewed,
    ..
  } = parts;
  let navigation = Navigation::of(ctx);
  let shown = PipelineEdit::new(Pipeline::clone(&viewed.body));
  let read = FlowRead::of_edit(&shown, assignables);
  vec![
    notice(ctx, parts),
    steps_head(ctx, viewed.body.steps().len()),
    flow::read_only(ctx, (project, &navigation), entry.id, &read),
    settings_section(ctx, &shown),
  ]
}

/// *Viewing v2*, when it was published and what holds it, over *Restore v2* and *Back to editing*.
fn notice(ctx: &Ctx, parts: &Parts<'_>) -> Element {
  let Parts {
    now,
    viewed,
    restore,
    back,
    ..
  } = parts;
  let version = viewed.number.to_string();
  let restore_button = ActionButton::new(
    "pipeline-hub-version-restore",
    ButtonKind::Primary,
    ctx.t_args("pipeline_hub.versions.restore", [("version", version.clone())]),
  )
  .icon(Icon::Undo2);
  let restore_button = match restore {
    Restore::Ready(restore) => {
      let restore = restore.clone();
      restore_button.on_click(move || restore())
    }
    Restore::Same => {
      restore_button.unavailable(ctx.t_args("pipeline_hub.versions.restoreSame", [("version", version.clone())]))
    }
    Restore::Busy => restore_button.disabled(),
  };
  let back = back.clone();
  Notice::new(
    "pipeline-hub-version-viewing",
    NoticeTone::Neutral,
    ctx.t_args("pipeline_hub.versions.viewing", [("version", version)]),
  )
  .icon(Icon::History)
  .title_style(HubText::NoticeTitle.role())
  .body(versions::meta(ctx, *now, (viewed.published_at, viewed.held_by)))
  .flat()
  .action(restore_button)
  .action(
    ActionButton::new(
      "pipeline-hub-version-back",
      ButtonKind::Secondary,
      ctx.t("pipeline_hub.versions.back"),
    )
    .on_click(move || back()),
  )
  .into()
}

/// *Steps 6*, as the panel's own head says it, without *Edit steps*: a version is not edited.
fn steps_head(ctx: &Ctx, count: usize) -> Element {
  Row::new()
    .id("pipeline-hub-version-steps")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipeline_hub.panel.steps"))
        .id("pipeline-hub-version-steps-label")
        .variant(HubText::PanelLabel.role()),
    )
    .child(
      Text::new(&count.to_string())
        .id("pipeline-hub-version-steps-count")
        .variant(HubText::PanelHint.role()),
    )
    .into()
}

/// *Pipeline* as plain values: what the version's settings say, the rows the panel's own section edits.
fn settings_section(ctx: &Ctx, shown: &PipelineEdit) -> Element {
  let description = if shown.description().is_empty() {
    ctx.t("pipeline_hub.versions.noDescription")
  } else {
    Arc::from(shown.description())
  };
  let sources: Element = if shown.sources().pipeline().is_empty() {
    rows::words(
      "pipeline-hub-version-sources-none",
      &ctx.t("pipeline_hub.versions.noSources"),
    )
    .into()
  } else {
    rows::tags(
      "pipeline-hub-version-sources",
      shown.sources().pipeline().iter().map(|path| path.as_str().to_owned()),
    )
    .into()
  };
  let (delivers, delivers_sub) = settings::delivers_words(shown.delivers());
  let rows = vec![
    rows::row(
      "pipeline-hub-version-description",
      &ctx.t("pipeline_hub.settings.description"),
      None,
      rows::words("pipeline-hub-version-description-value", &description),
    ),
    rows::row(
      "pipeline-hub-version-starts",
      &ctx.t("pipeline_hub.settings.starts"),
      None,
      rows::words(
        "pipeline-hub-version-starts-value",
        &ctx.t("pipeline_hub.settings.onTaskStart"),
      ),
    ),
    rows::row(
      "pipeline-hub-version-failure",
      &ctx.t("pipeline_hub.settings.failure"),
      Some(Sub::quiet(ctx.t("pipeline_hub.settings.failureSub"))),
      rows::words(
        "pipeline-hub-version-failure-value",
        &ctx.t("pipeline_hub.settings.failureValue"),
      ),
    ),
    rows::row(
      "pipeline-hub-version-sources-row",
      &ctx.t("pipeline_hub.settings.sources"),
      None,
      sources,
    ),
    rows::row(
      "pipeline-hub-version-delivers",
      &ctx.t("pipeline_hub.settings.delivers"),
      Some(Sub::quiet(ctx.t(delivers_sub))),
      rows::words("pipeline-hub-version-delivers-value", &ctx.t(delivers)),
    ),
  ];
  let section: Column = rows::section(
    "pipeline-hub-version-settings",
    &ctx.t("pipeline_hub.settings.title"),
    Vec::new(),
    rows,
  );
  section.into()
}

/// Whether the edit the panel shows is exactly `viewed`'s body, which leaves *Restore* nothing to put in.
pub(super) fn is_held(current: &PipelineEdit, viewed: &Viewed) -> bool {
  *current == PipelineEdit::new(Pipeline::clone(&viewed.body))
}
