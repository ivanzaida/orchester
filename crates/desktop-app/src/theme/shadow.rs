use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::theme::{PaletteColor, ShadowStyle, ThemeShadows},
  node::box_shadow::BoxShadow,
};

use super::{ExtraColor, markdown::LINK_RING_WIDTH};

/// Shadows without a built-in lurq role, registered as `ShadowStyle::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraShadow {
  /// The ring of a focused form control: 2 of `border_focus` inside the control's edge, as the design
  /// draws a focused button (`pWR5E`, `iFzXa`); a focused field takes the same color as its stroke. Inside
  /// the edge no parent clips it and it never moves layout; `selection` stays for what is selected.
  FocusRing,
  /// The ring of a focused control that is only words, a link or a disclosure line: 2 of `border_focus` outside
  /// them, as the design draws a focused link (`Mjhop`) and the Markdown links do. Paint only: it never moves layout.
  LinkFocusRing,
}

impl ExtraShadow {
  pub const ALL: [Self; 2] = [Self::FocusRing, Self::LinkFocusRing];

  pub fn role(self) -> ShadowStyle {
    ShadowStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FocusRing => "focus_ring",
      Self::LinkFocusRing => "link_focus_ring",
    }
  }

  fn shadows(self) -> Arc<[BoxShadow]> {
    match self {
      Self::FocusRing => Arc::from([BoxShadow::new(0.0, 0.0, 0.0, PaletteColor::BorderFocus)
        .spread(FOCUS_RING_WIDTH)
        .inset()]),
      Self::LinkFocusRing => {
        Arc::from([BoxShadow::new(0.0, 0.0, 0.0, PaletteColor::BorderFocus).spread(LINK_RING_WIDTH)])
      }
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
      .chain(super::orchestrator::shadows())
      .collect::<HashMap<_, _>>(),
  }
}
