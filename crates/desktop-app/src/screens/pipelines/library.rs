//! Pipelines library GaNsb and empty sI5K0.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, VirtualizedList},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  background, messages,
  pipelines::library::{LibraryEntry, VersionState, library_in},
  tasks::outside::Outside,
};

use super::parts;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelinePanel, ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    state::{EmptyState, load_error, loading},
    table::{ColumnWidth, HEADER_HEIGHT, TableRow, table_header},
  },
};

/// Columns of tgdj2 / DSUqE.
const STEPS: f32 = 48.0;
const ROW_HEIGHT: f32 = 58.0;
const VERSION: f32 = 190.0;
const USED: f32 = 140.0;
const PUBLISHED: f32 = 110.0;
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
    let nav = Navigation::of(ctx);
    let id = project.open.project.id();
    let new = ActionButton::new("pipeline-new", ButtonKind::Primary, ctx.t("pipelines.new"))
      .icon(Icon::Plus)
      .on_click(move || nav.go(Route::Project(id, ProjectPage::NewPipeline)));
    let header = Row::new()
      .align_items(lurq::layout::Alignment::Start)
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Sm)
          .child(Text::new(&ctx.t("pipelines.title")).variant(ExtraTypography::Display.role()))
          .child(Text::new(&ctx.t("pipelines.subtitle")).variant(TypographyStyle::Description)),
      )
      .child(
        ActionButton::new("pipeline-import", ButtonKind::Secondary, ctx.t("pipelines.import"))
          .unavailable(ctx.t("pipelines.planned")),
      )
      .child(new);
    let page = parts::page()
      .padding(
        Padding::new()
          .top(ExtraSpacing::Roomy.role())
          .left(crate::theme::pipelines::page_inset())
          .right(crate::theme::pipelines::page_inset()),
      )
      .spacing(SpacingSize::Lg)
      .child(header);
    if !loaded.is_pending()
      && let Some(error) = &loaded.error
    {
      let retry = self.retry.clone();
      return page.child(load_error(
        ctx,
        "pipelines-error",
        (&ctx.t("pipelines.loadFailed"), &message_text(ctx, error)),
        move || retry.update(|v| *v += 1),
      ));
    }
    let Some(entries) = loaded.data else {
      let content = match loaded.error {
        Some(error) if !loaded.is_pending() => {
          let retry = self.retry.clone();
          load_error(
            ctx,
            "pipelines-error",
            (&ctx.t("pipelines.loadFailed"), &message_text(ctx, &error)),
            move || retry.update(|v| *v += 1),
          )
        }
        _ => loading("pipelines-loading", &ctx.t("pipelines.reading")).into(),
      };
      return page.child(content);
    };
    if entries.is_empty() {
      return empty(ctx, id, page);
    }
    populated(ctx, id, &entries, page)
  }
}
#[derive(Clone, PartialEq)]
struct RowProps {
  project: orchester_core::id::ProjectId,
  words: (LibraryEntry, String, String, String),
}
impl lurq::app::component::DevtoolsInspectable for RowProps {}
struct LibraryRow;
impl Component for LibraryRow {
  type Props = RowProps;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let (entry, state, used, published) = props.words;
    let nav = Navigation::of(ctx);
    TableRow::new(format!("pipeline-row-{}", entry.id))
      .height(ROW_HEIGHT)
      .spacing(SpacingSize::Lg)
      .cell(
        ColumnWidth::Fill,
        Column::new()
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&entry.name).variant(ExtraTypography::Strong.role()))
          .child(parts::caption(
            &entry
              .stages
              .iter()
              .map(|stage| {
                stage
                  .iter()
                  .map(|name| name.as_ref())
                  .collect::<Vec<_>>()
                  .join(&ctx.t("pipelines.parallel"))
              })
              .collect::<Vec<_>>()
              .join(&ctx.t("pipelines.then")),
          )),
      )
      .cell(ColumnWidth::Fixed(STEPS), parts::caption(&entry.step_count.to_string()))
      .cell(
        ColumnWidth::Fixed(VERSION),
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(icon(
            if matches!(entry.state, VersionState::DraftOnly) {
              Icon::Pencil
            } else {
              Icon::CircleCheck
            },
            IconSize::Sm,
            if matches!(entry.state, VersionState::DraftOnly) {
              PaletteColor::TextMuted
            } else {
              PaletteColor::Success
            },
          ))
          .child(parts::caption(&state)),
      )
      .cell(ColumnWidth::Fixed(USED), parts::caption(&used))
      .cell(ColumnWidth::Fixed(PUBLISHED), parts::caption(&published))
      .on_open(move || {
        nav.go(Route::Project(
          props.project,
          ProjectPage::Pipeline(entry.id, PipelinePanel::default()),
        ));
      })
  }
}
fn row_words(ctx: &Ctx, entry: &LibraryEntry) -> (LibraryEntry, String, String, String) {
  let state = match entry.state {
    VersionState::DraftOnly => ctx.t("pipelines.draftOnly"),
    VersionState::Published(v) => ctx.t_args("pipelines.published", [("version", v.get().to_string())]),
    VersionState::PublishedWithDraft(v) => ctx.t_args("pipelines.withDraft", [("version", v.get().to_string())]),
  };
  let published = entry
    .last_published
    .map_or_else(|| ctx.t("pipelines.never").to_string(), |at| age(ctx, at).to_string());
  (
    entry.clone(),
    state.to_string(),
    ctx
      .t_args("pipelines.tasks", [("count", entry.used_by.to_string())])
      .to_string(),
    published,
  )
}
fn populated(ctx: &mut Ctx, id: orchester_core::id::ProjectId, entries: &[LibraryEntry], page: Column) -> Column {
  let labels = [
    "pipelines.columns.name",
    "pipelines.columns.steps",
    "pipelines.columns.version",
    "pipelines.columns.used",
    "pipelines.columns.published",
  ]
  .map(|k| ctx.t(k));
  let widths = [
    ColumnWidth::Fill,
    ColumnWidth::Fixed(STEPS),
    ColumnWidth::Fixed(VERSION),
    ColumnWidth::Fixed(USED),
    ColumnWidth::Fixed(PUBLISHED),
  ];
  let headings: Vec<_> = widths
    .into_iter()
    .zip(labels.iter().map(|s| Some(s.as_ref())))
    .collect();
  let rows = std::sync::Arc::new(
    entries
      .iter()
      .map(|entry| RowProps {
        project: id,
        words: row_words(ctx, entry),
      })
      .collect::<Vec<_>>(),
  );
  let list = VirtualizedList::new_shared(ctx, rows)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(ROW_HEIGHT * entries.len() as f32))
    .mount_keyed::<LibraryRow, _, _, _>(|row| row.words.0.id.to_string(), Clone::clone);
  let page = page.child(
    Column::new()
      .id("pipelines-library")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .max_height(Dimension::Px(HEADER_HEIGHT + ROW_HEIGHT * entries.len() as f32))
      .child(table_header("pipelines-headings", &headings, true).spacing(SpacingSize::Lg))
      .child(list),
  );
  page
    .child(parts::caption(
      &ctx.t_args(
        "pipelines.count",
        [
          ("count", entries.len().to_string()),
          (
            "published",
            entries
              .iter()
              .filter(|e| !matches!(e.state, VersionState::DraftOnly))
              .count()
              .to_string(),
          ),
        ],
      ),
    ))
    .child(versions_help(ctx))
    .child(Spacer::new())
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
fn versions_help(ctx: &Ctx) -> Row {
  let mut stages = Row::new().flex(1.0).spacing(ExtraSpacing::Inset.role());
  for (index, (glyph, title, detail)) in [
    (Icon::Save, "pipelines.draftStage", "pipelines.draftHint"),
    (Icon::ArrowUp, "pipelines.publishStage", "pipelines.publishHint"),
    (Icon::Lock, "pipelines.pinnedStage", "pipelines.pinnedHint"),
  ]
  .into_iter()
  .enumerate()
  {
    if index > 0 {
      stages = stages.child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted));
    }
    stages = stages.child(
      Row::new()
        .flex(1.0)
        .spacing(SpacingSize::Sm)
        .child(
          Row::new()
            .width(Dimension::Px(28.0))
            .height(Dimension::Px(28.0))
            .align_items(lurq::layout::Alignment::Center)
            .justify(lurq::layout::layout_kind::Justify::Center)
            .rounded(RadiusSize::Md)
            .background(PaletteColor::SurfaceRaised)
            .border_inside(BorderSize::Sm, PaletteColor::Border)
            .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary)),
        )
        .child(
          Column::new()
            .flex(1.0)
            .spacing(SpacingSize::Xs)
            .child(Text::new(&ctx.t(title)).variant(ExtraTypography::Strong.role()))
            .child(parts::note(&ctx.t(detail))),
        ),
    );
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Inset.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(
      Text::new(&ctx.t("pipelines.versionsTitle"))
        .variant(ExtraTypography::Strong.role())
        .width(Dimension::Px(150.0)),
    )
    .child(stages)
}
fn empty(ctx: &mut Ctx, project: orchester_core::id::ProjectId, page: Column) -> Column {
  let nav = Navigation::of(ctx);
  let starter = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(lurq::layout::Alignment::Center)
        .child(Text::new(&ctx.t("pipelines.starter")).variant(ExtraTypography::Strong.role()))
        .child(parts::caption(&ctx.t("pipelines.starterLibraryHint"))),
    )
    .child(super::new_pipeline::starter_chips(ctx, false));
  page
    .child(
      EmptyState::new(
        "pipelines-empty",
        Icon::Workflow,
        ctx.t("pipelines.emptyTitle"),
        ctx.t("pipelines.emptyDetail"),
      )
      .child(starter)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .wrap()
          .spacing(SpacingSize::Sm)
          .align_items(lurq::layout::Alignment::Center)
          .child(
            ActionButton::new(
              "pipeline-empty-create",
              ButtonKind::Primary,
              ctx.t("pipelines.createFromStarter"),
            )
            .icon(Icon::Sparkles)
            .on_click({
              let nav = nav.clone();
              move || nav.go(Route::Project(project, ProjectPage::NewPipeline))
            }),
          )
          .child(
            ActionButton::new(
              "pipeline-empty-start",
              ButtonKind::Secondary,
              ctx.t("pipelines.startEmpty"),
            )
            .icon(Icon::Plus)
            .on_click(move || nav.go(Route::Project(project, ProjectPage::NewEmptyPipeline))),
          )
          .child(parts::caption(&ctx.t("pipelines.emptyCreationHint"))),
      ),
    )
    .child(versions_help(ctx))
}
