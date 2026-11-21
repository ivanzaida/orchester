//! Controls that are on or off (`ui::toggle`): lurq's `ui::Checkbox`, `ui::Radio` and `ui::Switch`, each pressed as a
//! whole with its 12 label 8 from it, ringed inside its edge, radius 4, while the keyboard focused it.
//! - Checkbox (`Checkbox/off` `q3JN1`, `Checkbox/on` `Una0j`): a 16 box, radius 4, `surface_input` in a `border_strong`
//!   stroke; on, `accent` with a 12 `on_accent` check.
//! - Radio (`Radio/off` `RchTU`, `Radio/on` `vsbEb`): a round 16 mark, `surface_input` in a `border_strong` stroke; on,
//!   `accent` with a round 6 `on_accent` dot.
//! - Switch (`Switch/off` `H6LlDD`, `Switch/on` `XC5zU`): a round 28 × 16 track, 2 inside it, a round 12 thumb; off,
//!   `border_strong` with a `text_secondary` thumb at the start; on, `accent` with an `on_accent` thumb at the end.
//!
//! One whose value is fixed is faded to [`DISABLED_OPACITY`] with its label, but for one an earlier step settled
//! ([`SETTLED`]). The recipes are the app's whole: lurq's own looks are not the design's.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border},
  ui::{
    Checkbox, CheckboxPart, CheckboxState, Length, Name, PartStyle, Radio, RadioPart, RadioState, Recipe, Selector,
    Switch, SwitchPart, SwitchState,
  },
};

use crate::theme::{DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, IconSize};

/// A checkbox's box and a radio's mark (`ZZTTi`, `d4Pkdw`).
const MARK_SIZE: f32 = 16.0;
/// A radio's dot (`usaPb`).
const RADIO_DOT: f32 = 6.0;
/// A switch's track and thumb (`H6LlDD`, `dY0v7`).
const TRACK_WIDTH: f32 = 28.0;
const TRACK_HEIGHT: f32 = 16.0;
const THUMB_SIZE: f32 = 12.0;

/// A fixed value an earlier step decided, drawn at full strength.
pub const SETTLED: &str = "settled";
/// The label in the form's 12 `text_primary`, where the caller gives it no style of its own.
pub const PLAIN_LABEL: &str = "plain_label";
/// Room after the label, so the focus ring clears its last letter.
pub const RING_ROOM: &str = "ring_room";

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Checkbox>(checkbox());
  theme.set_recipe::<Radio>(radio());
  theme.set_recipe::<Switch>(switch());
}

fn edge(color: PaletteColor) -> Border {
  Border::inside(BorderSize::Sm, color)
}

fn off_mark(style: PartStyle) -> PartStyle {
  style
    .square(Length::Px(MARK_SIZE))
    .background(ExtraColor::InputSurface.role())
    .border(edge(PaletteColor::Input))
}

fn ring(mut style: PartStyle) -> PartStyle {
  style = style.shadow(ExtraShadow::FocusRing.role());
  style.focus_ring = Some(false);
  style
}

/// The whole, its label and its states, the same in the three: `root`, `label`, and the states' names.
fn whole<P: Into<Name> + Copy, S: Into<Name> + Copy>(
  rules: Recipe,
  (root, label): (P, P),
  (whole, disabled): (S, S),
) -> Recipe {
  rules
    .base(root, |style| {
      style.gap(SpacingSize::Sm).foreground(PaletteColor::Foreground)
    })
    .base(label, |style| {
      style
        .typography(TypographyStyle::Description)
        .cursor(CursorIcon::Pointer)
    })
    .state(PLAIN_LABEL, label, |style| style.foreground(PaletteColor::Foreground))
    .state(RING_ROOM, root, |style| style.padding_end(ExtraSpacing::Snug.role()))
    .state(whole, root, |style| {
      style.radius(RadiusSize::Sm).cursor(CursorIcon::Pointer)
    })
    .when(Selector::new().state(whole).focus_visible(), root, ring)
    .state(disabled, root, |style| {
      style.opacity(DISABLED_OPACITY).cursor(CursorIcon::NotAllowed)
    })
    .state(disabled, label, |style| style.cursor(CursorIcon::NotAllowed))
    .when(Selector::new().state(disabled).state(SETTLED), root, |style| {
      style.opacity(1.0)
    })
}

fn checkbox() -> Recipe {
  let rules = Recipe::new()
    .base(CheckboxPart::Box, |style| off_mark(style).radius(RadiusSize::Sm))
    .base(CheckboxPart::Indicator, |style| {
      style
        .square(Length::Px(IconSize::Sm.size()))
        .foreground(ExtraColor::OnAccent.role())
    })
    .state(CheckboxState::Checked, CheckboxPart::Box, |style| {
      style
        .background(PaletteColor::Primary)
        .border(edge(PaletteColor::Primary))
    });
  whole(
    rules,
    (CheckboxPart::Root, CheckboxPart::Label),
    (CheckboxState::Whole, CheckboxState::Disabled),
  )
}

fn radio() -> Recipe {
  let rules = Recipe::new()
    .base(RadioPart::Mark, |style| {
      off_mark(style).radius(ExtraRadius::Full.role())
    })
    .base(RadioPart::Dot, |style| {
      style
        .square(Length::Px(RADIO_DOT))
        .radius(ExtraRadius::Full.role())
        .background(ExtraColor::OnAccent.role())
    })
    .state(RadioState::Selected, RadioPart::Mark, |style| {
      style
        .background(PaletteColor::Primary)
        .border(edge(PaletteColor::Primary))
    });
  whole(
    rules,
    (RadioPart::Root, RadioPart::Label),
    (RadioState::Whole, RadioState::Disabled),
  )
}

fn switch() -> Recipe {
  let rules = Recipe::new()
    .base(SwitchPart::Track, |style| {
      style
        .width(Length::Px(TRACK_WIDTH))
        .height(Length::Px(TRACK_HEIGHT))
        .padding_x(ExtraSpacing::Tight.role())
        .radius(ExtraRadius::Full.role())
        .background(PaletteColor::Input)
    })
    .base(SwitchPart::Thumb, |style| {
      style
        .square(Length::Px(THUMB_SIZE))
        .radius(ExtraRadius::Full.role())
        .background(ExtraColor::TextSecondary.role())
    })
    .state(SwitchState::On, SwitchPart::Track, |style| {
      style.background(PaletteColor::Primary)
    })
    .state(SwitchState::On, SwitchPart::Thumb, |style| {
      style.background(ExtraColor::OnAccent.role())
    });
  whole(
    rules,
    (SwitchPart::Root, SwitchPart::Label),
    (SwitchState::Whole, SwitchState::Disabled),
  )
}
