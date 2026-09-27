//! Project agent profiles (`ORWrW`): New agent, one item per profile (`jNTVN`) with its
//! connection and model, and Import agents, which is not available yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, ScrollVertical, Text, TextOverflow},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::id::ResourceId;

use super::super::{ProjectNav, page::TOPBAR_BUTTON_HEIGHT};
use crate::{
  agents::data::{ProfileEntry, ProjectAgents},
  navigation::{
    import::ImportTab,
    route::{AgentSelection, Page},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::Icon,
    pressable::{pressable, surface},
  },
};

/// List and item sizes (`ORWrW`, `mmKpE`).
const LIST_WIDTH: f32 = 264.0;
const ITEM_HEIGHT: f32 = 70.0;

/// What the editor shows: a stored profile, a new one not saved yet, or nothing.
#[derive(Clone, Copy)]
pub enum ListSelection {
  None,
  Profile(ResourceId),
  New,
}

pub fn profile_list(ctx: &Ctx, agents: &ProjectAgents, selection: ListSelection, nav: &ProjectNav) -> Element {
  let new_agent = {
    let button = ActionButton::new(ctx.t("agents.new"))
      .icon(Icon::Plus)
      .id("new-agent")
      .fill_width()
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    let nav = nav.clone();
    if agents.connections.is_empty() {
      // A profile needs a connection; the empty state says so and links to Connections.
      button.on_click(|| {}).enabled(false)
    } else {
      button.on_click(move || nav.show(Page::Agents(AgentSelection::New)))
    }
  };
  let selected = match selection {
    ListSelection::Profile(id) => Some(id),
    ListSelection::None | ListSelection::New => None,
  };
  let mut items: Vec<Element> = agents
    .profiles
    .iter()
    .map(|entry| item(ctx, agents, entry, Some(entry.id) == selected, nav))
    .collect();
  if matches!(selection, ListSelection::New) {
    let placeholder = frame(&ctx.t("agents.newTitle"), &ctx.t("agents.notSaved"), true)
      .id("agent-new")
      .describe("state", "selected");
    items.push(placeholder.into());
  }
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
        .on_click({
          let nav = nav.clone();
          move || nav.show_import(ImportTab::Agents)
        })
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
  let item = frame(profile.role().as_str(), &metadata, selected).id(Arc::<str>::from(format!("agent-{}", entry.id)));
  if selected {
    return item.describe("state", "selected").into();
  }
  let (nav, id) = (nav.clone(), entry.id);
  pressable(item, move || nav.show(Page::Agents(AgentSelection::Profile(id))))
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .into()
}

/// A list item (`jNTVN`): title over metadata. Pointer-only, as the list is outside the form.
fn frame(title: &str, metadata: &str, selected: bool) -> Button {
  let text = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(Text::new(title).variant(ExtraTypography::FormLabel.role()).nowrap())
    .child(
      Text::new(metadata)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  surface()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(ExtraSpacing::Stack.role())
    .rounded(RadiusSize::Md)
    .background(if selected {
      PaletteColor::AccentMuted
    } else {
      PaletteColor::SurfacePanel
    })
    .child(text)
}
