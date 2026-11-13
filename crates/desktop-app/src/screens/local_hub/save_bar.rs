//! The pinned bar of a local project's panels that edit what they show (an MCP server's, a profile's, an instruction's,
//! a skill's): *Save* and *Cancel*, always there and always in the same place, from the first frame. Changing a field
//! only changes what the panel holds; *Save* stores it (for a resource with versions, as its next version at once) and
//! *Cancel* puts back what is stored. Both are off while nothing differs; *Save* says why it waits while what the panel
//! holds cannot be saved.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::resource::VersionNumber;

use crate::{
  screens::team_hub::bar::bar_saying,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
    tooltip::Tooltip,
  },
};

/// What *Save* can do now.
pub(in crate::screens) enum Save {
  /// Nothing differs from what is stored.
  Unchanged,
  /// What the panel holds cannot be saved, and why.
  Waits(Arc<str>),
  /// It saves what the panel holds.
  Ready(Press),
  /// A save is under way.
  Saving,
}

/// The bar: *Save*, then *Cancel* (`None` while nothing differs, or a save is under way), and `hint` at its end. The
/// buttons' ids start with `prefix`. A *Save* that waits says why in its tooltip, at the button.
pub(in crate::screens) fn bar(
  ctx: &mut Ctx,
  prefix: &str,
  (save, cancel): (Save, Option<Press>),
  hint: (String, PaletteColor),
) -> Element {
  let saving = ActionButton::new(
    format!("{prefix}-save"),
    ButtonKind::Primary,
    ctx.t("local_hub.mcp.save"),
  )
  .icon(Icon::Save)
  .pinned();
  let saving: Element = match save {
    Save::Unchanged => saving.disabled().into(),
    Save::Waits(why) => Tooltip::new(format!("{prefix}-save-why"), why.clone()).wrap(ctx, saving.unavailable(why)),
    Save::Ready(press) => saving.press(press).into(),
    Save::Saving => saving.loading().into(),
  };
  let cancelling = ActionButton::new(
    format!("{prefix}-cancel"),
    ButtonKind::Secondary,
    ctx.t("local_hub.mcp.cancel"),
  )
  .icon(Icon::X)
  // As tall as *Save*: the bar draws both 30 (`Save` `aOTTS`, `Cancel` `sNCRP` in P1).
  .pinned();
  let cancelling = match cancel {
    Some(press) => cancelling.press(press),
    None => cancelling.disabled(),
  };
  bar_saying(vec![saving, cancelling.into()], Some(hint))
}

/// What the bar of a resource with versions says at its end: *Save* makes the version after `published`.
pub(in crate::screens) fn makes(ctx: &Ctx, published: Option<VersionNumber>) -> (String, PaletteColor) {
  let next = published
    .and_then(VersionNumber::next)
    .map_or_else(String::new, |next| next.to_string());
  (
    ctx.t_args("local_hub.edit.makes", [("version", next)]).to_string(),
    PaletteColor::TextMuted,
  )
}
