//! A start that waits for *Services at once* slots, from the runner's `waiting_for_service_slots` note (10d of `10 ·
//! States` `oXVuM`, `Card · Waiting for 4 Services at once slots` `qsBGf`): in an `info` stroke, an `hourglass` before
//! *Waiting for 4 Services at once slots* and the task · step · since when; what it starts, how many it needs and how
//! many are free; a slot bar of the computer's slots, the held ones first (`Slots` `uBP8F`); a row per start of another
//! run that holds them, its task's key, what it runs, its slots and its step, and *Open ↗* its task (`FezXz`); then
//! *Change Services at once*, which opens Project settings › Runs, where the share is. A note stored before the runner
//! said how many are free names no holders: the card then says the limits it waits under, and its bar marks only what
//! the start needs.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  project::TaskKey,
  service::{ServiceSlotHolder, ServiceSlotWait, SlotUse},
  time::Timestamp,
};
use orchester_workspace::{display, tasks::transcript::Entry};

use super::super::{Timeline, step_label};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route, TaskTab, TaskView},
  theme::ServiceCardsText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    pressable::opening,
    service_card::{Meta, Slot, body, frame, head, slot_bar},
  },
};

/// A holder that has held its slots this long says for how long (`for 6 min`), one under a minute only its step.
const SAID_FOR_MILLIS: i64 = 60_000;

pub(super) fn card(ctx: &mut Ctx, timeline: &Timeline, id: &str, wait: &ServiceSlotWait, entry: &Entry) -> Element {
  let slots = wait.slots;
  let title = if slots == 1 {
    "service_cards.slots.title.one"
  } else {
    "service_cards.slots.title.other"
  };
  let said = said(ctx, wait);
  let step = timeline.step_of(ctx, Some(entry.execution)).unwrap_or_default();
  let meta = ctx.t_args(
    "service_cards.slots.meta",
    [
      ("task", timeline.key.to_string()),
      ("step", step.to_string()),
      ("time", display::clock(entry.at)),
    ],
  );
  let bar: Vec<Slot> = match wait.held {
    // The computer's slots, the held ones first.
    Some(held) => (0..wait.limit.max(held))
      .map(|at| if at < held { Slot::Held } else { Slot::Free })
      .collect(),
    // A note that does not say which are held: the ones this start needs first.
    None => (0..wait.limit.max(slots))
      .map(|at| if at < slots { Slot::Taken } else { Slot::Free })
      .collect(),
  };
  let holders: Vec<Element> = wait
    .holders
    .iter()
    .enumerate()
    .map(|(at, holder)| holder_row(ctx, &format!("{id}-holder-{at}"), holder, entry.at))
    .collect();
  let limits = Navigation::of(ctx).to(Route::Project(
    timeline.project,
    ProjectPage::Settings(ProjectSettingsTab::Runs),
  ));
  let change = ActionButton::new(
    format!("{id}-change"),
    ButtonKind::Ghost,
    ctx.t("service_cards.slots.change"),
  )
  .icon(Icon::SlidersHorizontal)
  .opens(limits)
  .in_form();
  frame(id, PaletteColor::Info)
    .describe("slots", slots.to_string())
    .child(head(
      id,
      (Icon::Hourglass, PaletteColor::Info),
      &ctx.t_args(title, [("count", slots.to_string())]),
      (&meta, Meta::AfterTitle),
    ))
    .child(body(&format!("{id}-body"), &said))
    .child(slot_bar(&format!("{id}-bar"), &bar, Dimension::Pct(100.0)))
    .with_children(holders)
    .child(change)
    .into()
}

/// What the start needs and how many are free, when the note says; else the limits it waits under.
fn said(ctx: &Ctx, wait: &ServiceSlotWait) -> String {
  let count = wait.slots.to_string();
  let (Some(free), Some(starts)) = (wait.free, wait.starts.as_ref()) else {
    let key = if wait.slots == 1 {
      "service_cards.slots.body.one"
    } else {
      "service_cards.slots.body.other"
    };
    return ctx
      .t_args(
        key,
        [
          ("count", count),
          ("share", wait.share.to_string()),
          ("limit", wait.limit.to_string()),
        ],
      )
      .to_string();
  };
  let stacked = starts.stack.is_some();
  let key = match (stacked, wait.slots, free) {
    (true, 1, _) => "service_cards.slots.free.stackOne",
    (true, _, 0) => "service_cards.slots.free.stackNone",
    (true, _, 1) => "service_cards.slots.free.stackFreeOne",
    (true, ..) => "service_cards.slots.free.stackOther",
    (false, 1, _) => "service_cards.slots.free.startOne",
    (false, _, 0) => "service_cards.slots.free.startNone",
    (false, _, 1) => "service_cards.slots.free.startFreeOne",
    (false, ..) => "service_cards.slots.free.startOther",
  };
  let stack = starts.stack.clone().unwrap_or_default();
  ctx
    .t_args(key, [("stack", stack), ("count", count), ("free", free.to_string())])
    .to_string()
}

/// The row `id` of `holder`, as the note at `noted` saw it: its task's key, what it runs, its slots and its step, and
/// *Open ↗* its task's Conversation.
fn holder_row(ctx: &mut Ctx, id: &str, holder: &ServiceSlotHolder, noted: Timestamp) -> Element {
  let one = holder.slots == 1;
  let (step, time) = match &holder.step {
    None => (None, None),
    Some(step) => {
      let label = step_label(ctx, step.number as usize, &step.name).to_string();
      let held = noted.unix_millis() - holder.since.unix_millis();
      let time =
        (held >= SAID_FOR_MILLIS).then(|| message_text(ctx, &display::waited(noted, holder.since)).to_string());
      (Some(label), time)
    }
  };
  let key = match (step.is_some(), time.is_some(), one) {
    (false, _, true) => "service_cards.slots.holder.held.one",
    (false, _, false) => "service_cards.slots.holder.held.other",
    (true, false, true) => "service_cards.slots.holder.heldIn.one",
    (true, false, false) => "service_cards.slots.holder.heldIn.other",
    (true, true, true) => "service_cards.slots.holder.heldFor.one",
    (true, true, false) => "service_cards.slots.holder.heldFor.other",
  };
  let line = ctx.t_args(
    key,
    [
      ("what", what(ctx, &holder.what)),
      ("count", holder.slots.to_string()),
      ("step", step.unwrap_or_default()),
      ("time", time.unwrap_or_default()),
    ],
  );
  let open = holder.task.parse::<TaskKey>().ok().map(|key| {
    let route = Route::Project(
      holder.project,
      ProjectPage::Task(key, TaskView::of(TaskTab::Conversation)),
    );
    let label = ctx.t("service_cards.slots.holder.open");
    opening(Navigation::of(ctx).to(route))
      .id(format!("{id}-open"))
      .child(Text::new(&label).variant(ServiceCardsText::Open.role()).nowrap())
  });
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&holder.task)
        .id(format!("{id}-key"))
        .variant(ServiceCardsText::HolderKey.role())
        .selectable(true)
        .nowrap(),
    )
    .child(
      Text::new(&line)
        .id(format!("{id}-what"))
        .variant(ServiceCardsText::Detail.role())
        .selectable(true)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(open)
    .into()
}

/// What a holder runs: its stack, its services, or a test suite's own process.
fn what(ctx: &Ctx, what: &SlotUse) -> String {
  match &what.stack {
    Some(stack) => ctx
      .t_args("service_cards.slots.holder.stack", [("stack", stack.as_str())])
      .to_string(),
    None if what.is_suite() => ctx.t("service_cards.slots.holder.suite").to_string(),
    None => super::listed(ctx, &what.services),
  }
}
