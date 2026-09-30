//! The indexes Orchester keeps (`hY0TN` in `i220W`, empty in `LC2V0`): a row per revision with
//! the runs that use it, its state and *Rebuild* / *Retry*, and the total size; what the setting's
//! read-back says about them.

use std::{sync::Arc, time::Duration};

use chrono::Datelike;
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{code_index::CodeIndexSetting, revision::Revision, time::Timestamp, tokens::grouped};
use orchester_index::{BuildPhase, BuildProgress, IndexState, IndexSummary};

use crate::{
  code_index::{IndexList, IndexRow, UsedBy},
  display::{self, MONTH_KEYS},
  messages::Message,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, wrapping_field_error},
    icon::{Icon, IconSize, icon},
    message_text,
    status_label::compact_status_label,
  },
};

/// A row's button height (`G6xG2w`).
const ROW_BUTTON_HEIGHT: f32 = 28.0;

/// Rebuilds the index of a revision.
pub type Rebuild = Arc<dyn Fn(Revision) + Send + Sync>;

/// Under the switch (`Xpsmn`): how many indexes are kept and how many build or failed.
pub fn readback(ctx: &Ctx, setting: CodeIndexSetting, list: Option<&Result<IndexList, Message>>) -> Arc<str> {
  if !setting.is_on() {
    return ctx.t("settings.codeIndex.offReadback");
  }
  let rows = match list {
    None => return Arc::from(""),
    Some(Err(error)) => return message_text(ctx, error),
    Some(Ok(list)) => &list.0,
  };
  if rows.is_empty() {
    return ctx.t("settings.codeIndex.noneReadback");
  }
  let count = |matches: fn(&IndexState) -> bool| rows.iter().filter(|row| matches(&row.state)).count();
  let kept = counted(ctx, "settings.codeIndex.kept", rows.len());
  let mut busy = Vec::new();
  for (key, number) in [
    (
      "settings.codeIndex.building",
      count(|state| matches!(state, IndexState::Building(_))),
    ),
    (
      "settings.codeIndex.failedCount",
      count(|state| matches!(state, IndexState::Failed { .. })),
    ),
  ] {
    if number > 0 {
      busy.push(ctx.t_args(key, [("count", number.to_string().as_str())]).to_string());
    }
  }
  if busy.is_empty() {
    return kept;
  }
  ctx.t_args(
    "settings.codeIndex.keptBusy",
    [
      ("kept", &*kept),
      ("busy", busy.join(&ctx.t("common.listSeparator")).as_str()),
    ],
  )
}

/// The index list panel (`hY0TN`): its title with the total size, what the list is, then a row per
/// index, or that there is none yet.
pub fn panel(ctx: &Ctx, list: Option<&Result<IndexList, Message>>, rebuild: &Rebuild) -> Element {
  let rows: &[IndexRow] = match list {
    Some(Ok(list)) => &list.0,
    _ => &[],
  };
  let body: Element = match list {
    None => Column::new().into(),
    Some(Err(error)) => field_error(&message_text(ctx, error)),
    Some(Ok(_)) if rows.is_empty() => empty(ctx),
    Some(Ok(_)) => Column::new()
      .id("settings-code-index-rows")
      .width(Dimension::Pct(100.0))
      .with_children(rows.iter().map(|row| index_row(ctx, row, rebuild)))
      .into(),
  };
  Column::new()
    .id("settings-code-index-list")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Inline.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("settings.codeIndex.listTitle")).variant(ExtraTypography::NoticeHeading.role()))
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&total(ctx, rows))
            .variant(TypographyStyle::Caption)
            .id("settings-code-index-total"),
        ),
    )
    .child(
      Text::new(&ctx.t("settings.codeIndex.listExplanation"))
        .variant(ExtraTypography::Detail.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(body)
    .into()
}

/// `3 indexes · 3.2 MiB in total` (`a7Ruz4`): the sizes of the indexes that are ready.
fn total(ctx: &Ctx, rows: &[IndexRow]) -> Arc<str> {
  if rows.is_empty() {
    return ctx.t("settings.codeIndex.noneKept");
  }
  let bytes = rows
    .iter()
    .filter_map(|row| match &row.state {
      IndexState::Ready(summary) => Some(summary.bytes),
      IndexState::Building(_) | IndexState::Failed { .. } => None,
    })
    .sum();
  let key = if rows.len() == 1 {
    "settings.codeIndex.total.one"
  } else {
    "settings.codeIndex.total.other"
  };
  ctx.t_args(
    key,
    [
      ("count", rows.len().to_string().as_str()),
      ("size", &message_text(ctx, &display::byte_size(bytes))),
    ],
  )
}

/// No index yet (`LC2V0`): a database beside what happens next.
fn empty(ctx: &Ctx) -> Element {
  Row::new()
    .id("settings-code-index-empty")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(lurq::layout::layout_kind::Justify::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(Icon::Database, IconSize::Dismiss, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("settings.codeIndex.empty")).variant(ExtraTypography::Explanation.role()))
    .into()
}

/// One index (`U3kR6Z`, `P1hmI`, `OsvWE`): its revision, the runs that use it, its state and
/// *Rebuild* (disabled while it builds) or *Retry*; then its size and when it was built, what the
/// build does now, or why it failed.
fn index_row(ctx: &Ctx, row: &IndexRow, rebuild: &Rebuild) -> Element {
  let short = display::short_revision(row.revision.as_str());
  let (glyph, state, button, detail) = match &row.state {
    IndexState::Ready(summary) => (
      Icon::Check,
      "settings.codeIndex.state.ready",
      (Icon::RefreshCw, "settings.codeIndex.rebuild", true),
      Text::new(&ready_detail(ctx, summary))
        .variant(TypographyStyle::Caption)
        .into(),
    ),
    IndexState::Building(progress) => (
      Icon::Loader,
      "settings.codeIndex.state.building",
      (Icon::RefreshCw, "settings.codeIndex.rebuild", false),
      Text::new(&building_detail(ctx, progress))
        .variant(TypographyStyle::Caption)
        .into(),
    ),
    IndexState::Failed { reason } => (
      Icon::CircleX,
      "settings.codeIndex.state.failed",
      (Icon::RotateCcw, "settings.codeIndex.retry", true),
      wrapping_field_error(&failed_detail(ctx, reason, &row.used_by)),
    ),
  };
  let (button_icon, label, enabled) = button;
  let press = {
    let (rebuild, revision) = (rebuild.clone(), row.revision.clone());
    move || rebuild(revision.clone())
  };
  let button = ActionButton::new(ctx.t(label))
    .icon(button_icon)
    .in_form()
    .height(Dimension::Px(ROW_BUTTON_HEIGHT))
    .on_click(press)
    .enabled(enabled)
    .render(ctx);
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(Text::new(short).variant(ExtraTypography::MonoField.role()))
    .child(Text::new(&used_by(ctx, &row.used_by)).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .child(compact_status_label(glyph, &ctx.t(state)))
    .child(
      Row::new()
        .id(format!("settings-code-index-button-{short}"))
        .child(button),
    );
  Column::new()
    .id(format!("settings-code-index-row-{short}"))
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Tight.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(line)
    .child(detail)
    .into()
}

/// `main · ORC-38 · Run 2`, `KON-1 · Runs 4, 5, 7`: the branches the runs started from that still
/// point at the revision, then the runs of each task.
fn used_by(ctx: &Ctx, runs: &[UsedBy]) -> String {
  let mut parts: Vec<String> = Vec::new();
  for branch in runs.iter().filter_map(|run| run.branch.as_deref()) {
    if !parts.iter().any(|part| part == branch) {
      parts.push(branch.to_owned());
    }
  }
  let mut tasks: Vec<(&str, Vec<String>)> = Vec::new();
  for run in runs {
    let number = run.run.to_string();
    match tasks.iter_mut().find(|(task, _)| *task == &*run.task) {
      Some((_, numbers)) => numbers.push(number),
      None => tasks.push((&run.task, vec![number])),
    }
  }
  let named: Vec<String> = tasks
    .iter()
    .map(|(task, numbers)| {
      let (key, number) = match numbers.as_slice() {
        [one] => ("settings.codeIndex.usedByRun", one.clone()),
        many => (
          "settings.codeIndex.usedByRuns",
          many.join(&ctx.t("common.listSeparator")),
        ),
      };
      ctx
        .t_args(key, [("task", *task), ("number", number.as_str())])
        .to_string()
    })
    .collect();
  if !named.is_empty() {
    parts.push(named.join(&ctx.t("common.listSeparator")));
  }
  parts.join(&ctx.t("settings.codeIndex.usedBySeparator"))
}

/// `3.2 MiB · 1,284 files · 18,406 symbols · built today 14:02 in 9 s` (`ODviP`).
fn ready_detail(ctx: &Ctx, summary: &IndexSummary) -> Arc<str> {
  let separator = ctx.t("number.groupSeparator");
  let count = |value: usize| grouped(u64::try_from(value).unwrap_or(u64::MAX), &separator);
  ctx.t_args(
    "settings.codeIndex.readyDetail",
    [
      ("size", &*message_text(ctx, &display::byte_size(summary.bytes))),
      ("files", count(summary.files).as_str()),
      ("symbols", count(summary.symbols).as_str()),
      ("when", &built_at(ctx, summary.built_at)),
      ("took", &message_text(ctx, &display::duration(summary.build_time))),
    ],
  )
}

/// `today 14:02`, else `Sep 28 14:02`.
fn built_at(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let time = display::clock(at);
  if at >= Timestamp::now().local_day_start() {
    return ctx.t_args("settings.codeIndex.builtToday", [("time", time.as_str())]);
  }
  let Some(utc) = chrono::DateTime::from_timestamp_millis(at.unix_millis()) else {
    return Arc::from(time);
  };
  let local = utc.with_timezone(&chrono::Local).date_naive();
  let month = ctx.t(MONTH_KEYS[local.month0() as usize]);
  ctx.t_args(
    "settings.codeIndex.builtOn",
    [
      ("month", &*month),
      ("day", local.day().to_string().as_str()),
      ("time", time.as_str()),
    ],
  )
}

/// `Indexing 1,291 files… · started 14:21, 6 s ago` (`bWtQo`).
fn building_detail(ctx: &Ctx, progress: &BuildProgress) -> Arc<str> {
  let millis = Timestamp::now().unix_millis() - progress.started_at.unix_millis();
  let elapsed = Duration::from_millis(u64::try_from(millis).unwrap_or_default());
  let start = display::clock(progress.started_at);
  let elapsed = message_text(ctx, &display::duration(elapsed));
  let args = [("start", start.as_str()), ("elapsed", &*elapsed)];
  match progress.phase {
    BuildPhase::Listing => ctx.t_args("settings.codeIndex.phase.listing", args),
    BuildPhase::Indexing { files } => {
      let files = grouped(
        u64::try_from(files).unwrap_or(u64::MAX),
        &ctx.t("number.groupSeparator"),
      );
      ctx.t_args(
        "settings.codeIndex.phase.indexing",
        [("files", files.as_str()), args[0], args[1]],
      )
    }
    BuildPhase::Writing => ctx.t_args("settings.codeIndex.phase.writing", args),
  }
}

/// Why the build failed and which runs go on without the index (`yxlgP`).
fn failed_detail(ctx: &Ctx, reason: &str, runs: &[UsedBy]) -> Arc<str> {
  let reason = reason.trim_end_matches('.');
  let key = match runs.len() {
    0 => return ctx.t_args("settings.codeIndex.failed.none", [("reason", reason)]),
    1 => "settings.codeIndex.failed.one",
    _ => "settings.codeIndex.failed.other",
  };
  let named: Vec<String> = runs
    .iter()
    .map(|run| {
      ctx
        .t_args(
          "settings.codeIndex.runNumber",
          [("number", run.run.to_string().as_str())],
        )
        .to_string()
    })
    .collect();
  ctx.t_args(
    key,
    [
      ("reason", reason),
      ("runs", named.join(&ctx.t("common.listSeparator")).as_str()),
    ],
  )
}

/// `key.one` or `key.other` with `count`.
fn counted(ctx: &Ctx, key: &str, count: usize) -> Arc<str> {
  let key = if count == 1 {
    format!("{key}.one")
  } else {
    format!("{key}.other")
  };
  ctx.t_args(&key, [("count", count.to_string().as_str())])
}
