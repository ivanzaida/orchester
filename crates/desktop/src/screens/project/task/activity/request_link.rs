//! *Show the request →* under a problem row that names a tool request (a request that ended without
//! the person's decision): pressing it brings the request's card into the chat and scrolls to it, also
//! when *Tools*, a search or a folded step hides the card. Offered while the run's timeline has the card.
//! *Open Secrets ↗* under a question for a secret that was refused (C·17 `UpHyY`, state H): the project's
//! Secrets, where the value is kept and granted to the run.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::Text,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::{
  navigation::route::Page,
  screens::project::ProjectNav,
  ui::pressable::{pressable, surface},
};

/// The link's hit area: as high as a quiet text action (`feLwP`), though it reads as a text link.
const LINK_HEIGHT: f32 = 22.0;

/// The card of the request a problem row names, and the row of the step whose items hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::project::task) struct RequestTarget {
  pub(in crate::screens::project::task) card: Arc<str>,
  pub(in crate::screens::project::task) step: Option<Arc<str>>,
}

/// Shows a target's card in the chat and scrolls to it.
pub(in crate::screens::project::task) type RevealRow = Arc<dyn Fn(&RequestTarget) + Send + Sync>;

/// What a problem row's link reveals, and how.
#[derive(Clone)]
pub(in crate::screens::project::task) struct RequestLink {
  pub(in crate::screens::project::task) target: RequestTarget,
  pub(in crate::screens::project::task) reveal: RevealRow,
}

/// *Open Secrets ↗* under the problem row `sequence`.
pub(super) fn secrets_link(ctx: &Ctx, nav: &ProjectNav, sequence: u64) -> Element {
  nav
    .link(Page::SECRETS)
    .on(surface().id(format!("activity-open-secrets-{sequence}")))
    .height(Dimension::Px(LINK_HEIGHT))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("runs.activity.problem.openSecrets")).variant(TypographyStyle::Link))
    .into()
}

/// The link of the problem row `sequence`.
pub(super) fn request_link(ctx: &Ctx, link: &RequestLink, sequence: u64) -> Element {
  let (reveal, target) = (link.reveal.clone(), link.target.clone());
  pressable(surface().id(format!("activity-show-request-{sequence}")), move || {
    reveal(&target);
  })
  .height(Dimension::Px(LINK_HEIGHT))
  .align_items(Alignment::Center)
  .child(Text::new(&ctx.t("runs.activity.problem.showRequest")).variant(TypographyStyle::Link))
  .into()
}
