//! Another app has the data folder open (`N0Lpc`): which folder, which process and since when, with
//! *Switch to that window* where the platform can bring it to the front and *Close this window*. When the
//! other window did not come to the front, the screen says so and what to do if it stays stuck
//! (`uiSod`). When the other app's window has closed and it is ending, it says to start again in a few
//! seconds and offers nothing that would treat it as stuck.

use std::{path::Path, sync::Arc};

use lurq::{
  app::ctx::Ctx,
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_store::location::LockHolder;
use orchester_workspace::{display, i18n::NAMESPACE};

use super::{
  front::{self, Front},
  parts,
  screen::screen_column,
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  detail_row::{DetailValue, detail_row},
  icon::Icon,
};

/// Which of the screen's variants shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shown {
  Open,
  /// *Switch to that window* got no answer.
  NoAnswer,
  /// The other app's window closed; it is ending.
  Closing,
}

/// The screen for `folder`, which `holder` has open; `no_answer` turns on when switching got no answer.
pub fn screen(ctx: &mut Ctx, folder: &Path, holder: Option<LockHolder>, no_answer: &Signal<bool>) -> Element {
  let shown = match (holder, no_answer.get()) {
    (Some(holder), _) if holder.closing => Shown::Closing,
    (_, true) => Shown::NoAnswer,
    (_, false) => Shown::Open,
  };
  let pid = holder.map(|holder| holder.pid.to_string()).unwrap_or_default();
  let (id, heading) = heading(ctx, shown, &pid);
  let other = holder.map_or_else(
    || ctx.t("startup.alreadyOpen.unknownWindow"),
    |holder| since(ctx, holder),
  );
  let facts = Column::new()
    .width(Dimension::Pct(100.0))
    .child(detail_row(
      "startup-data-folder",
      &ctx.t("startup.dataFolder"),
      &folder.display().to_string(),
      DetailValue::Mono,
    ))
    .child(detail_row(
      "startup-other-window",
      &ctx.t("startup.alreadyOpen.otherWindow"),
      &other,
      DetailValue::Text,
    ));
  let step = (shown == Shown::NoAnswer).then(|| {
    parts::step(
      &ctx.t("startup.noAnswer.step"),
      &ctx.t_args("startup.noAnswer.stepBody", [("pid", pid.as_str())]),
    )
  });
  let (switch, footnote) = match shown {
    Shown::Closing => (None, None),
    Shown::Open | Shown::NoAnswer => {
      let (switch, footnote) = switch(ctx, holder, no_answer);
      (switch, (shown == Shown::Open).then_some(footnote))
    }
  };
  screen_column(id)
    .child(parts::tile(Icon::AppWindow, false))
    .child(heading)
    .child(facts)
    .with_children(step)
    .child(parts::actions(switch.into_iter().chain([close_button(ctx)]).collect()))
    .with_children(footnote.map(|footnote| parts::footnote(&footnote, false)))
    .into()
}

/// The element id of the variant and its heading.
fn heading(ctx: &Ctx, shown: Shown, pid: &str) -> (&'static str, Column) {
  match shown {
    Shown::Open => (
      "startup-already-open",
      parts::heading(
        &ctx.t("startup.alreadyOpen.title"),
        None,
        &ctx.t("startup.alreadyOpen.description"),
      ),
    ),
    Shown::NoAnswer => (
      "startup-no-answer",
      parts::heading(
        &ctx.t("startup.noAnswer.title"),
        Some(&ctx.t_args("startup.noAnswer.reason", [("pid", pid)])),
        &ctx.t("startup.noAnswer.description"),
      ),
    ),
    Shown::Closing => (
      "startup-closing",
      parts::heading(
        &ctx.t("startup.closing.title"),
        None,
        &ctx.t("startup.closing.description"),
      ),
    ),
  }
}

/// Since when the other app has the folder open and its process: the time, and the day before it when
/// that was not today.
fn since(ctx: &Ctx, holder: LockHolder) -> Arc<str> {
  let time = display::clock(holder.started_at);
  let pid = holder.pid.to_string();
  if holder.started_at.local_day_start() == Timestamp::now().local_day_start() {
    return ctx.t_args("startup.alreadyOpen.since", [("time", time), ("pid", pid)]);
  }
  let (month, day) = display::month_day(holder.started_at);
  let month = ctx.t_ns(NAMESPACE, display::MONTH_KEYS[month]);
  ctx.t_args(
    "startup.alreadyOpen.sinceDay",
    [
      ("month", month.to_string()),
      ("day", day.to_string()),
      ("time", time),
      ("pid", pid),
    ],
  )
}

/// *Switch to that window*, when it can be offered, and the line under the actions that says what it
/// does or why it is not offered.
fn switch(ctx: &mut Ctx, holder: Option<LockHolder>, no_answer: &Signal<bool>) -> (Option<Element>, Arc<str>) {
  let Some(holder) = holder else {
    return (None, ctx.t("startup.alreadyOpen.unknownProcess"));
  };
  if !front::SUPPORTED {
    return (None, ctx.t("startup.alreadyOpen.unsupported"));
  }
  let window = ctx.window();
  let no_answer = no_answer.clone();
  let button = ActionButton::new(
    "startup-switch",
    ButtonKind::Primary,
    ctx.t("startup.alreadyOpen.switch"),
  )
  .icon(Icon::AppWindow)
  .on_click(move || match front::bring_to_front(holder) {
    Front::Brought => window.request_close(),
    Front::NoAnswer => no_answer.set(true),
  });
  (Some(button.into()), ctx.t("startup.alreadyOpen.footnote"))
}

fn close_button(ctx: &mut Ctx) -> Element {
  let window = ctx.window();
  ActionButton::new(
    "startup-close",
    ButtonKind::Secondary,
    ctx.t("startup.alreadyOpen.close"),
  )
  .on_click(move || window.request_close())
  .into()
}
