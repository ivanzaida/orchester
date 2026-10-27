//! Pipelines (`01 · Pipelines` `x1lmXb`, `01b · none yet` `l4Udx`): the title with *Import from project* and *New
//! pipeline*, the pipelines in the groups Published and Draft only, how many there are and how versions work; with
//! none yet, the two ways to start one.
use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background, messages,
  pipelines::library::{LibraryEntry, VersionState, library_in},
  tasks::outside::Outside,
};

use super::{library_row::RowProps, parts};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ImportTab, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    row_group::{GroupRow, RowGroup, note},
    state::{load_error, loading},
  },
};

pub struct Library {
  retry: Signal<u64>,
}
impl Component for Library {
  type Props = ShownProject;
  fn create(ctx: &mut Ctx) -> Self {
    Self { retry: ctx.signal(0) }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<ShownProject>().clone();
    let revision = self.retry.get() + project.feed.outside.get().of(Outside::Pipelines);
    let loaded = ctx
      .future((project.open.repo_root.clone(), revision), |(root, _)| async move {
        background::run(move || library_in(&root).map_err(|e| messages::store_error(&e))).await?
      })
      .state()
      .get();
    let id = project.open.project.id();
    let page = parts::page().child(header(ctx, id));
    let pending = loaded.is_pending();
    let body = match (loaded.data, loaded.error) {
      (_, Some(error)) if !pending => {
        let retry = self.retry.clone();
        load_error(
          ctx,
          "pipelines-error",
          (&ctx.t("pipelines.loadFailed"), &message_text(ctx, &error)),
          move || retry.update(|v| *v += 1),
        )
      }
      (Some(entries), _) if entries.is_empty() => empty(ctx, id).into(),
      (Some(entries), _) => populated(ctx, &project, &entries).into(),
      (None, _) => loading("pipelines-loading", &ctx.t("pipelines.reading")).into(),
    };
    page.child(parts::body("pipelines-body", body))
  }
}

/// The title with its actions (`Header` `rDRoY`): 24 over it and 12 under it.
fn header(ctx: &mut Ctx, project: ProjectId) -> Element {
  let nav = Navigation::of(ctx);
  let import = ActionButton::new("pipeline-import", ButtonKind::Secondary, ctx.t("pipelines.import"))
    .icon(Icon::Download)
    .opens(nav.to(Route::Project(project, ProjectPage::Import(ImportTab::Pipelines))));
  let new = ActionButton::new("pipeline-new", ButtonKind::Primary, ctx.t("pipelines.new"))
    .icon(Icon::Plus)
    .opens(nav.to(Route::Project(project, ProjectPage::NewPipeline)));
  parts::header()
    .padding(
      Padding::new()
        .top(lurq::app::theme::SpacingSize::Xl)
        .bottom(lurq::app::theme::SpacingSize::Md)
        .left(ExtraSpacing::Roomy.role())
        .right(ExtraSpacing::Roomy.role()),
    )
    .child(
      parts::title_row("pipelines-title", &ctx.t("pipelines.title"))
        .child(import)
        .child(new),
    )
    .into()
}

fn populated(ctx: &mut Ctx, shown: &ShownProject, entries: &[LibraryEntry]) -> lurq::components::Column {
  let (published, drafts): (Vec<_>, Vec<_>) = entries
    .iter()
    .partition(|entry| !matches!(entry.state, VersionState::DraftOnly));
  let mut pane = parts::pane("pipelines-library");
  for (id, title, group) in [
    ("pipelines-published", "pipelines.groups.published", published.clone()),
    ("pipelines-drafts", "pipelines.groups.draftOnly", drafts),
  ] {
    if group.is_empty() {
      continue;
    }
    let rows: Vec<Element> = group
      .into_iter()
      .map(|entry| {
        ctx.mount_keyed::<super::library_row::LibraryRow>(
          &format!("pipeline-row-{}", entry.id),
          RowProps {
            project: shown.open.project.id(),
            root: shown.open.repo_root.clone(),
            entry: entry.clone(),
          },
        )
      })
      .collect();
    pane = pane.child(RowGroup::new(id, ctx.t(title)).count(rows.len().to_string()).rows(rows));
  }
  let count = ctx.t_args(
    "pipelines.count",
    [
      ("count", entries.len().to_string()),
      ("published", published.len().to_string()),
    ],
  );
  pane.child(
    lurq::components::Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Sm)
      .child(
        lurq::components::Text::new(&count)
          .id("pipelines-count")
          .variant(crate::theme::GroupText::Hint.role()),
      )
      .child(note("pipelines-versions-note", &ctx.t("pipelines.versionsExplanation"))),
  )
}

/// No pipeline yet (`Pane` `O8C4Bn`): what a pipeline is, and starting from the starter or empty.
fn empty(ctx: &mut Ctx, project: ProjectId) -> lurq::components::Column {
  let nav = Navigation::of(ctx);
  let tile = |glyph| {
    lurq::components::Row::new()
      .width(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .align_items(lurq::layout::Alignment::Center)
      .justify(lurq::layout::layout_kind::Justify::Center)
      .rounded(lurq::app::theme::RadiusSize::Md)
      .border_inside(lurq::app::theme::BorderSize::Sm, PaletteColor::Border)
      .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
  };
  let starter = GroupRow::new("pipeline-empty-starter", ctx.t("pipelines.starter"))
    .tiled()
    .leading(tile(Icon::Sparkles))
    .hint(ctx.t("pipelines.starterLibraryHint"))
    .end(
      ActionButton::new(
        "pipeline-empty-create",
        ButtonKind::Secondary,
        ctx.t("pipelines.createFromStarter"),
      )
      .icon(Icon::Sparkles)
      .opens(nav.to(Route::Project(project, ProjectPage::NewPipeline))),
    );
  let blank = GroupRow::new("pipeline-empty-blank", ctx.t("pipelines.empty"))
    .tiled()
    .leading(tile(Icon::CircleDashed))
    .hint(ctx.t("pipelines.emptyRowHint"))
    .end(
      ActionButton::new("pipeline-empty-start", ButtonKind::Ghost, ctx.t("pipelines.startEmpty"))
        .icon(Icon::Plus)
        .opens(nav.to(Route::Project(project, ProjectPage::NewEmptyPipeline))),
    );
  parts::pane("pipelines-empty")
    .child(
      lurq::components::Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(
          lurq::components::Text::new(&ctx.t("pipelines.emptyTitle"))
            .id("pipelines-empty-title")
            .variant(lurq::app::theme::TypographyStyle::Title),
        )
        .child(parts::lead("pipelines-empty-lead", &ctx.t("pipelines.emptyDetail"))),
    )
    .child(
      RowGroup::new("pipelines-start", ctx.t("pipelines.startFrom"))
        .row(starter)
        .row(blank),
    )
    .child(note("pipelines-empty-note", &ctx.t("pipelines.emptyCreationHint")))
}

pub(super) fn age(ctx: &Ctx, at: orchester_core::time::Timestamp) -> std::sync::Arc<str> {
  let minutes = (orchester_core::time::Timestamp::now().unix_millis() - at.unix_millis()).max(0) / 60_000;
  let (key, count) = match minutes {
    0 => return ctx.t("pipelines.justNow"),
    1..60 => ("pipelines.minutesAgo", minutes),
    60..1440 => ("pipelines.hoursAgo", minutes / 60),
    _ => ("pipelines.daysAgo", minutes / 1440),
  };
  ctx.t_args(key, [("count", count.to_string())])
}
