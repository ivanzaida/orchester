//! Select (`Field/select` `pV2IA`): the field's box with the chosen option's label and a 14
//! `text_muted` chevron; open, it takes the `border_focus` stroke and the chevron points up (`vK0XW` in
//! `FpAul`). Its menu opens 6 under it (`FpAul`): `surface_raised` inside a `border_strong` stroke,
//! radius 8, 4 inside and 2 between options (`ClKqz`); an option is 28 high at least, 8 at its sides,
//! radius 4, its label 12 `text_primary` and its detail under it in 11 `text_muted` (`Menu item/detail`
//! `j6ZUhL`); the pointer's and the keyboard's option take the `hover` fill (`wzx2E`), and a 14 check
//! marks the chosen one at the end (`s3Dp0`). lurq 0.42's options carry no icon and center their check
//! on an option with a detail, so the design's option icons (`F0qLML`) are not drawn and the check is
//! not at the top.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Row, Select, SelectOption},
  core::{Signal, signal::SignalValue},
  node::{
    Element, SelectCheckmarkPosition, SelectIcon, SelectPartStyle, SelectStyle, dimension::Dimension, padding::Padding,
  },
};

use crate::{
  theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::icon::Icon,
};

/// The most an open menu grows to, nine options, before it scrolls.
const MENU_MAX_HEIGHT: f32 = 280.0;

fn glyph(icon: Icon) -> SelectIcon {
  SelectIcon::glyph(icon.glyph().to_string(), IconSize::Md.role())
}

/// The style of every select: its box like a field's and its menu.
pub fn select_style(ctx: &Ctx) -> SelectStyle {
  let theme = ctx.theme();
  let text = |typography, color| FormTextRole { typography, color }.resolve(&theme.typography(), &theme.palette());
  let stroke = |color| SelectPartStyle::new().border_inside(BorderSize::Sm, color);
  let highlight = || SelectPartStyle::new().background(ExtraColor::Hover.role());
  SelectStyle::unstyled()
    .trigger(
      stroke(PaletteColor::Border)
        .background(PaletteColor::SurfaceInput)
        .rounded(RadiusSize::Md)
        .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
        .min_height(ControlHeight::Md.value())
        .text(text(TypographyStyle::Description, PaletteColor::TextPrimary)),
    )
    .trigger_hovered(stroke(PaletteColor::BorderStrong))
    .trigger_focused(stroke(PaletteColor::BorderFocus))
    .trigger_open(stroke(PaletteColor::BorderFocus))
    .placeholder_text(text(TypographyStyle::Description, PaletteColor::TextMuted))
    .chevron(glyph(Icon::ChevronDown))
    .chevron_open(glyph(Icon::ChevronUp))
    .chevron_color(PaletteColor::TextMuted)
    .menu(
      stroke(PaletteColor::BorderStrong)
        .background(PaletteColor::SurfaceRaised)
        .rounded(ExtraRadius::Tile.role())
        .padding(Padding::all(SpacingSize::Xs))
        .box_shadow(ShadowStyle::Md),
    )
    .max_menu_height(MENU_MAX_HEIGHT)
    .menu_gap(ExtraSpacing::Snug.value())
    .option(
      SelectPartStyle::new()
        .min_height(ControlHeight::Md.value())
        .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
        .rounded(RadiusSize::Sm)
        .text(text(TypographyStyle::Description, PaletteColor::TextPrimary)),
    )
    .option_hovered(highlight())
    .option_selected_hovered(highlight())
    .option_highlighted(highlight())
    .option_selected_highlighted(highlight())
    .option_disabled(SelectPartStyle::new().text_color(PaletteColor::TextMuted))
    .option_detail(
      SelectPartStyle::new()
        .typography(TypographyStyle::Caption)
        .text_color(PaletteColor::TextMuted),
    )
    .single_checkmark(true)
    .checkmark(glyph(Icon::Check))
    .checkmark_color(PaletteColor::TextPrimary)
    .checkmark_size(IconSize::Md.size())
    .checkmark_position(SelectCheckmarkPosition::Trailing)
    .checkmark_gap(SpacingSize::Sm)
}

pub struct SelectField<T: SignalValue> {
  id: Arc<str>,
  name: Arc<str>,
  value: Signal<T>,
  options: Vec<SelectOption<T>>,
  width: Dimension,
}

impl<T> SelectField<T>
where
  T: Clone + PartialEq + Send + Sync + 'static + SignalValue,
  Vec<T>: SignalValue,
{
  /// A select of `options` bound to `value`, named `name` for tools (its field's label); its element id
  /// is `id`.
  pub fn new(
    id: impl Into<Arc<str>>,
    name: impl Into<Arc<str>>,
    value: Signal<T>,
    options: Vec<SelectOption<T>>,
  ) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      value,
      options,
      width: Dimension::Pct(100.0),
    }
  }

  pub fn width(mut self, width: impl Into<Dimension>) -> Self {
    self.width = width.into();
    self
  }

  pub fn render(self, ctx: &Ctx) -> Element {
    let select = Select::new(self.value)
      .name(self.name.clone())
      .options(self.options)
      .width(self.width)
      .height(Dimension::Px(ControlHeight::Md.value()))
      .style(select_style(ctx));
    // lurq's select takes no accessible name of its own; tools read it from the select's row.
    Row::new()
      .width(self.width)
      .describe("a11y_name", self.name)
      .child(Element::from(select).id(self.id.to_string()))
      .into()
  }
}
