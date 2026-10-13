use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::theme::{PaletteColor, ShadowStyle, ThemeShadows},
  node::box_shadow::BoxShadow,
};

use super::ExtraColor;

/// Shadows without a built-in lurq role, registered as `ShadowStyle::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraShadow {
  /// The ring of a focused form control: 2 of `border_focus` inside the control's edge, as the design
  /// draws a focused button (`pWR5E`, `iFzXa`); a focused field takes the same color as its stroke. Inside
  /// the edge no parent clips it and it never moves layout; `selection` stays for what is selected.
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
      Self::FocusRing => Arc::from([BoxShadow::new(0.0, 0.0, 0.0, PaletteColor::BorderFocus)
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
      .chain(super::conversation::shadows())
      .chain(super::needs_you::shadows())
      .collect::<HashMap<_, _>>(),
  }
}
