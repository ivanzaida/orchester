//! Leaving the panel of a pipeline while its edit is not saved: the edit would be gone, so the hub asks first, in a
//! dialog over the page, as the Agents hub does ([`crate::screens::local_hub::leave`]). Every way out of the panel asks
//! the same question: the hub's own presses through that module's `Leave` (another pipeline, a filter, *New pipeline*),
//! and every other departure through the navigation's [`LeaveGuard`] (Back and Forward, a tab switched or closed, the
//! sidebar, a link, *Edit steps*, a card). *Keep editing* leaves everything as it is; *Discard changes* drops the edit
//! and goes where the person pressed.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
  node::Element,
};
use orchester_core::id::ResourceId;

use super::model::Listing;
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  routes::Route,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone},
    icon::Icon,
  },
};

/// The pipeline whose panel has changes that are not saved, which its row says: read by the list, written by the panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Unsaved(pub Option<ResourceId>);
impl DevtoolsInspectable for Unsaved {}

pub(super) type UnsavedSignal = Signal<Unsaved>;

/// Tells the list that `pipeline` has changes that are not saved (`changed`); set only when it differs so the page is
/// not read again for nothing.
pub(super) fn mark_unsaved(unsaved: &UnsavedSignal, pipeline: ResourceId, changed: bool) {
  let wanted = Unsaved(changed.then_some(pipeline));
  if unsaved.get_untracked() != wanted {
    unsaved.set(wanted);
  }
}

/// The panel of `pipeline` is gone, with its edit: the list must not go on saying it is there.
pub(super) fn clear_unsaved(unsaved: &UnsavedSignal, pipeline: ResourceId) {
  if unsaved.get_untracked().0 == Some(pipeline) {
    unsaved.set(Unsaved(None));
  }
}

/// The question while a press or a departure waits, naming the pipeline whose edit it would drop. A press of the hub's
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
  let name = listing.name_of(unsaved.get().0?)?.to_owned();
  let discard = {
    let (unsaved, open, navigation, guard) = (unsaved.clone(), open.clone(), navigation.clone(), guard.clone());
    ActionButton::new(
      "pipeline-hub-leave-discard",
      ButtonKind::Danger,
      ctx.t("pipeline_hub.leave.discard"),
    )
    .icon(Icon::X)
    .on_click(move || {
      unsaved.set(Unsaved(None));
      match &route {
        Some(route) => navigation.go(route.clone()),
        None => guard.leave(&navigation),
      }
      open.set(false);
    })
  };
  Some(
    Dialog::new(
      "pipeline-hub-leave",
      Icon::Pencil,
      ctx.t_args("pipeline_hub.leave.title", [("name", name.clone())]),
      ctx.t("pipeline_hub.leave.keep"),
    )
    .tone(Tone::Neutral)
    .what(ctx.t_args("pipeline_hub.leave.what", [("name", name)]))
    .action(discard)
    .render(ctx, open),
  )
}
