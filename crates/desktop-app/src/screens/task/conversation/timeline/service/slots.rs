//! A start that waits for *Services at once* slots, from the runner's `waiting_for_service_slots` note (10d of `10 ·
//! States` `oXVuM`, `Card · Waiting for 4 Services at once slots` `qsBGf`): in an `info` stroke, an `hourglass` before
//! *Waiting for 4 Services at once slots* and the task · step · since when; how many it needs under the project's share
//! and the computer's limit; a slot bar of the computer's slots with the ones this start needs (`Slots` `uBP8F`); then
//! *Change Services at once*, which opens Project settings › Runs, where the share is. The note names neither the runs
//! that hold slots nor how many are free, so the card lists no holders and the bar marks only what this start needs.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{display, tasks::transcript::Entry};

use super::super::Timeline;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    service_card::{Meta, Slot, body, frame, head, slot_bar},
  },
};

pub(super) fn card(
  ctx: &mut Ctx,
  timeline: &Timeline,
  id: &str,
  (slots, share, limit): (u32, u32, u32),
  entry: &Entry,
) -> Element {
  let count = [("count", slots.to_string())];
  let (title, said) = if slots == 1 {
    ("service_cards.slots.title.one", "service_cards.slots.body.one")
  } else {
    ("service_cards.slots.title.other", "service_cards.slots.body.other")
  };
  let said = ctx.t_args(
    said,
    [
      ("count", slots.to_string()),
      ("share", share.to_string()),
      ("limit", limit.to_string()),
    ],
  );
  let step = timeline.step_of(ctx, Some(entry.execution)).unwrap_or_default();
  let meta = ctx.t_args(
    "service_cards.slots.meta",
    [
      ("task", timeline.key.to_string()),
      ("step", step.to_string()),
      ("time", display::clock(entry.at)),
    ],
  );
  // The computer's slots, the ones this start needs first: the note does not say which are held.
  let bar: Vec<Slot> = (0..limit.max(slots))
    .map(|at| if at < slots { Slot::Taken } else { Slot::Free })
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
      &ctx.t_args(title, count),
      (&meta, Meta::AfterTitle),
    ))
    .child(body(&format!("{id}-body"), &said))
    .child(slot_bar(&format!("{id}-bar"), &bar, Dimension::Pct(100.0)))
    .child(change)
    .into()
}
