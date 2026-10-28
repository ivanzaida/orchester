//! The side of a profile's page (`Side` `y8gUZ` in 01c): what enforces its permissions, what else reaches its prompts,
//! where it is used and the runs using it now. A new profile has only the first.
use lurq::{app::ctx::Ctx, components::Column, node::Element};
use orchester_core::name::Name;

use super::{active, editor::Props, fields::Fields, policy_panel, prompts, uses};
use crate::theme::HubSpacing;

pub fn render(ctx: &mut Ctx, props: &Props, fields: &Fields, blocked: Option<&'static str>) -> Element {
  let mut side = Column::new()
    .spacing(HubSpacing::SideSections.role())
    .child(policy_panel::enforcement(ctx, fields, &props.listed));
  if let Some(entry) = &props.entry {
    let role = Name::new(&fields.role.get()).unwrap_or_else(|_| entry.profile.role().clone());
    side = side
      .child(prompts::render(ctx, &props.listed, (entry.id, &role)))
      .child(uses::render(ctx, props, blocked))
      .child(active::render(ctx, props));
  }
  side.into()
}
