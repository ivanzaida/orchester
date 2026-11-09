//! The *Add* chip of the MCP servers row (`Add` `z4SVTX` in `Profile` `Gk3js` of L1): a menu of the project's MCP
//! servers, each with a check while the profile chooses it. Choosing one adds it to the profile's draft, or takes it
//! out when it is checked.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ConnectionId;

use crate::{
  screens::hub::page_head::MORE_MENU_WIDTH,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// What a press on a server does: the server, and whether the profile chooses it from now on.
pub(in crate::screens) type Toggle = Arc<dyn Fn(ConnectionId, bool) + Send + Sync>;

/// A server the menu lists and whether the profile chooses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens) struct Item {
  pub id: ConnectionId,
  pub name: String,
  pub chosen: bool,
}

#[derive(Clone)]
pub(in crate::screens) struct Props {
  pub items: Vec<Item>,
  pub label: Arc<str>,
  /// An edit is under way: nothing is chosen until it ends.
  pub locked: bool,
  pub on_toggle: Toggle,
}

/// The props are equal when they say the same: the handler is made anew on every render.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.items, &self.label, self.locked) == (&other.items, &other.label, other.locked)
  }
}
impl DevtoolsInspectable for Props {}

pub(in crate::screens) fn add(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<AddServer>("local-hub-profile-add-server", props)
}

struct AddServer {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for AddServer {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      items,
      label,
      locked,
      on_toggle,
    } = ctx.props::<Props>().clone();
    let toggle = self.open.clone();
    let button = ActionButton::new(
      "local-hub-profile-add-server-button",
      ButtonKind::Ghost,
      label.to_string(),
    )
    .icon(Icon::Plus)
    .small()
    .element_ref(self.anchor.clone());
    let button = if locked {
      button.disabled()
    } else {
      button.on_click(move || toggle.update(|open| *open = !*open))
    };
    let rows: Vec<Element> = items
      .into_iter()
      .map(|item| {
        let on_toggle = on_toggle.clone();
        MenuItem::new(
          format!("local-hub-profile-server-{}", item.id),
          item.name.clone(),
          move || on_toggle(item.id, !item.chosen),
        )
        .chosen(item.chosen)
        .render(ctx, &self.open)
      })
      .collect();
    let panel = menu_panel("local-hub-profile-servers-menu")
      .width(Dimension::Px(MORE_MENU_WIDTH))
      .with_children(rows);
    Column::new()
      .child(button)
      .child(menu_at_end(self.anchor.clone(), self.open.clone(), panel))
  }
}
