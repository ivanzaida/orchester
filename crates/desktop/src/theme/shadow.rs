use lurq::{
  app::theme::{ShadowStyle, ThemeShadows},
  node::{BoxShadow, color::Color},
};

use super::{
  palette::ExtraColor,
  tokens::{FOCUS_EXTENT, FOCUS_WIDTH, POPOVER_SHADOW_BLUR, POPOVER_SHADOW_COLOR, POPOVER_SHADOW_Y},
};

/// Pencil shadows, registered as `ShadowStyle::Extra`. The design has no general elevation
/// scale, so `Sm`/`Md`/`Lg` keep lurq's defaults and nothing uses them.
#[derive(Clone, Copy)]
pub enum ExtraShadow {
  /// Status bar operations popover (`E1msJS`).
  Popover,
  /// Focus ring outside a control: the gap, then the ring around it (`xURvJ`).
  FocusRing,
  /// Focus ring on the edge of a control without room around it: the ring, the gap inside it.
  FocusRingInset,
}

impl ExtraShadow {
  const ALL: [Self; 3] = [Self::Popover, Self::FocusRing, Self::FocusRingInset];

  fn name(self) -> &'static str {
    match self {
      Self::Popover => "popover",
      Self::FocusRing => "focus-ring",
      Self::FocusRingInset => "focus-ring-inset",
    }
  }

  /// Lists paint first on top.
  fn value(self) -> Vec<BoxShadow> {
    let spread = |width: f32, color: ExtraColor| BoxShadow::new(0.0, 0.0, 0.0, color.role()).spread(width);
    match self {
      Self::Popover => vec![BoxShadow::new(
        0.0,
        POPOVER_SHADOW_Y,
        POPOVER_SHADOW_BLUR,
        Color::from_hex(POPOVER_SHADOW_COLOR),
      )],
      Self::FocusRing => vec![
        spread(FOCUS_WIDTH, ExtraColor::FocusGap),
        spread(FOCUS_EXTENT, ExtraColor::FocusRing),
      ],
      Self::FocusRingInset => vec![
        spread(FOCUS_WIDTH, ExtraColor::FocusRing).inset(),
        spread(FOCUS_EXTENT, ExtraColor::FocusGap).inset(),
      ],
    }
  }

  pub fn role(self) -> ShadowStyle {
    ShadowStyle::extra(self.name())
  }
}

pub fn shadows() -> ThemeShadows {
  let mut shadows = ThemeShadows::default();
  for shadow in ExtraShadow::ALL {
    shadows.extra.insert(shadow.name().into(), shadow.value().into());
  }
  shadows
}
