//! A resource of the source to select (`cuocA`): its check, its name over what it is, and the
//! content its copy takes. A selected resource that has both a published version and a draft offers
//! both in a menu; otherwise the content is only shown.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Select, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::ResourceId, import::Pick};

use super::Picked;
use crate::{
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    field::select_style,
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop, pressable, surface},
    status_label::status_label,
  },
};

/// Row height (`cuocA`), and the size of the version menu that takes the status label's place
/// when there is a choice.
const ROW_HEIGHT: f32 = 72.0;
const PICK_WIDTH: f32 = 176.0;
const PICK_HEIGHT: f32 = 30.0;

/// A pick as a menu value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PickValue(pub Pick);

impl lurq::app::component::DevtoolsInspectable for PickValue {}

#[derive(Clone)]
pub(super) struct ResourceRowProps {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub description: Arc<str>,
  /// The contents the copy can take, labelled for the menu.
  pub picks: Vec<(PickValue, Arc<str>)>,
  pub pick: Pick,
  pub pick_label: Arc<str>,
  pub selected: bool,
  /// The dialog's selection, which the row changes.
  pub picked: Signal<Picked>,
}

impl PartialEq for ResourceRowProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.name == other.name
      && self.description == other.description
      && self.picks == other.picks
      && self.pick == other.pick
      && self.pick_label == other.pick_label
      && self.selected == other.selected
  }
}

impl lurq::app::component::DevtoolsInspectable for ResourceRowProps {}

pub(super) struct ResourceRow {
  pick: Signal<PickValue>,
}

impl Component for ResourceRow {
  type Props = ResourceRowProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ResourceRowProps>().clone();
    let pick = ctx.signal(PickValue(props.pick));
    ctx.watch(&pick, move |PickValue(chosen)| {
      let chosen = *chosen;
      props.picked.update(|Picked(picked)| {
        if let Some(entry) = picked.iter_mut().find(|(id, _)| *id == props.id) {
          entry.1 = chosen;
        }
      });
    });
    Self { pick }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ResourceRowProps>().clone();
    let check = if props.selected {
      Icon::SquareCheck
    } else {
      Icon::Square
    };
    let (picked, id, pick) = (props.picked.clone(), props.id, self.pick.clone());
    let toggle = move || {
      picked.update(
        |Picked(picked)| match picked.iter().position(|(picked, _)| *picked == id) {
          Some(position) => {
            picked.remove(position);
          }
          None => picked.push((id, pick.get_untracked().0)),
        },
      );
    };
    let pressed = toggle.clone();
    // The checkbox is the dialog form's control: Tab reaches it and Space or Enter toggles it.
    // The rest of the row toggles it with the pointer only.
    let check = keyboard_stop(Button::empty().button(), Ring::Outer)
      .id(format!("import-check-{id}"))
      .describe("selected", if props.selected { "true" } else { "false" })
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .on_click(move |_| pressed())
      .child(icon(check, IconSize::Notice, PaletteColor::TextPrimary));
    let identity = pressable(surface().id(format!("import-resource-{id}")), toggle)
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&props.name).variant(TypographyStyle::Body).nowrap())
          .child(
            Text::new(&props.description)
              .variant(TypographyStyle::Caption)
              .nowrap()
              .text_overflow(TextOverflow::Elipsis),
          ),
      );
    let background = if props.selected {
      PaletteColor::SurfaceRaised
    } else {
      ExtraColor::Surface.role()
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding_vertical(ExtraSpacing::Stack.role())
      .padding_horizontal(SpacingSize::Md)
      .spacing(ExtraSpacing::Group.role())
      .align_items(Alignment::Center)
      .background(background)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(check)
      .child(identity)
      .child(self.version(ctx, &props))
  }
}

impl ResourceRow {
  /// The content the copy takes (`LBzRw`): a menu while there is a choice, a label otherwise.
  fn version(&self, ctx: &Ctx, props: &ResourceRowProps) -> Element {
    let glyph = match props.pick {
      Pick::Published(_) => Icon::CircleCheck,
      Pick::Draft => Icon::Pencil,
    };
    if !props.selected || props.picks.len() < 2 {
      return status_label(glyph, &props.pick_label);
    }
    Element::from(
      Select::new(self.pick.clone())
        .options(props.picks.clone())
        .width(Dimension::Px(PICK_WIDTH))
        .height(Dimension::Px(PICK_HEIGHT))
        .style(select_style(ctx, PICK_HEIGHT)),
    )
    .id(format!("import-version-{}", props.id))
  }
}
