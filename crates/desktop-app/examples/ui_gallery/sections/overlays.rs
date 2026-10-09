//! What opens over the window: a menu under its button (`Pipeline menu` `ClKqz`, `Menu item` `mAHGP`,
//! `Menu item/detail` `j6ZUhL`, `Menu separator` `UoDdI`), a popover (`Run permissions popover`
//! `ojhrZ`), a tooltip with its shortcut (`Tooltip` `b6q3OZ`), and the loader.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::Element,
};
use orchester_app::{
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    menu::{MenuItem, menu, menu_panel, separator},
    popover::{PopoverSide, popover, popover_panel},
    toggle::{Toggle, ToggleKind},
    tooltip::Tooltip,
  },
};

use super::{cell, line, stack};
use crate::gallery::State;

/// The menu's and the popover's widths (`ClKqz`, `ojhrZ`).
const MENU_WIDTH: f32 = 300.0;
const POPOVER_WIDTH: f32 = 440.0;

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  stack([line([
    cell("Pipeline menu · ClKqz", pipeline_menu(ctx, state)),
    cell("Run permissions popover · ojhrZ", permissions(state)),
    cell("Tooltip · b6q3OZ (rest the pointer)", tooltip(ctx, state)),
    cell(
      "loader-circle · turning",
      icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextSecondary).id("g-loader"),
    ),
  ])
  .into()])
}

fn pipeline_menu(ctx: &mut Ctx, state: &State) -> Row {
  let open = state.menu_open.clone();
  let toggle = open.clone();
  // The menu closes itself on a choice.
  let choose = |what: &'static str| {
    let pressed = state.pressed.clone();
    move || pressed.set(what.to_owned())
  };
  let items = [
    MenuItem::new("g-menu-feature", "Feature delivery v3", choose("Feature delivery v3"))
      .icon(Icon::Workflow)
      .detail("Published v3 · 6 steps · approval at Plan")
      .chosen(true)
      .render(ctx, &state.menu_open),
    MenuItem::new("g-menu-quick", "Quick fix v2", choose("Quick fix v2"))
      .icon(Icon::Workflow)
      .detail("Published v2 · 3 steps · no approval gates")
      .render(ctx, &state.menu_open),
    separator(ctx),
    MenuItem::new("g-menu-new", "New pipeline", choose("New pipeline"))
      .icon(Icon::Plus)
      .shortcut("Ctrl N")
      .render(ctx, &state.menu_open),
    MenuItem::new(
      "g-menu-long",
      "Release with a review after every step, a docs pass and a changelog",
      choose("Release"),
    )
    .icon(Icon::Workflow)
    .render(ctx, &state.menu_open),
    MenuItem::unavailable("g-menu-import", "Import a pipeline", "Importing pipelines is planned.")
      .icon(Icon::Download)
      .render(ctx, &state.menu_open),
  ];
  let panel = menu_panel("g-menu")
    .width(lurq::node::dimension::Dimension::Px(MENU_WIDTH))
    .with_children(items);
  Row::new()
    .child(
      Row::new().ref_element(state.menu_anchor.clone()).child(
        ActionButton::new("g-menu-button", ButtonKind::Secondary, "Pipeline")
          .icon(Icon::Workflow)
          .on_click(move || toggle.update(|open| *open = !*open)),
      ),
    )
    .child(menu(state.menu_anchor.clone(), open, panel))
}

fn permissions(state: &State) -> Row {
  let open = state.popover_open.clone();
  let toggle = open.clone();
  let switched = state.switched.get();
  let flip = state.switched.clone();
  let panel = popover_panel("g-popover", POPOVER_WIDTH)
    .child(
      Text::new("Run permissions")
        .variant(ExtraTypography::Strong.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(Text::new("Builder · 03 Implement · Claude · sonnet-4.5").variant(ExtraTypography::Note.role()))
    .child(
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(
          Toggle::new("g-popover-network", ToggleKind::Switch, switched, "Network").on_change(move |on| flip.set(on)),
        )
        .child(Text::new("Changes apply from the next agent invocation.").variant(TypographyStyle::Caption)),
    );
  Row::new()
    .child(
      Row::new().ref_element(state.popover_anchor.clone()).child(
        ActionButton::new(
          "g-popover-button",
          ButtonKind::Secondary,
          "Edit files · asks for the rest",
        )
        .icon(Icon::Shield)
        .on_click(move || toggle.update(|open| *open = !*open)),
      ),
    )
    .child(popover(state.popover_anchor.clone(), open, PopoverSide::Below, panel))
}

fn tooltip(ctx: &mut Ctx, state: &State) -> Element {
  let pressed = state.pressed.clone();
  let button = ActionButton::new("g-tooltip-button", ButtonKind::Ghost, "Search")
    .icon(Icon::Search)
    .on_click(move || pressed.set("Search".to_owned()));
  Tooltip::new("g-tooltip", "Search tasks")
    .shortcut("Ctrl K")
    .wrap(ctx, button)
}
