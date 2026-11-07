//! The head's New (`+ New` in `Header` `NRqwv`, A1), for an admin of the team: under a kind's filter it opens the page
//! that creates that kind, under All a menu of the kinds that have one. A kind with no create page yet (an MCP server,
//! ORC-835) has no New, and nothing disabled in its place.

use lurq::{
  app::ctx::Ctx,
  components::{Placement, Popup, Row},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::Element,
};
use orchester_core::id::ProjectId;

use crate::{
  navigation::Navigation,
  routes::{AgentTab, HubFilter, InstructionsTab, InstructionsView, ProjectPage, Route},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu_panel},
    menu_button::MenuButton,
  },
};

/// The filters whose kind has a create page, in the order of the All menu.
const CREATABLE: [HubFilter; 4] = [
  HubFilter::Profiles,
  HubFilter::Instructions,
  HubFilter::Skills,
  HubFilter::Tools,
];

/// The page that creates the kind `filter` holds; `None` under All and for a kind with no create page.
const fn page(filter: HubFilter) -> Option<ProjectPage> {
  match filter {
    HubFilter::Profiles => Some(ProjectPage::NewAgent(AgentTab::Profile)),
    HubFilter::Instructions => Some(ProjectPage::Instructions(InstructionsView::new_rule(
      InstructionsTab::Project,
    ))),
    HubFilter::Skills => Some(ProjectPage::NewSkill),
    HubFilter::Tools => Some(ProjectPage::NewTool),
    HubFilter::All | HubFilter::McpServers => None,
  }
}

const fn label_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::Profiles => "team_hub.new.profiles",
    HubFilter::Instructions => "team_hub.new.instructions",
    HubFilter::Skills => "team_hub.new.skills",
    HubFilter::Tools => "team_hub.new.tools",
    HubFilter::All | HubFilter::McpServers => "team_hub.new.all",
  }
}

/// A menu row's words: what the kind is, as the panel names it.
const fn kind_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::Profiles => "team_hub.kind.where.profile",
    HubFilter::Instructions => "team_hub.kind.where.instruction",
    HubFilter::Skills => "team_hub.kind.where.skill",
    HubFilter::Tools => "team_hub.kind.where.tool",
    HubFilter::All | HubFilter::McpServers => "team_hub.kind.where.mcp",
  }
}

/// The menu All's New opens, and where it stands.
pub(super) struct NewMenu {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
}

/// New for `filter`; `None` for a filter whose kind has no create page.
pub(super) fn new_button(
  ctx: &mut Ctx,
  (project, filter): (ProjectId, HubFilter),
  (navigation, menu): (&Navigation, &NewMenu),
) -> Option<Element> {
  if filter == HubFilter::All {
    return Some(all(ctx, project, navigation, menu));
  }
  let page = page(filter)?;
  Some(
    ActionButton::new("team-hub-new", ButtonKind::Primary, ctx.t(label_key(filter)))
      .icon(Icon::Plus)
      .opens(navigation.to(Route::Project(project, page)))
      .into(),
  )
}

fn all(ctx: &mut Ctx, project: ProjectId, navigation: &Navigation, menu: &NewMenu) -> Element {
  let mut panel = menu_panel("team-hub-new-menu");
  for filter in CREATABLE {
    let Some(page) = page(filter) else { continue };
    panel = panel.child(
      MenuItem::opens(
        format!("team-hub-new-{}", filter.name()),
        ctx.t(kind_key(filter)),
        navigation.to(Route::Project(project, page)),
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
    .child(
      Popup::new(menu.anchor.clone(), panel)
        .open(menu.open.clone())
        .placement(Placement::BottomEnd)
        .offset(0.0, ExtraSpacing::Snug.value()),
    )
    .into()
}
