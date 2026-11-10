//! The Agents hub's *New* (`+ New` in `Header` `NRqwv`, A1; L2): the same primary button in a team and a local project,
//! saying the kind the open tab holds (*New skill* under Skills) and opening that kind's form; All and Archived hold no
//! one kind, so there it makes an agent profile. It is always as wide as its longest words, so switching tabs moves
//! nothing in the head.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};

use crate::{
  routes::HubFilter,
  ui::{
    button::{ActionButton, ButtonKind, as_wide_as_widest},
    icon::Icon,
    pressable::Press,
  },
};

/// The kinds a hub makes, each with its button's words.
const KINDS: [(HubFilter, &str); 5] = [
  (HubFilter::Profiles, "hub.new.profiles"),
  (HubFilter::McpServers, "hub.new.mcpServers"),
  (HubFilter::Instructions, "hub.new.instructions"),
  (HubFilter::Skills, "hub.new.skills"),
  (HubFilter::Tools, "hub.new.tools"),
];

/// The kind *New* makes under `filter`: its own, or a profile under All and Archived.
#[must_use]
pub(crate) const fn made_under(filter: HubFilter) -> HubFilter {
  match filter {
    HubFilter::All | HubFilter::Archived => HubFilter::Profiles,
    kind => kind,
  }
}

/// The words of *New <kind>* under `filter`: of the kind [`made_under`] it.
#[must_use]
pub(crate) fn new_words(filter: HubFilter) -> &'static str {
  let made = made_under(filter);
  KINDS
    .iter()
    .find(|(kind, _)| *kind == made)
    .map_or("hub.new.profiles", |(_, key)| key)
}

/// *New <kind>* for `filter`, its element id `id`, which `press` answers by opening the form of [`made_under`].
pub(crate) fn new_button(ctx: &Ctx, id: &str, filter: HubFilter, press: Press) -> Element {
  let words = |key: &str| -> Arc<str> { ctx.t(key) };
  let button = ActionButton::new(id.to_owned(), ButtonKind::Primary, words(new_words(filter)))
    .icon(Icon::Plus)
    .press(press);
  let sizes = KINDS.iter().map(|(kind, key)| {
    ActionButton::new(format!("{id}-size-{}", kind.name()), ButtonKind::Primary, words(key)).icon(Icon::Plus)
  });
  as_wide_as_widest(&format!("{id}-slot"), button, sizes).into()
}
