//! The *Add* chip of a list row of a local profile (`Add` `z4SVTX` in `Profile` `Gk3js` of L1): a menu of what the
//! project offers for the list — its MCP servers, instructions, skills or tools — each with a check while the profile
//! chooses it. Choosing one adds it to what the panel holds, or takes it out when it is checked.

use std::{fmt::Display, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};

use crate::{
  screens::hub::page_head::MORE_MENU_WIDTH,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu, menu_panel},
  },
};

/// What a press on an entry does: the entry, and whether the profile chooses it from now on.
pub(in crate::screens) type Toggle<I> = Arc<dyn Fn(I, bool) + Send + Sync>;

/// An entry the menu lists and whether the profile chooses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens) struct Item<I> {
  pub id: I,
  pub name: String,
  pub chosen: bool,
}

#[derive(Clone)]
pub(in crate::screens) struct Props<I> {
  /// What the list holds, singular, in the menu's element ids: `server` makes `local-hub-profile-add-server`.
  pub what: &'static str,
  pub items: Vec<Item<I>>,
  pub label: Arc<str>,
  /// An edit is under way: nothing is chosen until it ends.
  pub locked: bool,
  pub on_toggle: Toggle<I>,
}

/// The props are equal when they say the same: the handler is made anew on every render.
impl<I: PartialEq> PartialEq for Props<I> {
  fn eq(&self, other: &Self) -> bool {
    (self.what, &self.items, &self.label, self.locked) == (other.what, &other.items, &other.label, other.locked)
  }
}
impl<I> DevtoolsInspectable for Props<I> {}

pub(in crate::screens) fn add<I>(ctx: &mut Ctx, props: Props<I>) -> Element
where
  I: Copy + PartialEq + Display + Send + Sync + 'static,
{
  ctx.mount_keyed::<AddMenu<I>>(&format!("local-hub-profile-add-{}", props.what), props)
}

struct AddMenu<I> {
  open: Signal<bool>,
  anchor: ElementRef,
  of: std::marker::PhantomData<I>,
}

impl<I> Component for AddMenu<I>
where
  I: Copy + PartialEq + Display + Send + Sync + 'static,
{
  type Props = Props<I>;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
      of: std::marker::PhantomData,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      what,
      items,
      label,
      locked,
      on_toggle,
    } = ctx.props::<Props<I>>().clone();
    let toggle = self.open.clone();
    let button = ActionButton::new(
      format!("local-hub-profile-add-{what}-button"),
      ButtonKind::Ghost,
      label.to_string(),
    )
    .icon(Icon::Plus)
    .small()
    .element_ref(self.anchor.clone());
    let button = if locked {
      button.disabled()
    } else if items.is_empty() {
      button.unavailable_with_icon(ctx.t("local_hub.profile.nothingToAdd"))
    } else {
      button.on_click(move || toggle.update(|open| *open = !*open))
    };
    let rows: Vec<Element> = items
      .into_iter()
      .map(|item| {
        let on_toggle = on_toggle.clone();
        MenuItem::new(
          format!("local-hub-profile-{what}-{}", item.id),
          item.name.clone(),
          move || on_toggle(item.id, !item.chosen),
        )
        .chosen(item.chosen)
        .render(ctx, &self.open)
      })
      .collect();
    let panel = menu_panel(&format!("local-hub-profile-{what}s-menu"))
      .width(Dimension::Px(MORE_MENU_WIDTH))
      .with_children(rows);
    Column::new()
      .child(button)
      .child(menu(self.anchor.clone(), self.open.clone(), panel))
  }
}
