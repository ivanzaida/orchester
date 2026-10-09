use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::theme::{ShadowStyle, ThemeShadows},
  node::box_shadow::BoxShadow,
};

use super::ExtraColor;

/// Shadows without a built-in lurq role, registered as `ShadowStyle::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraShadow {
  /// The ring of a focused form control: 2 of `selection` blue inside the control's edge, so no parent
  /// clips it, it never moves layout, and it reads on a primary button's `accent` and on dark and light
  /// surfaces alike. The design draws no focus state; the handover asks for a ring distinct from hover,
  /// pressed and selected (`docs/design/kontur-handover.md` §5.5).
  FocusRing,
}

impl ExtraShadow {
  pub const ALL: [Self; 1] = [Self::FocusRing];

  pub fn role(self) -> ShadowStyle {
    ShadowStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FocusRing => "focus_ring",
    }
  }

  fn shadows(self) -> Arc<[BoxShadow]> {
    match self {
      Self::FocusRing => Arc::from([BoxShadow::new(0.0, 0.0, 0.0, ExtraColor::Selection.role())
        .spread(FOCUS_RING_WIDTH)
        .inset()]),
    }
  }
}

/// The focus ring's width.
const FOCUS_RING_WIDTH: f32 = 2.0;

/// Elevations of the design: `sm` controls, `md` menus, tooltips and popovers, `lg` dialogs and toasts
/// (`ubwYH`, `U9syaQ`). The color is the `shadow` role, so it follows the mode.
pub fn shadows() -> ThemeShadows {
  let shadow = |offset_y, blur, spread| BoxShadow::new(0.0, offset_y, blur, ExtraColor::Shadow.role()).spread(spread);
  ThemeShadows {
    sm: Arc::from([shadow(1.0, 2.0, 0.0)]),
    md: Arc::from([shadow(4.0, 12.0, -2.0), shadow(1.0, 3.0, 0.0)]),
    lg: Arc::from([shadow(16.0, 40.0, -8.0), shadow(2.0, 6.0, -2.0)]),
    extra: ExtraShadow::ALL
      .into_iter()
      .map(|extra| (Arc::from(extra.name()), extra.shadows()))
      .collect::<HashMap<_, _>>(),
  }
}
