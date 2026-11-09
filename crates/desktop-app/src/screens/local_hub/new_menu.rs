//! *New* under All and Archived (`Agents · Archived` L13 `MXma9`): those filters hold no one kind, so the primary
//! button in the head says only *New* and opens a menu of what can be made, each of which opens the page that makes it,
//! as the New of a kind's own filter does.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use super::{head::new_of, leave::Leave};
use crate::{
  routes::{HubFilter, Route},
  screens::hub::page_head::MORE_MENU_WIDTH,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// The kinds the menu offers, in the order of the filters.
const KINDS: [(HubFilter, Icon); 5] = [
  (HubFilter::Profiles, Icon::Bot),
  (HubFilter::McpServers, Icon::Plug),
  (HubFilter::Instructions, Icon::FileText),
  (HubFilter::Skills, Icon::Sparkles),
  (HubFilter::Tools, Icon::Terminal),
];

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ProjectId,
  /// Where a press goes: to the question first while a server's edits are not saved.
  pub leave: Leave,
}
impl DevtoolsInspectable for Props {}

pub(super) fn new_menu(ctx: &mut Ctx, project: ProjectId, leave: &Leave) -> Element {
  ctx.mount_keyed::<NewMenu>(
    "local-hub-new-menu",
    Props {
      project,
      leave: leave.clone(),
    },
  )
}

struct NewMenu {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for NewMenu {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, leave } = ctx.props::<Props>().clone();
    let toggle = self.open.clone();
    let button = ActionButton::new("local-hub-new", ButtonKind::Primary, ctx.t("local_hub.new.any"))
      .icon(Icon::Plus)
      .element_ref(self.anchor.clone())
      .on_click(move || toggle.update(|open| *open = !*open));
    let items: Vec<Element> = KINDS
      .into_iter()
      .filter_map(|(filter, glyph)| {
        let (page, words) = new_of(filter)?;
        let leave = leave.clone();
        let route = Route::Project(project, page);
        Some(
          MenuItem::new(format!("local-hub-new-{}", filter.name()), ctx.t(words), move || {
            leave.go(route.clone());
          })
          .icon(glyph)
          .render(ctx, &self.open),
        )
      })
      .collect();
    let panel = menu_panel("local-hub-new-menu-panel")
      .width(Dimension::Px(MORE_MENU_WIDTH))
      .with_children(items);
    Column::new()
      .child(button)
      .child(menu_at_end(self.anchor.clone(), self.open.clone(), panel))
  }
}
