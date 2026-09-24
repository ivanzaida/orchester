//! Project agent profiles (`ORWrW`): New agent, one item per profile (`jNTVN`) with its
//! connection and model, and Import agents, which is not available yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, ScrollVertical, Text, TextOverflow},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::id::ResourceId;

use super::super::{AgentSelection, ProjectNav, page::TOPBAR_BUTTON_HEIGHT};
use crate::{
  agents::data::{ProfileEntry, ProjectAgents},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon},
};

/// List and item sizes (`ORWrW`, `mmKpE`).
const LIST_WIDTH: f32 = 264.0;
const ITEM_HEIGHT: f32 = 70.0;

/// `selected` is the profile in the editor, if it is a stored one.
pub fn profile_list(ctx: &Ctx, agents: &ProjectAgents, selected: Option<ResourceId>, nav: &ProjectNav) -> Element {
  let new_agent = {
    let button = ActionButton::new(ctx.t("agents.new"))
      .icon(Icon::Plus)
      .id("new-agent")
      .fill_width()
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    let agent = nav.agent.clone();
    if agents.connections.is_empty() {
      // A profile needs a connection; the empty state says so and links to Connections.
      button.on_click(|| {}).enabled(false)
    } else {
      button.on_click(move || agent.set(AgentSelection::New))
    }
  };
  let items = agents
    .profiles
    .iter()
    .map(|entry| item(ctx, agents, entry, Some(entry.id) == selected, nav));
  Column::new()
    .id("agent-profiles")
    .width(Dimension::Px(LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfacePanel)
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(new_agent.render(ctx))
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Stack.role())
          .with_children(items),
      )
      .width(Dimension::Pct(100.0))
      .flex(1.0),
    )
    .child(
      ActionButton::new(ctx.t("agents.import"))
        .icon(Icon::Download)
        .id("import-agents")
        .fill_width()
        .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
        .render(ctx),
    )
    .into()
}

/// A profile (`jNTVN` as in `mmKpE`): its role over its connection and model.
fn item(ctx: &Ctx, agents: &ProjectAgents, entry: &ProfileEntry, selected: bool, nav: &ProjectNav) -> Element {
  let profile = &entry.profile;
  let connection = agents
    .connections
    .iter()
    .find(|connection| connection.id() == profile.connection())
    .map_or_else(
      || ctx.t("agents.connectionMissing"),
      |connection| connection.name().as_str().into(),
    );
  let metadata = ctx.t_args(
    "agents.assignment",
    [("connection", &*connection), ("model", profile.model().as_str())],
  );
  let column = Column::new()
    .id(Arc::<str>::from(format!("agent-{}", entry.id)))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Md)
    .background(if selected {
      PaletteColor::AccentMuted
    } else {
      PaletteColor::SurfacePanel
    })
    .child(
      Text::new(profile.role().as_str())
        .variant(ExtraTypography::FormLabel.role())
        .nowrap(),
    )
    .child(
      Text::new(&metadata)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  if selected {
    return column.describe("state", "selected").into();
  }
  let (agent, id) = (nav.agent.clone(), entry.id);
  column
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .on_click(move |_| agent.set(AgentSelection::Profile(id)))
    .into()
}
