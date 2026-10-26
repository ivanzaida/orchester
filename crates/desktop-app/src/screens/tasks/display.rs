//! The list's Display (`Display` in `Header` `AL7qv`): a ghost button that opens a menu of how the rows are ordered —
//! the manual order, newest first, by title, by workflow status or by priority — under which it says that sorting
//! never changes the run queue. The order belongs to the open list.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::browse::TaskSort;

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    filter::filter_group,
    icon::{Icon, icon},
    menu::{MenuItem, menu, menu_panel, separator},
  },
};

/// The menu's width, as the Filter menu's: wide enough for its note on one line.
const MENU_WIDTH: f32 = 260.0;

/// The orders, in the menu's order, each with the i18n key of its words and its name for element ids.
const SORTS: [(TaskSort, &str, &str); 5] = [
  (TaskSort::Manual, "tasks.sort.manual", "manual"),
  (TaskSort::Newest, "tasks.sort.newest", "newest"),
  (TaskSort::Title, "tasks.sort.title", "title"),
  (TaskSort::Status, "tasks.sort.status", "status"),
  (TaskSort::Priority, "tasks.sort.priority", "priority"),
];

#[derive(Clone)]
pub(super) struct DisplayProps {
  pub sort: Signal<TaskSort>,
  /// The tasks are read: until then the button cannot be pressed.
  pub ready: bool,
}

/// Compared by what it shows; the sort by its signal.
impl PartialEq for DisplayProps {
  fn eq(&self, other: &Self) -> bool {
    self.sort.id() == other.sort.id() && self.ready == other.ready
  }
}

impl DevtoolsInspectable for DisplayProps {}

pub(super) struct DisplayControl {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for DisplayControl {
  type Props = DisplayProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DisplayProps>().clone();
    let button = ActionButton::new("tasks-display", ButtonKind::Ghost, ctx.t("tasks.display.button"))
      .icon(Icon::SlidersHorizontal)
      .element_ref(self.anchor.clone());
    if !props.ready {
      return Element::from(Row::new().child(button));
    }
    let open = self.open.clone();
    let button = button.on_click(move || open.update(|open| *open = !*open));
    let popup = self.open.get().then(|| {
      let now = props.sort.get();
      let items: Vec<Element> = SORTS
        .into_iter()
        .map(|(sort, key, name)| {
          let chosen = props.sort.clone();
          MenuItem::new(format!("tasks-sort-{name}"), ctx.t(key), move || chosen.set(sort))
            .chosen(sort == now)
            .render(ctx, &self.open)
        })
        .collect();
      let note = Row::new()
        .id("tasks-sort-note")
        .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("tasks.display.note")).variant(ExtraTypography::Note.role()));
      let panel = menu_panel("tasks-display-menu")
        .width(Dimension::Px(MENU_WIDTH))
        .child(filter_group(
          "tasks-display-sort",
          Icon::ListOrdered,
          &ctx.t("tasks.display.sort"),
        ))
        .with_children(items)
        .child(separator(ctx))
        .child(note);
      menu(self.anchor.clone(), self.open.clone(), panel)
    });
    Row::new().child(button).with_children(popup).into()
  }
}
