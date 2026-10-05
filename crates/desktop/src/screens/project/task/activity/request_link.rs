//! *Show the request →* under a problem row that names a tool request (a request that ended without
//! the person's decision): pressing it scrolls the chat to that request's card. Offered only while the
//! chat shows the card, so it never does nothing.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::Text,
  node::Element,
};

use crate::ui::pressable::{pressable, surface};

/// Scrolls the chat to the row with the given key.
pub(in crate::screens::project::task) type RevealRow = Arc<dyn Fn(Arc<str>) + Send + Sync>;

/// The card row a problem row's link reveals, and how.
#[derive(Clone)]
pub(in crate::screens::project::task) struct RequestLink {
  pub(in crate::screens::project::task) card: Arc<str>,
  pub(in crate::screens::project::task) reveal: RevealRow,
}

/// The link of the problem row `sequence`.
pub(super) fn request_link(ctx: &Ctx, link: &RequestLink, sequence: u64) -> Element {
  let (reveal, card) = (link.reveal.clone(), link.card.clone());
  pressable(surface().id(format!("activity-show-request-{sequence}")), move || {
    reveal(card.clone());
  })
  .child(Text::new(&ctx.t("runs.activity.problem.showRequest")).variant(TypographyStyle::Link))
  .into()
}
