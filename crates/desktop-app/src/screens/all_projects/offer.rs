//! The team project offered above the list (`Team strip` `ZyS3T` in 09 `i5G19A`): the team's icon, *Acme shares
//! Billing with you. It isn't on this computer yet.*, *Open Billing…*, which links a checkout of it on the invite
//! landing, and *Not now*, which puts it off for good. 16 at its sides and 12 over and under, 12 between its parts, on
//! `surface_raised` inside a `border` stroke, radius 10.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::sharing::OfferedProject;

use super::teams::Answer;
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, OrchesterView, Route},
  theme::{IconSize, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

pub(super) fn strip(ctx: &mut Ctx, offer: &OfferedProject, answer: &Answer) -> Element {
  let project = offer.project.name.as_str();
  let text = ctx.t_args(
    "projects.header.offer.text",
    [("team", offer.team.as_str()), ("project", project)],
  );
  // A server names every project by a slug; one without would have no landing, so its server's page lists it.
  let place = JoinTarget::new(offer.server, &offer.project.slug)
    .map_or_else(|| Route::Orchester(OrchesterView::server(offer.server)), Route::Invite);
  let open = ActionButton::new(
    "all-projects-offer-open",
    ButtonKind::Secondary,
    ctx.t_args("projects.header.offer.open", [("project", project)]),
  )
  .icon(Icon::ArrowRight)
  .opens(Navigation::of(ctx).to(place));
  let (answer, offered) = (answer.clone(), offer.clone());
  let not_now = ActionButton::new(
    "all-projects-offer-not-now",
    ButtonKind::Ghost,
    ctx.t("projects.header.offer.notNow"),
  )
  .on_click(move || answer.not_now(&offered));
  Row::new()
    .id("all-projects-offer")
    .describe("project", project.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(icon(Icon::Users, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&text)
        .id("all-projects-offer-text")
        .variant(ProjectsText::Offer.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(open)
    .child(not_now)
    .into()
}
