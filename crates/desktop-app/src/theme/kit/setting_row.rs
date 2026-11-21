//! Settings' rows (`ui::setting_rows::SettingRow`, `App settings · redesign` `iUmo3`, as `Appearance` `g6P4CU` in
//! `01 · General` draws them): lurq's `ui::ListItem`, 11 over and under and 14 at its sides, 12 between its parts: an
//! optional tile, the label in 13/500 `text_primary` with at most one short hint 2 under it in 12 `text_muted` (or its
//! tone's color, [`WARNING`], [`DANGER`], [`SUCCESS`]), the (i) 6 after the label, a link 2 further under the hint, and
//! the control at the end. What a row says under itself runs under it, 14 at its sides and 12 under it, 8 apart. A row
//! that opens a page, and the row of the page shown, have the `hover` fill under the pointer. A setting that needs the
//! one above it starts further in ([`DEPENDENT`]); a label of what is not in use now is `text_muted` ([`FADED`]). The
//! recipe is the app's whole.

use std::sync::Arc;

use lurq::{
  app::{
    events::MouseEvent,
    theme::{PaletteColor, SpacingSize, Theme, TypographyStyle},
  },
  components::{Column, Row, TextOverflow},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{CursorIcon, Element, dimension::Dimension},
  ui::{Length, ListItem, ListItemPart, ListItemState, ListItemView, Recipe, Selector},
};

use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography, SettingsSpacing, skeleton_line};

/// A setting that needs the one above it (`JMLwI`).
pub const DEPENDENT: &str = "dependent";
/// The label of what is not in use now (`Address` `GXDxS`, the server off).
pub const FADED: &str = "faded";
/// A hint that says how the setting stands.
pub const WARNING: &str = "warning";
pub const DANGER: &str = "danger";
pub const SUCCESS: &str = "success";

pub(super) fn install(theme: &Theme) {
  theme.set_renderer::<ListItem>(Arc::new(render));
  use ListItemPart::{Below, Content, Description, DescriptionSkeleton, Detail, Root, Tags, Title, TitleSkeleton};
  use ListItemState::{Pressable, Selected};
  let title_bone = skeleton_line(theme, ExtraTypography::Strong.role()).1;
  let hint_bone = skeleton_line(theme, TypographyStyle::Description).1;
  theme.set_recipe::<ListItem>(
    Recipe::new()
      .base(Root, |style| {
        style
          .width(Length::Fill)
          .padding_x(ExtraSpacing::Inset.role())
          .padding_y(SettingsSpacing::RowY.role())
          .gap(SpacingSize::Md)
      })
      .state(DEPENDENT, Root, |style| {
        style.padding_start(SettingsSpacing::Dependent.role())
      })
      .state(Pressable, Root, |style| style.cursor(CursorIcon::Pointer))
      .when(Selector::new().state(Pressable).hovered(), Root, |style| {
        style.background(ExtraColor::Hover.role())
      })
      .state(Selected, Root, |style| style.background(ExtraColor::Hover.role()))
      .base(Content, |style| style.gap(ExtraSpacing::Tight.role()))
      .base(Title, |style| {
        style
          .typography(ExtraTypography::Strong.role())
          .foreground(PaletteColor::Foreground)
      })
      .state(FADED, Title, |style| style.foreground(PaletteColor::MutedForeground))
      .base(Description, |style| {
        style
          .typography(TypographyStyle::Description)
          .foreground(PaletteColor::MutedForeground)
      })
      .state(WARNING, Description, |style| style.foreground(PaletteColor::Warning))
      .state(DANGER, Description, |style| style.foreground(PaletteColor::Destructive))
      .state(SUCCESS, Description, |style| style.foreground(PaletteColor::Success))
      .base(Tags, |style| {
        style
          .padding_start(ExtraSpacing::Snug.role())
          .gap(ExtraSpacing::Snug.role())
      })
      .base(Detail, |style| style.padding_top(ExtraSpacing::Tight.role()))
      .base(TitleSkeleton, |style| style.height(Length::Px(title_bone)))
      .base(DescriptionSkeleton, |style| style.height(Length::Px(hint_bone)))
      .base(Below, |style| {
        style
          .padding_x(ExtraSpacing::Inset.role())
          .padding_bottom(SpacingSize::Md)
          .gap(SpacingSize::Sm)
      }),
  );
}

/// lurq's row, but its label keeps its room beside a wide control, which gives way to it (a long path, `settings-log`),
/// as the kit's rows did: lurq's words take only the room its end leaves them, and its end keeps its whole width, which
/// a recipe cannot change.
fn render(mut view: ListItemView) -> Element {
  let mut children: Vec<Element> = Vec::new();
  children.extend(view.leading().map(Element::from));
  children.push(words(&view).into());
  // The words give way to the end down to their title's line; then the end gives way, cut to what they leave it.
  children.extend(view.trailing().map(|end| {
    end
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .shrink_order(1)
      .into()
  }));
  let Some(below) = view.below() else {
    return view.root(children);
  };
  // The row and what it says under itself are one item, `x-item`, as lurq's own renderer holds them.
  Column::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Stretch)
    .part(&view.part(ListItemPart::Item))
    .child(view.root(children))
    .child(below)
    .into()
}

/// The title's line (the title, then its tags), the subtitles, the description and the details: lurq's text column,
/// as wide as the room the end leaves and never narrower than the title's line.
fn words(view: &ListItemView) -> Column {
  let part = view.part(ListItemPart::Title);
  let title: Element = if view.title_pending {
    view.bone(ListItemPart::Title, ListItemPart::TitleSkeleton).into()
  } else {
    part
      .text(&view.title)
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .part(&part)
      .into()
  };
  let line = Row::new()
    .align_items(Alignment::Center)
    .overflow_visible()
    .child(title)
    .with_children((!view.tags.is_empty()).then(|| {
      held(view, Row::new().flex_shrink(0.0).part(&view.part(ListItemPart::Tags)))
        .with_children(view.tags.iter().cloned())
    }));
  let mut words = Column::new()
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .shrink_limit(ShrinkLimit::Content)
    .part(&view.part(ListItemPart::Content))
    .child(line);
  for subtitle in &view.subtitles {
    words = words.child(held(view, Row::new().part(&view.part(ListItemPart::Subtitle))).child(subtitle.clone()));
  }
  if view.description_pending {
    words = words.child(view.bone(ListItemPart::Description, ListItemPart::DescriptionSkeleton));
  } else if let Some(description) = &view.description {
    let part = view.part(ListItemPart::Description);
    words = words.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .min_width(Dimension::Px(0.0))
        .part(&part)
        .child(part.text(description).width(Dimension::Pct(100.0))),
    );
  }
  for detail in &view.details {
    words = words.child(held(view, Row::new().part(&view.part(ListItemPart::Detail))).child(detail.clone()));
  }
  words
}

/// `row` holding what the app put in the row, whose presses are its own in a row that is pressed.
fn held(view: &ListItemView, row: Row) -> Row {
  // No narrower floor than the title's line: a long path under the title is cut, not kept whole.
  let row = row
    .max_width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .overflow_visible();
  if view.pressable() {
    row.on_click(|event: MouseEvent| event.stop_propagation())
  } else {
    row
  }
}
