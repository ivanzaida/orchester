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
  components::{Row, Select, SelectOption, Text, TextOverflow},
  core::{Signal, signal::SignalValue},
  layout::Alignment,
  node::{
    Element, SelectCheckmarkPosition, SelectIcon, SelectPartStyle, SelectStyle, dimension::Dimension, padding::Padding,
  },
};

use crate::{
  theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
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
    .trigger(trigger_style(ctx, PaletteColor::Border))
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

fn trigger_style(ctx: &Ctx, color: PaletteColor) -> SelectPartStyle {
  let theme = ctx.theme();
  SelectPartStyle::new()
    .border_inside(BorderSize::Sm, color)
    .background(PaletteColor::SurfaceInput)
    .rounded(RadiusSize::Md)
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .min_height(ControlHeight::Md.value())
    .text(
      FormTextRole {
        typography: TypographyStyle::Description,
        color: PaletteColor::TextPrimary,
      }
      .resolve(&theme.typography(), &theme.palette()),
    )
}

pub struct SelectField<T: SignalValue> {
  id: Arc<str>,
  name: Arc<str>,
  value: Signal<T>,
  options: Vec<SelectOption<T>>,
  placeholder: Option<Arc<str>>,
  width: Dimension,
  leading: Option<Icon>,
  /// The chosen option's label in Geist Mono: a path or an identifier.
  mono: bool,
  /// What the box says in place of an option's label while that option is chosen.
  shows: Option<(Arc<str>, Arc<str>)>,
  invalid: bool,
  warning: bool,
  locked: bool,
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
      placeholder: None,
      width: Dimension::Pct(100.0),
      leading: None,
      mono: false,
      shows: None,
      invalid: false,
      warning: false,
      locked: false,
    }
  }

  /// What the box says while no option is chosen: `value` holds none of the options.
  pub fn placeholder(mut self, placeholder: impl Into<Arc<str>>) -> Self {
    self.placeholder = Some(placeholder.into());
    self
  }

  pub fn width(mut self, width: impl Into<Dimension>) -> Self {
    self.width = width.into();
    self
  }

  /// A 13 `text_secondary` glyph before the chosen option's label (`history` in `Picker` `ZQd2D`). The
  /// box then draws its own chevron, which does not turn while the menu is open.
  pub fn leading(mut self, glyph: Icon) -> Self {
    self.leading = Some(glyph);
    self
  }

  /// The chosen option's label in Geist Mono, e.g. a program's path (`Program` in `cEJy4`).
  pub fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  /// While the option labelled `label` is chosen, the box says `shown` instead, e.g. the model id a pipeline step
  /// takes from its profile, which the menu lists as *From the profile* (`DuMJh` in `IsU5u`). The box then draws
  /// its own chevron, as with [`Self::leading`].
  pub fn shows(mut self, label: impl Into<Arc<str>>, shown: impl Into<Arc<str>>) -> Self {
    self.shows = Some((label.into(), shown.into()));
    self
  }

  /// A refused choice keeps its rounded field outline in the error color.
  pub fn invalid(mut self) -> Self {
    self.invalid = true;
    self
  }

  /// A draft choice that must be published first keeps the warning outline (adK4e).
  pub fn warning(mut self) -> Self {
    self.warning = true;
    self
  }

  /// An unavailable choice is visibly disabled and cannot open its menu.
  pub fn locked(mut self) -> Self {
    self.locked = true;
    self
  }

  pub fn render(self, ctx: &Ctx) -> Element {
    let mut style = select_style(ctx);
    if self.warning {
      style = style.trigger(trigger_style(ctx, PaletteColor::Warning));
    }
    if self.invalid {
      style = style.trigger(trigger_style(ctx, PaletteColor::Danger));
    }
    let mut select = Select::new(self.value)
      .name(self.name.clone())
      .options(self.options)
      .width(self.width)
      .height(Dimension::Px(ControlHeight::Md.value()))
      .style(style);
    if let Some(placeholder) = self.placeholder {
      select = select.placeholder(placeholder);
    }
    if self.leading.is_some() || self.mono || self.shows.is_some() {
      let (id, glyph, mono, shows) = (self.id.clone(), self.leading, self.mono, self.shows);
      select = select.trigger(move |state| {
        let label = state.label.as_deref().unwrap_or_default();
        let label = match &shows {
          Some((chosen, shown)) if **chosen == *label => &**shown,
          _ => label,
        };
        leading_trigger(&id, (glyph, mono), label)
      });
    }
    // lurq's select takes no accessible name of its own; tools read it from the select's row.
    Row::new()
      .width(self.width)
      .describe("a11y_name", self.name)
      .describe("locked", if self.locked { "true" } else { "false" })
      .hit_test(if self.locked {
        lurq::node::HitTestBehavior::None
      } else {
        lurq::node::HitTestBehavior::Auto
      })
      .opacity(if self.locked {
        crate::theme::DISABLED_OPACITY
      } else {
        1.0
      })
      .child(Element::from(select).id(self.id.to_string()))
      .into()
  }
}

/// The chosen option's `label` (element id `<id>-label`) after `glyph`, if any, in Geist Mono when `mono`, cut with an
/// ellipsis where the box runs out of room (its menu lists the whole label), and the chevron at the end.
fn leading_trigger(id: &str, (glyph, mono): (Option<Icon>, bool), label: &str) -> Element {
  let style = if mono {
    TypographyStyle::Mono
  } else {
    TypographyStyle::Description
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(glyph.map(|glyph| icon(glyph, IconSize::Base, PaletteColor::TextSecondary).flex_shrink(0.0)))
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(style)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(icon(Icon::ChevronDown, IconSize::Md, PaletteColor::TextMuted).flex_shrink(0.0))
    .into()
}
