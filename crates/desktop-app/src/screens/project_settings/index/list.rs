//! The index rows (`Index` `aYAYJ` in `03 · Context`): one per kept revision, its state as a badge (Ready, Building,
//! Failed, Outdated) with Rebuild or Retry, and in its hint when it was built and how many files it holds, how far a
//! build got, why it failed or that an earlier Orchester built it; its size, symbols and the runs using it in its (i).
//! No index yet is the empty row (`Empty · No index yet` `sDP3O`).
use std::{path::Path, sync::Arc, time::Duration};

use lurq::{
  app::ctx::{Ctx, FutureState},
  node::Element,
};
use orchester_core::{time::Timestamp, tokens::grouped};
use orchester_workspace::{
  code_index::{BuildPhase, BuildProgress, IndexRow, KeptIndexState as State},
  display,
  messages::Message,
};

use super::{Listed, Rebuild};
use crate::{
  screens::project_settings::kit::{EmptyRow, SettingRow, Tone},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    icon::Icon,
    message::message_text,
    state::loading,
  },
};

/// How many indexes are kept and their size, for the group's heading; nothing until they are read, or with none.
pub fn total(ctx: &Ctx, state: &FutureState<Listed, Message>) -> Option<Arc<str>> {
  let rows = state.data.as_ref().map(|list| list.0.0.as_slice())?;
  if rows.is_empty() {
    return None;
  }
  let bytes = rows
    .iter()
    .filter_map(|row| match &row.state {
      State::Ready(s) => Some(s.bytes),
      _ => None,
    })
    .sum();
  Some(ctx.t_args(
    "project_settings.indexTotal",
    [
      ("count", rows.len().to_string()),
      ("size", message_text(ctx, &display::byte_size(bytes)).to_string()),
    ],
  ))
}

pub fn rows(ctx: &mut Ctx, state: &FutureState<Listed, Message>, rebuild: &Rebuild, root: &Path) -> Vec<Element> {
  let reading = SettingRow::new("settings-index-list", ctx.t("project_settings.indexRow"));
  if let Some(error) = &state.error {
    return vec![reading.toned(message_text(ctx, error), Tone::Danger).render(ctx)];
  }
  let Some(list) = &state.data else {
    let words = ctx.t("project_settings.reading");
    return vec![
      reading
        .under(loading("settings-index-list-loading", &words))
        .render(ctx),
    ];
  };
  if list.0.0.is_empty() {
    return vec![
      EmptyRow::new(
        "settings-no-indexes",
        Icon::Database,
        ctx.t("project_settings.noIndexes"),
        ctx.t("project_settings.indexBuiltNext"),
      )
      .into(),
    ];
  }
  list.0.0.iter().map(|row| index_row(ctx, row, rebuild, root)).collect()
}

fn index_row(ctx: &mut Ctx, row: &IndexRow, rebuild: &Rebuild, root: &Path) -> Element {
  let short = display::short_revision(row.revision.as_str());
  let (tone, key, retry, hint) = match &row.state {
    State::Ready(_) => (BadgeTone::Success, "project_settings.ready", false, Tone::Muted),
    State::Building(_) => (BadgeTone::Neutral, "project_settings.building", false, Tone::Muted),
    State::Failed { .. } => (BadgeTone::Danger, "project_settings.indexFailed", true, Tone::Danger),
    State::Outdated => (BadgeTone::Warning, "project_settings.outdated", false, Tone::Warning),
  };
  let (action, revision, root) = (rebuild.clone(), row.revision.clone(), root.to_path_buf());
  let button = ActionButton::new(
    format!("settings-index-rebuild-{short}"),
    ButtonKind::Secondary,
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
  let used = used_by(ctx, row);
  let info = if used.is_empty() {
    detail(ctx, &row.state)
  } else {
    ctx
      .t_args(
        "project_settings.indexInfoUsed",
        [("detail", detail(ctx, &row.state)), ("runs", used)],
      )
      .to_string()
  };
  SettingRow::new(format!("settings-index-{short}"), ctx.t("project_settings.indexRow"))
    .toned(summary(ctx, short, &row.state), hint)
    .info(info)
    .control(badge(&format!("settings-index-state-{short}"), tone, &ctx.t(key)))
    .control(button)
    .render(ctx)
}

/// The runs that use the index, each with its branch and task.
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

/// The row's one line: the revision, and when it was built with how many files, how far its build got, why it failed
/// or that an earlier Orchester built it.
fn summary(ctx: &Ctx, short: &str, state: &State) -> Arc<str> {
  let what = match state {
    State::Ready(summary) => ctx
      .t_args(
        "project_settings.indexBuilt",
        [
          ("built", built_at(ctx, summary.built_at)),
          ("files", count(ctx, summary.files)),
        ],
      )
      .to_string(),
    other => detail(ctx, other),
  };
  ctx.t_args(
    "project_settings.indexAt",
    [("revision", short), ("what", what.as_str())],
  )
}

fn count(ctx: &Ctx, n: usize) -> String {
  grouped(
    u64::try_from(n).unwrap_or(u64::MAX),
    &ctx.t("project_settings.groupSeparator"),
  )
}

fn detail(ctx: &Ctx, state: &State) -> String {
  match state {
    State::Ready(summary) => ctx
      .t_args(
        "project_settings.indexReadyDetail",
        [
          (
            "size",
            message_text(ctx, &display::byte_size(summary.bytes)).to_string(),
          ),
          ("files", count(ctx, summary.files)),
          ("symbols", count(ctx, summary.symbols)),
          ("built", built_at(ctx, summary.built_at)),
          (
            "elapsed",
            message_text(ctx, &display::duration(summary.build_time)).to_string(),
          ),
        ],
      )
      .to_string(),
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
    BuildPhase::Indexing { files } => ctx.t_args("project_settings.indexIndexing", [("count", count(ctx, files))]),
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
