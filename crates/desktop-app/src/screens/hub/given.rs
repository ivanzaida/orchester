//! A row of what a profile gives its agents from a list (`Instructions`, `MCP servers`, `Tools`, `Skills` in A26
//! `WSw0W` and L17 `b7Tbxb`): the chosen ones in tags, *None* while there are none, and *Add*, a menu that checks them
//! and offers the others ([`add_menu`]). Every profile form has it the same: a new profile's, an admin's edit of a
//! team one and a local one's panel.

use std::{fmt::Display, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::{
  screens::{
    hub::agent_row::{added_tag_chip, tag_chip},
    local_hub::profile::add_menu,
    team_hub::rows::{self, Sub},
  },
  theme::HubText,
};

/// One list: what it chooses and what it could choose.
pub(in crate::screens) struct List<I> {
  /// The row's element id; its value's is this with `-value`, each tag's that with `-<index>`.
  pub id: String,
  /// What one entry is, for the menu's ids ([`add_menu::Props::what`]).
  pub what: &'static str,
  pub label: Arc<str>,
  /// What the row's label says under it.
  pub sub: Option<Sub>,
  /// The ones it chooses, in order, by id and name.
  pub chosen: Vec<(I, String)>,
  /// The ones the version an edit starts from chooses: a tag of one not among them was added in the edit and has the
  /// `accent` outline until Save. `None` while the profile is made.
  pub before: Option<Vec<I>>,
  /// The others it could choose, which the menu offers unchecked.
  pub offered: Vec<(I, String)>,
  pub on_toggle: add_menu::Toggle<I>,
  /// Nothing is chosen now: a save or a create is under way.
  pub locked: bool,
}

/// The row of `list`.
pub(in crate::screens) fn row<I>(ctx: &mut Ctx, list: List<I>) -> Row
where
  I: Copy + PartialEq + Display + Send + Sync + 'static,
{
  let value_id = format!("{}-value", list.id);
  let item = |(id, name): &(I, String), chosen| add_menu::Item {
    id: *id,
    name: name.clone(),
    chosen,
  };
  let items: Vec<add_menu::Item<I>> = list
    .chosen
    .iter()
    .map(|entry| item(entry, true))
    .chain(list.offered.iter().map(|entry| item(entry, false)))
    .collect();
  let tags: Vec<Element> = if list.chosen.is_empty() {
    vec![
      Text::new(&ctx.t("local_hub.profile.none"))
        .id(format!("{value_id}-none"))
        .variant(HubText::Value.role())
        .color(PaletteColor::TextMuted)
        .into(),
    ]
  } else {
    list
      .chosen
      .iter()
      .enumerate()
      .map(|(index, (id, name))| {
        let tag_id = format!("{value_id}-{index}");
        let added = list.before.as_ref().is_some_and(|before| !before.contains(id));
        if added {
          added_tag_chip(&tag_id, name).into()
        } else {
          tag_chip(&tag_id, name).into()
        }
      })
      .collect()
  };
  // *Add* is always there, in the same place in every list; with nothing to offer it says so.
  let add = add_menu::add(
    ctx,
    add_menu::Props {
      what: list.what,
      items,
      label: ctx.t("local_hub.profile.add"),
      locked: list.locked,
      on_toggle: list.on_toggle,
    },
  );
  let value = Row::new()
    .id(value_id)
    .width(Dimension::Pct(100.0))
    .wrap()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(tags)
    .child(add);
  rows::row(&list.id, &list.label, list.sub, value)
}
