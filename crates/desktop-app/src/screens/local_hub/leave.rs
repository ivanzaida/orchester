//! Leaving a local MCP server's panel while its edits are not saved (`Agents · MCP server · editing` L8): the panel
//! would be gone with them, and a typed secret value with it, so the hub asks first, in a dialog over the page. A
//! profile's, an instruction's and a skill's panel ask the same while what they hold differs from what is stored. Every
//! way out of the panel asks the same question: the hub's own presses through [`Leave`] (another row, a filter, *New*,
//! *Share with a team…*, *Memory*, *Import*), and every other departure through the navigation's [`LeaveGuard`] (Back
//! and Forward, a tab switched or closed, the sidebar, a link). *Keep editing* leaves everything as it is; *Discard
//! changes* puts back what was saved and goes where the person pressed.

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::local_archive::Archivable;

use super::{mcp::UnsavedSignal, model::Listing, words::Unsaved};
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  routes::Route,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone},
    icon::Icon,
    pressable::Press,
  },
};

/// Where a press that leaves the panel goes: straight to its place, or, while a server's edits are not saved, to the
/// question first.
#[derive(Clone)]
pub(super) struct Leave {
  navigation: Navigation,
  /// Where a press waits for the person's answer.
  leaving: Signal<Option<Route>>,
  /// Whether a server's edits are not saved.
  unsaved: bool,
}

/// Two leaves are alike while the question applies the same way: the navigation and the signal are the screen's own,
/// the same for as long as it lives.
impl PartialEq for Leave {
  fn eq(&self, other: &Self) -> bool {
    self.unsaved == other.unsaved
  }
}

impl Leave {
  pub(super) fn new(navigation: &Navigation, leaving: &Signal<Option<Route>>, unsaved: bool) -> Self {
    Self {
      navigation: navigation.clone(),
      leaving: leaving.clone(),
      unsaved,
    }
  }

  /// A press that opens `route`, after the question while edits are not saved.
  pub(super) fn press(&self, route: Route) -> Press {
    if self.unsaved {
      let leaving = self.leaving.clone();
      Press::run(move || leaving.set(Some(route.clone())))
    } else {
      Press::Open(self.navigation.to(route))
    }
  }

  /// Opens `route` now, or asks first while edits are not saved.
  pub(super) fn go(&self, route: Route) {
    if self.unsaved {
      self.leaving.set(Some(route));
    } else {
      self.navigation.go(route);
    }
  }
}

/// Tells the list that `what` has edits that are not saved (`changed`), and whether a save was refused; set only when
/// it differs so the page is not read again for nothing.
pub(super) fn mark_unsaved(unsaved: &UnsavedSignal, what: Archivable, (changed, refused): (bool, bool)) {
  let mark = match (changed, refused) {
    (true, true) => Some(Unsaved::Refused),
    (true, false) => Some(Unsaved::Edited),
    (false, _) => None,
  };
  let wanted = mark.map(|mark| (what, mark));
  if unsaved.get_untracked() != wanted {
    unsaved.set(wanted);
  }
}

/// The panel of `what` is gone, with its edits: the list must not go on saying they are there.
pub(super) fn clear_unsaved(unsaved: &UnsavedSignal, what: Archivable) {
  if unsaved.get_untracked().is_some_and(|(marked, _)| marked == what) {
    unsaved.set(None);
  }
}

/// The question while a press or a departure waits, naming the server whose edits it would drop. A press of the hub's
/// own waits in `leaving`; any other departure waits in the `guard`.
pub(super) fn question(
  ctx: &Ctx,
  (leaving, open): (&Signal<Option<Route>>, &Signal<bool>),
  (listing, unsaved): (&Listing, &UnsavedSignal),
  (navigation, guard): (&Navigation, &LeaveGuard),
) -> Option<Element> {
  let route = leaving.get();
  if route.is_none() && !guard.requested() {
    return None;
  }
  let (what, _) = unsaved.get()?;
  let name = listing.name_of(what)?;
  let discard = {
    let (unsaved, open, navigation, guard) = (unsaved.clone(), open.clone(), navigation.clone(), guard.clone());
    ActionButton::new(
      "local-hub-leave-discard",
      ButtonKind::Danger,
      ctx.t("local_hub.leave.discard"),
    )
    .icon(Icon::X)
    .on_click(move || {
      unsaved.set(None);
      match &route {
        Some(route) => navigation.go(route.clone()),
        None => guard.leave(&navigation),
      }
      open.set(false);
    })
  };
  Some(
    Dialog::new(
      "local-hub-leave",
      Icon::Pencil,
      ctx.t_args("local_hub.leave.title", [("name", name.to_owned())]),
      ctx.t("local_hub.leave.keep"),
    )
    .tone(Tone::Neutral)
    .what(ctx.t_args("local_hub.leave.what", [("name", name.to_owned())]))
    .action(discard)
    .render(ctx, open),
  )
}
