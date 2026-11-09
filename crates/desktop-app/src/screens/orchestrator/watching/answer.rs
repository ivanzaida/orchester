//! A `/watching` row (`06 · /watching` `Ff7AD`): the person's command, its chip in their bubble (`Your command`
//! `C5u9y`) with *Applied* and when under it, then Orchester's answer (`Answer · Orchester` `Z5i1V`): the `list-tree`
//! avatar, *Orchester* and when, *the live state, no agent turn*; the one-line total (`E1LN7W`), and one bordered list
//! (`Watching` `SuoBc`) grouped per subscription. A subscription's head row (36 high, `surface_raised`) shows its icon,
//! its name, its kind, since when and the order it follows quoted, and *Stop watching*; under it a 34 high row per task
//! with something going on: its key and run, its title, its status, its note and the link where the person answers
//! it; then its other open tasks counted (30 high). The list is read from the live state, never from an agent.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{orchestrator::WatchTarget, time::Timestamp};
use orchester_workspace::{
  display,
  orchestrator::watching::{Going, WatchOrder, WatchedSubscription, WatchedTask, Watching},
};

use super::{
  super::{data::Read, timeline::TurnRow},
  Live,
};
use crate::{
  navigation::Navigation,
  routes::Route,
  theme::{
    DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, IconSize, OrchestratorSpacing, WatchingSpacing,
    WatchingText,
  },
  ui::{
    chat::{Avatar, chat_row, delivery_line, own_bubble},
    icon::{Icon, icon},
    message::message_text,
    pressable::{Press, pressed},
    slash::sent_command_chip,
    status::{Status, StatusChip},
  },
};

/// A subscription's head row (`mRauZ`), a task's row (`fzdsr`) and the row that counts the rest (`DMq1Y`).
const HEAD_HEIGHT: f32 = 36.0;
const TASK_HEIGHT: f32 = 34.0;
const REST_HEIGHT: f32 = 30.0;

/// The row of `row`, a kept `/watching`, keyed `key`: the command and the answer from the live state.
pub(in crate::screens::orchestrator) fn row(ctx: &mut Ctx, row: &TurnRow, key: &str) -> Column {
  let id = format!("orchestrator-watching-{key}");
  let time = row.turn.at.map(display::clock).unwrap_or_default();
  let command = command(ctx, &id, &time);
  let answer = answer(ctx, &id, &time, &row.env.watching);
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(OrchestratorSpacing::Items.role())
    .child(command)
    .child(answer)
}

/// The person's command: the `/watching` chip in their bubble and *Applied* with when.
fn command(ctx: &mut Ctx, id: &str, time: &str) -> Element {
  let id = format!("{id}-command");
  let chip = sent_command_chip(&id, Icon::ListTree, super::COMMAND);
  let words: Arc<str> = Arc::from(format!("/{}", super::COMMAND));
  let applied = delivery_line(
    &format!("{id}-delivery"),
    (Icon::CheckCheck, PaletteColor::Success),
    &ctx.t_args("orchestrator.watching.applied", [("time", time)]),
  );
  own_bubble(ctx, &id, Some((chip, words)), (None, Some(applied))).into()
}

/// Orchester's answer: the total and the list, from the live state.
fn answer(ctx: &mut Ctx, id: &str, time: &str, live: &Live) -> Element {
  let id = format!("{id}-answer");
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role());
  let body = match &live.read {
    Some(Read::Ready(watching)) => {
      let total = total(ctx, watching);
      let list = (!watching.subscriptions.is_empty()).then(|| list(ctx, &id, watching, live));
      body.child(words(&format!("{id}-total"), &total)).with_children(list)
    }
    Some(Read::Failed(why)) => {
      let reason = message_text(ctx, why);
      let said = ctx.t_args("orchestrator.watching.unreadable", [("reason", reason)]);
      body.child(words(&format!("{id}-unreadable"), &said))
    }
    Some(Read::Loading) | None => body
      .child(words(&format!("{id}-reading"), &ctx.t("orchestrator.watching.reading")).color(PaletteColor::TextMuted)),
  };
  let (from, meta) = (
    ctx.t("orchestrator.watching.from"),
    ctx.t_args("orchestrator.watching.meta", [("time", time)]),
  );
  chat_row(id, Avatar::Orchester, (&from, &meta), vec![body.into()])
}

/// What the answer says in 13 `text_primary`, 1.6, named `id`.
fn words(id: &str, said: &str) -> Text {
  Text::new(said)
    .id(id.to_owned())
    .variant(WatchingText::Answer.role())
    .selectable(true)
    .width(Dimension::Pct(100.0))
}

/// *This conversation leads 2 modules and 1 task. 1 thing waits for you, 1 run is stopped at a limit.*
fn total(ctx: &Ctx, watching: &Watching) -> Arc<str> {
  let total: Vec<Arc<str>> = watching.total.iter().map(|part| message_text(ctx, part)).collect();
  let total = joined(ctx, &total, false);
  let leads: Vec<Arc<str>> = watching
    .counts
    .parts()
    .iter()
    .map(|part| message_text(ctx, part))
    .collect();
  if leads.is_empty() {
    return ctx.t_args("orchestrator.watching.only", [("total", total)]);
  }
  ctx.t_args(
    "orchestrator.watching.total",
    [("leads", joined(ctx, &leads, true)), ("total", total)],
  )
}

/// `parts` one after another: each after a comma, the last after *and* when `and` says so.
fn joined(ctx: &Ctx, parts: &[Arc<str>], and: bool) -> Arc<str> {
  let Some((last, rest)) = parts.split_last() else {
    return Arc::from("");
  };
  let Some((first, middle)) = rest.split_first() else {
    return last.clone();
  };
  let rest = middle.iter().fold(first.clone(), |rest, next| {
    ctx.t_args("orchestrator.watching.comma", [("rest", rest), ("next", next.clone())])
  });
  if and {
    ctx.t_args("orchestrator.watching.and", [("rest", rest), ("last", last.clone())])
  } else {
    ctx.t_args("orchestrator.watching.comma", [("rest", rest), ("next", last.clone())])
  }
}

/// The bordered list, grouped per subscription.
fn list(ctx: &mut Ctx, id: &str, watching: &Watching, live: &Live) -> Element {
  let mut rows: Vec<Element> = Vec::new();
  let mut before: Option<&WatchOrder> = None;
  for (at, watched) in watching.subscriptions.iter().enumerate() {
    let same = before == Some(&watched.order) && watched.order != WatchOrder::None;
    rows.push(head(ctx, (id, at), (watched, same), live));
    for task in &watched.tasks {
      rows.push(task_row(ctx, id, task));
    }
    if let Some(rest) = &watched.rest {
      rows.push(
        line(Line::Under, REST_HEIGHT)
          .id(format!("{id}-rest-{at}"))
          .child(
            Text::new(&message_text(ctx, rest))
              .variant(WatchingText::Meta.role())
              .nowrap(),
          )
          .into(),
      );
    }
    before = Some(&watched.order);
  }
  Column::new()
    .id(format!("{id}-list"))
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .with_children(rows)
    .into()
}

/// What a row of the list is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Line {
  /// The list's first row, a subscription's head: the list's outline is its top.
  First,
  /// A later subscription's head.
  Head,
  /// A row under a head: a task's or the rest counted.
  Under,
}

/// A row of the list, `height` high: a head row on `surface_raised`, a row under it in past the head's icon; each after
/// the first ruled off by a `border` line over it.
fn line(kind: Line, height: f32) -> Row {
  let start = if kind == Line::Under {
    WatchingSpacing::Indent.role()
  } else {
    WatchingSpacing::Side.role()
  };
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .padding(Padding::new().left(start).right(WatchingSpacing::Side.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center);
  let row = if kind == Line::First {
    row
  } else {
    row.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
  };
  if kind == Line::Under {
    row
  } else {
    row.background(PaletteColor::SurfaceRaised)
  }
}

/// Subscription `at`'s head row: its icon, name, kind, since when and order, and *Stop watching*; its order is *same
/// order* when it follows the one before it (`same`).
fn head(ctx: &mut Ctx, (id, at): (&str, usize), (watched, same): (&WatchedSubscription, bool), live: &Live) -> Element {
  let row_id = format!("{id}-subscription-{at}");
  let target = watched.target.target;
  let meta = head_meta(ctx, watched, same);
  let stop = {
    let stop = live.stop.clone();
    action(
      &format!("{row_id}-stop"),
      &ctx.t("orchestrator.watching.stop"),
      Press::run(move || stop(target)),
    )
  };
  let stop = if live.stopping {
    Row::new().opacity(DISABLED_OPACITY).child(stop).into()
  } else {
    stop
  };
  let kind = if at == 0 { Line::First } else { Line::Head };
  line(kind, HEAD_HEIGHT)
    .id(row_id.clone())
    .describe("target", target_name(target))
    .child(icon(target_icon(target), IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&message_text(ctx, &watched.target.name))
        .id(format!("{row_id}-name"))
        .variant(WatchingText::Name.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&meta)
        .id(format!("{row_id}-meta"))
        .variant(WatchingText::Meta.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(stop)
    .into()
}

/// *module · since 09:14 · “Answer agents from the brief…”*: what the subscription is, since when, and its order.
fn head_meta(ctx: &Ctx, watched: &WatchedSubscription, same: bool) -> Arc<str> {
  let kind = message_text(ctx, &watched.target.kind);
  let since = since(ctx, watched.since);
  let args = |order: Option<&str>| {
    let mut args = vec![("kind", kind.to_string()), ("since", since.to_string())];
    args.extend(order.map(|order| ("order", one_line(order))));
    args
  };
  if same {
    return ctx.t_args("orchestrator.watching.head.same", args(None));
  }
  match &watched.order {
    WatchOrder::Own(order) => ctx.t_args("orchestrator.watching.head.own", args(Some(order))),
    WatchOrder::ProjectDefault(order) => ctx.t_args("orchestrator.watching.head.projectDefault", args(Some(order))),
    WatchOrder::None => ctx.t_args("orchestrator.watching.head.none", args(None)),
  }
}

/// When a subscription started: its time today, its date on an earlier day.
pub(super) fn since(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  if display::calendar_date(at) == display::calendar_date(Timestamp::now()) {
    Arc::from(display::clock(at))
  } else {
    super::super::date(ctx, at)
  }
}

/// An order written over several lines, on one.
fn one_line(order: &str) -> String {
  order.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A task's row: its key and run, title, status, note, and the link where the person answers it.
fn task_row(ctx: &mut Ctx, id: &str, task: &WatchedTask) -> Element {
  let row_id = format!("{id}-task-{}", task.key);
  // The template writes the run's `#`: it gets the bare number, not the run's display (`#17`).
  let key = match task.run {
    Some(run) => ctx.t_args(
      "orchestrator.watching.keyRun",
      [("key", task.key.to_string()), ("run", run.get().to_string())],
    ),
    None => task.key.clone(),
  };
  let status = StatusChip::new(format!("{row_id}-status"), status(task.going)).render(ctx);
  let note = task.note.as_ref().map(|note| {
    Text::new(&message_text(ctx, note))
      .id(format!("{row_id}-note"))
      .variant(WatchingText::Meta.role())
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
  });
  let link = task.action.as_ref().and_then(|word| {
    let route = Route::parse(&task.link).ok()?;
    let words = ctx.t_args("orchestrator.watching.open", [("action", message_text(ctx, word))]);
    Some(action(
      &format!("{row_id}-open"),
      &words,
      Press::Open(Navigation::of(ctx).to(route)),
    ))
  });
  line(Line::Under, TASK_HEIGHT)
    .id(row_id.clone())
    .describe("going", going_name(task.going))
    .child(
      Text::new(&key)
        .id(format!("{row_id}-key"))
        .variant(WatchingText::Key.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&task.title)
        .id(format!("{row_id}-title"))
        .variant(WatchingText::Title.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Row::new().flex_shrink(0.0).child(status))
    .child(Spacer::new().flex(1.0))
    .with_children(note)
    .with_children(link)
    .into()
}

/// A text action at a row's end, named `id`: `words` in 12 `text_secondary`, on `hover` under the pointer.
fn action(id: &str, words: &str, press: Press) -> Element {
  pressed(press)
    .id(id.to_owned())
    .describe("a11y_name", words.to_owned())
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(Text::new(words).variant(WatchingText::Action.role()).nowrap())
    .into()
}

/// A target's icon on its head row and in the `/unwatch` picker: a module's `boxes`, a task's `square-check`, the tasks
/// without a module's `inbox` and the whole project's `folder-kanban`.
pub(super) const fn target_icon(target: WatchTarget) -> Icon {
  match target {
    WatchTarget::Module(_) => Icon::Boxes,
    WatchTarget::Task(_) => Icon::SquareCheck,
    WatchTarget::WithoutModule => Icon::Inbox,
    WatchTarget::Project => Icon::FolderKanban,
  }
}

/// The name `describe` reports a target by, for tests and QA.
pub(super) const fn target_name(target: WatchTarget) -> &'static str {
  match target {
    WatchTarget::Module(_) => "module",
    WatchTarget::Task(_) => "task",
    WatchTarget::WithoutModule => "without-module",
    WatchTarget::Project => "project",
  }
}

/// The status a task's row shows for what goes on in it.
const fn status(going: Going) -> Status {
  match going {
    Going::Running => Status::Running,
    Going::Waits(_) => Status::NeedsInput,
    Going::Queued { .. } => Status::Queued,
    Going::StoppedAtLimit { .. } => Status::StoppedAtLimit,
    Going::ReadyToMerge => Status::ReadyToMerge,
  }
}

/// The name `describe` reports what goes on in a task by, for tests and QA.
const fn going_name(going: Going) -> &'static str {
  match going {
    Going::Running => "running",
    Going::Waits(_) => "waits",
    Going::Queued { .. } => "queued",
    Going::StoppedAtLimit { .. } => "stopped-at-limit",
    Going::ReadyToMerge => "ready-to-merge",
  }
}
