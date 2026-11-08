//! An editor hosted in the Agents hub's panel (`Agents · v2` L1, `docs/design/team-configuration.md` rule 11): in a
//! local project the profile, tool, skill and instruction editors are the panel of the hub beside its list, not a page
//! of their own. The hub provides [`Hosted`] around the editor it mounts; the editor's frame, its crumb and its body
//! read it to draw the panel's way: no title, tabs or New of the old page, the side column under the pane instead of
//! beside it, and no scroll or padding of their own, which the hub's panel gives all of them.

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};

use crate::theme::HubSpacing;

/// Provided by the hub around an editor it hosts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Hosted;

impl Hosted {
  /// Whether the editor drawn with `ctx` is the hub's panel.
  pub(crate) fn is(ctx: &mut Ctx) -> bool {
    ctx.use_context::<Self>().is_some()
  }
}

/// The hosted editor's body: `pane` over `side`, one column of the panel (`Detail` in L1).
pub(crate) fn body(id: &str, pane: Element, side: Option<Element>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Sections.role())
    .child(
      Column::new()
        .id(format!("{id}-pane"))
        .width(Dimension::Pct(100.0))
        .child(pane),
    )
    .with_children(side.map(|side| {
      Column::new()
        .id(format!("{id}-side"))
        .width(Dimension::Pct(100.0))
        .spacing(HubSpacing::SideSections.role())
        .child(side)
    }))
    .into()
}

/// The hosted editor's frame: its save state, when it has one, over its body; the old page's title, tabs and actions
/// are the hub's.
pub(super) fn page(saved: Option<Element>, body: Element) -> Element {
  Column::new()
    .id("hub-hosted")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Row.role())
    .with_children(saved)
    .child(body)
    .into()
}
