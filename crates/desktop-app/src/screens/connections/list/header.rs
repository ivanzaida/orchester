//! The top of Connections (`Header` `SUpbd`, `Tabs` `qVM0O`): its title and what it holds, the add action of
//! its tab, and the tabs with their counts. On All connections the add action is a menu of the kinds
//! (`Add connection · menu open` in `oK3aE`); on a kind's tab it adds that kind directly. Adding a GitHub
//! connection is not built in this app yet: it stays in place, locked, and the tab says why under its notice.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use crate::{
  navigation::{Navigation, link::Open},
  routes::{ConnectionsTab, ConnectionsView, NewConnection, ProjectPage, Route},
  screens::connections::data::count,
  theme::{ConnectionsText, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu_panel},
    menu_button::MenuButton,
    tabs::{PageTab, counted, page_tabs},
  },
};

/// The add menu's width (`Add connection · menu open` in `oK3aE`).
const MENU_WIDTH: f32 = 300.0;

/// The add menu's state: whether it is open, and the button it opens at.
#[derive(Clone)]
pub(super) struct AddMenu {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
}

/// The title, what the screen holds, and the add action of `tab`.
pub(super) fn header(ctx: &mut Ctx, project: ProjectId, tab: ConnectionsTab, menu: &AddMenu) -> Row {
  let title = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t("connections.title"))
        .id("connections-title")
        .variant(ExtraTypography::Display.role()),
    )
    .child(Text::new(&ctx.t("connections.description")).variant(ConnectionsText::Lead.role()));
  Row::new()
    .id("connections-header")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(title)
    .child(add_action(ctx, project, tab, menu))
}

/// What adds a connection on `tab`.
fn add_action(ctx: &mut Ctx, project: ProjectId, tab: ConnectionsTab, menu: &AddMenu) -> Element {
  let navigation = Navigation::of(ctx);
  let add = move |kind: NewConnection| navigation.to(Route::Project(project, ProjectPage::NewConnection(kind)));
  match tab {
    ConnectionsTab::All => {
      let button = MenuButton::new(
        "connections-add",
        (Icon::Plus, ctx.t("connections.add")),
        menu.open.clone(),
        menu.anchor.clone(),
      );
      let panel = add_menu(ctx, menu, &add);
      Row::new()
        .child(button)
        .child(
          Popup::new(menu.anchor.clone(), panel)
            .open(menu.open.clone())
            .placement(Placement::BottomEnd)
            .offset(0.0, ExtraSpacing::Snug.value()),
        )
        .into()
    }
    ConnectionsTab::AgentProviders => primary(ctx, "connections.add", add(NewConnection::AgentProvider)),
    ConnectionsTab::McpServers => primary(ctx, "connections.mcp.add", add(NewConnection::McpServer)),
    ConnectionsTab::GitHub => {
      // Locked; why is said under the tab's notice, so the header keeps the height of the other tabs'.
      ActionButton::new("connections-add", ButtonKind::Primary, ctx.t("connections.github.add"))
        .icon(Icon::Plus)
        .unavailable(ctx.t("connections.github.addUnavailable"))
        .into()
    }
  }
}

fn primary(ctx: &Ctx, key: &str, add: Open) -> Element {
  ActionButton::new("connections-add", ButtonKind::Primary, ctx.t(key))
    .icon(Icon::Plus)
    .opens(add)
    .into()
}

/// The kinds Add connection adds, each with what it is.
fn add_menu(ctx: &mut Ctx, menu: &AddMenu, add: &impl Fn(NewConnection) -> Open) -> Column {
  let agent = MenuItem::opens(
    "connections-add-agent",
    ctx.t("connections.menu.agent"),
    add(NewConnection::AgentProvider),
  )
  .icon(Icon::Bot)
  .detail(ctx.t("connections.menu.agentDetail"));
  let github = MenuItem::unavailable(
    "connections-add-github",
    ctx.t("connections.menu.github"),
    ctx.t("connections.github.addUnavailable"),
  )
  .icon(Icon::GitPullRequest);
  let mcp = MenuItem::opens(
    "connections-add-mcp",
    ctx.t("connections.menu.mcp"),
    add(NewConnection::McpServer),
  )
  .icon(Icon::Server)
  .detail(ctx.t("connections.menu.mcpDetail"));
  menu_panel("connections-add-menu")
    .width(Dimension::Px(MENU_WIDTH))
    .child(agent.render(ctx, &menu.open))
    .child(github.render(ctx, &menu.open))
    .child(mcp.render(ctx, &menu.open))
}

/// The tabs, each with how many connections it lists once they are read, `—` until then.
pub(super) fn tabs(ctx: &mut Ctx, project: ProjectId, tab: ConnectionsTab, counts: Option<[usize; 4]>) -> Element {
  let navigation = Navigation::of(ctx);
  let unknown = ctx.t("connections.tabs.unknownCount");
  let tabs: Vec<PageTab> = ConnectionsTab::ALL
    .into_iter()
    .map(|shown| {
      let word = ctx.t(tab_key(shown));
      let label: Arc<str> = match counts {
        Some(counts) => counted(ctx, &word, count(counts, shown)),
        None => ctx.t_args("ui.tabCount", [("label", word), ("count", unknown.clone())]),
      };
      PageTab::to(
        format!("connections-tab-{}", shown.name()),
        label,
        shown == tab,
        navigation.to(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::new(shown)),
        )),
      )
    })
    .collect();
  page_tabs(ctx, "connections-tabs", tabs).into()
}

const fn tab_key(tab: ConnectionsTab) -> &'static str {
  match tab {
    ConnectionsTab::All => "connections.tabs.all",
    ConnectionsTab::AgentProviders => "connections.tabs.agentProviders",
    ConnectionsTab::GitHub => "connections.tabs.github",
    ConnectionsTab::McpServers => "connections.tabs.mcpServers",
  }
}
