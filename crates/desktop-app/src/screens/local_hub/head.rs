//! What a local project puts in the hub's head (`Header` in L2): at its right, that the project is only on this
//! computer. The head itself is the frame every hub shares ([`crate::screens::hub::agents`]).

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};

use crate::{
  theme::{ExtraSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// That the project is local.
pub(in crate::screens) fn status(ctx: &Ctx) -> Vec<Element> {
  let local = Row::new()
    .id("local-hub-local")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Laptop, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("local_hub.local"))
        .id("local-hub-local-label")
        .variant(HubText::Hint.role())
        .nowrap(),
    );
  vec![local.into()]
}
