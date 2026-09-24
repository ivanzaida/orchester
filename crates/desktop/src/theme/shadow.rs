use lurq::{
  app::theme::{ShadowStyle, ThemeShadows},
  node::{BoxShadow, color::Color},
};

use super::tokens::{POPOVER_SHADOW_BLUR, POPOVER_SHADOW_COLOR, POPOVER_SHADOW_Y};

/// Pencil shadows, registered as `ShadowStyle::Extra`. The design has no general elevation
/// scale, so `Sm`/`Md`/`Lg` keep lurq's defaults and nothing uses them.
#[derive(Clone, Copy)]
pub enum ExtraShadow {
  /// Status bar operations popover (`E1msJS`).
  Popover,
}

impl ExtraShadow {
  const ALL: [Self; 1] = [Self::Popover];

  fn name(self) -> &'static str {
    match self {
      Self::Popover => "popover",
    }
  }

  fn value(self) -> Vec<BoxShadow> {
    match self {
      Self::Popover => vec![BoxShadow::new(
        0.0,
        POPOVER_SHADOW_Y,
        POPOVER_SHADOW_BLUR,
        Color::from_hex(POPOVER_SHADOW_COLOR),
      )],
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
