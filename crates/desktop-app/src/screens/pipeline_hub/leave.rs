//! Leaving the panel of a pipeline while its edit is not saved: the edit would be gone, so the hub asks first, in a
//! dialog over the page, as the Agents hub does ([`crate::screens::local_hub::leave`]). Every way out of the panel asks
//! the same question: the hub's own presses through that module's `Leave` (another pipeline, a filter, *New pipeline*),
//! and every other departure through the navigation's [`LeaveGuard`] (Back and Forward, a tab switched or closed, the
//! sidebar, a link, *Edit steps*, a card, the ⋯ menu's *Duplicate*). The ⋯ menu's *Archive* asks it too, before the
//! archive confirmation: an archived pipeline has no panel to keep the edit in. *Keep editing* leaves everything as it
//! is; *Discard changes* drops the edit and goes where the person pressed.

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
  screens::local_hub::archive::{Asking, Target},
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

/// What the screen hosts for the panel of the chosen pipeline: where the list hears of its changes, and the
/// confirmations that outlive the panel.
#[derive(Clone)]
pub(super) struct Hosted {
  pub unsaved: UnsavedSignal,
  /// The archive confirmation over the page.
  pub archiving: Asking,
  /// The pipeline whose *Archive* waits for the question about its unsaved changes.
  pub archive_waits: Signal<Option<Target>>,
  /// Whether the panel's save is under way: its changes are being stored, so they cannot be discarded.
  pub saving: Signal<bool>,
  /// The pipeline a save in the panel just published, which the page follows out of the *Drafts* tab.
  pub published: Signal<Option<ResourceId>>,
}

impl PartialEq for Hosted {
  fn eq(&self, other: &Self) -> bool {
    self.archiving == other.archiving
  }
}

impl Hosted {
  /// *Archive* on `target`: the archive confirmation, after the question about the panel's changes while it `holds`
  /// some.
  pub(super) fn archive(&self, target: Target, holds: bool) {
    if holds {
      self.archive_waits.set(Some(target));
    } else {
      self.archiving.ask(target);
    }
  }
}

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

/// The question while a save is under way: the changes are being stored, so there is nothing to discard and the press
/// waits; it closes by itself when the save ends, and the person presses again.
fn saving_question(ctx: &Ctx, name: &str, open: &Signal<bool>) -> Element {
  Dialog::new(
    "pipeline-hub-leave-saving",
    Icon::Save,
    ctx.t_args("pipeline_hub.leave.savingTitle", [("name", name.to_owned())]),
    ctx.t("pipeline_hub.leave.wait"),
  )
  .tone(Tone::Neutral)
  .what(ctx.t_args("pipeline_hub.leave.savingWhat", [("name", name.to_owned())]))
  .render(ctx, open)
}

/// Whether the departure the `guard` holds closes the tab the panel is in.
fn closes_tab(navigation: &Navigation, guard: &LeaveGuard) -> bool {
  let shown = navigation.tabs_now().active().id();
  guard
    .attempt()
    .get_untracked()
    .is_some_and(|tabs| !tabs.tabs().iter().any(|tab| tab.id() == shown))
}

/// The question while a press or a departure waits, naming the pipeline whose edit it would drop; a tab closed with
/// the panel in it is said to close. A press of the hub's
/// own waits in `leaving`; any other departure waits in the `guard`.
pub(super) fn question(
  ctx: &Ctx,
  (leaving, open): (&Signal<Option<Route>>, &Signal<bool>),
  (listing, hosted): (&Listing, &Hosted),
  (navigation, guard): (&Navigation, &LeaveGuard),
) -> Option<Element> {
  let route = leaving.get();
  let archiving = hosted.archive_waits.get();
  if route.is_none() && archiving.is_none() && !guard.requested() {
    return None;
  }
  let closing = route.is_none() && archiving.is_none() && closes_tab(navigation, guard);
  let name = listing.name_of(hosted.unsaved.get().0?)?.to_owned();
  if hosted.saving.get() {
    return Some(saving_question(ctx, &name, open));
  }
  let discard = {
    let (hosted, open, navigation, guard) = (hosted.clone(), open.clone(), navigation.clone(), guard.clone());
    ActionButton::new(
      "pipeline-hub-leave-discard",
      ButtonKind::Danger,
      ctx.t("pipeline_hub.leave.discard"),
    )
    .icon(Icon::X)
    .on_click(move || {
      hosted.unsaved.set(Unsaved(None));
      match (&route, &archiving) {
        (Some(route), _) => navigation.go(route.clone()),
        (None, Some(target)) => hosted.archiving.ask(target.clone()),
        (None, None) => guard.leave(&navigation),
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
    .what(ctx.t_args(
      if closing {
        "pipeline_hub.leave.whatClose"
      } else {
        "pipeline_hub.leave.what"
      },
      [("name", name)],
    ))
    .action(discard)
    .render(ctx, open),
  )
}
