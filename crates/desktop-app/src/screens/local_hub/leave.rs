//! Opening another entry while a local MCP server's edits are not saved (`Agents · MCP server · editing` L8): the panel
//! would be gone with them, so the hub asks first, in a dialog over the page. *Keep editing* leaves everything as it
//! is; *Discard changes* puts back what was saved and opens the entry.

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::local_archive::Archivable;

use super::{model::Listing, words::Unsaved};
use crate::{
  navigation::Navigation,
  routes::Route,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone},
    icon::Icon,
  },
};

/// The question while a press on a row waits, naming the server whose edits it would drop.
pub(super) fn question(
  ctx: &Ctx,
  (leaving, open): (&Signal<Option<Route>>, &Signal<bool>),
  (listing, unsaved): (&Listing, &Signal<Option<(Archivable, Unsaved)>>),
  navigation: &Navigation,
) -> Option<Element> {
  let route = leaving.get()?;
  let (what, _) = unsaved.get()?;
  let name = listing.name_of(what)?;
  let discard = {
    let (unsaved, open, navigation) = (unsaved.clone(), open.clone(), navigation.clone());
    ActionButton::new(
      "local-hub-leave-discard",
      ButtonKind::Danger,
      ctx.t("local_hub.leave.discard"),
    )
    .icon(Icon::X)
    .on_click(move || {
      unsaved.set(None);
      open.set(false);
      navigation.go(route.clone());
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
