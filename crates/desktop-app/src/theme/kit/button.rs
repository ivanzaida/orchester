//! Buttons (`Button/primary` `yJ2jo`, `Button/secondary` `Dswcx`, `Button/ghost` `cnlv1`, `Button/danger`
//! `O3M8iH`): lurq's `ui::Button` in the kit's kinds and sizes. An optional 14 icon and a 12/500 label, 6 apart, 28
//! high, 12 at each side, radius 6; a large one 36 high with a 13/500 label, 16 at each side, 14 when it is not
//! primary. The design draws no hover or press: they follow the shell's `hover` and `pressed` overlays and the
//! accent's own hover, with no fade, no veil and the focus ring as the kit's inset shadow.
//!
//! The kit's kinds keep their names: `primary`, `secondary` and `ghost` are lurq's variants too, laid over here, so a
//! lurq component's button of those draws as the kit's.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border, color::Color},
  ui::{Button, ButtonPart, ButtonState, PartStyle, Recipe, Selector},
};

use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraShadow, ExtraSpacing, ExtraTypography},
  ui::button::{ButtonKind, ButtonSize, state},
};

/// The kinds drawn by lurq's button: a select and a stacked answer keep the kit's own frame (`ui::button`).
const KINDS: [ButtonKind; 5] = [
  ButtonKind::Primary,
  ButtonKind::Secondary,
  ButtonKind::Ghost,
  ButtonKind::Danger,
  ButtonKind::DangerGhost,
];

/// No veil over a pressed fill: each kind has a pressed fill of its own.
const NO_VEIL: Color = Color::new(0, 0, 0, 0);

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Button>(|rules| states(sizes(KINDS.into_iter().fold(rules, kind))));
}

/// What every kind shares, and its fill, stroke and content, at rest, under the pointer and pressed.
fn kind(rules: Recipe, kind: ButtonKind) -> Recipe {
  let name = kind.name();
  let rules = rules
    .variant(name, ButtonPart::Root, |style| {
      let mut style = style
        .gap(ExtraSpacing::Snug.role())
        .radius(RadiusSize::Md)
        .cursor(CursorIcon::Pointer);
      style.transitions = Some(Vec::new());
      dressed(style, kind)
    })
    .variant(name, ButtonPart::Label, |style| {
      style.typography(TypographyStyle::Button).foreground(kind.content())
    })
    .when(Selector::new().variant(name).pressed(), ButtonPart::Root, |style| {
      pressed(style.veil(NO_VEIL), kind)
    })
    .when(Selector::new().variant(name).hovered(), ButtonPart::Root, |style| {
      hovered(style.foreground(kind.content()), kind)
    })
    .when(
      Selector::new().variant(name).focus_visible(),
      ButtonPart::Root,
      |style| {
        let mut style = style.shadow(ExtraShadow::FocusRing.role());
        style.focus_ring = Some(false);
        style
      },
    );
  // A large primary has 16 at each side, a large one of another kind 14.
  rules.when(
    Selector::new().variant(name).size(ButtonSize::Large.name()),
    ButtonPart::Root,
    |style| {
      style.padding_x(match kind {
        ButtonKind::Primary => SpacingSize::Lg,
        _ => ExtraSpacing::Inset.role(),
      })
    },
  )
}

fn dressed(style: PartStyle, kind: ButtonKind) -> PartStyle {
  match kind {
    ButtonKind::Primary => style.background(PaletteColor::Primary),
    ButtonKind::Secondary => style
      .background(PaletteColor::Card)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Input)),
    ButtonKind::Ghost | ButtonKind::DangerGhost => style,
    ButtonKind::Select => style
      .background(ExtraColor::InputSurface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border)),
    ButtonKind::Danger => style.background(PaletteColor::DestructiveMuted),
  }
}

fn hovered(style: PartStyle, kind: ButtonKind) -> PartStyle {
  match kind {
    ButtonKind::Primary => style.background(PaletteColor::PrimaryHover),
    ButtonKind::Secondary | ButtonKind::Select => style.background(PaletteColor::Border),
    ButtonKind::Ghost | ButtonKind::DangerGhost => style.background(ExtraColor::Hover.role()),
    ButtonKind::Danger => style.border(Border::inside(BorderSize::Sm, PaletteColor::Destructive)),
  }
}

fn pressed(style: PartStyle, kind: ButtonKind) -> PartStyle {
  match kind {
    ButtonKind::Primary => style.background(PaletteColor::PrimaryHover),
    ButtonKind::Secondary | ButtonKind::Select => style.background(PaletteColor::Input),
    ButtonKind::Ghost | ButtonKind::DangerGhost => style.background(ExtraColor::Pressed.role()),
    ButtonKind::Danger => style.border(Border::inside(BorderSize::Sm, PaletteColor::Destructive)),
  }
}

/// Each size's height and the room at its sides; the helper's and the large one's label.
fn sizes(rules: Recipe) -> Recipe {
  let rules = ButtonSize::ALL.into_iter().fold(rules, |rules, size| {
    let (height, side) = match size {
      ButtonSize::Regular => (ControlHeight::Md, SpacingSize::Md),
      ButtonSize::Heading => (ControlHeight::Heading, SpacingSize::Md),
      ButtonSize::Helper => (ControlHeight::Helper, SpacingSize::Md),
      ButtonSize::Small => (ControlHeight::Sm, SpacingSize::Md),
      ButtonSize::Snug => (ControlHeight::Snug, SpacingSize::Md),
      ButtonSize::Pinned => (ControlHeight::Row, SpacingSize::Md),
      ButtonSize::Large => (ControlHeight::Lg, ExtraSpacing::Inset.role()),
    };
    rules.size(size.name(), ButtonPart::Root, |style| {
      style.height(height.value()).padding_x(side)
    })
  });
  rules
    .size(ButtonSize::Helper.name(), ButtonPart::Label, |style| {
      style.typography(ExtraTypography::Note.role())
    })
    .size(ButtonSize::Large.name(), ButtonPart::Label, |style| {
      style.typography(ExtraTypography::ButtonLarge.role())
    })
}

/// The states: faded while it cannot be pressed, but for a loading button and an answer shown as it was; and the
/// fills of a held, open, applied or refused one.
fn states(rules: Recipe) -> Recipe {
  let not_built = Selector::new().state(ButtonState::Disabled);
  rules
    .state(state::TIGHT, ButtonPart::Root, |style| {
      style.padding_x(ExtraSpacing::Loose.role())
    })
    .when(
      Selector::new().size(ButtonSize::Large.name()).state(state::ROOMY),
      ButtonPart::Root,
      |style| style.padding_x(SpacingSize::Lg),
    )
    .state(ButtonState::Disabled, ButtonPart::Root, |style| {
      style.opacity(DISABLED_OPACITY).cursor(CursorIcon::NotAllowed)
    })
    .when(
      not_built.clone().state(ButtonState::Loading),
      ButtonPart::Root,
      |style| style.opacity(1.0).cursor(CursorIcon::Progress),
    )
    .when(not_built.state(state::SHOWN), ButtonPart::Root, |style| {
      style.opacity(1.0).cursor(CursorIcon::Default)
    })
    .state(state::HELD, ButtonPart::Root, |style| {
      style.background(ExtraColor::Pressed.role())
    })
    .state(state::MENU_OPEN, ButtonPart::Root, |style| {
      style
        .background(PaletteColor::Card)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
    })
    .state(state::APPLIED, ButtonPart::Root, |style| {
      style
        .background(PaletteColor::Highlight)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
    })
    .state(state::APPLIED, ButtonPart::Label, |style| {
      style.foreground(PaletteColor::Foreground)
    })
    .state(state::INVALID, ButtonPart::Root, |style| {
      style.border(Border::inside(BorderSize::Sm, PaletteColor::Destructive))
    })
}
