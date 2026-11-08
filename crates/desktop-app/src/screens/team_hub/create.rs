//! The head's New (`+ New` in `Header` `NRqwv`, A1), for an admin of the team: under a kind's filter it opens that
//! kind's form in the panel (A7–A12: [`panel`]), under All a menu of the kinds. A form has only Create and Cancel: no
//! draft, Create makes v1 (`docs/design/team-configuration.md`, decision 6).

mod fields;
mod instruction;
mod kinds;
mod mcp;
mod notices;
mod panel;
mod profile;
mod skill;
mod tool;
mod work;

use lurq::{
  app::ctx::Ctx,
  components::Row,
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

pub(super) use self::{
  kinds::new_row,
  mcp::{EditView, Fields as McpFields},
  panel::{Props, panel},
};
use super::model::Kind;
use crate::{
  navigation::Navigation,
  routes::{HubFilter, HubView, ProjectPage, Route},
  theme::HUB_NEW_MENU_WIDTH,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu_at_end, menu_panel},
    menu_button::MenuButton,
  },
};

/// The filters whose kind a form creates, in the order of the All menu.
const CREATABLE: [HubFilter; 5] = [
  HubFilter::Profiles,
  HubFilter::McpServers,
  HubFilter::Instructions,
  HubFilter::Skills,
  HubFilter::Tools,
];

/// The hub that shows the form of the kind `filter` holds.
fn form_route(project: ProjectId, filter: HubFilter) -> Route {
  Route::Project(project, ProjectPage::hub(HubView::adding(filter)))
}

const fn label_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::Profiles => "team_hub.new.profiles",
    HubFilter::McpServers => "team_new.new.mcpServers",
    HubFilter::Instructions => "team_hub.new.instructions",
    HubFilter::Skills => "team_hub.new.skills",
    HubFilter::Tools => "team_hub.new.tools",
    HubFilter::All | HubFilter::Archived => "team_hub.new.all",
  }
}

/// The menu All's New opens, and where it stands.
pub(super) struct NewMenu {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
}

/// New for `filter`: its form, or under All a menu of the kinds.
pub(super) fn new_button(
  ctx: &mut Ctx,
  (project, filter): (ProjectId, HubFilter),
  (navigation, menu): (&Navigation, &NewMenu),
) -> Option<Element> {
  if filter == HubFilter::All {
    return Some(all(ctx, project, navigation, menu));
  }
  Some(
    ActionButton::new("team-hub-new", ButtonKind::Primary, ctx.t(label_key(filter)))
      .icon(Icon::Plus)
      .opens(navigation.to(form_route(project, filter)))
      .into(),
  )
}

fn all(ctx: &mut Ctx, project: ProjectId, navigation: &Navigation, menu: &NewMenu) -> Element {
  let mut panel = menu_panel("team-hub-new-menu").width(Dimension::Px(HUB_NEW_MENU_WIDTH));
  for filter in CREATABLE {
    let Some(kind) = Kind::of_filter(filter) else { continue };
    panel = panel.child(
      MenuItem::opens(
        format!("team-hub-new-{}", filter.name()),
        ctx.t(kind.where_key()),
        navigation.to(form_route(project, filter)),
      )
      .render(ctx, &menu.open),
    );
  }
  Row::new()
    .align_items(Alignment::Center)
    .child(MenuButton::new(
      "team-hub-new",
      (Icon::Plus, ctx.t(label_key(HubFilter::All))),
      menu.open.clone(),
      menu.anchor.clone(),
    ))
    .child(menu_at_end(menu.anchor.clone(), menu.open.clone(), panel))
    .into()
}
