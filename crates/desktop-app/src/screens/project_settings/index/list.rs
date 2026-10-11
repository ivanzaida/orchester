//! The index panel: real per-revision state, usage, size, progress and rebuild/retry.
use std::{path::Path, time::Duration};

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{time::Timestamp, tokens::grouped};
use orchester_workspace::{
  code_index::{BuildPhase, BuildProgress, IndexRow, KeptIndexState as State},
  display,
  messages::Message,
};

use super::{Listed, Rebuild};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

pub fn panel(ctx: &Ctx, state: &FutureState<Listed, Message>, rebuild: &Rebuild, root: &Path) -> Element {
  let rows = state.data.as_ref().map(|list| list.0.0.as_slice()).unwrap_or_default();
  let bytes = rows
    .iter()
    .filter_map(|row| match &row.state {
      State::Ready(s) => Some(s.bytes),
      _ => None,
    })
    .sum();
  let total = if rows.is_empty() {
    ctx.t("project_settings.noneKept")
  } else {
    ctx.t_args(
      "project_settings.indexTotal",
      [
        ("count", rows.len().to_string()),
        ("size", message_text(ctx, &display::byte_size(bytes)).to_string()),
      ],
    )
  };
  let title = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("project_settings.indexes")).variant(ExtraTypography::Strong.role()))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&total).variant(TypographyStyle::Caption));
  let mut panel = Column::new()
    .id("settings-index-panel")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(title)
    .child(super::super::page::note(
      "settings-index-list-detail",
      &ctx.t("project_settings.indexListDetail"),
    ));
  if let Some(error) = &state.error {
    panel = panel.child(field_error("settings-index-list-error", &message_text(ctx, error)));
  } else if state.data.is_none() {
    panel = panel.child(crate::ui::state::loading(
      "settings-index-list-loading",
      &ctx.t("project_settings.reading"),
    ));
  } else if rows.is_empty() {
    panel = panel.child(
      Row::new()
        .id("settings-no-indexes")
        .width(Dimension::Pct(100.0))
        .justify(lurq::layout::layout_kind::Justify::Center)
        .align_items(Alignment::Center)
        .spacing(SpacingSize::Sm)
        .padding(
          Padding::new()
            .top(ExtraSpacing::IndexEmpty.role())
            .bottom(ExtraSpacing::Loose.role()),
        )
        .child(icon(Icon::Database, IconSize::Md, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("project_settings.noIndexesDetail")).variant(TypographyStyle::Description)),
    );
  } else {
    panel = panel.with_children(rows.iter().map(|row| index_row(ctx, row, rebuild, root)));
  }
  panel.into()
}
fn index_row(ctx: &Ctx, row: &IndexRow, rebuild: &Rebuild, root: &Path) -> Element {
  let short = display::short_revision(row.revision.as_str());
  let (glyph, key, tone, retry) = match &row.state {
    State::Ready(_) => (
      Icon::CircleCheck,
      "project_settings.ready",
      PaletteColor::Success,
      false,
    ),
    State::Building(_) => (
      Icon::LoaderCircle,
      "project_settings.building",
      PaletteColor::TextSecondary,
      false,
    ),
    State::Failed { .. } => (
      Icon::CircleX,
      "project_settings.indexFailed",
      PaletteColor::Danger,
      true,
    ),
    State::Outdated => (
      Icon::TriangleAlert,
      "project_settings.outdated",
      PaletteColor::Warning,
      false,
    ),
  };
  let (action, revision, root) = (rebuild.clone(), row.revision.clone(), root.to_path_buf());
  let button = ActionButton::new(
    format!("settings-index-rebuild-{short}"),
    ButtonKind::Ghost,
    ctx.t(if retry {
      "project_settings.retryIndex"
    } else {
      "project_settings.rebuild"
    }),
  )
  .icon(if retry { Icon::RotateCcw } else { Icon::RefreshCw })
  .in_form();
  let button = if matches!(row.state, State::Building(_)) || rebuild.state().get().is_pending() {
    button.disabled()
  } else {
    button.on_click(move || action.run((root.clone(), revision.clone())))
  };
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(Text::new(short).variant(TypographyStyle::Mono))
    .child(
      Text::new(&used_by(ctx, row))
        .variant(TypographyStyle::Caption)
        .flex(1.0),
    )
    .child(icon(glyph, IconSize::Md, tone.clone()))
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).color(tone))
    .child(button);
  Column::new()
    .id(format!("settings-index-{short}"))
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(top)
    .child(super::super::page::note(
      &format!("settings-index-detail-{short}"),
      &detail(ctx, &row.state),
    ))
    .into()
}
fn used_by(ctx: &Ctx, row: &IndexRow) -> String {
  let separator = ctx.t("project_settings.usedBySeparator");
  row
    .used_by
    .iter()
    .map(|run| {
      let branch = run
        .branch
        .as_ref()
        .map(|name| format!("{name}{separator}"))
        .unwrap_or_default();
      ctx
        .t_args(
          "project_settings.usedBy",
          [
            ("branch", branch.as_str()),
            ("task", &run.task),
            ("run", run.run.to_string().as_str()),
          ],
        )
        .to_string()
    })
    .collect::<Vec<_>>()
    .join(&separator)
}
fn detail(ctx: &Ctx, state: &State) -> String {
  match state {
    State::Ready(summary) => {
      let count = |n: usize| {
        grouped(
          u64::try_from(n).unwrap_or(u64::MAX),
          &ctx.t("project_settings.groupSeparator"),
        )
      };
      ctx
        .t_args(
          "project_settings.indexReadyDetail",
          [
            (
              "size",
              message_text(ctx, &display::byte_size(summary.bytes)).to_string(),
            ),
            ("files", count(summary.files)),
            ("symbols", count(summary.symbols)),
            ("built", built_at(ctx, summary.built_at)),
            (
              "elapsed",
              message_text(ctx, &display::duration(summary.build_time)).to_string(),
            ),
          ],
        )
        .to_string()
    }
    State::Building(progress) => building(ctx, progress),
    State::Failed { reason } => reason.clone(),
    State::Outdated => ctx.t("project_settings.outdatedDetail").to_string(),
  }
}
fn built_at(ctx: &Ctx, at: Timestamp) -> String {
  let (month, day) = display::month_day(at);
  let date = if at >= Timestamp::now().local_day_start() {
    ctx.t_args("project_settings.builtToday", [("time", display::clock(at))])
  } else {
    ctx.t_args(
      "project_settings.builtOn",
      [
        (
          "month",
          message_text(ctx, &Message::new(display::MONTH_KEYS[month])).to_string(),
        ),
        ("day", day.to_string()),
        ("time", display::clock(at)),
      ],
    )
  };
  date.to_string()
}
fn building(ctx: &Ctx, progress: &BuildProgress) -> String {
  let phase = match progress.phase {
    BuildPhase::Listing => ctx.t("project_settings.indexListing"),
    BuildPhase::Writing => ctx.t("project_settings.indexWriting"),
    BuildPhase::Indexing { files } => ctx.t_args(
      "project_settings.indexIndexing",
      [(
        "count",
        grouped(
          u64::try_from(files).unwrap_or(u64::MAX),
          &ctx.t("project_settings.groupSeparator"),
        ),
      )],
    ),
  };
  let millis = Timestamp::now()
    .unix_millis()
    .saturating_sub(progress.started_at.unix_millis());
  let elapsed = Duration::from_millis(u64::try_from(millis).unwrap_or_default());
  ctx
    .t_args(
      "project_settings.indexBuildingDetail",
      [
        ("phase", phase.to_string()),
        ("time", display::clock(progress.started_at)),
        ("elapsed", message_text(ctx, &display::duration(elapsed)).to_string()),
      ],
    )
    .to_string()
}
