use lurq::{
  app::theme::{ThemeBorderSizes, ThemeRadii, ThemeSpacing},
  node::dimension::Dimension,
};

use super::tokens::{FOCUS_STROKE, RADIUS_CONTROL, RADIUS_ITEM, RADIUS_PANEL, SPACE_LG, SPACE_MD, SPACE_SM};

pub fn radii() -> ThemeRadii {
  ThemeRadii {
    sm: RADIUS_ITEM,
    md: RADIUS_CONTROL,
    lg: RADIUS_PANEL,
    ..ThemeRadii::default()
  }
}

/// Pencil defines `space-sm/md/lg` only; `xs`, `xl` and `section` keep lurq defaults.
pub fn spacing() -> ThemeSpacing {
  ThemeSpacing {
    sm: Dimension::Px(SPACE_SM),
    md: Dimension::Px(SPACE_MD),
    lg: Dimension::Px(SPACE_LG),
    ..ThemeSpacing::default()
  }
}

pub fn border_sizes() -> ThemeBorderSizes {
  ThemeBorderSizes {
    md: FOCUS_STROKE,
    ..ThemeBorderSizes::default()
  }
}
